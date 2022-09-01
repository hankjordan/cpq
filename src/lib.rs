use std::collections::hash_map::{RandomState};
use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::atomic::Ordering::{Acquire, Release, Relaxed};

use std::hash::{Hash, BuildHasher};

use crossbeam::epoch::{self, Atomic, Owned};

struct Node<I, P, H = RandomState> {
    item: ManuallyDrop<I>,
    priority: ManuallyDrop<P>,
    hasher: ManuallyDrop<H>,

    next: Atomic<Node<I, P, H>>,
    next_item: Atomic<Node<I, P, H>>,
    next_priority: Atomic<Node<I, P, H>>,
}

impl<I, P, H> Node<I, P, H> {
    fn new(item: I, priority: P, hasher: H) -> Self {
        Self {
            item: ManuallyDrop::new(item),
            priority: ManuallyDrop::new(priority),
            hasher: ManuallyDrop::new(hasher),
            next: Atomic::null(),
            next_item: Atomic::null(),
            next_priority: Atomic::null(),
        }
    }
}

pub struct ConcurrentPriorityQueue<I, P, H = RandomState>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher + Clone,
{
    hasher: H,

    head: Atomic<Node<I, P, H>>,
    items_head: Atomic<Node<I, P, H>>,
    priorities_head: Atomic<Node<I, P, H>>,
}

impl<I, P, H> Drop for ConcurrentPriorityQueue<I, P, H>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher + Clone,
{
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

impl<I, P> Default for ConcurrentPriorityQueue<I, P>
where
    I: Hash + Eq,
    P: Ord,
{
    fn default() -> Self {
        Self::new()
    }
}


impl<I, P> ConcurrentPriorityQueue<I, P> 
where
    I: Hash + Eq,    
    P: Ord,
{
    pub fn new() -> Self {
        Self::with_default_hasher()
    }
}

impl<I, P, H> ConcurrentPriorityQueue<I, P, H>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher + Clone + Default,
{
    pub fn with_default_hasher() -> Self {
        Self::with_hasher(H::default())
    }
}

impl<I, P, H> ConcurrentPriorityQueue<I, P, H>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher + Clone
{
    pub fn with_hasher(hash_builder: H) -> Self {
        Self {
            hasher: hash_builder,
            head: Atomic::null(),
            items_head: Atomic::null(),
            priorities_head: Atomic::null(),
        }
    }
}

impl<I, P, H> ConcurrentPriorityQueue<I, P, H>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher + Clone
{
    pub fn push(&self, item: I, priority: P) {
        let mut n = Owned::new(Node::new(
            item,
            priority,
            self.hasher.clone(),
        ));

        let guard = epoch::pin();

        loop {
            let head = self.head.load(Relaxed, &guard);

            n.next.store(head, Relaxed);

            match self.head.compare_exchange(head, n, Release, Relaxed, &guard) {
                Ok(_) => break,
                Err(e) => n = e.new,
            }
        }
    }

    pub fn pop(&self) -> Option<(I, P)> {
        let guard = epoch::pin();
        loop {
            let head_shared = self.head.load(Acquire, &guard);
            match unsafe { head_shared.as_ref() } {
                Some(head) => {
                    let next = head.next.load(Relaxed, &guard);
                    if self
                        .head
                        .compare_exchange(head_shared, next, Release, Relaxed, &guard)
                        .is_ok()
                    {
                        unsafe {
                            guard.defer_destroy(head_shared);

                            let item = ManuallyDrop::into_inner(ptr::read(&(*head).item));
                            let priority = ManuallyDrop::into_inner(ptr::read(&(*head).priority));

                            return Some((item, priority));
                        }
                    }
                }
                None => return None,
            }
        }
    }

    pub fn capacity(&self) -> usize {
        todo!()
    }

    pub fn len(&self) -> usize {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        let guard = epoch::pin();
        self.head.load(Acquire, &guard).is_null()
    }
}