use crate::{events::EVENT_TYPES, get_user_table_path};
use inotify::{EventStream, Inotify, WatchDescriptor, WatchMask};
use std::{collections::HashMap, error, fs, path::PathBuf};
use walkdir::WalkDir;

pub type ActiveWatches = HashMap<WatchDescriptor, (String, WatchMask, String)>;

pub struct HandlerSettings {
    pub current_user: String,
    pub recursive_watch: bool
}

pub struct Handler<'a> {
    pub inotify: Inotify,
    pub event_types: HashMap<&'a str, WatchMask>,
    pub active_watches: ActiveWatches,
    pub handler_settings: HandlerSettings,
}

impl<'a> Handler<'a> {
    pub fn new() -> Result<Self, Box<dyn error::Error>> {
        Ok(Self {
            inotify: Inotify::init()?,
            event_types: HashMap::from(EVENT_TYPES),
            active_watches: HashMap::new(),
            handler_settings: HandlerSettings {
                current_user: std::env::var("USER")?,
                recursive_watch: true
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

            let mask = {
                let Some(masks) = fields.next() else {
                    continue;
                };

                masks.split(',').fold(WatchMask::empty(), |mut mask, new| {
                    mask.insert(*instance.event_types.get(new).unwrap());
                    return mask;
                })
            };

            let Some(command) = fields.next() else {
                continue;
            };

            instance.add_watch((path.to_string(), mask, command.to_string()), None);

            if instance.handler_settings.recursive_watch {
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

                            instance.add_watch((path.to_string(), mask, command.to_string()), None);
                        }
            };

        }

        Ok(instance)
    }

    pub fn add_watch(
        &mut self,
        (file_path, mask, command): (String, WatchMask, String),
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

        println!("setup watch: {} for masks {:?}", path, mask);
        self.active_watches
            .entry(descriptor)
            .or_insert((path.to_string(), mask, command));
    }

    pub fn get_event_stream(&mut self) -> EventStream<[u8; 1024]> {
        let buffer = [0; 1024];
        self.inotify.event_stream(buffer).expect("error opening event stream: exiting")
    }
}
