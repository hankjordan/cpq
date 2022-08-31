use std::collections::hash_map::{RandomState};

use std::hash::{Hash, BuildHasher};

pub mod iterators;
pub mod refs;

use crossbeam_epoch as epoch;

use crossbeam_skiplist_piedb::SkipList;
use dashmap::DashMap;
pub use iterators::*;
pub use refs::*;

pub struct ConcurrentPriorityQueue<I, P, H = RandomState>
where
    I: Hash + Eq,
    P: Ord,
{
    items: DashMap<I, P, H>,
    priorities: SkipList<P, Vec<I>>,
}

impl<I, P> ConcurrentPriorityQueue<I, P> 
where
    I: Hash + Eq,    
    P: Ord,
{
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_default_hasher(capacity)
    }
}

impl<I, P, H> ConcurrentPriorityQueue<I, P, H>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher + Clone + Default,
{
    pub fn with_default_hasher() -> Self {
        Self::with_capacity_and_default_hasher(0)
    }

    pub fn with_capacity_and_default_hasher(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, H::default())
    }
}

impl<I, P, H> ConcurrentPriorityQueue<I, P, H>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher + Clone
{
    pub fn with_hasher(hash_builder: H) -> Self {
        Self::with_capacity_and_hasher(0, hash_builder)
    }

    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: H) -> Self {
        Self {
            items: DashMap::with_capacity_and_hasher(capacity, hash_builder),
            priorities: SkipList::new(epoch::default_collector().clone()),
        }
    }

    /// Returns an iterator in arbitrary order over the
    /// (item, priority) elements in the queue
    pub fn iter(&self) -> dashmap::iter::Iter<'_, I, P, H> {
        self.items.iter()
    }
}

impl<'a, I, P, H> ConcurrentPriorityQueue<I, P, H>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher + Clone
{
    pub fn iter_mut(&mut self) -> dashmap::iter::IterMut<'_, I, P, H> {
        self.items.iter_mut()
    }

    pub fn peek(&'a self) -> Option<Ref<'a, I, P, H>>  {
        let guard = epoch::pin();

        if let Some(entry) = self.priorities.front(&guard) {
            if let Some(key) = entry.value().get(0) {
                let idx = self.items.determine_map(key);

                let shard = unsafe { self.items.shards().get_unchecked(idx).read() };

                let (item, priority) = (key, entry.key());
                
                unsafe {
                    let i_ptr: *const I = item;
                    let p_ptr: *const P = priority;

                    return Some(Ref::new(guard, shard, i_ptr, p_ptr));
                }
            }
        }

        None
    }

    pub fn capacity(&self) -> usize {
        todo!()
    }

    pub fn pop(&self) -> Option<(I, P)> {
        todo!()
    }

    pub fn len(&self) -> usize {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        todo!()
    }
}