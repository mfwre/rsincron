use clap::{Parser, ValueEnum};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use log::{error, info};
use rsincronlib::{events::EVENT_TYPES, handler_config::HandlerConfig};
use std::{
    collections::HashMap,
    fs::{self, read_to_string, File},
    io::{self, Write},
    path::Path,
    process::Command,
};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Mode {
    Edit,
    List,
    Remove,
}

#[derive(Parser, Debug)]
#[clap(author, version)]
struct Args {
    #[clap(value_enum)]
    mode: Mode,

    #[arg(
        short,
        long,
        default_value_t = format!(
            "{}/.config/rsincron.toml",
            std::env::var("HOME")
                .expect("HOME envvar is not set: exiting")))
    ]
    config: String,
}

#[derive(Debug)]
enum ParseError {
    InvalidMask { _mask: String },
    InvalidFlag { _flag: String, _value: String },
    UnknownFlag { _flag: String },
    FieldMissing { _field: String },
}

fn parse_line(line: String) -> Result<String, ParseError> {
    if line.chars().nth(0) == Some('#') {
        return Ok(format!("{line}\n"));
    };

    let mut fields = line.split_whitespace();
    // TODO: do better error logging here
    let path = fields.next().ok_or(ParseError::FieldMissing {
        _field: String::from("path"),
    })?;
    let masks = fields.next().ok_or(ParseError::FieldMissing {
        _field: String::from("mask"),
    })?;

    let command = fields
        .map(|m| m.to_string())
        .collect::<Vec<String>>()
        .join(" ");

    let types = HashMap::from(EVENT_TYPES);
    let mut valid_masks = Vec::new();
    for m in masks.split(',') {
        if types.get(m).is_some() {
            valid_masks.push(m);
            continue;
        } else if m.contains('=') {
            match m.split_once('=') {
                Some(("recursive", value)) => {
                    value.parse::<bool>().map_err(|_| ParseError::InvalidFlag {
                        _flag: String::from("recursive"),
                        _value: value.to_string(),
                    })?;

                    valid_masks.push(m);
                    continue;
                }
                Some((flag, _)) => {
                    return Err(ParseError::UnknownFlag {
                        _flag: flag.to_string(),
                    })
                }
                _ => (),
            }
        }

        return Err(ParseError::InvalidMask {
            _mask: m.to_string(),
        });
    }

    Ok(format!(
        "{}\t{}\t{}\n",
        path,
        valid_masks.join(","),
        command
    ))
}

fn main() {
    let args = Args::parse();
    let editor = std::env::var("EDITOR").unwrap_or(String::from("/usr/bin/vi"));

    let config: HandlerConfig = Figment::new()
        .join(Toml::file(args.config))
        .extract()
        .unwrap();

    config
        .dispatch_log()
        .expect("failed to set up logging: exiting");

    match args.mode {
        Mode::Edit => {
            let tmpfile_path = std::env::temp_dir().join(Uuid::new_v4().to_string());
            if Path::new(&config.watch_table).exists() {
                fs::copy(&config.watch_table, &tmpfile_path)
                    .expect("failed to open tmp file: exiting");
            } else {
                File::create(&tmpfile_path).expect("couldn't open tmp file for writing: exiting");
            };

            let _exitstatus = Command::new(editor.clone())
                .arg(&tmpfile_path)
                .status()
                .expect(&format!("failed to open EDITOR ({editor})"));

            let mut buf = String::new();
            for line in read_to_string(tmpfile_path).unwrap_or_default().lines() {
                match parse_line(line.to_string().clone()) {
                    Ok(line) => buf.push_str(&line),
                    Err(err) => error!("{err:?}"),
                }
            }

            fs::write(&config.watch_table, buf).expect(&format!(
                "failed to write to {}: exiting",
                config.watch_table.to_string_lossy()
            ));
        }

        Mode::List => {
            let _ = io::stdout().write_all(
                read_to_string(&config.watch_table)
                    .unwrap_or_default()
                    .as_bytes(),
            );
            info!("user table saved");
        }

        Mode::Remove => {
            fs::remove_file(&config.watch_table).expect(&format!(
                "failed to delete {}: exiting",
                config.watch_table.to_string_lossy()
            ));
            info!("user table cleared");
        }
    }
}
