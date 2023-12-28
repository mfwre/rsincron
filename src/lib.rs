#![feature(lazy_cell)]
pub mod config;
pub mod events;
pub mod parser;
pub mod watch;

use std::{io, path::PathBuf, sync::LazyLock};

use serde::{Deserialize, Serialize};
use xdg::BaseDirectories;

pub static XDG: LazyLock<BaseDirectories> =
    LazyLock::new(|| BaseDirectories::new().expect("failed to get XDG env vars: are they set?"));

pub static SOCKET: LazyLock<Result<PathBuf, io::Error>> =
    LazyLock::new(|| XDG.place_runtime_file("rsincron.socket"));

#[derive(Serialize, Deserialize)]
pub enum SocketMessage {
    UpdateWatches,
}
