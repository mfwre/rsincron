use crate::handler_config::HandlerConfig;
use inotify::{EventStream, Inotify, WatchDescriptor, WatchMask};
use log::{info, warn};
use std::{
    collections::{HashMap, HashSet},
    error,
    path::PathBuf,
};
use walkdir::WalkDir;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct WatchConfig {
    pub recursive: bool,
    pub table_watch: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            recursive: false,
            table_watch: false,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Watch {
    pub path: PathBuf,
    pub mask: WatchMask,
    pub command: String,
    pub config: WatchConfig,
}

pub type Watches = HashMap<WatchDescriptor, Watch>;
pub type FailedWatches = HashSet<Watch>;

pub enum AddWatchError {
    PathToStr,
    FolderDoesntExit(Watch),
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
        mut watch: Watch,
        from_setup: bool,
        folder_name: Option<String>,
    ) -> Result<(), AddWatchError> {
        let mut pathbuf = PathBuf::from(&watch.path);

        if let Some(folder_name) = folder_name {
            pathbuf.push(folder_name.to_string());
        };

        let Some(path) = pathbuf.to_str() else {
            return Err(AddWatchError::PathToStr)
        };

        watch.config.table_watch = from_setup;

        info!("watch setup: {} for masks {:?}", path, watch.mask);
        let Ok(descriptor) = self.inotify.add_watch(&path.to_string(), watch.mask) else {
            self.failed_watches.insert(watch.clone());
            warn!("watch setup failed: {} for masks {:?}", path, watch.mask);
            return Err(AddWatchError::FolderDoesntExit(watch))
        };

        self.active_watches.entry(descriptor).or_insert(watch);

        Ok(())
    }

    pub fn get_event_stream(&mut self) -> EventStream<[u8; 1024]> {
        let buffer = [0; 1024];
        self.inotify
            .event_stream(buffer)
            .expect("error opening event stream: exiting")
    }

    pub fn recursive_add_watch(&mut self, watch: Watch) {
        for entry in WalkDir::new(&watch.path)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| e.file_type().is_dir())
        {
            let Ok(entry) = entry else {
                continue;
            };

            let watch = Watch {
                path: entry.path().to_path_buf(),
                ..watch.clone()
            };

            match self.add_watch(watch, false, None) {
                Err(AddWatchError::FolderDoesntExit(watch)) => self.failed_watches.insert(watch),
                _ => true,
            };
        }
    }
}
