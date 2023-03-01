use clap::Parser;
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

        let input_data = read_to_string(tmpfile_path).unwrap_or_default();

        // TODO: parsing

        fs::write(&table_path, input_data).expect(&format!(
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
