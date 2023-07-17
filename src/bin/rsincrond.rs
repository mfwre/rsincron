use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use inotify::{EventMask, Inotify, WatchDescriptor};
use rsincronlib::{config::Config, watch::WatchData, SOCKET, XDG};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    os::unix::net::UnixListener,
    path::Path,
    process,
    sync::{Arc, Mutex},
    thread,
};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        default_value_t = XDG
            .place_config_file("rsincron.toml")
            .expect("failed to get `config.toml`: do I have permissions?")
            .to_string_lossy()
            .to_string()
        )]
    config: String,
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

fn add_watch(
    inotify: &mut Inotify,
    watch: WatchData,
    old_watches: &mut Vec<WatchDescriptor>,
    watch_map: &mut HashMap<WatchDescriptor, WatchData>,
) -> bool {
    if let Ok(descriptor) = inotify.watches().add(&watch.path, watch.mask.clone()) {
        old_watches.retain(|d| descriptor != *d);
        watch_map.insert(descriptor, watch);
        return true;
    }

    return false;
}

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

            let Ok(metadata) =  entry.metadata() else {
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

fn handle_socket(update_watches: Arc<Mutex<bool>>) {
    if Path::new(SOCKET).exists() {
        fs::remove_file(SOCKET).expect(&format!("failed to remove `{SOCKET}`"));
    }

    let Ok(listener) = UnixListener::bind(SOCKET) else {
        eprintln!("failed to bind to socket");
        process::exit(1);
    };

    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            let update_watches = Arc::clone(&update_watches);
            thread::spawn(move || {
                let mut buffer = String::new();
                if stream.read_to_string(&mut buffer).is_err() {
                    return;
                }

                if buffer.as_str() == "RELOAD" {
                    *update_watches.lock().unwrap() = true;
                }
            });
        }
    }
}

fn main() {
    let args = Args::parse();

    let config: Config = Figment::new()
        .join(Toml::file(args.config))
        .extract()
        .unwrap();

    let update_watches = Arc::new(Mutex::new(false));
    {
        let update_watches = Arc::clone(&update_watches);
        thread::spawn(move || {
            handle_socket(update_watches);
        });
    }

    let mut buffer = [0; 4096];
    let mut inotify = Inotify::init().expect("Error while initializing inotify instance");
    let mut watches = reload_watches(Vec::new(), config.parse(), &mut inotify);
    loop {
        match inotify.read_events_blocking(&mut buffer) {
            Err(_) => continue,
            Ok(events) => {
                if let Ok(mut update) = update_watches.lock() {
                    if *update {
                        watches = reload_watches(
                            watches.into_keys().collect(),
                            config.parse(),
                            &mut inotify,
                        );
                        *update = false;
                    }
                }

                for event in events {
                    if let Some(watch) = watches.get(&event.wd) {
                        watch
                            .command
                            .execute(&watch.path, event.name, event.mask, event.mask.bits())
                            .unwrap();

                        if watch.flags.recursive && event.mask.contains(EventMask::ISDIR) {
                            *update_watches.lock().unwrap() = true;
                        }
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
