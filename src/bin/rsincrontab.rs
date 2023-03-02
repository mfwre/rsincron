use clap::Parser;
use rsincronlib::events::EVENT_TYPES;
use std::{
    collections::HashMap,
    fs::{self, read_to_string, DirBuilder, File},
    path::Path,
    process::Command,
};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[clap(author, version)]
#[clap(group(
        clap::ArgGroup::new("modes")
            .required(true)
            .args(&["edit", "list", "remove"])
        ))]
struct Args {
    #[clap(short, long)]
    edit: bool,

    #[clap(short, long)]
    list: bool,

    #[clap(short, long)]
    remove: bool,
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
    let user = std::env::var("USER").expect("USER is not set: exiting");
    let rsincron_dir = rsincronlib::get_user_table_path();
    let table_path = rsincron_dir.join(user);

    if args.edit {
        DirBuilder::new()
            .recursive(true)
            .create(&rsincron_dir)
            .expect(&format!(
                "failed to create {} folder: exiting",
                rsincron_dir.to_string_lossy()
            ));

        let tmpfile_path = std::env::temp_dir().join(Uuid::new_v4().to_string());
        if Path::new(&table_path).exists() {
            fs::copy(&table_path, &tmpfile_path).expect("failed to open tmp file: exiting");
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
                Err(err) => eprintln!("{err:?}"),
            }
        }

        fs::write(&table_path, buf).expect(&format!(
            "failed to write to {}: exiting",
            table_path.to_string_lossy()
        ));
    }

    if args.list {
        println!("{}", read_to_string(&table_path).unwrap_or_default());
    }

    if args.remove {
        fs::remove_file(&table_path).expect(&format!(
            "failed to delete {}: exiting",
            table_path.to_string_lossy()
        ));
        println!("user table cleared");
    }
}
