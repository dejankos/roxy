use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Instant};

type MinHeap<T> = BinaryHeap<Reverse<DelayItem<T>>>;

pub struct BlockingDelayQueue<T>
where
    T: Ord,
{
    heap: Mutex<MinHeap<T>>,
    condvar: Condvar,
    capacity: usize,
}

struct DelayItem<T> {
    data: T,
    delay: Instant,
}

impl<T> Ord for DelayItem<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.delay.cmp(&other.delay)
    }
}

impl<T> PartialOrd for DelayItem<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.delay.cmp(&other.delay))
    }
}

impl<T> PartialEq for DelayItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.delay == other.delay
    }
}

impl<T> Eq for DelayItem<T> {}

impl<T> DelayItem<T> {
    fn is_expired(&self) -> bool {
        self.delay <= Instant::now()
    }
}

impl<T> BlockingDelayQueue<T>
where
    T: Ord,
{
    pub fn new(capacity: usize) -> Self {
        BlockingDelayQueue {
            heap: Mutex::new(BinaryHeap::with_capacity(capacity)),
            condvar: Condvar::new(),
            capacity,
        }
    }

    pub fn put(&self, data: T, delay: Instant) {
        let heap_mutex = self.heap.lock().expect("Queue lock poisoned");
        if heap_mutex.len() < self.capacity {
            Self::push(heap_mutex, data, delay);
        } else {
            let mutex = self
                .condvar
                .wait_while(heap_mutex, |h| h.len() >= self.capacity)
                .expect("Queue lock poisoned");
            Self::push(mutex, data, delay);
        }
        self.condvar.notify_one();
    }

    fn push(mut mutex: MutexGuard<MinHeap<T>>, data: T, delay: Instant) {
        let item = DelayItem { data, delay };
        mutex.push(Reverse(item));
    }

    pub fn take(&self) -> T {
        let item = self.take_inner();
        self.condvar.notify_one();

        item
    }

    fn take_inner(&self) -> T {
        let guard = self.heap.lock().expect("Queue lock poisoned");
        let condition = |heap: &mut MinHeap<T>| {
            heap.peek()
                .map_or(true, |item| item.0.delay > Instant::now())
        };

        if let Some(item) = guard.peek() {
            if item.0.is_expired() {
                Self::pop(guard)
            } else {
                let delay = item.0.delay - Instant::now();
                let (guard, _) = self
                    .condvar
                    .wait_timeout_while(guard, delay, condition)
                    .expect("Queue lock poisoned");
                Self::pop(guard)
            }
        } else {
            let guard = self
                .condvar
                .wait_while(guard, condition)
                .expect("Queue lock poisoned");
            Self::pop(guard)
        }
    }

    fn pop(mut mutex: MutexGuard<MinHeap<T>>) -> T {
        mutex.pop().unwrap().0.data
    }

    fn size(&self) -> usize {
        self.heap.lock().expect("Queue lock poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::blocking_delay_queue::BlockingDelayQueue;

    // todo timeout on tests
    #[test]
    fn should_put_and_take_ordered() {
        let queue = BlockingDelayQueue::new(2);
        queue.put(1, Instant::now());
        queue.put(2, Instant::now());

        assert_eq!(1, queue.take());
        assert_eq!(2, queue.take());
        assert_eq!(0, queue.size());
    }

    #[test]
    fn should_put_and_take_delayed_items() {
        let queue = BlockingDelayQueue::new(2);
        queue.put(1, Instant::now() + Duration::from_secs(2));
        queue.put(2, Instant::now());

        assert_eq!(2, queue.take());
        assert_eq!(1, queue.take());
        assert_eq!(0, queue.size());
    }

    #[test]
    fn should_put_and_take_delayed_items_1() {
        let queue = BlockingDelayQueue::new(2);
        queue.put(1, Instant::now() + Duration::from_secs(2));

        assert_eq!(1, queue.take());
        assert_eq!(0, queue.size());
    }

    #[test]
    fn should_block_until_item_is_available() {
        let queue = Arc::new(BlockingDelayQueue::new(2));
        let queue_rc = queue.clone();
        let handle = thread::spawn(move || queue_rc.take());
        queue.put(1, Instant::now() + Duration::from_millis(50));
        let res = handle.join().unwrap();
        assert_eq!(1, res);
        assert_eq!(0, queue.size());
    }

    #[test]
    fn should_block_until_item_can_be_put() {
        let queue = Arc::new(BlockingDelayQueue::new(1));
        queue.put(1, Instant::now() + Duration::from_secs(2));
        let queue_rc = queue.clone();
        let handle = thread::spawn(move || queue_rc.put(2, Instant::now()));
        assert_eq!(1, queue.take());
        handle.join().unwrap();
        assert_eq!(1, queue.size());
        assert_eq!(2, queue.take());
    }
}
