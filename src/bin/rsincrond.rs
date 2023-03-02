use std::{ffi::OsString, process::Command, time::Duration};

use async_std::{
    sync::{Arc, RwLock, RwLockWriteGuard},
    task,
};
use futures::TryStreamExt;
use inotify::Event;
use rsincronlib::handler::Handler;

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

    let (path, mask, command, watch_config) =
        handler.active_watches.get(&event.wd).unwrap().clone();

    let filename = match event.name {
        Some(string) => string.to_str().unwrap_or_default().to_owned(),
        _ => String::default(),
    };

    if watch_config.recursive {
        handler.add_watch(
            (
                path.to_owned(),
                mask.to_owned(),
                command.clone(),
                watch_config,
            ),
            Some(filename.clone()),
        );
    }

    let masks = format!("{:?}", event.mask)
        .split(" | ")
        .map(|f| format!("IN_{f}"))
        .collect::<Vec<String>>()
        .join(",");

    println!("watch event: {masks} for {path} {filename}");
    let command = expand_variables(
        &command,
        &filename,
        &path.to_string(),
        &masks,
        &event.mask.bits().to_string(),
    );

    let _ = Command::new("bash").arg("-c").arg(command).spawn();
}

#[async_std::main]
async fn main() {
    let handler = Arc::new(RwLock::new(
        Handler::setup(Handler::new().unwrap()).unwrap(),
    ));

    /* TODO: implement this with recursive_add_watch if set
    {
        let handler = handler.clone();
        task::spawn(async move {
            loop {
                let settings = &handler.read().await.handler_settings;
                task::sleep(Duration::from_secs(settings.recursive_watch_poll_time)).await;
            }
        });
    }
    */

    let mut event_stream = handler.write().await.get_event_stream();
    while let Ok(event) = event_stream.try_next().await {
        let handler = handler.clone();
        let lock = handler.write();

        process_event(lock.await, event).await;
    }
}
