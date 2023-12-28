use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use inotify::{EventMask, Inotify, WatchDescriptor};
use rsincronlib::{config::Config, watch::WatchData, SocketMessage, SOCKET, XDG};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender},
    thread,
};

use tracing::{event, Level};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        default_value_os_t = XDG
            .place_config_file("rsincron.toml")
            .expect("failed to get `config.toml`: do I have permissions?")
        )]
    config: PathBuf,
}

/*
    if event.mask == EventMask::IGNORED {
        if let Some(watch) = handler.active_watches.remove(&event.wd) {
            handler.failed_watches.insert(watch.clone());
            debug!(
                "event mask IGNORED (starting watch: {:?}): removed watch on {}",
                watch.config.table_watch,
                watch.path.to_string_lossy()
            );
        }
        return;
    }
*/

// TODO: cont
fn add_watch(
    inotify: &mut Inotify,
    watch: WatchData,
    old_watches: &mut Vec<WatchDescriptor>,
    watch_map: &mut HashMap<WatchDescriptor, WatchData>,
) -> bool {
    if let Ok(descriptor) = inotify.watches().add(&watch.path, watch.masks) {
        old_watches.retain(|d| descriptor != *d);
        watch_map.insert(descriptor, watch);
        return true;
    }

    false
}

// TODO: cont
fn reload_watches(
    mut old_watches: Vec<WatchDescriptor>,
    new_watches: Vec<WatchData>,
    inotify: &mut Inotify,
) -> HashMap<WatchDescriptor, WatchData> {
    let mut watch_map = HashMap::new();

    for watch in new_watches {
        if !add_watch(inotify, watch.clone(), &mut old_watches, &mut watch_map) {
            continue;
        }

        if !watch.flags.recursive {
            continue;
        }

        for entry in WalkDir::new(&watch.path).min_depth(1) {
            let Ok(entry) = entry else {
                continue;
            };

            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if !metadata.is_dir() {
                continue;
            }

            let watch = WatchData {
                path: watch.path.join(entry.path()),
                ..watch.clone()
            };

            add_watch(inotify, watch, &mut old_watches, &mut watch_map);
        }
    }

    let mut watches = inotify.watches();
    for watch in old_watches {
        watches.remove(watch).unwrap();
    }

    watch_map
}

#[tracing::instrument]
fn handle_socket(tx: &Sender<SocketMessage>) {
    let Ok(ref socket) = *SOCKET else {
        return;
    };

    if Path::new(&socket).exists() {
        if let Err(error) = fs::remove_file(socket) {
            event!(Level::WARN, ?error, "failed to remove existing socket");
            return;
        }
    }

    let listener = match UnixListener::bind(socket) {
        Ok(l) => l,
        Err(error) => {
            event!(Level::WARN, ?error, "failed to bind to socket");
            return;
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
}

#[tracing::instrument]
fn main() {
    let args = Args::parse();

    let config: Config = Figment::new()
        .join(Toml::file(args.config))
        .extract()
        .unwrap();

    let (tx, rx) = mpsc::channel();
    handle_socket(&tx);

    let mut buffer = [0; 4096];
    let mut inotify = Inotify::init().expect("Error while initializing inotify instance");
    let mut watches = reload_watches(Vec::new(), config.parse(), &mut inotify);
    loop {
        match inotify.read_events_blocking(&mut buffer) {
            Err(_) => continue,
            Ok(events) => {
                match rx.try_recv() {
                    Ok(SocketMessage::UpdateWatches) => {
                        watches = reload_watches(
                            watches.into_keys().collect(),
                            config.parse(),
                            &mut inotify,
                        );
                    }
                    Err(error) => event!(Level::WARN, ?error, "failed to receive from socket"),
                };

                for event in events {
                    if let Some(watch) = watches.get(&event.wd) {
                        watch.command.execute(&watch.path, &event).unwrap();

                        if watch.flags.recursive && event.mask.contains(EventMask::ISDIR) {
                            if let Err(error) = tx.send(SocketMessage::UpdateWatches) {
                                event!(Level::WARN, ?error, "failed to send message via channel");
                                panic!("mpsc channel error");
                            }
                        };
                    }
                }
            }
        }
    }

    /*
    let poll_time = config.poll_time;
    let handler = Arc::new(RwLock::new(config.setup().unwrap()));
    {
        let handler = handler.clone();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(poll_time));
            debug!("loop: failed watches");

            let Ok(mut lock) = handler.write() else {
                continue;
            };

            lock.failed_watches = lock
                .failed_watches
                .clone()
                .drain()
                .filter(|watch| {
                    if watch.config.table_watch {
                        lock.add_watch(watch.clone(), true, None).is_err()
                    } else {
                        false
                }
            })
            .collect::<FailedWatches>();
        });
    }
    */
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
