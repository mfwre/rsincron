use clap::Parser;
use log::debug;
use rsincronlib::EVENT_TYPES;
use std::{
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

fn main() {
    let args = Args::parse();
    fern::Dispatch::new()
        // Perform allocation-free log formatting
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(fern::log_file("/var/log/rsincron.log").expect("couldn't open logfile: exiting"))
        .apply()
        .expect("logging didnt't start: exiting");

    let editor = std::env::var("EDITOR").unwrap_or(String::from("/usr/bin/vi"));
    let user = std::env::var("USER").expect("USER is not set: exiting");
    let home_dir = std::env::var("HOME").expect("HOME is not set: exiting");

    let rsincron_dir = Path::new(&home_dir)
        .join(".local")
        .join("share")
        .join("rsincron");
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
        File::create(&tmpfile_path).expect("couldn't open tmp file for writing: exiting");

        let _exitstatus = Command::new(editor.clone())
            .arg(&tmpfile_path)
            .status()
            .expect(&format!("failed to open EDITOR ({editor})"));

        let mut buf = String::new();
        for line in read_to_string(tmpfile_path).unwrap_or_default().lines() {
            // TODO: implement all sort of checks plus error logging
            let mut fields = line.split_whitespace();

            let Some(path) = fields.next() else {
                continue
            };

            let Some(masks) = fields.next() else {
                continue
            };

            let command = String::from_iter(
                fields
                    .map(|item| format!("{item} "))
                    .collect::<Vec<String>>(),
            );

            for mask in masks.split(',') {
                if !EVENT_TYPES.contains(&mask) {
                    debug!("{mask} not in EVENT_TYPES: exiting");
                    return;
                }
            }

            let table_entry = format!("{}\t{}\t{}", path, masks, command);
            buf.push_str(&table_entry);
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
    }
}
