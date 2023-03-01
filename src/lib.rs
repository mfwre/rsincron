use std::path::{Path, PathBuf};

use inotify::WatchMask;

pub const EVENT_TYPES: [(&str, WatchMask); 1] = [
    // "IN_ACCESS",
    // "IN_ATTRIB",
    // "IN_CLOSE_WRITE",
    // "IN_CLOSE_NOWRITE",
    ("IN_CREATE", WatchMask::CREATE),
    // "IN_DELETE",
    // "IN_DELETE_SELF",
    // "IN_MODIFY",
    // "IN_MOVE_SELF",
    // "IN_MOVED_FROM",
    // "IN_MOVED_TO",
    // "IN_OPEN",
    // "IN_IGNORED",
    // "IN_Q_OVERFLOW",
    // "IN_UNMOUNT",
];

pub fn get_user_table_path() -> PathBuf {
    let home_dir = std::env::var("HOME").expect("HOME is not set: exiting");

    Path::new(&home_dir)
        .join(".local")
        .join("share")
        .join("rsincron")
}
