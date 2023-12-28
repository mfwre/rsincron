use std::{path::PathBuf, str::FromStr};

use crate::watch;
use winnow::{
    ascii::space0,
    combinator::{delimited, rest, separated, terminated},
    stream::AsChar,
    token::take_till,
    PResult, Parser,
};

#[derive(Debug, PartialEq)]
pub enum WatchOption {
    Mask(String),
    Flag(String, bool),
}

impl FromStr for WatchOption {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains('=') {
            let Some((name, value)) = s.split_once('=') else {
                return Err(());
            };

            let Ok(value) = value.parse::<bool>() else {
                return Err(());
            };

            return Ok(Self::Flag(name.to_owned(), value));
        }

        Ok(Self::Mask(s.to_owned()))
    }
}

pub fn parse_path(input: &mut &str) -> PResult<PathBuf> {
    delimited(space0, take_till(0.., AsChar::is_space), space0)
        .parse_to()
        .parse_next(input)
}

pub fn parse_masks(input: &mut &str) -> PResult<Vec<WatchOption>> {
    terminated(
        separated(1.., take_till(0.., (AsChar::is_space, ',')).parse_to(), ","),
        space0,
    )
    .parse_next(input)
}

pub fn parse_command(input: &mut &str) -> PResult<watch::Command> {
    rest.try_map(|r: &str| {
        let argv = shell_words::split(r)?;

        Ok::<watch::Command, shell_words::ParseError>(watch::Command {
            program: argv.first().ok_or(shell_words::ParseError)?.clone(),
            argv: argv[1..].to_vec(),
        })
    })
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use winnow::{combinator::preceded, Parser};

    use crate::parser::{parse_command, parse_masks, parse_path, WatchOption};

    const LINE_DATA: &str = include_str!("../assets/test/test-line");

    #[test]
    fn test_get_path() {
        let mut input = LINE_DATA;
        assert_eq!(parse_path(&mut input).unwrap(), PathBuf::from("/var/tmp"));
    }

    #[test]
    fn test_get_masks() {
        let mut input = LINE_DATA;
        assert_eq!(
            preceded(parse_path, parse_masks)
                .parse_next(&mut input)
                .unwrap(),
            vec![
                WatchOption::Mask(String::from("IN_CREATE")),
                WatchOption::Flag(String::from("recursive"), true),
                WatchOption::Mask(String::from("IN_DELETE"))
            ],
        );
    }

    #[test]
    fn test_get_command() {
        let mut input = LINE_DATA;
        assert_eq!(
            preceded((parse_path, parse_masks), parse_command)
                .parse_next(&mut input)
                .unwrap(),
            crate::watch::Command {
                program: String::from("echo"),
                argv: ["$@", "$#", "&>", "/dev/null"].map(String::from).to_vec()
            }
        );
    }
}
