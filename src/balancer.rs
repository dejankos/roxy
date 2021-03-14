use std::collections::HashMap;

use std::sync::Arc;

use actix_web::HttpRequest;
use anyhow::Result;

use crossbeam::sync::ShardedLock;

use crate::config::Configuration;
use url::Url;

pub struct Balancer {
    config: Arc<Configuration>,
    distributions: ShardedLock<HashMap<Arc<str>, usize>>,
}

#[derive(Debug)]
pub struct Instance {
    pub address: String,
}

impl Balancer {
    pub fn new(config: Arc<Configuration>) -> Self {
        let distributions = ShardedLock::new(HashMap::new());
        Balancer {
            config,
            distributions,
        }
    }

    pub async fn balance(&self, req: &HttpRequest) -> Result<Url> {
        let mut group = self.config.find_group(req.path())?;
        let count = self.current_count(group.name);
        let len = group.servers.len();
        Ok(group.servers.remove(count.rem_euclid(len)))
    }

    fn current_count(&self, group_name: Arc<str>) -> usize {
        let mut write_lock = self
            .distributions
            .write()
            .expect("distributions write lock poisoned!");
        if let Some(v) = write_lock.get_mut(&group_name) {
            *v += 1;
            *v
        } else {
            write_lock.insert(group_name, 1);
            1
        }
    }
}
