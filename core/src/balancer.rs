use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use std::time::Duration;

use actix_web::HttpRequest;
use anyhow::Result;
use crossbeam::sync::ShardedLock;
use url::Url;

use crate::config::Configuration;

pub struct Balancer {
    config: Arc<Configuration>,
    distributions: ShardedLock<HashMap<String, usize>>,
}

#[derive(Debug)]
pub struct Instance {
    pub url: Url,
    pub timeout: Duration,
}

impl Balancer {
    pub fn new(config: Arc<Configuration>) -> Self {
        let distributions = ShardedLock::new(HashMap::new());
        Balancer {
            config,
            distributions,
        }
    }

    pub async fn balance(&self, req: &HttpRequest) -> Result<Instance> {
        let mut group = self.config.find_group(req.path()).await?;
        let count = self.current_count(group.name);
        let len = group.servers.len();
        let url = group.servers.remove(count.rem_euclid(len));

        Ok(Instance {
            url,
            timeout: group.timeout,
        })
    }

    fn current_count(&self, group_name: String) -> usize {
        let mut lock = self
            .distributions
            .write()
            .expect("distributions write lock poisoned!");

        let entry = lock.entry(group_name).or_insert(0);

        *entry = self.next(*entry);
        *entry
    }

    fn next(&self, c: usize) -> usize {
        if c == usize::MAX {
            1
        } else {
            c + 1
        }
    }
}
