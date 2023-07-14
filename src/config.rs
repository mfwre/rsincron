use crate::{watch::WatchData, XDG};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

#[derive(Deserialize, Clone)]
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

impl<'a> Config {
    pub fn parse(&self) -> Vec<WatchData> {
        let mut watches = vec![];
        let table_content = match fs::read_to_string(&self.watch_table_file) {
            Ok(table_content) => table_content,
            _ => {
                eprintln!("failed to read `{:?}`", self.watch_table_file);
                process::exit(1);
            }
        };

        for line in table_content.lines() {
            if let Ok(watch) = WatchData::try_from_str(line) {
                watches.push(watch);
            }
        }

        watches
    }
}
