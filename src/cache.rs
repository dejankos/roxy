use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use actix_web::web::Bytes;
use actix_web::{HttpRequest, HttpResponse};
use anyhow::Result;
use awc::http::{HeaderMap, StatusCode};
use crossbeam::sync::ShardedLock;

use crate::blocking_delay_queue::BlockingDelayQueue;

#[derive(Clone)]
pub struct CachedResponse {
    pub status_code: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub ttl: Instant,
}

pub struct Cache<K, V>
where
    K: Clone + Ord + Hash + Send + Sync,
    V: Clone + Send + Sync,
{
    map: Arc<ShardedLock<HashMap<K, V>>>,
    delay_q: Arc<BlockingDelayQueue<K>>,
    capacity: usize,
}

pub struct ResponseCache {
    cache: Cache<Arc<str>, CachedResponse>,
}

impl CachedResponse {
    fn expired(&self) -> bool {
        self.ttl < Instant::now()
    }
}

impl ResponseCache {
    pub fn new(capacity: usize) -> Result<Self> {
        Ok(ResponseCache {
            cache: Cache::new(capacity)?,
        })
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
        self.cache.store(str_ptr, response, ttl);
    }

    pub fn get(&self, cache_key: &str) -> Option<CachedResponse> {
        self.cache.get(Arc::from(cache_key))
    }

    pub fn build_cache_key(req: &HttpRequest) -> String {
        req.uri().to_string()
    }
}

impl<K, V> Cache<K, V>
where
    K: Clone + Ord + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn new(capacity: usize) -> Result<Self> {
        let delay_q = Arc::new(BlockingDelayQueue::new(capacity));
        let map = Arc::new(ShardedLock::new(HashMap::with_capacity(capacity)));
        Self::run_cache_expire_thread(delay_q.clone(), map.clone())?;

        Ok(Cache {
            map,
            delay_q,
            capacity,
        })
    }

    fn store(&self, k: K, v: V, ttl: Instant) {
        let mut guard = self.map.write().expect("Cache map lock poisoned!");
        if guard.len() < self.capacity {
            guard.insert(k.clone(), v);
            // avoid blocking api, len should be same as map
            let _ = self.delay_q.add(k.clone(), ttl);
        }
    }

    fn get(&self, k: K) -> Option<V> {
        self.map
            .read()
            .expect("Cache map lock poisoned!")
            .get(&k)
            .map_or_else(|| None, |v| Some(v.clone())) // fixme
    }

    fn len(&self) -> usize {
        self.map.read().expect("Cache map lock poisoned!").len()
    }

    fn run_cache_expire_thread(
        q: Arc<BlockingDelayQueue<K>>,
        map: Arc<ShardedLock<HashMap<K, V>>>,
    ) -> Result<()> {
        thread::Builder::new()
            .name("cache-expire-thread".into())
            .spawn(move || loop {
                let k = q.take();
                map.write().expect("Cache map lock poisoned!").remove(&k);
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::cache::Cache;

    #[test]
    fn should_expire_value() {
        let ttl = Duration::from_millis(50);
        let cache = Cache::new(1).unwrap();
        cache.store(1, 2, Instant::now() + ttl);
        assert_eq!(Some(2), cache.get(1));
        thread::sleep(ttl);
        assert_eq!(None, cache.get(1));
    }

    #[test]
    fn should_not_block_when_capacity_is_reached() {
        let ttl = Instant::now() + Duration::from_millis(50);
        let cache = Cache::new(1).unwrap();
        cache.store(1, 2, ttl);
        cache.store(2, 1, ttl);
        assert_eq!(1, cache.len());
        assert_eq!(Some(2), cache.get(1));
    }
}
