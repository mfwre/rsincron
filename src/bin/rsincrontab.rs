use clap::{Parser, ValueEnum};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use rsincronlib::{config::Config, watch::Watch, XDG};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
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
        default_value = XDG
            .place_config_file("rsincron.toml")
            .expect("failed to get `config.toml`: do I have permissions?")
            .into_os_string()
        )]
    config: PathBuf,
}

fn main() {
    let args = Args::parse();
    let editor = std::env::var("EDITOR").unwrap_or(String::from("/usr/bin/vi"));

    let config: Config = Figment::new()
        .join(Toml::file(args.config))
        .extract()
        .unwrap();
    //.expect(&format!("couldn't parse `{}` file", args.config));

    match args.mode {
        Mode::Edit => {
            let tmpfile_path = std::env::temp_dir().join(Uuid::new_v4().to_string());
            if Path::new(&config.watch_table_file).exists() {
                fs::copy(&config.watch_table_file, &tmpfile_path)
                    .expect("failed to open tmp file: exiting");
            } else {
                File::create(&tmpfile_path).expect("couldn't open tmp file for writing: exiting");
            };

            let _exitstatus = Command::new(editor.clone())
                .arg(&tmpfile_path)
                .status()
                .expect(&format!("failed to open EDITOR ({editor})"));

            let mut buf = String::new();
            for line in fs::read_to_string(tmpfile_path).unwrap_or_default().lines() {
                match Watch::try_from_str(line) {
                    Ok(_) => buf.push_str(&line),
                    _ => continue,
                };
            }

            fs::write(&config.watch_table_file, buf).expect(&format!(
                "failed to write to {}: exiting",
                config.watch_table_file.to_string_lossy()
            ));
        }

        Mode::List => {
            let _ = io::stdout().write_all(
                fs::read_to_string(&config.watch_table_file)
                    .unwrap_or_default()
                    .as_bytes(),
            );
        }

        Mode::Remove => {
            fs::remove_file(&config.watch_table_file).expect(&format!(
                "failed to delete {}: exiting",
                config.watch_table_file.to_string_lossy()
            ));
        }
    }
}
