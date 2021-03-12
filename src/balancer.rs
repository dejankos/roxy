use crate::config::Configuration;
use actix_web::dev::Url;
use actix_web::HttpRequest;
use async_trait::async_trait;
use std::sync::Arc;

pub struct Balancer {
    config: Arc<Configuration>,
}

#[derive(Debug)]
pub struct Instance {
    pub address: String,
}

impl Balancer {
    pub fn new(config: Arc<Configuration>) -> Self {
        Balancer { config }
    }

    pub async fn balance(&self, req: &HttpRequest) -> Instance {
        Instance {
            address: "http://127.0.0.1:8080/push".into(),
        }
    }
}
