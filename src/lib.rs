pub mod config;
pub mod events;
pub mod parser;
pub mod state;
pub mod watch;

use lazy_static::lazy_static;
use std::{io, path::PathBuf};
use tracing_subscriber::EnvFilter;

use serde::{Deserialize, Serialize};
use xdg::BaseDirectories;

lazy_static! {
    pub static ref XDG: BaseDirectories =
        BaseDirectories::new().expect("failed to get XDG env vars: are they set?");
    pub static ref SOCKET: Result<PathBuf, io::Error> = XDG.place_runtime_file("rsincron.socket");
}

#[derive(Serialize, Deserialize)]
pub enum SocketMessage {
    UpdateWatches,
}

pub fn with_logging() {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}
