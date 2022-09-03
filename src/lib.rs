#![allow(clippy::collapsible_else_if)]

use std::collections::hash_map::{RandomState};

use std::hash::{Hash, BuildHasher};

use crossbeam::epoch;

use crate::raw::RawCPQ;

mod raw;


pub struct ConcurrentPriorityQueue<I, P, H = RandomState> {
    inner: RawCPQ<I, P, H>
}

impl<I, P, H> Default for ConcurrentPriorityQueue<I, P, H>
where
    H: Default
{
    fn default() -> Self {
        Self::with_hasher(H::default())
    }
}


impl<I, P> ConcurrentPriorityQueue<I, P> 
{
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I, P, H> ConcurrentPriorityQueue<I, P, H>
{
    pub fn with_hasher(hash_builder: H) -> Self {
        Self {
            inner: RawCPQ::new(epoch::default_collector().clone(), hash_builder)
        }
    }
}

impl<I, P, H> ConcurrentPriorityQueue<I, P, H>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher
{
    pub fn push(&self, item: I, priority: P) {
        let guard = &epoch::pin();
        self.inner.push(item, priority, guard);
    }

    pub fn pop(&self) -> Option<(I, P)> {
        todo!()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}