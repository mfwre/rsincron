use std::{
    collections::{HashMap, HashSet},
    error, fs,
    path::PathBuf,
};

use inotify::{Inotify, WatchMask};
use serde::Deserialize;

use crate::{
    events::EVENT_TYPES,
    handler::{Handler, Watch, WatchConfig},
};

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct LoggingConfig {
    pub file: PathBuf,
    pub stdout: bool,
    pub level: log::LevelFilter,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            file: PathBuf::from("/var/log/rsincron.log"),
            stdout: true,
            level: log::LevelFilter::Warn,
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct HandlerConfig {
    pub current_user: String,
    pub home_directory: PathBuf,
    pub watch_table: PathBuf,
    pub poll_time: u64,
    pub logging: LoggingConfig,
}

impl Default for HandlerConfig {
    fn default() -> Self {
        let current_user = std::env::var("USER").expect("USER envvar is not set: exiting");
        let home_directory = std::env::var("HOME")
            .map(|home| PathBuf::from(home))
            .expect("HOME envvar is not set: exiting");

        Self {
            current_user,
            home_directory: home_directory.clone(),
            watch_table: home_directory.join(".local/share/rsincron.table"),
            poll_time: 1000,
            logging: LoggingConfig::default(),
        }
    }
}

impl HandlerConfig {
    pub fn setup(self) -> Result<Handler, Box<dyn error::Error>> {
        let table = fs::read_to_string(&self.watch_table)?;
        let event_types = HashMap::from(EVENT_TYPES);
        let mut handler = Handler {
            inotify: Inotify::init()?,
            active_watches: HashMap::new(),
            failed_watches: HashSet::new(),
            handler_config: self,
        };

        for line in table.lines() {
            if line.clone().chars().nth(0) == Some('#') {
                continue;
            };

            let mut fields = line.split('\t');
            let Some(path) = fields.next() else {
                continue;
            };

            let mut mask = WatchMask::empty();
            let mut config = WatchConfig::default();
            let Some(masks) = fields.next() else {
                continue;
            };

            for m in masks.split(',') {
                match event_types.get(m) {
                    Some(m) => mask.insert(*m),
                    _ => match m.split_once('=') {
                        Some(("recursive", value)) => config.recursive = value.parse().unwrap(),
                        _ => continue,
                    },
                }
            }

            let Some(command) = fields.next() else {
                continue;
            };

            let mut watch = Watch {
                path: PathBuf::from(path),
                mask,
                command: command.to_string(),
                config,
            };

            match handler.add_watch(watch.clone(), true, None) {
                Err(_) => {
                    watch.config.table_watch = true;
                    handler.failed_watches.insert(watch.clone())
                }
                _ => true,
            };

            if config.recursive {
                handler.recursive_add_watch(watch);
            }
        }

        Ok(handler)
    }

    pub fn dispatch_log(&self) -> Result<(), Box<dyn error::Error>> {
        let mut dispatch = fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{}[{}][{}] {}",
                    chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                    record.target(),
                    record.level(),
                    message
                ))
            })
            .level(self.logging.level);

        if self.logging.stdout {
            dispatch = dispatch.chain(std::io::stdout());
        }

        if let Ok(log_file) = fern::log_file(&self.logging.file) {
            dispatch = dispatch.chain(log_file);
        }

        Ok(dispatch.apply()?)
    }
}
