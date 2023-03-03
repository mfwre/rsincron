use crate::handler_config::HandlerConfig;
use inotify::{EventStream, Inotify, WatchDescriptor, WatchMask};
use log::{debug, info, warn};
use std::{
    collections::{HashMap, HashSet},
    error,
    path::PathBuf,
};
use walkdir::WalkDir;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct WatchConfig {
    pub recursive: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self { recursive: false }
    }
}

pub type Watch = (String, WatchMask, String, WatchConfig);
pub type Watches = HashMap<WatchDescriptor, Watch>;
pub type FailedWatches = HashSet<Watch>;

pub enum AddWatchError {
    PathToStr,
    FolderDoesntExit,
}

pub struct Handler {
    pub inotify: Inotify,
    pub active_watches: Watches,
    pub failed_watches: FailedWatches,
    pub handler_config: HandlerConfig,
}

impl Handler {
    pub fn new() -> Result<Self, Box<dyn error::Error>> {
        Ok(HandlerConfig::default().setup()?)
    }

    pub fn add_watch(
        &mut self,
        (file_path, mask, command, watch_config): Watch,
        folder_name: Option<String>,
    ) -> Result<(), AddWatchError> {
        debug!("watch add: {file_path}");
        let mut pathbuf = PathBuf::from(file_path);

        if let Some(folder_name) = folder_name {
            pathbuf.push(folder_name.to_string());
        };

        let Some(path) = pathbuf.to_str() else {
            return Err(AddWatchError::PathToStr)
        };

        let Ok(descriptor) = self.inotify.add_watch(&path.to_string(), mask) else {
            self.failed_watches.insert((path.to_string(), mask, command, watch_config));
            warn!("watch setup failed: {} for masks {:?}", path, mask);
            return Err(AddWatchError::FolderDoesntExit)
        };

        info!("watch setup: {} for masks {:?}", path, mask);
        self.active_watches.entry(descriptor).or_insert((
            path.to_string(),
            mask,
            command,
            watch_config,
        ));

        Ok(())
    }

    pub fn get_event_stream(&mut self) -> EventStream<[u8; 1024]> {
        let buffer = [0; 1024];
        self.inotify
            .event_stream(buffer)
            .expect("error opening event stream: exiting")
    }

    pub fn recursive_add_watch(&mut self, (file_path, mask, command, watch_config): Watch) {
        for entry in WalkDir::new(file_path)
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

            let watch = (path.to_string(), mask, command.clone(), watch_config);
            match self.add_watch(watch.clone(), None) {
                Err(_) => self.failed_watches.insert(watch),
                _ => true,
            };
        }
    }
}
