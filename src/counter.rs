use std::sync::atomic::{
    AtomicUsize,
    Ordering::Relaxed,
};

#[derive(Debug, Default)]
pub struct RelaxedCounter(AtomicUsize);

impl RelaxedCounter {
    pub fn new(value: usize) -> Self {
        Self(AtomicUsize::new(value))
    }

    pub fn add(&self, amount: usize) -> usize {
        self.0.fetch_add(amount, Relaxed)
    }

    pub fn sub(&self, amount: usize) -> usize {
        self.0.fetch_sub(amount, Relaxed)
    }

    pub fn inc(&self) -> usize {
        self.add(1)
    }

    pub fn dec(&self) -> usize {
        self.sub(1)
    }

    pub fn get(&self) -> usize {
        self.0.load(Relaxed)
    }

    pub fn reset(&self) -> usize {
        self.0.swap(0, Relaxed)
    }
}
