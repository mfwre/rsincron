pub mod events;
pub mod handler;

use std::path::{Path, PathBuf};

pub fn get_user_table_path() -> PathBuf {
    let home_dir = std::env::var("HOME").expect("HOME is not set: exiting");

    Path::new(&home_dir)
        .join(".local")
        .join("share")
        .join("rsincron")
}
