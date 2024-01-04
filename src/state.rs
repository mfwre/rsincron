use crate::{
    config::Config,
    watch::{WatchData, WatchDataAttributes},
    SocketMessage, SOCKET,
};
use inotify::{Inotify, WatchDescriptor, WatchMask};
use tracing::{event, Level};

use std::{
    collections::HashMap,
    fs,
    io::Read,
    os::unix::net::UnixListener,
    path::Path,
    str::FromStr,
    sync::mpsc::Sender,
    sync::mpsc::{self, Receiver},
    thread,
};

#[tracing::instrument(skip_all)]
fn setup_watch(
    map: &mut HashMap<WatchDescriptor, WatchData>,
    watches: &mut inotify::Watches,
    watch: WatchData,
) {
    let Ok(descriptor) = watches.add(&watch.path, watch.masks) else {
        event!(Level::WARN, "failed to add watch");
        return;
    };

    event!(
        Level::INFO,
        id = descriptor.get_watch_descriptor_id(),
        ?watch.path,
        ?watch.masks,
        "ADD"
    );

    if watch.attributes.recursive && watch.masks.contains(WatchMask::CREATE) {
        for entry in fs::read_dir(&watch.path).unwrap() {
            let Ok(entry) = entry else {
                continue;
            };

            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if !metadata.is_dir() {
                continue;
            }

            let watch = WatchData {
                path: watch.path.join(entry.file_name()),
                attributes: WatchDataAttributes {
                    starting: false,
                    recursive: true,
                },
                ..watch.clone()
            };

            setup_watch(map, watches, watch)
        }
    };

    map.insert(descriptor, watch);
}

#[tracing::instrument(skip_all)]
fn setup_socket(tx: Sender<SocketMessage>) -> bool {
    let Ok(ref socket) = *SOCKET else {
        event!(Level::WARN, error = ?SOCKET.as_deref().unwrap_err(), "failed to get socket path");
        return false;
    };

    if Path::new(&socket).exists() {
        if let Err(error) = fs::remove_file(&socket) {
            event!(Level::WARN, ?error, "failed to remove existing socket");
            return false;
        }
    }

    let listener = match UnixListener::bind(socket) {
        Ok(l) => l,
        Err(error) => {
            event!(Level::WARN, ?error, "failed to bind to socket");
            return false;
        }
    };

    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buffer = [0; 100];
            if stream.read(&mut buffer).is_err() {
                return;
            }

            let Ok(SocketMessage::UpdateWatches) = bincode::deserialize(&buffer) else {
                return;
            };

            if let Err(error) = tx.send(SocketMessage::UpdateWatches) {
                event!(
                    Level::WARN,
                    ?error,
                    "failed to send update message through channel"
                );
            }
        }
    });

    true
}

type Watches = HashMap<WatchDescriptor, WatchData>;

pub struct State {
    pub failed_watches: Vec<WatchData>,
    pub has_socket: bool,
    pub rx: Receiver<SocketMessage>,

    config: Config,
    inotify_watches: inotify::Watches,
    watches: Watches,
}

impl State {
    pub fn new(inotify: &mut Inotify, config: Config) -> Self {
        let (tx, rx) = mpsc::channel();

        Self {
            rx,
            config,
            watches: HashMap::new(),
            failed_watches: Vec::new(),
            has_socket: setup_socket(tx),
            inotify_watches: inotify.watches(),
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn reload_watches(&mut self) {
        self.watches.clear();
        event!(Level::INFO, table = ?self.config.watch_table_file, "RELOAD");
        event!(Level::DEBUG, ?self.watches);
        let table_content = match fs::read_to_string(&self.config.watch_table_file) {
            Ok(table_content) => table_content,
            _ => {
                event!(Level::ERROR, filename = ?self.config.watch_table_file, "failed to read file");
                panic!("failed to read watch table file");
            }
        };

        for line in table_content.lines() {
            let watch = match WatchData::from_str(line) {
                Ok(w) => w,
                Err(error) => {
                    event!(Level::WARN, ?error, line, "failed to parse line");
                    continue;
                }
            };

            setup_watch(&mut self.watches, &mut self.inotify_watches, watch);
        }
    }

    pub fn recover_watches(&mut self) {
        self.failed_watches.retain(|watch| {
            let Ok(descriptor) = self.inotify_watches.add(watch.path.clone(), watch.masks) else {
                return true;
            };

            event!(
                Level::INFO,
                id = descriptor.get_watch_descriptor_id(),
                ?watch.path,
                ?watch.masks,
                "ADD"
            );
            self.watches.insert(descriptor, watch.clone());
            false
        });
    }

    pub fn get_watch(&self, wd: &WatchDescriptor) -> Option<&WatchData> {
        self.watches.get(wd)
    }

    pub fn remove_watch(&mut self, wd: &WatchDescriptor) -> Option<WatchData> {
        self.watches.remove(wd)
    }
}
