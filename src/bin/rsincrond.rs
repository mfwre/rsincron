use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use futures::{lock::Mutex, StreamExt};
use inotify::{Event, EventMask, Inotify, WatchDescriptor};
use rsincronlib::{
    config::Config,
    watch::{WatchData, Watches},
    with_logging, SocketMessage, SOCKET, XDG,
};
use std::{
    ffi::OsString,
    fs,
    io::Read,
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, OnceLock,
    },
    thread,
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
fn setup_reload_socket(tx: Sender<SocketMessage>) -> bool {
    let Ok(ref socket) = *SOCKET else {
        event!(Level::WARN, "failed to setup socket");
        return false;
    };

    if Path::new(&socket).exists() {
        if let Err(error) = fs::remove_file(socket) {
            event!(Level::WARN, ?error, "failed to remove existing socket");
            return false;
        }
    }

    let listener = match UnixListener::bind(socket) {
        Ok(l) => l,
        Err(error) => {
            event!(Level::WARN, ?error, "failed to bind to socket");
            return false;
        }
    };

    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buffer = [0; 100];
            if stream.read(&mut buffer).is_err() {
                return;
            }

            let Ok(SocketMessage::UpdateWatches) = bincode::deserialize(&buffer) else {
                return;
            };

            if let Err(error) = tx.send(SocketMessage::UpdateWatches) {
                event!(
                    Level::WARN,
                    ?error,
                    "failed to send update message through channel"
                );
            }
        }
    });

    true
}

#[tracing::instrument(skip_all)]
fn handle_event(event: Event<OsString>, state: &mut State) {
    event!(Level::DEBUG, ?event);
    if state.has_socket {
        match state.rx.try_recv() {
            Ok(SocketMessage::UpdateWatches) => state.reload(),
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
        state.reload()
    }
}

struct State {
    inotify_watches: inotify::Watches,
    watches: Watches,
    failed_watches: Vec<WatchData>,
    has_socket: bool,
    rx: Receiver<SocketMessage>,
}

impl State {
    fn new(inotify: &mut Inotify) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut watches = Watches::default();
        let mut inotify_watches = inotify.watches();

        watches.reload_table(
            &CONFIG.get().unwrap().watch_table_file,
            &mut inotify_watches,
        );

        Self {
            watches,
            rx,
            failed_watches: Vec::new(),
            has_socket: setup_reload_socket(tx),
            inotify_watches: inotify.watches(),
        }
    }

    fn reload(&mut self) {
        self.watches.reload_table(
            &CONFIG.get().unwrap().watch_table_file,
            &mut self.inotify_watches,
        )
    }

    fn recover_watches(&mut self) {
        self.failed_watches.retain(|watch| {
            let Ok(descriptor) = self.inotify_watches.add(watch.path.clone(), watch.masks) else {
                return true;
            };

            event!(Level::DEBUG, ?descriptor, ?watch, "adding watch");
            self.watches.0.insert(descriptor, watch.clone());
            false
        });
    }

    fn get_watch(&self, wd: &WatchDescriptor) -> Option<&WatchData> {
        self.watches.0.get(wd)
    }

    fn remove_watch(&mut self, wd: &WatchDescriptor) -> Option<WatchData> {
        self.watches.0.remove(wd)
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

    let state = Arc::new(Mutex::new(State::new(&mut inotify)));

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

#[cfg(test)]
mod tests {
    use inotify::{EventMask, Inotify, WatchMask};
    use std::{error::Error, fs};
    use tempfile::{tempdir, TempDir};

    type Result<T> = std::result::Result<T, Box<dyn Error>>;

    fn setup_inotify(mask: WatchMask) -> (Inotify, TempDir) {
        let inotify = Inotify::init().unwrap();
        let tmpdir = tempdir().unwrap();

        inotify.watches().add(tmpdir.path(), mask).unwrap();

        (inotify, tmpdir)
    }

    #[test]
    fn test_create_watch() -> Result<()> {
        let (mut inotify, tmpdir) = setup_inotify(WatchMask::CREATE);
        let mut buffer = [0; 100];

        fs::write(tmpdir.path().join("tempfile"), "").unwrap();
        for event in inotify.read_events(&mut buffer).unwrap() {
            assert_eq!(event.mask, EventMask::CREATE);
        }

        Ok(())
    }
}
