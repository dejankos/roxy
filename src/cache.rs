use std::collections::HashMap;

use std::sync::Arc;
use std::thread;
use std::time::Instant;


use actix_web::web::Bytes;
use actix_web::HttpResponse;
use awc::http::{HeaderMap, StatusCode};
use crossbeam::sync::ShardedLock;

use crate::blocking_delay_queue::BlockingDelayQueue;

#[derive(Clone)]
struct CachedResponse {
    status_code: StatusCode,
    headers: HeaderMap,
    body: Bytes,
    ttl: Instant,
}

pub struct Cache {
    map: Arc<ShardedLock<HashMap<Arc<str>, CachedResponse>>>,
    delay_q: Arc<BlockingDelayQueue<Arc<str>>>,
}

impl CachedResponse {
    fn expired(&self) -> bool {
        self.ttl < Instant::now()
    }
}

impl Cache {
    pub fn new(capacity: usize) -> Self {
        let delay_q = Arc::new(BlockingDelayQueue::new(capacity));
        let map = Arc::new(ShardedLock::new(HashMap::with_capacity(capacity)));
        Self::run_cache_expire_thread(delay_q.clone(), map.clone());

        Cache { map, delay_q }
    }

    pub fn store(&self, cache_key: &str, res: &HttpResponse, body: Bytes, ttl: Instant) {
        let status_code = res.status();
        let headers = res.headers().clone();

        let response = CachedResponse {
            status_code,
            body,
            headers,
            ttl,
        };

        let str_ptr: Arc<str> = Arc::from(cache_key);
        self.map
            .write()
            .expect("Cache map lock poisoned!")
            .insert(str_ptr.clone(), response);

        self.delay_q.put(str_ptr.clone(), ttl);
    }

    pub fn get(&self, cache_key: &str) -> Option<CachedResponse> {
        self.map
            .read()
            .expect("Cache map lock poisoned!")
            .get(cache_key)
            .map_or_else(|| None, |r| Some(r.clone())) // fixme
    }

    fn run_cache_expire_thread(
        q: Arc<BlockingDelayQueue<Arc<str>>>,
        map: Arc<ShardedLock<HashMap<Arc<str>, CachedResponse>>>,
    ) {
        thread::Builder::new()
            .name("cache-expire-thread".into())
            .spawn(move || loop {
                let expired_key = q.take();
                map.write()
                    .expect("Cache map lock poisoned!")
                    .remove(&expired_key);
            });
    }
}
