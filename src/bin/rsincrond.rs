use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use inotify::{Inotify, WatchDescriptor};
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
fn process_event<'a>(mut handler: RwLockWriteGuard<'a, Handler>, event: Option<Event<OsString>>) {
    let Some(event) = event else {
        return;
    };

    let watch = handler.active_watches.get(&event.wd).unwrap().clone();

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

    let filename = match event.name {
        Some(string) => string.to_str().unwrap_or_default().to_owned(),
        _ => String::default(),
    };

    if watch.config.recursive && event.mask.contains(EventMask::ISDIR) {
        handler.recursive_add_watch(watch.clone());
    }

    let masks = format!("{:?}", event.mask)
        .split(" | ")
        .map(|f| format!("IN_{f}"))
        .collect::<Vec<String>>()
        .join(",");

    info!(
        "watch event: {masks} for {path} {filename}",
        path = watch.path.to_string_lossy()
    );

    let command = expand_variables(
        &watch.command,
        &filename,
        &watch.path.to_string_lossy(),
        &masks,
        &event.mask.bits().to_string(),
    );

    let _ = Command::new("bash").arg("-c").arg(command).spawn();
}
*/

fn reload_watches(
    mut current_watches: Vec<WatchDescriptor>,
    new_watches: Vec<WatchData>,
    inotify: &mut Inotify,
) -> HashMap<WatchDescriptor, WatchData> {
    let mut watches = inotify.watches();
    let mut new_map = HashMap::new();

    for watch in new_watches {
        if let Ok(descriptor) = watches.add(&watch.path, watch.mask.clone()) {
            current_watches.retain(|d| descriptor != *d);
            new_map.insert(descriptor, watch);
        }
    }

    for watch in current_watches {
        watches.remove(watch).unwrap();
    }

    new_map
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
                        println!("reloading watch table");
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
