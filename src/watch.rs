use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    process::{self, ExitStatus},
    str::FromStr,
};

use crate::{
    events::MaskWrapper,
    parser::WatchOption,
    parser::{parse_command, parse_masks, parse_path},
};
use inotify::{Event, WatchMask};
use tracing::{event, Level};
use winnow::{combinator::cut_err, Parser};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub program: String,
    pub argv: Vec<String>,
}

impl Command {
    pub fn execute(&self, path: &Path, event: &Event<OsString>) -> Result<ExitStatus, io::Error> {
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
                            '#' => formatted.push_str(
                                event
                                    .name
                                    .as_deref()
                                    .map(|s| s.to_str().unwrap_or_default())
                                    .unwrap_or_default(),
                            ),
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
pub struct WatchDataAttributes {
    pub starting: bool,
    pub recursive: bool,
}

impl Default for WatchDataAttributes {
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
    pub attributes: WatchDataAttributes,
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
                let mut attributes = WatchDataAttributes::default();

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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use inotify::WatchMask;

    use crate::watch::{Command, ParseWatchError, WatchData, WatchDataAttributes};

    const LINE_DATA: &str = include_str!("../assets/test/test-line");
    const DATA: &str = include_str!("../assets/test/test-table");

    fn get_test_watch() -> WatchData {
        WatchData {
            path: PathBuf::from("/var/tmp"),
            masks: WatchMask::CREATE | WatchMask::DELETE,
            attributes: WatchDataAttributes {
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
