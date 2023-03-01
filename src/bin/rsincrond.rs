use std::{collections::HashMap, fs::read_to_string, process::Command};

use futures::TryStreamExt;
use inotify::{Inotify, WatchMask};
use rsincronlib::{get_user_table_path, EVENT_TYPES};

#[async_std::main]
async fn main() {
    let mut inotify = Inotify::init().expect("Error while initializing inotify instance");
    let user = std::env::var("USER").expect("USER is not set: exiting");

    let types = HashMap::from(EVENT_TYPES);
    let table = read_to_string(get_user_table_path().join(user))
        .expect("failed to read user table: exiting");

    let mut commands = HashMap::new();
    for line in table.lines() {
        let mut fields = line.split('\t');
        let Some(path) = dbg!(fields.next()) else {
            continue;
        };

        let mask = {
            let Some(masks) = dbg!(fields.next()) else {
                continue;
            };

            masks.split(',').fold(WatchMask::empty(), |mut mask, new| {
                mask.insert(*types.get(new).unwrap());
                return mask;
            })
        };

        let Ok(descriptor) = dbg!(inotify.add_watch(path, mask)) else {
            continue;
        };

        let mut command = Command::new("bash");
        let Some(arguments) = dbg!(fields.next()) else {
            continue;
        };

        command.arg("-c");
        commands
            .entry(descriptor)
            .or_insert((command, path, arguments));
    }

    let buffer = [0; 1024];
    let mut stream = inotify.event_stream(buffer).unwrap();

    loop {
        let event = stream.try_next().await;
        match event {
            Ok(event) => {
                if let Some(event) = event {
                    let (command, path, arguments) = commands.get_mut(&event.wd).unwrap();
                    let filename = match event.name {
                        Some(string) => string.to_str().unwrap_or_default().to_owned(),
                        _ => String::default(),
                    };

                    let arguments = [("$#", &filename), ("$@", &path.to_string())]
                        .into_iter()
                        .fold(arguments.to_string(), |arg, new| arg.replace(new.0, new.1));

                    let _ = command.arg(arguments).spawn();
                }
            }
            Err(err) => println!("{:#?}", err),
        }
    }
}
