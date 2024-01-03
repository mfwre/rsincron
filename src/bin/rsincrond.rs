use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use inotify::{EventMask, Inotify};
use rsincronlib::{config::Config, watch::Watches, with_logging, SocketMessage, SOCKET, XDG};
use std::{
    fs,
    io::Read,
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::mpsc::{self, Receiver},
    thread,
};

use tracing::{event, Level};

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
fn handle_socket() -> Option<Receiver<SocketMessage>> {
    let (tx, rx) = mpsc::channel();

    let Ok(ref socket) = *SOCKET else {
        event!(Level::WARN, "failed to setup socket");
        return None;
    };

    if Path::new(&socket).exists() {
        if let Err(error) = fs::remove_file(socket) {
            event!(Level::WARN, ?error, "failed to remove existing socket");
            return None;
        }
    }

    let listener = match UnixListener::bind(socket) {
        Ok(l) => l,
        Err(error) => {
            event!(Level::WARN, ?error, "failed to bind to socket");
            return None;
        }
    };

    let tx = tx.clone();
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let tx = tx.clone();
            thread::spawn(move || {
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
            });
        }
    });

    Some(rx)
}

#[tracing::instrument(skip_all)]
fn handle_events(
    inotify: &mut Inotify,
    rx: &Option<Receiver<SocketMessage>>,
    buffer: &mut [u8],
    watches: &mut Watches,
    watch_table: &PathBuf,
) -> u8 {
    match inotify.read_events_blocking(buffer) {
        Err(error) => {
            event!(Level::ERROR, ?error, "inotify error");
            1
        }
        Ok(events) => {
            if let Some(ref rx) = rx {
                match rx.try_recv() {
                    Ok(SocketMessage::UpdateWatches) => watches.reload_table(&watch_table, inotify),
                    Err(mpsc::TryRecvError::Empty) => (),
                    Err(error) => event!(Level::WARN, ?error),
                };
            }

            for event in events {
                event!(Level::DEBUG, ?event);
                let Some(watch) = watches.0.get(&event.wd) else {
                    continue;
                };

                if let Err(error) = watch.command.execute(&watch.path, &event) {
                    event!(
                        Level::ERROR,
                        ?error,
                        command = watch.command.program,
                        argv = ?watch.command.argv,
                        "failed to execute command"
                    );
                    continue;
                }

                if event.mask == EventMask::IGNORED {
                    let Some(watch) = watches.0.remove(&event.wd) else {
                        continue;
                    };

                    event!(Level::WARN, ?event.mask, path = ?watch.path, "removing watch");
                    continue;
                }

                if watch.attributes.recursive && event.mask.contains(EventMask::ISDIR) {
                    watches.reload_table(&watch_table, inotify);
                }
            }
            0
        }
    }
}

#[tracing::instrument]
fn main() -> ExitCode {
    with_logging();

    let args = Args::parse();
    let config: Config = match Figment::new().join(Toml::file(args.config)).extract() {
        Ok(c) => c,
        Err(error) => {
            event!(
                Level::WARN,
                ?error,
                "failed to parse configuration file. Using default configuration"
            );
            Config::default()
        }
    };

    let rx = handle_socket();

    let Ok(mut inotify) = Inotify::init() else {
        event!(Level::ERROR, "failed to set up inotify instance");
        return ExitCode::from(1);
    };

    let mut buffer = [0; 4096];
    let mut watches = Watches::default();
    watches.reload_table(&config.watch_table_file, &mut inotify);

    loop {
        let exit_code = handle_events(
            &mut inotify,
            &rx,
            &mut buffer,
            &mut watches,
            &config.watch_table_file,
        );

        if exit_code != 0 {
            return ExitCode::from(exit_code);
        }
    }
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
