use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use crossbeam::sync::ShardedLock;
use crossbeam::sync::ShardedLockWriteGuard;
use log::error;
use regex::Regex;
use std::sync::Arc;
use url::Url;

use crate::config::{Configuration, Group, Outbound, ProxyProperties};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct PathMatcher {
    matchers: Vec<(Regex, Option<Group>)>,
}

impl PathMatcher {
    pub fn new(config: &ProxyProperties) -> Result<Self> {
        let matchers = create_path_matchers(config)?;
        Ok(PathMatcher { matchers })
    }

    pub fn rebuild(&mut self, config: &ProxyProperties) -> Result<()> {
        self.matchers = create_path_matchers(config)?;
        Ok(())
    }

    pub fn find_matching_group(&self, req_path: &str) -> Option<(Regex, Option<Group>)> {
        self.matchers
            .iter()
            .cloned()
            .find(|(r, _g)| r.is_match(req_path))
    }
}

//FIXME
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

//FIXME
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
                .map_or(DEFAULT_TIMEOUT, Duration::from_secs);
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
