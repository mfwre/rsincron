use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use futures::{lock::Mutex, StreamExt};
use inotify::{Event, EventMask, Inotify};
use rsincronlib::{config::Config, state::State, with_logging, SocketMessage, XDG};
use std::{
    ffi::OsString,
    path::PathBuf,
    process::ExitCode,
    sync::{mpsc, Arc, OnceLock},
    time::Duration,
};

use tracing::{event, Level};

const ONE_SECOND: Duration = Duration::from_secs(1);

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        default_value_os_t = XDG
            .place_config_file("rsincron.toml")
            .expect("failed to get `rsincron.toml`: do I have permissions?")
        )]
    config: PathBuf,
}

#[tracing::instrument(skip_all)]
fn handle_event(event: Event<OsString>, state: &mut State) {
    event!(Level::INFO, ?event);
    if state.has_socket {
        match state.rx.try_recv() {
            Ok(SocketMessage::UpdateWatches) => state.reload_watches(),
            Err(mpsc::TryRecvError::Empty) => (),
            Err(error) => {
                event!(Level::WARN, ?error);
                state.has_socket = false;
            }
        };
    }

    let Some(watch) = state.get_watch(&event.wd) else {
        return;
    };

    if let Err(error) = watch.command.execute(&watch.path, &event) {
        event!(
            Level::ERROR,
            ?error,
            command = watch.command.program,
            argv = ?watch.command.argv,
            "failed to execute command"
        );
        return;
    }

    if event.mask == EventMask::IGNORED {
        let Some(watch) = state.remove_watch(&event.wd) else {
            return;
        };

        event!(Level::WARN, ?event.mask, path = ?watch.path, "removing watch");
        state.failed_watches.push(watch);
        return;
    }

    if watch.attributes.recursive && event.mask.contains(EventMask::ISDIR) {
        state.reload_watches()
    }
}

#[tokio::main]
#[tracing::instrument]
async fn main() -> ExitCode {
    with_logging();

    let args = Args::parse();

    CONFIG
        .set(
            match Figment::new().join(Toml::file(args.config)).extract() {
                Ok(c) => c,
                Err(error) => {
                    event!(
                        Level::WARN,
                        ?error,
                        "failed to parse configuration file. Using default configuration"
                    );
                    Config::default()
                }
            },
        )
        .unwrap();

    let Ok(mut inotify) = Inotify::init() else {
        event!(Level::ERROR, "failed to set up inotify instance");
        return ExitCode::FAILURE;
    };

    let state = Arc::new(Mutex::new({
        let mut state = State::new(&mut inotify, CONFIG.get().unwrap().to_owned());
        state.reload_watches();
        state
    }));

    let buffer = [0; 4096];
    let Ok(mut events_stream) = inotify.into_event_stream(buffer) else {
        return ExitCode::FAILURE;
    };

    {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(ONE_SECOND).await;
                state.lock().await.recover_watches();
            }
        });
    }

    while let Some(event) = events_stream.next().await {
        let Ok(event) = event else {
            event!(Level::ERROR, ?event, "failed to parse event");
            break;
        };

        handle_event(event, &mut *state.lock().await);
    }

    ExitCode::SUCCESS
}
