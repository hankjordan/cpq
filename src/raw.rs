use std::{
    borrow::Borrow,
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hash, Hasher},
    mem::{ManuallyDrop, self},
    sync::atomic::{AtomicUsize, fence}, ptr, cmp::Ordering, alloc::{dealloc, Layout},
};

use core::ops::{Bound, Deref, Index, RangeBounds};

use std::sync::atomic::Ordering::{SeqCst, Acquire, Release, Relaxed};

use crossbeam::epoch::{Atomic, Collector, Guard, Shared, self, Owned};

struct Node<I, P> {
    item: I,
    priority: P,

    refs: AtomicUsize,

    next_item: Atomic<Node<I, P>>,
    next_priority: Atomic<Node<I, P>>,
}

impl<I, P> Node<I, P> {
    fn new(item: I, priority: P, refs: usize) -> Self {
        Self {
            item,
            priority,

            refs: AtomicUsize::new(refs),

            next_item: Atomic::null(),
            next_priority: Atomic::null(),
        }
    }

    unsafe fn alloc(item: I, priority: P, refs: usize) -> *mut Self {
        Box::into_raw(Box::new(Self::new(item, priority, refs)))
    }

    unsafe fn finalize(ptr: *const Self) {
        Box::from_raw(ptr as *mut Self);
    }

    /// Decrements the reference count of a node, destroying it if the count becomes zero.
    #[inline]
    unsafe fn decrement(&self, guard: &Guard) {
        if self
            .refs
            .fetch_sub(1, Release)
            == 1
        {
            fence(Acquire);
            guard.defer_unchecked(move || Self::finalize(self));
        }
    }

    fn mark(&self) -> bool {
        let tag_item = self.next_item.fetch_or(1, SeqCst, unsafe { epoch::unprotected() }).tag();
        let tag_priority = self.next_priority.fetch_or(1, SeqCst, unsafe { epoch::unprotected()}).tag();

        if tag_item == 1 || tag_priority == 1 {
            return false;
        }

        true
    }

    #[inline]
    unsafe fn try_increment(&self) -> bool {
        let mut refs = self.refs.load(Relaxed);

        loop {
            // If the reference count is zero, then the node has already been
            // queued for deletion. Incrementing it again could lead to a
            // double-free.
            if refs == 0 {
                return false;
            }

            // If all bits in the reference count are ones, we're about to overflow it.
            let new_refs = refs
                .checked_add(1)
                .expect("ConcurrentPriorityQueue reference count overflow");

            // Try incrementing the count.
            match self.refs.compare_exchange_weak(
                refs,
                new_refs,
                Relaxed,
                Relaxed,
            ) {
                Ok(_) => return true,
                Err(current) => refs = current,
            }
        }
    }
}

pub struct RawCPQ<I, P, H = RandomState>
{
    collector: Collector,
    hasher: H,

    length: AtomicUsize,

    items: Atomic<Node<I, P>>,
    priorities: Atomic<Node<I, P>>,
}

impl<I, P, H> RawCPQ<I, P, H>
where
    I: Hash + Eq,
    P: Ord,
    H: BuildHasher + Clone,
{
    pub fn with_collector_and_hasher(collector: Collector, hasher: H) -> Self {
        Self {
            collector,
            hasher,
            length: AtomicUsize::new(0),
            items: Atomic::null(),
            priorities: Atomic::null(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        let len = self.length.load(Relaxed);

        if len > isize::max_value() as usize {
            0
        } else {
            len
        }
    }

    fn check_guard(&self, guard: &Guard) {
        if let Some(c) = guard.collector() {
            assert!(c == &self.collector);
        }
    }
}

impl<I, P> RawCPQ<I, P>
where
    I: Hash + Eq,
    P: Ord,
{
    pub fn get_priority<'a: 'g, 'g, Q>(&'a self, priority: &Q, guard: &'g Guard) -> Option<Entry<'a, 'g, I, P>>
    where
        P: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.check_guard(guard);
        let n = self.search_priority_bound(Bound::Included(priority), false, guard)?;

        if n.priority.borrow() != priority {
            return None;
        }

        Some(Entry {
            parent: self,
            node: n,
            guard,
        })
    }

    fn search_priority_bound<'a, Q>(
        &'a self,
        bound: Bound<&Q>,
        upper_bound: bool,
        guard: &'a Guard,
    ) -> Option<&'a Node<I, P>>
    where
        P: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let mut result = None;
        let mut prev = &self.priorities;
        let mut curr = prev.load_consume(guard);

        'search: loop {

            while let Some(c) = unsafe { curr.as_ref() } {
                let next = c.next_priority.load_consume(guard);

                if next.tag() == 1 {
                    if let Some(c) = unsafe { self.help_unlink(prev, c, next, guard) } {
                        curr = c;
                        continue;
                    } else {
                        continue 'search;
                    }
                }

                if upper_bound {
                    if !below_upper_bound(&bound, c.priority.borrow()) {
                        break;
                    }
                    result = Some(c);
                } else if above_lower_bound(&bound, c.priority.borrow()) {
                    result = Some(c);
                    break;
                }

                prev = &c.next_priority;
                curr = next;
            }

            return result;
        }
    }

    fn search_priority_position<'a, Q>(
        &'a self, 
        priority: &Q, 
        guard: &'a Guard
    ) -> Position<'a, I, P>
    where
        P: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let mut result = Position {
            found: None,
            left: &self.priorities,
            right: Shared::null(),
        };

        let mut prev = &self.priorities;
        let mut curr = prev.load_consume(guard);

        'search: loop {

            while let Some(c) = unsafe { curr.as_ref() } {
                let next = c.next_priority.load_consume(guard);

                if next.tag() == 1 {
                    if let Some(c) = unsafe { self.help_unlink(prev, c, next, guard) } {
                        curr = c;
                        continue;
                    } else {
                        continue 'search;
                    }
                }

                match c.priority.borrow().cmp(priority) {
                    Ordering::Greater => break,
                    Ordering::Equal => {
                        result.found = Some(c);
                        break;
                    }
                    Ordering::Less => {}
                }

                prev = &c.next_priority;
                curr = next;

                result.left = prev;
                result.right = curr;
            }

            return result;
        }
    }

    fn search_item<'a, Q>(
        &'a self, 
        item: &Q, 
        guard: &'a Guard
    ) -> Position<'a, I, P>
    where
        I: Borrow<Q>,
        Q: Hash + Eq,
    {
        let mut hasher = self.hasher.build_hasher();

        item.hash(&mut hasher);
        let hash = hasher.finish();

        let mut result = Position {
            found: None,
            left: &self.items,
            right: Shared::null(),
        };

        let mut prev = &self.items;
        let mut curr = prev.load_consume(guard);

        'search: loop {

            while let Some(c) = unsafe { curr.as_ref() } {
                let next = c.next_item.load_consume(guard);

                if next.tag() == 1 {
                    if let Some(c) = unsafe { self.help_unlink(prev, c, next, guard) } {
                        curr = c;
                        continue;
                    } else {
                        continue 'search;
                    }
                }

                c.item.borrow().hash(&mut hasher);
                let curr_hash = hasher.finish();

                match hash.cmp(&curr_hash) {
                    Ordering::Greater => break,
                    Ordering::Equal => {
                        result.found = Some(c);
                        break;
                    }
                    Ordering::Less => {}
                }

                prev = &c.next_item;
                curr = next;

                result.left = prev;
                result.right = curr;
            }

            return result;
        }
    }

    #[cold]
    unsafe fn help_unlink<'a>(
        &'a self,
        prev: &'a Atomic<Node<I, P>>,
        curr: &'a Node<I, P>,
        next: Shared<'a, Node<I, P>>,
        guard: &'a Guard,
    ) -> Option<Shared<'a, Node<I, P>>> {
        // If `next` is marked, that means `curr` is removed.
        // Let's try unlinking it from the list.
        match prev.compare_exchange(
            Shared::from(curr as *const _),
            next.with_tag(0),
            Release,
            Relaxed,
            guard,
        ) {
            Ok(_) => {
                curr.decrement(guard);
                Some(next.with_tag(0))
            }
            Err(_) => None,
        }
    }

    fn insert_internal(
        &self,
        item: I,
        priority: P,
        replace: bool,
        guard: &Guard,
    ) -> RefEntry<'_, I, P>
    {
        self.check_guard(guard);

        unsafe {
            // Rebind the guard to the lifetime of self. This is a bit of a
            // hack but it allows us to return references that are not bound to
            // the lifetime of the guard.
            let guard = &*(guard as *const _);

            let mut search;
            loop {
                // First try searching for the key.
                // Note that the `Ord` implementation for `K` may panic during the search.
                search = self.search_item(&item, guard);

                let r = match search.found {
                    Some(r) => r,
                    None => break,
                };

                if replace {
                    // If a node with the key was found and we should replace it.
                    // Mark it as removed and then repeat the search.
                    if r.mark() {
                        self.length.fetch_sub(1, Relaxed);
                    }
                } else {
                    // If a node with the key was found and we're not going to replace it, let's
                    // try returning it as an entry.
                    if let Some(e) = RefEntry::try_acquire(self, r) {
                        return e;
                    }

                    // If we couldn't increment the reference count, that means someone has just
                    // now removed the node.
                    break;
                }
            }

            let (node, n) = {
                // The reference count is initially three to account for:
                // 1. The entry that will be returned.
                // 2. The link from the 'items' list
                // 3. The link from the 'priorities' list
                let n = Node::alloc(
                    item,
                    priority,
                    3
                );

                (Shared::<Node<I, P>>::from(n as *const _), &*n)
            };

            // Optimistically increment `len`.
            self.length.fetch_add(1, Relaxed);

            // Add node to 'items' list
            loop {
                n.next_item.store(search.right, Relaxed);

                if search.left.compare_exchange(search.right, node, SeqCst, SeqCst, guard).is_ok() {
                    break;
                }

                // We failed. Let's search for the key and try again.
                {
                    // Create a guard that destroys the new node in case search panics.
                    let sg = scopeguard::guard((), |_| {
                        Node::finalize(node.as_raw());
                    });
                    search = self.search_item(&n.item, guard);
                    mem::forget(sg);
                }

                if let Some(r) = search.found {
                    if replace {
                        // If a node with the key was found and we should replace it.
                        // Mark it as removed and then repeat the search.
                        if r.mark() {
                            self.length.fetch_sub(1, Relaxed);
                        }
                    } else {
                        if let Some(e) = RefEntry::try_acquire(self, r) {
                            // Destroy the new node.
                            Node::finalize(node.as_raw());
                            self.length.fetch_sub(1, Relaxed);

                            return e;
                        }
                    }
                }
            }

            // Add node to 'priorities' list
            // TODO

            // Finally, return the new entry.
            RefEntry {
                parent: self,
                node: n,
            }
        }
    }
}

/// Helper function to check if a value is above a lower bound
fn above_lower_bound<T: Ord + ?Sized>(bound: &Bound<&T>, other: &T) -> bool {
    match *bound {
        Bound::Unbounded => true,
        Bound::Included(key) => other >= key,
        Bound::Excluded(key) => other > key,
    }
}

/// Helper function to check if a value is below an upper bound
fn below_upper_bound<T: Ord + ?Sized>(bound: &Bound<&T>, other: &T) -> bool {
    match *bound {
        Bound::Unbounded => true,
        Bound::Included(key) => other <= key,
        Bound::Excluded(key) => other < key,
    }
}

pub struct Entry<'a: 'g, 'g, I, P> {
    parent: &'a RawCPQ<I, P>,
    node: &'g Node<I, P>,
    guard: &'g Guard,
}

/// A reference-counted entry in a CPQ.
///
/// You *must* call `release` to free this type, otherwise the node will be
/// leaked. This is because releasing the entry requires a `Guard`.
pub struct RefEntry<'a, I, P> {
    parent: &'a RawCPQ<I, P>,
    node: &'a Node<I, P>,
}

impl<'a, I, P> RefEntry<'a, I, P> {
    /// Tries to create a new `RefEntry` by incrementing the reference count of
    /// a node.
    unsafe fn try_acquire(
        parent: &'a RawCPQ<I, P>,
        node: &Node<I, P>,
    ) -> Option<RefEntry<'a, I, P>> {
        if node.try_increment() {
            Some(RefEntry {
                parent,

                // We re-bind the lifetime of the node here to that of the skip
                // list since we now hold a reference to it.
                node: &*(node as *const _),
            })
        } else {
            None
        }
    }
}

struct Position<'a, I, P> {
    found: Option<&'a Node<I, P>>,
    left: &'a Atomic<Node<I, P>>,
    right: Shared<'a, Node<I, P>>,
}