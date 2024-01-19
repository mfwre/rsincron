use clap::{Parser, ValueEnum};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use rsincronlib::{
    config::Config,
    watch::{ParseWatchError, WatchData},
    with_logging, SocketMessage, SOCKET, XDG,
};
use std::{
    fs::{self, File},
    io::{self, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    str::FromStr,
};
use tracing::{event, Level};
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
            .expect("failed to get `rsincron.toml`: do I have permissions?")
            .into_os_string()
        )]
    config: PathBuf,
}

#[tracing::instrument]
fn main() -> ExitCode {
    with_logging();
    let args = Args::parse();
    let editor = std::env::var("EDITOR").unwrap_or("/usr/bin/vi".to_string());

    let config: Config = match Figment::new().join(Toml::file(args.config)).extract() {
        Ok(c) => c,
        Err(error) => {
            event!(
                Level::WARN,
                error = ?error.kind,
                "failed to parse configuration file. Using default configuration"
            );
            Config::default()
        }
    };

    match args.mode {
        Mode::Edit => 'arm: {
            let tmpfile_path = std::env::temp_dir().join(Uuid::new_v4().to_string());
            if Path::new(&config.watch_table_file).exists() {
                fs::copy(&config.watch_table_file, &tmpfile_path)
                    .expect("failed to open tmp file: exiting");
            } else {
                File::create(&tmpfile_path).expect("couldn't open tmp file for writing: exiting");
            };

            let Ok(_exitstatus) = Command::new(editor.clone()).arg(&tmpfile_path).status() else {
                event!(Level::ERROR, editor, "failed to open $EDITOR");
                return ExitCode::FAILURE;
            };

            let mut buf = String::new();
            for line in fs::read_to_string(tmpfile_path).unwrap_or_default().lines() {
                match WatchData::from_str(line) {
                    Ok(_) | Err(ParseWatchError::IsComment) => buf.push_str(&format!("{line}\n")),
                    _ => continue,
                };
            }

            if let Err(error) = fs::write(&config.watch_table_file, buf) {
                event!(Level::ERROR, ?error, filename = ?config.watch_table_file, "failed to write rsincron table");
                return ExitCode::FAILURE;
            }

            let socket = match *SOCKET {
                Ok(ref socket) => socket.to_owned(),
                Err(ref error) => {
                    event!(
                        Level::WARN,
                        ?error,
                        socket = ?*SOCKET,
                        "failed to bind to socket: reload daemon manually"
                    );
                    break 'arm;
                }
            };

            if let Err(error) = UnixStream::connect(&socket).map(|mut stream| {
                stream.write_all(
                    bincode::serialize(&SocketMessage::UpdateWatches)
                        .unwrap()
                        .as_slice(),
                )
            }) {
                event!(
                    Level::WARN,
                    ?error,
                    ?socket,
                    "failed to send update socket message: reload daemon manually"
                );
            }
        }

        Mode::List => {
            let _ = io::stdout().write_all(
                fs::read_to_string(&config.watch_table_file)
                    .unwrap_or_default()
                    .as_bytes(),
            );
        }

        Mode::Remove => {
            if let Err(error) = fs::remove_file(&config.watch_table_file) {
                event!(
                    Level::ERROR,
                    ?error,
                    table = ?config.watch_table_file,
                    "failed to delete table"
                )
            }
        }
    }

    ExitCode::SUCCESS
}
