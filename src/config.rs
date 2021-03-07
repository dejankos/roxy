use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use crossbeam::sync::{ShardedLock, ShardedLockReadGuard, ShardedLockWriteGuard};
use log::error;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use serde::Deserialize;

use crate::file_watcher::FileListener;

#[derive(Debug, Deserialize)]
struct ProxyProperties {
    test: u8,
}

struct ProxyConfig {
    props: ProxyProperties,
}

pub struct Configuration {
    proxy_config: ShardedLock<ProxyConfig>,
}

impl FileListener for Configuration {
    fn notify_file_changed(&self, path: &PathBuf) {
        self.reload_config(path);
    }
}

impl Configuration {
    pub fn new<P>(p: P) -> Self
    where
        P: Into<PathBuf>,
    {
        let pa = p.into();
        let props = load_properties(&pa).unwrap();

        let config = Configuration {
            proxy_config: ShardedLock::new(ProxyConfig { props }),
        };

        config
    }

    fn reload_config(&self, path: &PathBuf) {
        match load_properties(path) {
            Ok(props) => {
                self.proxy_config
                    .write()
                    .expect("proxy config write lock poisoned!")
                    .props = props;
            }
            Err(e) => {
                error!("Error reloading proxy config. Err = {}", e);
            }
        }
    }
}

fn load_properties(path: &PathBuf) -> Result<ProxyProperties> {
    Ok(serde_yaml::from_reader(File::open(path)?)?)
}
