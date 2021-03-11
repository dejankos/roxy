use crate::config::Configuration;
use actix_web::dev::Url;
use actix_web::HttpRequest;
use async_trait::async_trait;

pub struct Balancer {
    config: Configuration,
}

#[derive(Debug)]
pub struct Instance {
    pub address: String,
}

impl Balancer {
    pub fn new(config: Configuration) -> Self {
        Balancer { config }
    }

    pub async fn balance(&self, req: &HttpRequest) -> Instance {
        Instance {
            address: "http://127.0.0.1:8080/push".into(),
        }
    }
}
