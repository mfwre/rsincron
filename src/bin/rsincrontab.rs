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
            if line.clone().chars().nth(0) == Some('#') {
                buf.push_str(line);
                buf.push('\n');
                continue;
            };

            let mut fields = line.split_whitespace();

            let Some(path) = fields.next() else {
                eprintln!("watched path missing: skipping");
                continue
            };

            let Some(masks) = fields.next() else {
                eprintln!("watch masks missing: skipping");
                continue
            };

            let command = String::from_iter(
                fields
                    .map(|item| format!("{item} "))
                    .collect::<Vec<String>>(),
            );

            let types = HashMap::from(EVENT_TYPES);
            let masks = masks
                .split(',')
                .take_while(|m| types.get(m).is_some())
                .collect::<Vec<&str>>()
                .join(",");

            let table_entry = format!("{}\t{}\t{}", path, masks, command);
            buf.push_str(&table_entry);
            buf.push('\n');
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
