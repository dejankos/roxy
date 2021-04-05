use std::time::Instant;

use actix_web::web::Bytes;

use awc::http::StatusCode;

#[derive(Clone)]
struct CachedResponse {
    status_code: StatusCode,
    body: Bytes,
    ttl: Instant,
}

struct Cache {
    //todo
}

impl CachedResponse {
    fn expired(&self) -> bool {
        self.ttl < Instant::now()
    }
}
