use std::collections::HashMap;

use std::time::Duration;

use anyhow::bail;
use anyhow::Result;
use log::error;
use regex::Regex;
use url::Url;

use crate::config::{Group, Outbound, ProxyProperties};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct PathMatcher {
    matchers: Vec<Matcher>,
}

#[derive(Debug, Clone)]
pub struct Matcher {
    regex: Regex,
    group: Group,
}


impl PathMatcher {
    pub fn new(config: & ProxyProperties) -> Result<Self> {
        let matchers = Self::create_path_matchers(config)?;
        Ok(PathMatcher { matchers })
    }

    pub fn rebuild(&mut self, config: & ProxyProperties) -> Result<()> {
        self.matchers = Self::create_path_matchers(config)?;
        Ok(())
    }

    pub fn find_group(&self, req_path: &str) -> Result<Group> {
        if let Some(found) = self.find_matching_group(req_path) {
            Ok(found.group)
        } else {
            bail!("Matching group for request path {} not found", req_path)
        }
    }

    fn find_matching_group(&self, req_path: &str) -> Option<Matcher> {
        self.matchers
            .iter()
            .cloned()
            .find(|m| m.regex.is_match(req_path))
    }


    fn create_path_matchers(props:  &ProxyProperties) -> Result<Vec<Matcher>> {
        let lookup = props
            .outbound
            .iter()
            .map(|o| (o.group.as_str(), o))
            .collect::<HashMap<&str, &Outbound>>();

        Ok(
            props
                .inbound
                .iter()
                .filter_map(|i| {
                    Self::create_matcher(
                        i.group.as_str(),
                        lookup.get(i.group.as_str())
                    ).ok()
                })
                .collect())
    }


    fn create_matcher(path: &str, outbound: Option<&&Outbound>) -> Result<Matcher> {
        if let Some(out) = outbound {
            let regex = Regex::new(&path)?;
            if let Some(group)= Self::convert_to_group(path, out) {
                Ok(
                    Matcher {
                        regex,
                        group
                    }
                )
            }
            else {
                bail!("Matching group for request path {path} not found")
            }
        }
        else {
            bail!("Matching group for request path {path} doesn't contain any servers")
        }
    }


    fn convert_to_group(group: &str, outbound: &Outbound) -> Option<Group> {
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
            Some(Group {
                servers,
                name: group.into(),
                timeout,
            })
        }
    }
}