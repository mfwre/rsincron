use crate::events::EVENT_TYPES;
use inotify::WatchMask;
use std::{path::PathBuf, vec};

type Result<T> = std::result::Result<T, ParseWatchError>;

pub enum ParseWatchError {
    IsComment,
    MissingArgument,
}

#[derive(Clone)]
struct Command {
    program: String,
    args: Vec<String>,
}

pub struct Watch {
    path: PathBuf,
    mask: WatchMask,
    command: Command,
}

impl<'a> Watch {
    pub fn try_from_str(input: &'a str) -> Result<Self> {
        let input = input.trim();
        if input.starts_with('#') {
            return Err(ParseWatchError::IsComment);
        };

        let mut path = None;
        let mut mask = None;
        let mut command = Command {
            program: String::new(),
            args: vec![],
        };

        for substring in input.split_whitespace() {
            if path.is_none() {
                path = Some(PathBuf::from(substring));
                continue;
            };

            if mask.is_none() {
                mask = Some(WatchMask::empty());

                for m in substring.split(',') {
                    match EVENT_TYPES.get(m) {
                        Some(m) => mask.insert(*m),
                        _ => continue,
                    };
                }
                continue;
            }

            if command.program.is_empty() {
                command.program.push_str(substring);
            } else {
                command.args.push(substring.to_string());
            }
        }

        if let (Some(path), Some(mask)) = (path, mask) {
            Ok(Self {
                path,
                mask,
                command,
            })
        } else {
            Err(ParseWatchError::MissingArgument)
        }
    }
}
