use std::str::FromStr;

use inotify::WatchMask;

#[derive(Debug)]
pub struct MaskWrapper(pub WatchMask);

impl FromStr for MaskWrapper {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "IN_ACCESS" => Ok(MaskWrapper(WatchMask::ACCESS)),
            "IN_CLOSE_WRITE" => Ok(MaskWrapper(WatchMask::CLOSE_WRITE)),
            "IN_CLOSE_NOWRITE" => Ok(MaskWrapper(WatchMask::CLOSE_NOWRITE)),
            "IN_CREATE" => Ok(MaskWrapper(WatchMask::CREATE)),
            "IN_DELETE" => Ok(MaskWrapper(WatchMask::DELETE)),
            "IN_DELETE_SELF" => Ok(MaskWrapper(WatchMask::DELETE_SELF)),
            "IN_MODIFY" => Ok(MaskWrapper(WatchMask::MODIFY)),
            "IN_MOVE_SELF" => Ok(MaskWrapper(WatchMask::MOVE_SELF)),
            "IN_MOVED_FROM" => Ok(MaskWrapper(WatchMask::MOVED_FROM)),
            "IN_MOVED_TO" => Ok(MaskWrapper(WatchMask::MOVED_TO)),
            "IN_OPEN" => Ok(MaskWrapper(WatchMask::OPEN)),
            "IN_ALL_EVENTS" => Ok(MaskWrapper(WatchMask::ALL_EVENTS)),
            _ => Err(String::from("invalid descriptor")),
        }
    }
}
