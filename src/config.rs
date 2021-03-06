use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, thread};

use anyhow::Result;
use notify::{watcher, RecursiveMode, Watcher, DebouncedEvent};
use serde::Deserialize;
use std::str::FromStr;
use std::sync::mpsc::channel;
use crate::file_watcher::Notify;

#[derive(Debug, Deserialize)]
struct ProxyConfig {
    test: u8,
}

pub struct Configuration {
    path: PathBuf,
    proxy_config: ProxyConfig,
}

impl Notify for Configuration {
    fn change_event(&mut self, e: DebouncedEvent) {
        unimplemented!()
    }

    fn path(&self) -> &Path {
        unimplemented!()
    }
}

impl Configuration {
    pub fn new<P>(p: P) -> Self
    where
        P: Into<PathBuf>,
    {
        let pa = p.into();
        let config_file = File::open(pa.clone()).unwrap();
        let proxy_config = load_config(&config_file).unwrap();
        let path = PathBuf::from(pa);

        let config = Configuration { path, proxy_config };

        let path = config.path.clone();
        println!("registering thread");
        config
    }
}

fn load_config(file: &File) -> Result<ProxyConfig> {
    Ok(serde_yaml::from_reader(file)?)
}
