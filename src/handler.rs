use crate::{events::EVENT_TYPES, get_user_table_path};
use inotify::{EventStream, Inotify, WatchDescriptor, WatchMask};
use std::{collections::HashMap, error, fs, path::PathBuf};
use walkdir::WalkDir;

#[derive(Copy, Clone)]
pub struct WatchConfig {
    pub recursive: bool
}

pub type ActiveWatches = HashMap<WatchDescriptor, (String, WatchMask, String, WatchConfig)>;

pub struct HandlerSettings {
    pub current_user: String,
    pub recursive_watch_poll_time: u64
}

pub struct Handler {
    pub inotify: Inotify,
    pub event_types: HashMap<&'static str, WatchMask>,
    pub active_watches: ActiveWatches,
    pub handler_settings: HandlerSettings,
}

impl Handler {
    pub fn new() -> Result<Self, Box<dyn error::Error>> {
        Ok(Self {
            inotify: Inotify::init()?,
            event_types: HashMap::from(EVENT_TYPES),
            active_watches: HashMap::new(),
            handler_settings: HandlerSettings {
                current_user: std::env::var("USER")?,
                recursive_watch_poll_time: 2
            }
        })
    }

    pub fn setup(mut instance: Self) -> Result<Self, Box<dyn error::Error>> {
        let table = fs::read_to_string(get_user_table_path().join(&instance.handler_settings.current_user))?;
        for line in table.lines() {
            if line.clone().chars().nth(0) == Some('#') {
                continue;
            };

            let mut fields = line.split('\t');
            let Some(path) = fields.next() else {
                continue;
            };

            let mut mask = WatchMask::empty();
            let mut watch_config = WatchConfig { recursive: false };
            let Some(masks) = fields.next() else {
                continue;
            };

            for m in masks.split(',') {
                match instance.event_types.get(m) {
                    Some(m) => mask.insert(*m),
                    _ => match m.split_once('=') {
                        Some(("recursive", value)) => watch_config.recursive = value.parse().unwrap(),
                        _ => continue
                    }
                }
            };

            let Some(command) = fields.next() else {
                continue;
            };

            if watch_config.recursive {
                instance.recursive_add_watch(path.to_string(), mask, command.to_string(), watch_config);
            }
        }

        Ok(instance)
    }

    pub fn add_watch(
        &mut self,
        (file_path, mask, command, watch_config): (String, WatchMask, String, WatchConfig),
        folder_name: Option<String>,
        ) {
        let mut pathbuf = vec![file_path];

        if let Some(folder_name) = folder_name {
            pathbuf.push(folder_name.to_string());
        };

        let pathbuf = PathBuf::from_iter(pathbuf);
        let Some(path) = pathbuf.to_str() else {
            return 
        };

        let Ok(descriptor) = self.inotify.add_watch(&path.to_string(), mask) else {
            return 
        };

        println!("watch setup: {} for masks {:?}", path, mask);
        self.active_watches
            .entry(descriptor)
            .or_insert((path.to_string(), mask, command, watch_config));
    }

    pub fn get_event_stream(&mut self) -> EventStream<[u8; 1024]> {
        let buffer = [0; 1024];
        self.inotify.event_stream(buffer).expect("error opening event stream: exiting")
    }

    pub fn recursive_add_watch(&mut self, path: String, mask: WatchMask, command: String, watch_config: WatchConfig) {
        for entry in WalkDir::new(path).into_iter()
            .filter_entry(|e| e.file_type().is_dir())
            {
                let Ok(entry) = entry else {
                    continue;
                };

                let Some(path) = entry.path().to_str() else {
                    continue;
                };

                self.add_watch((path.to_string(), mask, command.clone(), watch_config), None);
            }
    }
}
