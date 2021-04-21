use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossbeam::sync::ShardedLock;
use log::{debug, error};

use serde::Deserialize;
use url::Url;

use crate::file_watcher::FileListener;
use crate::matcher::PathMatcher;
use crate::utils::yaml_to_struct;

const CONFIG_FILE: &str = "proxy.yaml";

#[derive(Debug, Deserialize)]
pub struct ProxyProperties {
    pub inbound: Vec<Inbound>,
    pub outbound: Vec<Outbound>,
}

#[derive(Debug, Deserialize)]
pub struct Inbound {
    pub path: String,
    pub group: String,
}

#[derive(Debug, Deserialize)]
pub struct Outbound {
    pub timeout: Option<u64>,
    pub group: String,
    pub servers: Vec<String>,
}

#[derive(Debug)]
struct ProxyConfig {
    props: ProxyProperties,
}

pub struct Configuration {
    proxy_config: ShardedLock<ProxyConfig>,
    matchers: ShardedLock<PathMatcher>,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub servers: Vec<Url>,
    pub name: Arc<str>,
    pub timeout: Duration,
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
    fn notify_file_changed(&self, path: &Path) {
        let p = path.to_str();
        if p.is_some() && !self.interested(p.unwrap()) {
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
        let props = yaml_to_struct(&path)?;
        debug!("Loaded props {:?}", &props);
        let path_matchers = PathMatcher::new(&props)?;
        debug!("Path matchers {:?}", &path_matchers);

        let path_matchers = ShardedLock::new(path_matchers);
        Ok(Configuration {
            proxy_config: ShardedLock::new(ProxyConfig { props }),
            matchers: path_matchers,
        })
    }

    pub async fn find_group(&self, req_path: &str) -> Result<Group> {
        self.matchers
            .read()
            .expect("matchers read lock poisoned!")
            .find_group(req_path)
    }

    fn interested(&self, file_name: &str) -> bool {
        CONFIG_FILE == file_name
    }

    fn reload_config(&self, path: &Path) {
        match yaml_to_struct(path) {
            Ok(props) => {
                debug!(
                    "Reloading properties:\n old: {:?} \n new: {:?}",
                    self.proxy_config
                        .read()
                        .expect("proxy config read lock poisoned!")
                        .props,
                    &props
                );

                match self
                    .matchers
                    .write()
                    .expect("matchers write lock poisoned!")
                    .rebuild(&props)
                {
                    Ok(_) => {
                        debug!("Reloaded properties {:?}", &props);
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
            Err(e) => {
                error!("Error loading proxy config. Err = {}", e);
            }
        }
    }
}
