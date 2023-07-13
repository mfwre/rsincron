#![feature(lazy_cell)]
pub mod config;
pub mod events;
pub mod watch;

// Old libraries
// pub mod handler;
// pub mod handler_config;

use std::sync::LazyLock;

use xdg::BaseDirectories;

pub static XDG: LazyLock<BaseDirectories> =
    LazyLock::new(|| BaseDirectories::new().expect("failed to get XDG env vars: are they set?"));
