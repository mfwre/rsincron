use crate::XDG;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub watch_table_file: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            watch_table_file: XDG
                .place_data_file(Path::new("rsincron.table"))
                .expect("failed to create `rsincron.table`: is XDG_DATA_HOME set?"),
        }
    }
}
