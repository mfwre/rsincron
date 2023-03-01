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
        if line.clone().chars().nth(0) == Some('#') {
            continue;
        };

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
        commands.entry(descriptor).or_insert((path, arguments));
    }

    let buffer = [0; 1024];
    let mut stream = inotify.event_stream(buffer).unwrap();

    while let Ok(event) = stream.try_next().await {
        let Some(event) = event else {
            continue;
        };

        println!("{event:#?}");
        let (path, arguments) = commands.get(&event.wd).unwrap();
        let filename = match event.name {
            Some(string) => string.to_str().unwrap_or_default().to_owned(),
            _ => String::default(),
        };

        let masks = format!("{:?}", event.mask).replace(" | ", ",");
        let arguments = {
            let mut formatted = String::new();
            let mut dollar = false;
            for c in arguments.chars() {
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
                            '#' => formatted.push_str(&filename),
                            '@' => formatted.push_str(&path.to_string()),
                            '%' => formatted.push_str(&masks),
                            '&' => formatted.push_str(&event.mask.bits().to_string()),
                            _ => (),
                        }
                        dollar = false;
                    } else {
                        formatted.push(c);
                    }
                }
            }
            formatted
        };

        let _ = Command::new("bash").arg("-c").arg(arguments).spawn();
    }
}
