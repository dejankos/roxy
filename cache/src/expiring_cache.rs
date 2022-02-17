use std::collections::HashMap;
use std::time::{Duration, Instant};

use actix_web::http::{HeaderMap, StatusCode};
use actix_web::web::Bytes;
use blocking_delay_queue::{BlockingDelayQueue, DelayItem};
use crossbeam::sync::ShardedLock;

const INSERT_TIMEOUT: Duration = Duration::from_millis(1);
const POLL_TIMEOUT: Duration = Duration::from_nanos(1);

#[derive(Clone)]
pub struct CachedResponse {
    pub status_code: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub ttl: Instant,
}

pub trait ExpiringCache {
    type K;
    type V;

    fn with_capacity(c: usize) -> Self;

    fn put(&self, k: Self::K, v: Self::V, ttl: Instant) -> bool;

    fn get(&self, k: Self::K) -> Option<Self::V>;

    fn expire(&self);
}

struct ResponseCache<'a> {
    cache: ShardedLock<HashMap<&'a str, CachedResponse>>,
    expire_q: BlockingDelayQueue<DelayItem<&'a str>>,
    capacity: usize,
}

impl<'a> ExpiringCache for ResponseCache<'a> {
    type K = &'a str;
    type V = CachedResponse;

    fn with_capacity(capacity: usize) -> Self {
        ResponseCache {
            cache: ShardedLock::new(HashMap::new()),
            expire_q: BlockingDelayQueue::new_with_capacity(capacity),
            capacity,
        }
    }

    fn put(&self, k: Self::K, v: Self::V, ttl: Instant) -> bool {
        let mut cache = self.cache.write().expect("Cache write lock poisoned!");
        if cache.len() < self.capacity {
            // avoid blocking api, len should be same as map
            let success = self.expire_q.offer(DelayItem::new(k, ttl), INSERT_TIMEOUT);
            if success {
                cache.insert(k, v);
            }
            success
        } else {
            false
        }
    }

    fn get(&self, k: Self::K) -> Option<Self::V> {
        self.cache
            .read()
            .expect("Cache map lock poisoned!")
            .get(k)
            .map_or_else(|| None, |v| Some(v.clone()))
    }

    fn expire(&self) {
        let mut expired = vec![];
        while let Some(v) = self.expire_q.poll(POLL_TIMEOUT) {
            expired.push(v.data)
        }

        if !expired.is_empty() {
            let mut lock = self.cache.write().expect("Cache write lock poisoned");
            expired.into_iter().for_each(|e| {
                lock.remove(e);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::{Duration, Instant};

    use actix_web::http::{HeaderMap, StatusCode};
    use actix_web::web::Bytes;

    use crate::expiring_cache::ResponseCache;
    use crate::{CachedResponse, ExpiringCache};

    impl<'a> ResponseCache<'a> {
        fn len(&self) -> usize {
            self.cache.read().expect("Cache map lock poisoned!").len()
        }
    }

    #[test]
    fn should_expire_value() {
        let ttl = Duration::from_millis(50);
        let cache = ResponseCache::with_capacity(1);
        cache.put("1", dummy_resp(), Instant::now() + ttl);
        assert!(cache.get("1").is_some());
        thread::sleep(ttl);
        cache.expire();
        assert!(cache.get("1").is_none());
    }

    #[test]
    fn should_not_block_when_capacity_is_reached() {
        let ttl = Instant::now() + Duration::from_millis(50);
        let cache = ResponseCache::with_capacity(1);
        let first = cache.put("1", dummy_resp(), ttl);
        let second = cache.put("2", dummy_resp(), ttl);
        assert_eq!(1, cache.len());
        assert!(first);
        assert!(!second);
        assert!(cache.get("1").is_some());
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
