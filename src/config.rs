use crate::{watch::WatchData, XDG};
use serde::Deserialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
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
    pub fn parse(self) -> Result<Vec<WatchData>, io::Error> {
        let mut watches = vec![];
        for line in fs::read_to_string(self.watch_table_file)?.lines() {
            if let Ok(watch) = WatchData::try_from_str(line) {
                watches.push(watch);
            }
        }

        Ok(watches)
    }
}
