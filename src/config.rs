use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use serde::Deserialize;

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
    fn change_event(&self, e: &DebouncedEvent) {
        println!("received event {:?}", e);
    }
}

impl Configuration {
    pub fn new<P>(p: P) -> Self
    where
        P: Into<PathBuf>,
    {
        let pa = p.into();
        let config_file = File::open(&pa).unwrap();
        let proxy_config = load_config(&config_file).unwrap();
        let path = PathBuf::from(pa);

        let config = Configuration { path, proxy_config };

        let _path = config.path.clone();
        println!("registering thread");
        config
    }
}

fn load_config(file: &File) -> Result<ProxyConfig> {
    Ok(serde_yaml::from_reader(file)?)
}
