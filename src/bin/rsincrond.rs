use std::{
    ffi::OsString,
    process::Command,
    sync::{Arc, RwLock, RwLockWriteGuard},
    thread,
    time::Duration,
};

use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use futures::TryStreamExt;
use inotify::{Event, EventMask};
use log::{debug, info};
use rsincronlib::{
    handler::{FailedWatches, Handler},
    handler_config::HandlerConfig,
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        default_value_t = format!(
            "{}/.config/rsincron.toml",
            std::env::var("HOME")
                .expect("HOME envvar is not set: exiting")))
    ]
    config: String,
}

fn expand_variables(
    input: &str,
    filename: &str,
    path: &str,
    mask_text: &str,
    mask_bits: &str,
) -> String {
    let mut formatted = String::new();
    let mut dollar = false;
    for c in input.chars() {
        if c == '$' {
            if !dollar {
                dollar = true;
            } else {
                formatted.push(c);
                dollar = false;
            }
        } else {
            if dollar {
                match c {
                    '#' => formatted.push_str(filename),
                    '@' => formatted.push_str(path),
                    '%' => formatted.push_str(mask_text),
                    '&' => formatted.push_str(mask_bits),
                    _ => formatted.push(c),
                }
                dollar = false;
            } else {
                formatted.push(c);
            }
        }
    }
    formatted
}

async fn process_event<'a>(
    mut handler: RwLockWriteGuard<'a, Handler>,
    event: Option<Event<OsString>>,
) {
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

#[async_std::main]
async fn main() {
    let args = Args::parse();

    let config: HandlerConfig = Figment::new()
        .join(Toml::file(args.config))
        .extract()
        .unwrap();

    config
        .dispatch_log()
        .expect("failed to set up logging: exiting");

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

    // TODO: evaluate these two .unwrap() calls
    let mut event_stream = handler.write().unwrap().get_event_stream();
    while let Ok(event) = event_stream.try_next().await {
        let lock = handler.write();
        process_event(lock.unwrap(), event).await;
    }
}
