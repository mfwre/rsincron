use std::path::{Path, PathBuf};

use inotify::WatchMask;

pub const EVENT_TYPES: [(&str, WatchMask); 12] = [
    ("IN_ACCESS", WatchMask::ACCESS),
    ("IN_CLOSE_WRITE", WatchMask::CLOSE_WRITE),
    ("IN_CLOSE_NOWRITE", WatchMask::CLOSE_NOWRITE),
    ("IN_CREATE", WatchMask::CREATE),
    ("IN_DELETE", WatchMask::DELETE),
    ("IN_DELETE_SELF", WatchMask::DELETE_SELF),
    ("IN_MODIFY", WatchMask::MODIFY),
    ("IN_MOVE_SELF", WatchMask::MOVE_SELF),
    ("IN_MOVED_FROM", WatchMask::MOVED_FROM),
    ("IN_MOVED_TO", WatchMask::MOVED_TO),
    ("IN_OPEN", WatchMask::OPEN),
    ("IN_ALL_EVENTS", WatchMask::ALL_EVENTS),
];

pub fn get_user_table_path() -> PathBuf {
    let home_dir = std::env::var("HOME").expect("HOME is not set: exiting");

    Path::new(&home_dir)
        .join(".local")
        .join("share")
        .join("rsincron")
}
