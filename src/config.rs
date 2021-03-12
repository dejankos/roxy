use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::Result;
use crossbeam::sync::ShardedLock;
use log::{debug, error};
use serde::Deserialize;

use crate::file_watcher::FileListener;
use std::sync::Arc;
use std::collections::HashMap;
use regex::Regex;

const CONFIG_FILE: &str = "proxy.yaml";

#[derive(Debug, Deserialize)]
struct ProxyProperties {
    inbound: Vec<Inbound>,
    outbound: Vec<Outbound>,
}

#[derive(Debug, Deserialize)]
struct Inbound {
    path: String,
    group: String,
}

#[derive(Debug, Deserialize)]
struct Outbound {
    group: String,
    servers: Vec<String>,
}

struct ProxyConfig {
    props: ProxyProperties,
}

pub struct Configuration {
    proxy_config: ShardedLock<ProxyConfig>,
    // path_matchers: ShardedLock<Vec<(Regex, String)>>

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

impl FileListener for Arc<Configuration> {
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
                debug!(
                    "Reloading properties:\n old: {:?} \n new: {:?}",
                    self.proxy_config
                        .read()
                        .expect("proxy config read lock poisoned!")
                        .props,
                    &props
                );
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


// fn create_path_matchers(props:&ProxyProperties) -> Vec<(Regex, String)> {
//     props.inbound.iter()
//         .map(|i| {
//                Regex::new()
//         })
}