use std::collections::HashMap;
use std::fs::File;

use std::path::{Path, PathBuf};

use std::sync::Arc;

use anyhow::bail;
use anyhow::Result;
use crossbeam::sync::ShardedLock;
use crossbeam::sync::ShardedLockWriteGuard;

use log::{debug, error};

use regex::Regex;
use serde::Deserialize;
use url::Url;

use crate::file_watcher::FileListener;
use std::time::Duration;

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
    timeout: Option<u64>,
    group: String,
    servers: Vec<String>,
}

#[derive(Debug)]
struct ProxyConfig {
    props: ProxyProperties,
    path_matchers: Vec<(Regex, Option<Group>)>,
}

pub struct Configuration {
    proxy_config: ShardedLock<ProxyConfig>,
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
        let path_matchers = create_path_matchers(&props)?;
        debug!("Path matchers {:?}", &path_matchers);
        Ok(Configuration {
            proxy_config: ShardedLock::new(ProxyConfig {
                props,
                path_matchers,
            }),
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
        self.proxy_config
            .read()
            .expect("proxy config read lock poisoned!")
            .path_matchers
            .iter()
            .cloned()
            .find(|(r, _g)| r.is_match(req_path))
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

                let mut write_lock = self
                    .proxy_config
                    .write()
                    .expect("proxy config write lock poisoned!");

                match self.reload_path_matchers(&props, &mut write_lock) {
                    Ok(_) => {
                        debug!("Reloaded properties {:?}", &props);
                        write_lock.props = props;
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

    fn reload_path_matchers(
        &self,
        props: &ProxyProperties,
        write_lock: &mut ShardedLockWriteGuard<ProxyConfig>,
    ) -> Result<()> {
        write_lock.path_matchers = create_path_matchers(&props)?;
        Ok(())
    }
}

fn load_properties<P>(path: P) -> Result<ProxyProperties>
where
    P: AsRef<Path>,
{
    Ok(serde_yaml::from_reader(File::open(path)?)?)
}

fn create_path_matchers(props: &ProxyProperties) -> Result<Vec<(Regex, Option<Group>)>> {
    let lookup = props
        .outbound
        .iter()
        .map(|o| (o.group.as_str(), o))
        .collect();

    props
        .inbound
        .iter()
        .map(|i| {
            Ok((
                Regex::new(i.path.as_str())?,
                convert_to_group(&i.group, &lookup),
            ))
        })
        .collect()
}

fn convert_to_group(group: &str, lookup: &HashMap<&str, &Outbound>) -> Option<Group> {
    let mut value = lookup.get(group);
    if let Some(outbound) = value.take() {
        let servers = outbound
            .servers
            .iter()
            .filter_map(|v| {
                if let Ok(url) = Url::parse(v) {
                    Some(url)
                } else {
                    error!("Error parsing configuration url {} for group {}", v, group);
                    None
                }
            })
            .collect::<Vec<Url>>();

        if servers.is_empty() {
            None
        } else {
            let timeout = outbound
                .timeout
                .map_or_else(|| Duration::from_secs(60), |t| Duration::from_secs(t));
            let name = Arc::from(group);
            Some(Group {
                servers,
                name,
                timeout,
            })
        }
    } else {
        None
    }
}
