use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use actix_web::web::Bytes;
use actix_web::{HttpRequest, HttpResponse};
use anyhow::Result;
use crossbeam::sync::ShardedLock;

use crate::http_utils::Headers;
use actix_web::http::{HeaderMap, StatusCode};
use blocking_delay_queue::{BlockingDelayQueue, DelayItem};

type DelayQueue<T> = BlockingDelayQueue<DelayItem<T>>;
const INSERT_TIMEOUT: Duration = Duration::from_millis(1);

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
    delay_q: Arc<DelayQueue<K>>,
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

    pub fn store(&self, cache_key: &str, res: &HttpResponse, body: Bytes) {
        if res.status() != StatusCode::OK {
            return;
        }
        let max_age = res.headers().max_age();
        if max_age.is_none() {
            return;
        }

        let status_code = res.status();
        let headers = res.headers().clone();
        let ttl = Instant::now() + Duration::from_secs(max_age.unwrap());

        let response = CachedResponse {
            status_code,
            body,
            headers,
            ttl,
        };

        let str_ptr: Arc<str> = Arc::from(cache_key);
        self.cache.store(str_ptr, response, ttl);
    }

    pub fn get(&self, cache_key: &str) -> Option<HttpResponse> {
        if let Some(res) = self.cache.get(Arc::from(cache_key)) {
            if res.expired() {
                None
            } else {
                let mut response = HttpResponse::build(res.status_code).body(res.body);
                *response.headers_mut() = res.headers;
                Some(response)
            }
        } else {
            None
        }
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
        let delay_q = Arc::new(BlockingDelayQueue::<DelayItem<K>>::new_with_capacity(
            capacity,
        ));
        let map = Arc::new(ShardedLock::new(HashMap::with_capacity(capacity)));
        Self::run_cache_expire_thread(delay_q.clone(), map.clone())?;

        Ok(Cache {
            map,
            delay_q,
            capacity,
        })
    }

    fn store(&self, k: K, v: V, ttl: Instant) {
        let mut cache = self.map.write().expect("Cache map lock poisoned!");
        if cache.len() < self.capacity {
            // avoid blocking api, len should be same as map
            let success = self
                .delay_q
                .offer(DelayItem::new(k.clone(), ttl), INSERT_TIMEOUT);
            if success {
                cache.insert(k, v);
            }
        }
    }

    fn get(&self, k: K) -> Option<V> {
        self.map
            .read()
            .expect("Cache map lock poisoned!")
            .get(&k)
            .map_or_else(|| None, |v| Some(v.clone())) // fixme
    }

    fn run_cache_expire_thread(
        q: Arc<DelayQueue<K>>,
        map: Arc<ShardedLock<HashMap<K, V>>>,
    ) -> Result<()> {
        thread::Builder::new()
            .name("cache-expire-thread".into())
            .spawn(move || loop {
                let item = q.take();
                map.write()
                    .expect("Cache map lock poisoned!")
                    .remove(&item.data);
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::hash::Hash;
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::cache::Cache;

    impl<K, V> Cache<K, V>
    where
        K: Clone + Ord + Hash + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        fn len(&self) -> usize {
            self.map.read().expect("Cache map lock poisoned!").len()
        }
    }

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
