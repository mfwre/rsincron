use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use futures::StreamExt;
use inotify::{Event, EventMask, Inotify};
use rsincronlib::{
    config::Config,
    state::{ArcShared, Shared, State},
    with_logging, SocketMessage, XDG,
};

use tokio::sync::mpsc;

use std::{
    ffi::OsString,
    path::PathBuf,
    process::ExitCode,
    sync::{Arc, OnceLock},
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
async fn handle_event(event: Event<OsString>, state: ArcShared) {
    event!(
        Level::INFO,
        event_id = event.wd.get_watch_descriptor_id(),
        mask = ?event.mask,
        name = ?event.name
    );

    if state.has_socket() {
        match state.rx_try_recv() {
            Ok(SocketMessage::UpdateWatches) => state.reload_watches(),
            Err(mpsc::error::TryRecvError::Empty) => (),
            Err(error) => {
                event!(Level::WARN, ?error);
                state.unset_socket();
            }
        };
    }

    let Some(watch) = state.get_watch(&event.wd) else {
        return;
    };

    if let Err(error) = watch.command.execute(&watch.path, &event).await {
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
        state.push_failed_watch(watch);
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
                        error = ?error.kind,
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

    let state = Arc::new(Shared {
        state: {
            let mut state = State::new(&mut inotify, CONFIG.get().unwrap().to_owned());
            state.reload_watches();
            state.into()
        },
    });

    let buffer = [0; 4096];
    let Ok(events_stream) = inotify.into_event_stream(buffer) else {
        return ExitCode::FAILURE;
    };

    {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(ONE_SECOND).await;
                state.recover_watches();
            }
        });
    }

    events_stream
        .for_each_concurrent(None, |event| async {
            let Ok(event) = event else {
                event!(Level::ERROR, ?event, "failed to parse event");
                return;
            };

            handle_event(event, state.clone()).await;
        })
        .await;

    ExitCode::SUCCESS
}
