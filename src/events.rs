use std::{collections::HashMap, sync::LazyLock};

use inotify::WatchMask;

pub static EVENT_TYPES: LazyLock<HashMap<&str, WatchMask>> = LazyLock::new(|| {
    HashMap::from([
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
    ])
});
