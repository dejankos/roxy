use crate::config::Configuration;
use actix_web::HttpRequest;

trait Balancer {
    fn balance(&self, req: &HttpRequest) -> Instance;
}

struct Instance {
    server: String,
}

struct RoundRobin {
    config: Configuration,
}

impl Balancer for RoundRobin {
    fn balance(&self, req: &HttpRequest) -> Instance {
        unimplemented!()
    }
}
