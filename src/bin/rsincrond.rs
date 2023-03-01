use std::{collections::HashMap, fs::read_to_string, path::PathBuf, process::Command};

use futures::TryStreamExt;
use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use rsincronlib::{get_user_table_path, EVENT_TYPES};
use walkdir::WalkDir;

type Config<'a> = HashMap<WatchDescriptor, (String, WatchMask, &'a str)>;

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

fn runtime_add_watch<'a>(
    mut inotify: Inotify,
    mut configs: Config<'a>,
    (path, mask, command): (String, WatchMask, &'a str),
    filename: Option<&str>,
) -> (Config<'a>, Inotify) {
    let mut pathbuf = vec![path];

    if let Some(filename) = filename {
        pathbuf.push(filename.to_string());
    };

    let pathbuf = PathBuf::from_iter(pathbuf);
    let Some(path) = pathbuf.to_str() else {
        return (configs, inotify)
    };

    let Ok(descriptor) = inotify.add_watch(&path.to_string(), mask) else {
        return (configs, inotify)
    };

    println!("setup watch: {} for masks {:?}", path, mask);
    configs
        .entry(descriptor)
        .or_insert((path.to_string(), mask, command));

    (configs, inotify)
}

pub fn process_table<'a>(mut inotify: Inotify, table: &'a str) -> (Config<'a>, Inotify) {
    let mut configs = HashMap::new();
    let types = HashMap::from(EVENT_TYPES);
    for line in table.lines() {
        if line.clone().chars().nth(0) == Some('#') {
            continue;
        };

        let mut fields = line.split('\t');
        let Some(path) = fields.next() else {
            continue;
        };

        let mask = {
            let Some(masks) = fields.next() else {
                continue;
            };

            masks.split(',').fold(WatchMask::empty(), |mut mask, new| {
                mask.insert(*types.get(new).unwrap());
                return mask;
            })
        };

        let Some(command) = fields.next() else {
            continue;
        };

        (configs, inotify) =
            runtime_add_watch(inotify, configs, (path.to_string(), mask, command), None);

        for entry in WalkDir::new(path)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| e.file_type().is_dir())
        {
            let Ok(entry) = entry else {
                continue;
            };

            let Some(path) = entry.path().to_str() else {
                continue;
            };

            (configs, inotify) =
                runtime_add_watch(inotify, configs, (path.to_string(), mask, command), None);
        }
    }

    (configs, inotify)
}

#[async_std::main]
async fn main() {
    let mut inotify = Inotify::init().expect("Error while initializing inotify instance");
    let user = std::env::var("USER").expect("USER is not set: exiting");

    let table = read_to_string(get_user_table_path().join(user))
        .expect("failed to read user table: exiting");

    let buffer = [0; 1024];
    let mut stream = inotify.event_stream(buffer).unwrap();

    let (mut configs, mut inotify) = process_table(inotify, &table);
    while let Ok(event) = stream.try_next().await {
        let Some(event) = event else {
            continue;
        };

        let (path, mask, command) = configs.get(&event.wd).unwrap().clone();
        let filename = match event.name {
            Some(string) => string.to_str().unwrap_or_default().to_owned(),
            _ => String::default(),
        };

        if event.mask == EventMask::CREATE | EventMask::ISDIR {
            (configs, inotify) = runtime_add_watch(
                inotify,
                configs,
                (path.to_owned(), mask.to_owned(), command),
                Some(&filename),
            );
        }

        let masks = format!("{:?}", event.mask).replace(" | ", ",");
        let command = expand_variables(
            command,
            &filename,
            &path.to_string(),
            &masks,
            &event.mask.bits().to_string(),
        );

        let _ = Command::new("bash").arg("-c").arg(command).spawn();
    }
}
