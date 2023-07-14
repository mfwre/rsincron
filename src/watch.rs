use crate::events::EVENT_TYPES;
use inotify::{EventMask, WatchMask};
use std::{
    ffi::OsStr,
    io,
    path::PathBuf,
    process::{self, ExitStatus},
    vec,
};

type WatchResult<T> = std::result::Result<T, ParseWatchError>;

pub enum ParseWatchError {
    IsComment,
    MissingArgument,
}

#[derive(Clone, Debug)]
pub struct Command {
    program: String,
    args: Vec<String>,
}

impl Command {
    pub fn execute(
        &self,
        path: &PathBuf,
        filename: Option<&OsStr>,
        mask_text: EventMask,
        mask_bits: u32,
    ) -> Result<ExitStatus, io::Error> {
        process::Command::new(&self.program)
            .args(self.args.iter().map(|arg| {
                let mut formatted = String::new();
                let mut dollar = false;
                for c in arg.chars() {
                    if c == '$' {
                        if !dollar {
                            dollar = true;
                        } else {
                            formatted.push(c);
                            dollar = false;
                        }
                    } else {
                        if dollar {
                            match c {
                                '#' => formatted.push_str(
                                    filename.map(|s| s.to_str()).flatten().unwrap_or_default(),
                                ),
                                '@' => formatted.push_str(path.to_str().unwrap_or_default()),
                                '%' => formatted.push_str(&format!("\"{:?}\"", mask_text)),
                                '&' => formatted.push_str(&mask_bits.to_string()),
                                _ => formatted.push(c),
                            }
                            dollar = false;
                        } else {
                            formatted.push(c);
                        }
                    }
                }
                formatted
            }))
            .status()
    }
}

#[derive(Debug)]
pub struct WatchData {
    pub path: PathBuf,
    pub mask: WatchMask,
    pub command: Command,
}

impl<'a> WatchData {
    pub fn try_from_str(input: &'a str) -> WatchResult<Self> {
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
