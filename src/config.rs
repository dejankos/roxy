use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use anyhow::Result;
use crossbeam::sync::ShardedLock;
use log::{debug, error};
use regex::Regex;
use serde::Deserialize;
use url::Url;

use crate::file_watcher::FileListener;
use crate::matcher::PathMatcher;

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
        let path_matchers = PathMatcher::new(&props)?;
        debug!("Path matchers {:?}", &path_matchers);

        let path_matchers = ShardedLock::new(path_matchers);
        Ok(Configuration {
            proxy_config: ShardedLock::new(ProxyConfig { props }),
            matchers: path_matchers,
        })
    }

    pub fn find_group(&self, req_path: &str) -> Result<Group> {
        if let Some(found) = self.find_match_group(req_path) {
            if let Some(group) = found.1 {
                Ok(group)
            } else {
                bail!(
                    "Matching group for request path {} doesn't contain any servers",
                    req_path
                )
            }
        } else {
            bail!("Matching group for request path {} not found", req_path)
        }
    }

    fn find_match_group(&self, req_path: &str) -> Option<(Regex, Option<Group>)> {
        self.matchers
            .read()
            .expect("proxy config read lock poisoned!")
            .find_matching_group(req_path)
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

fn load_properties<P>(path: P) -> Result<ProxyProperties>
where
    P: AsRef<Path>,
{
    Ok(serde_yaml::from_reader(File::open(path)?)?)
}
