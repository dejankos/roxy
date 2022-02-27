use std::collections::HashMap;
use std::sync::Arc;

use std::time::{Duration, Instant};

use actix_web::http::{HeaderMap, StatusCode};
use actix_web::web::Bytes;
use blocking_delay_queue::{BlockingDelayQueue, DelayItem};
use crossbeam::sync::{ShardedLock, ShardedLockWriteGuard};

const INSERT_TIMEOUT: Duration = Duration::from_millis(1);

#[derive(Clone)]
pub struct CachedResponse {
    pub status_code: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub ttl: Instant,
}

pub struct ResponseCache {
    cache: ShardedLock<HashMap<Arc<str>, CachedResponse>>,
    expire_q: BlockingDelayQueue<DelayItem<Arc<str>>>,
    capacity: usize,
}

impl CachedResponse {
    pub fn expired(&self) -> bool {
        self.ttl < Instant::now()
    }
}

impl ResponseCache {
    pub fn with_capacity(capacity: usize) -> Self {
        let cache = ResponseCache {
            cache: ShardedLock::new(HashMap::new()),
            expire_q: BlockingDelayQueue::new_with_capacity(capacity),
            capacity,
        };
        cache
    }

    pub fn expire_head(&self) {
        let item = self.expire_q.take();
        self.cache_write_lock().remove(&item.data);
    }

    pub fn put(&self, k: Arc<str>, v: CachedResponse, ttl: Instant) -> bool {
        let mut cache = self.cache_write_lock();
        if cache.len() < self.capacity {
            // avoid blocking api, len should be same as map
            let success = self
                .expire_q
                .offer(DelayItem::new(k.clone(), ttl), INSERT_TIMEOUT);
            if success {
                cache.insert(k, v);
            }
            success
        } else {
            false
        }
    }

    pub fn get(&self, k: Arc<str>) -> Option<CachedResponse> {
        self.cache
            .read()
            .expect("Cache map lock poisoned!")
            .get(&k)
            .map_or_else(|| None, |v| Some(v.clone()))
    }

    fn cache_write_lock(&self) -> ShardedLockWriteGuard<'_, HashMap<Arc<str>, CachedResponse>> {
        self.cache.write().expect("Cache write lock poisoned!")
    }
}

#[cfg(test)]
mod tests {
    use actix_web::http::{HeaderMap, StatusCode};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use actix_web::web::Bytes;

    use crate::expiring_cache::ResponseCache;
    use crate::CachedResponse;

    impl ResponseCache {
        fn len(&self) -> usize {
            self.cache.read().expect("Cache map lock poisoned!").len()
        }
    }

    #[test]
    fn should_expire_value() {
        let ttl = Duration::from_millis(50);
        let cache = ResponseCache::with_capacity(1);
        let key = Arc::from("1");
        cache.put(key.clone(), dummy_resp(), Instant::now() + ttl);
        assert!(cache.get(key.clone()).is_some());
        thread::sleep(ttl);
        cache.expire_head();
        assert!(cache.get(key.clone()).is_none());
    }

    #[test]
    fn should_not_block_when_capacity_is_reached() {
        let ttl = Instant::now() + Duration::from_millis(50);
        let cache = ResponseCache::with_capacity(1);
        let first_key = Arc::from("1");
        let second_key = Arc::from("2");
        let first = cache.put(first_key.clone(), dummy_resp(), ttl);
        let second = cache.put(second_key.clone(), dummy_resp(), ttl);
        assert_eq!(1, cache.len());
        assert!(first);
        assert!(!second);
        assert!(cache.get(first_key.clone()).is_some());
    }

    fn dummy_resp() -> CachedResponse {
        CachedResponse {
            status_code: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::new(),
            ttl: Instant::now(),
        }
    }
}
