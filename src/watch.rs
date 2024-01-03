use std::{
    collections::HashMap,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::{self, ExitStatus},
    str::FromStr,
};

use crate::{
    events::MaskWrapper,
    parser::WatchOption,
    parser::{parse_command, parse_masks, parse_path},
};
use inotify::{Event, Inotify, WatchDescriptor, WatchMask};
use tracing::{event, Level};
use walkdir::WalkDir;
use winnow::{combinator::cut_err, Parser};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub program: String,
    pub argv: Vec<String>,
}

impl Command {
    pub fn execute(&self, path: &Path, event: &Event<&OsStr>) -> Result<ExitStatus, io::Error> {
        process::Command::new(&self.program)
            .args(self.argv.iter().map(|arg| {
                let mut formatted = String::new();
                let mut parsing_dollar = false;

                for c in arg.chars() {
                    if c == '$' {
                        if parsing_dollar {
                            formatted.push(c);
                        }
                        parsing_dollar = !parsing_dollar;
                    } else if parsing_dollar {
                        match c {
                            '#' => formatted
                                .push_str(event.name.and_then(|s| s.to_str()).unwrap_or_default()),
                            '@' => formatted.push_str(path.to_str().unwrap_or_default()),
                            '%' => formatted.push_str(&format!("\"{:?}\"", event.mask)),
                            '&' => formatted.push_str(&event.mask.bits().to_string()),
                            _ => formatted.push(c),
                        }
                        parsing_dollar = false;
                    } else {
                        formatted.push(c);
                    }
                }
                formatted
            }))
            .status()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseWatchError {
    InvalidMask,
    IsComment,
    CorruptInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attributes {
    pub starting: bool,
    pub recursive: bool,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            starting: true,
            recursive: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchData {
    pub path: PathBuf,
    pub masks: WatchMask,
    pub command: Command,
    pub attributes: Attributes,
}

impl FromStr for WatchData {
    type Err = ParseWatchError;

    #[tracing::instrument]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.starts_with('#') {
            return Err(ParseWatchError::IsComment);
        };

        (
            cut_err(parse_path),
            cut_err(parse_masks),
            cut_err(parse_command),
        )
            .map(|(path, watch_options, command)| {
                let mut masks = WatchMask::empty();
                let mut attributes = Attributes::default();

                for option in watch_options {
                    match option {
                        WatchOption::Mask(mask) => {
                            let mask = match mask.parse::<MaskWrapper>() {
                                Ok(m) => m,
                                Err(_) => {
                                    event!(Level::ERROR, mask, "invalid mask");
                                    return Err(ParseWatchError::InvalidMask);
                                }
                            };

                            masks = masks.union(mask.0);
                        }
                        WatchOption::Attribute(flag, value) => match flag.as_str() {
                            "recursive" => attributes.recursive = value,
                            _ => continue,
                        },
                    }
                }

                Ok(WatchData {
                    path,
                    command,
                    masks,
                    attributes,
                })
            })
            .parse(s)
            .map_err(|error| {
                event!(Level::ERROR, ?error);
                ParseWatchError::CorruptInput
            })?
    }
}

#[derive(Debug, Default)]
pub struct Watches(pub HashMap<WatchDescriptor, WatchData>);

impl Watches {
    #[tracing::instrument(skip(self, inotify))]
    pub fn reload_table(&mut self, table: &PathBuf, inotify: &mut Inotify) {
        event!(Level::DEBUG, ?self, "reloading watches");
        let table_content = match fs::read_to_string(table) {
            Ok(table_content) => table_content,
            _ => {
                event!(Level::ERROR, filename = ?table, "failed to read file");
                panic!("failed to read watch table file");
            }
        };

        let watches = table_content
            .lines()
            .map(|line| {
                let watch = match WatchData::from_str(line) {
                    Ok(w) => w,
                    Err(error) => {
                        event!(Level::WARN, ?error, line, "failed to parse line");
                        return None;
                    }
                };

                let Ok(descriptor) = inotify.watches().add(&watch.path, watch.masks) else {
                    event!(Level::WARN, "failed to add watch");
                    return None;
                };

                event!(Level::DEBUG, ?descriptor, ?watch, "adding watch");
                Some((descriptor, watch))
            })
            .take_while(Option::is_some)
            .map(Option::unwrap);

        let recursive_watches = watches
            .clone()
            .filter(|w| w.1.attributes.recursive && w.1.masks.contains(WatchMask::CREATE))
            .map(|(_, w)| {
                let mut tmp = vec![];
                for entry in WalkDir::new(&w.path).min_depth(1) {
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
                        path: w.path.join(entry.path()),
                        attributes: Attributes {
                            starting: false,
                            recursive: true,
                        },
                        ..w.clone()
                    };

                    let Ok(descriptor) = inotify.watches().add(&watch.path, watch.masks) else {
                        event!(Level::WARN, "failed to add watch");
                        continue;
                    };

                    event!(Level::DEBUG, ?descriptor, ?watch, "adding watch");
                    tmp.push((descriptor, watch));
                }
                tmp
            })
            .flatten();

        self.0.clear();
        self.0.extend(watches);
        self.0.extend(recursive_watches);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use inotify::WatchMask;

    use crate::watch::{Attributes, Command, ParseWatchError, WatchData};

    const LINE_DATA: &str = include_str!("../assets/test/test-line");
    const DATA: &str = include_str!("../assets/test/test-table");

    fn get_test_watch() -> WatchData {
        WatchData {
            path: PathBuf::from("/var/tmp"),
            masks: WatchMask::CREATE | WatchMask::DELETE,
            attributes: Attributes {
                starting: true,
                recursive: true,
            },
            command: Command {
                program: String::from("echo"),
                argv: ["$@", "$#", "&>", "/dev/null"].map(String::from).to_vec(),
            },
        }
    }

    #[test]
    fn test_parse_line() {
        assert_eq!(LINE_DATA.parse::<WatchData>().unwrap(), get_test_watch());
    }

    #[test]
    fn test_parse_table() {
        assert_eq!(
            DATA.lines()
                .map(|l| l.parse::<WatchData>())
                .collect::<Vec<Result<WatchData, ParseWatchError>>>(),
            vec![
                Ok(get_test_watch()),
                Ok(get_test_watch()),
                Err(ParseWatchError::InvalidMask),
                Err(ParseWatchError::IsComment),
                Err(ParseWatchError::CorruptInput),
            ]
        )
    }
}
