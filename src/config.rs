use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::Result;
use crossbeam::sync::ShardedLock;
use log::{debug, error};
use serde::Deserialize;

use crate::file_watcher::FileListener;

const CONFIG_FILE: &str = "proxy.yaml";

#[derive(Debug, Deserialize)]
struct ProxyProperties {
    test: u8,
    groups: Vec<Group>,
    outbounds: Vec<Rack>,
}

#[derive(Debug, Deserialize)]
struct Group {
    path: String,
    outbound: String,
}

#[derive(Debug, Deserialize)]
struct Rack {
    rack: String,
    servers: Vec<String>,
}

struct ProxyConfig {
    props: ProxyProperties,
}

pub struct Configuration {
    proxy_config: ShardedLock<ProxyConfig>,
}

trait FileName {
    fn file_name_to_str(&self) -> &str;
}

impl FileName for &PathBuf {
    fn file_name_to_str(&self) -> &str {
        self.file_name()
            .as_ref()
            .and_then(|os_str| os_str.to_str())
            .unwrap_or("")
    }
}

impl FileListener for Configuration {
    fn notify_file_changed(&self, path: &PathBuf) {
        if !self.interested(path.file_name_to_str()) {
            return;
        }

        debug!("Received change event on {:?}", &path);
        self.reload_config(path);
    }
}

impl Configuration {
    pub fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let props = load_properties(&path)?;
        debug!("Loaded props {:?}", &props);
        Ok(Configuration {
            proxy_config: ShardedLock::new(ProxyConfig { props }),
        })
    }

    fn interested(&self, file_name: &str) -> bool {
        CONFIG_FILE == file_name
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

fn load_properties<P>(path: P) -> Result<ProxyProperties>
where
    P: AsRef<Path>,
{
    Ok(serde_yaml::from_reader(File::open(path)?)?)
}
