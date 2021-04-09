use std::collections::HashMap;
use std::time::Instant;

use actix_web::web::Bytes;
use awc::http::{StatusCode, HeaderMap};
use crossbeam::sync::ShardedLock;

use crate::blocking_delay_queue::BlockingDelayQueue;
use actix_web::HttpResponse;
use actix_web::body::Body;

#[derive(Clone)]
struct CachedResponse {
    status_code: StatusCode,
    headers: HeaderMap,
    body: Bytes,
    ttl: Instant,
}

struct Cache {
    map: ShardedLock<HashMap<String, CachedResponse>>,
    delay_q: BlockingDelayQueue<String>,
}

impl CachedResponse {
    fn expired(&self) -> bool {
        self.ttl < Instant::now()
    }
}

impl Cache {
    fn new(capacity: usize) -> Self {
        Cache {
            map: ShardedLock::new(HashMap::with_capacity(capacity)),
            delay_q: BlockingDelayQueue::new(capacity),
        }
    }

    fn store(&self,  res: &HttpResponse, body: Bytes, ttl: Instant) {
        let status_code = res.status();
        let headers = res.headers().clone();

        let response = CachedResponse {
            status_code,
            body,
            headers,
            ttl
        };
    }

    fn build_key(res: &HttpResponse) -> String {
       "".into()
    }
}
