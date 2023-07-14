use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use inotify::{Inotify, WatchDescriptor};
use rsincronlib::{config::Config, watch::WatchData, XDG};
use std::{collections::HashMap, process};

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
    current_watches: Option<HashMap<WatchDescriptor, WatchData>>,
    new_watches: Vec<WatchData>,
    inotify: &mut Inotify,
) -> HashMap<WatchDescriptor, WatchData> {
    let mut watches = inotify.watches();
    let mut new_map = HashMap::new();

    for descriptor in current_watches.unwrap_or_default().into_keys() {
        watches.remove(descriptor).unwrap()
    }

    for watch in new_watches {
        if let Ok(descriptor) = watches.add(&watch.path, watch.mask.clone()) {
            new_map.insert(descriptor, watch);
        }
    }

    println!("{:#?}", new_map);
    new_map
}

fn main() {
    let args = Args::parse();

    let config: Config = Figment::new()
        .join(Toml::file(args.config))
        .extract()
        .unwrap();

    let mut inotify = Inotify::init().expect("Error while initializing inotify instance");
    let watches = match config.parse() {
        Ok(watches) => watches,
        Err(err) => {
            eprintln!("failed to read watch table: {err}");
            process::exit(1);
        }
    };

    let watches = reload_watches(None, watches, &mut inotify);

    let mut buffer = [0; 4096];

    loop {
        match inotify.read_events_blocking(&mut buffer) {
            Err(_) => continue,
            Ok(events) => {
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

    /*
    while let Ok(event) = event_stream.try_next().await {
        let lock = handler.write();
        process_event(lock.unwrap(), event).await;
    }
    */
}
