use std::collections::{BTreeMap, VecDeque};

use parking_lot::RwLock;

use crate::RelaxedCounter;

pub struct ConcurrentPriorityQueue<I, P> {
    buckets: RwLock<BTreeMap<P, RwLock<VecDeque<I>>>>,
    length: RelaxedCounter,
}

impl<I, P> std::fmt::Debug for ConcurrentPriorityQueue<I, P>
where
    I: std::fmt::Debug,
    P: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcurrentPriorityQueue")
            .field("buckets", &self.buckets)
            .finish()
    }
}

impl<I, P> Default for ConcurrentPriorityQueue<I, P> {
    fn default() -> Self {
        Self {
            buckets: RwLock::default(),
            length: RelaxedCounter::default(),
        }
    }
}

impl<I, P> ConcurrentPriorityQueue<I, P> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I, P> ConcurrentPriorityQueue<I, P>
where
    P: Ord,
{
    pub fn push(&self, item: I, priority: P) {
        {
            let buckets = self.buckets.read();

            if let Some(bucket) = buckets.get(&priority) {
                let mut bucket = bucket.write();
                bucket.push_back(item);
                self.length.inc();
                return;
            }
        }

        let mut buckets = self.buckets.write();
        buckets.insert(priority, RwLock::new(VecDeque::from([item])));
        self.length.inc();
    }

    pub fn pop(&self) -> Option<I> {
        let mut result = None;

        {
            let entries = self.buckets.read();
            let buckets = entries.values();

            for bucket in buckets {
                let mut bucket = bucket.write();

                if let Some(item) = bucket.pop_front() {
                    result = Some(item);
                    break;
                }
            }
        }

        if result.is_some() {
            self.length.dec();
            return result;
        }

        None
    }

    pub fn drain(&self, amount: usize) -> impl Iterator<Item = I> + '_ {
        (0..amount).map_while(|_| {
            if let Some(value) = self.pop() {
                return Some(value);
            }

            None
        })
    }

    pub fn len(&self) -> usize {
        self.length.get()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
