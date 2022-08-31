use dashmap::{RwLockReadGuard, RwLockWriteGuard, SharedValue};
use core::hash::{BuildHasher, Hash};
use core::ops::{Deref, DerefMut};
use std::collections::hash_map::RandomState;
use std::fmt::{Debug, Formatter};

use crossbeam_epoch::Guard;

type InternalHashMap<K, V, S> = hashbrown::HashMap<K, SharedValue<V>, S>;

pub struct Ref<'a, K, V, S = RandomState> {
    guard_sl: Guard,
    guard_dm: RwLockReadGuard<'a, InternalHashMap<K, V, S>>,
    k: *const K,
    v: *const V,
}

unsafe impl<'a, K: Eq + Hash + Sync, V: Sync, S: BuildHasher> Send for Ref<'a, K, V, S> {}
unsafe impl<'a, K: Eq + Hash + Sync, V: Sync, S: BuildHasher> Sync for Ref<'a, K, V, S> {}

impl<'a, K: Eq + Hash, V, S: BuildHasher> Ref<'a, K, V, S> {
    pub(crate) unsafe fn new(
        guard_sl: Guard,
        guard_dm: RwLockReadGuard<'a, InternalHashMap<K, V, S>>,
        k: *const K,
        v: *const V,
    ) -> Self {
        Self {
            guard_sl, 
            guard_dm,
            k,
            v,
        }
    }

    pub fn key(&self) -> &K {
        self.pair().0
    }

    pub fn value(&self) -> &V {
        self.pair().1
    }

    pub fn pair(&self) -> (&K, &V) {
        unsafe { (&*self.k, &*self.v) }
    }

    pub fn map<F, T>(self, f: F) -> MappedRef<'a, K, V, T, S>
    where
        F: FnOnce(&V) -> &T,
    {
        MappedRef {
            guard_sl: self.guard_sl,
            guard_dm: self.guard_dm,
            k: self.k,
            v: f(unsafe { &*self.v }),
        }
    }

    pub fn try_map<F, T>(self, f: F) -> Result<MappedRef<'a, K, V, T, S>, Self>
    where
        F: FnOnce(&V) -> Option<&T>,
    {
        if let Some(v) = f(unsafe { &*self.v }) {
            Ok(MappedRef {
                guard_sl: self.guard_sl,
                guard_dm: self.guard_dm,
                k: self.k,
                v,
            })
        } else {
            Err(self)
        }
    }
}

impl<'a, K: Eq + Hash + Debug, V: Debug, S: BuildHasher> Debug for Ref<'a, K, V, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ref")
            .field("k", &self.k)
            .field("v", &self.v)
            .finish()
    }
}

impl<'a, K: Eq + Hash, V, S: BuildHasher> Deref for Ref<'a, K, V, S> {
    type Target = V;

    fn deref(&self) -> &V {
        self.value()
    }
}

pub struct RefMut<'a, K, V, S = RandomState> {
    guard_sl: Guard,
    guard_dm: RwLockWriteGuard<'a, InternalHashMap<K, V, S>>,
    k: *const K,
    v: *mut V,
}

unsafe impl<'a, K: Eq + Hash + Sync, V: Sync, S: BuildHasher> Send for RefMut<'a, K, V, S> {}
unsafe impl<'a, K: Eq + Hash + Sync, V: Sync, S: BuildHasher> Sync for RefMut<'a, K, V, S> {}

impl<'a, K: Eq + Hash, V, S: BuildHasher> RefMut<'a, K, V, S> {
    pub(crate) unsafe fn new(
        guard_sl: Guard,
        guard_dm: RwLockWriteGuard<'a, InternalHashMap<K, V, S>>,
        k: *const K,
        v: *mut V,
    ) -> Self {
        Self { guard_sl, guard_dm, k, v }
    }

    pub fn key(&self) -> &K {
        self.pair().0
    }

    pub fn value(&self) -> &V {
        self.pair().1
    }

    pub fn value_mut(&mut self) -> &mut V {
        self.pair_mut().1
    }

    pub fn pair(&self) -> (&K, &V) {
        unsafe { (&*self.k, &*self.v) }
    }

    pub fn pair_mut(&mut self) -> (&K, &mut V) {
        unsafe { (&*self.k, &mut *self.v) }
    }

    pub fn downgrade(self) -> Ref<'a, K, V, S> {
        unsafe { Ref::new(self.guard_sl, RwLockWriteGuard::downgrade(self.guard_dm), self.k, self.v) }
    }

    pub fn map<F, T>(self, f: F) -> MappedRefMut<'a, K, V, T, S>
    where
        F: FnOnce(&mut V) -> &mut T,
    {
        MappedRefMut {
            guard_sl: self.guard_sl,
            guard_dm: self.guard_dm,
            k: self.k,
            v: f(unsafe { &mut *self.v }),
        }
    }

    pub fn try_map<F, T>(self, f: F) -> Result<MappedRefMut<'a, K, V, T, S>, Self>
    where
        F: FnOnce(&mut V) -> Option<&mut T>,
    {
        let v = match f(unsafe { &mut *(self.v as *mut _) }) {
            Some(v) => v,
            None => return Err(self),
        };

        let guard_sl = self.guard_sl;
        let guard_dm = self.guard_dm;
        let k = self.k;
        Ok(MappedRefMut {
            guard_sl,
            guard_dm,
            k,
            v,
        })
    }
}

impl<'a, K: Eq + Hash + Debug, V: Debug, S: BuildHasher> Debug for RefMut<'a, K, V, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RefMut")
            .field("k", &self.k)
            .field("v", &self.v)
            .finish()
    }
}

impl<'a, K: Eq + Hash, V, S: BuildHasher> Deref for RefMut<'a, K, V, S> {
    type Target = V;

    fn deref(&self) -> &V {
        self.value()
    }
}

impl<'a, K: Eq + Hash, V, S: BuildHasher> DerefMut for RefMut<'a, K, V, S> {
    fn deref_mut(&mut self) -> &mut V {
        self.value_mut()
    }
}

pub struct MappedRef<'a, K, V, T, S = RandomState> {
    guard_sl: Guard,
    guard_dm: RwLockReadGuard<'a, InternalHashMap<K, V, S>>,
    k: *const K,
    v: *const T,
}

impl<'a, K: Eq + Hash, V, T, S: BuildHasher> MappedRef<'a, K, V, T, S> {
    pub fn key(&self) -> &K {
        self.pair().0
    }

    pub fn value(&self) -> &T {
        self.pair().1
    }

    pub fn pair(&self) -> (&K, &T) {
        unsafe { (&*self.k, &*self.v) }
    }

    pub fn map<F, T2>(self, f: F) -> MappedRef<'a, K, V, T2, S>
    where
        F: FnOnce(&T) -> &T2,
    {
        MappedRef {
            guard_sl: self.guard_sl,
            guard_dm: self.guard_dm,
            k: self.k,
            v: f(unsafe { &*self.v }),
        }
    }

    pub fn try_map<F, T2>(self, f: F) -> Result<MappedRef<'a, K, V, T2, S>, Self>
    where
        F: FnOnce(&T) -> Option<&T2>,
    {
        let v = match f(unsafe { &*self.v }) {
            Some(v) => v,
            None => return Err(self),
        };
        let guard_sl = self.guard_sl;
        let guard_dm = self.guard_dm;
        let k = self.k;
        Ok(MappedRef {
            guard_sl,
            guard_dm,
            k,
            v,
        })
    }
}

impl<'a, K: Eq + Hash + Debug, V, T: Debug, S: BuildHasher> Debug for MappedRef<'a, K, V, T, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappedRef")
            .field("k", &self.k)
            .field("v", &self.v)
            .finish()
    }
}

impl<'a, K: Eq + Hash, V, T, S: BuildHasher> Deref for MappedRef<'a, K, V, T, S> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value()
    }
}

pub struct MappedRefMut<'a, K, V, T, S = RandomState> {
    guard_sl: Guard,
    guard_dm: RwLockWriteGuard<'a, InternalHashMap<K, V, S>>,
    k: *const K,
    v: *mut T,
}

impl<'a, K: Eq + Hash, V, T, S: BuildHasher> MappedRefMut<'a, K, V, T, S> {
    pub fn key(&self) -> &K {
        self.pair().0
    }

    pub fn value(&self) -> &T {
        self.pair().1
    }

    pub fn value_mut(&mut self) -> &mut T {
        self.pair_mut().1
    }

    pub fn pair(&self) -> (&K, &T) {
        unsafe { (&*self.k, &*self.v) }
    }

    pub fn pair_mut(&mut self) -> (&K, &mut T) {
        unsafe { (&*self.k, &mut *self.v) }
    }

    pub fn map<F, T2>(self, f: F) -> MappedRefMut<'a, K, V, T2, S>
    where
        F: FnOnce(&mut T) -> &mut T2,
    {
        MappedRefMut {
            guard_sl: self.guard_sl,
            guard_dm: self.guard_dm,
            k: self.k,
            v: f(unsafe { &mut *self.v }),
        }
    }

    pub fn try_map<F, T2>(self, f: F) -> Result<MappedRefMut<'a, K, V, T2, S>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut T2>,
    {
        let v = match f(unsafe { &mut *(self.v as *mut _) }) {
            Some(v) => v,
            None => return Err(self),
        };
        let guard_sl = self.guard_sl;
        let guard_dm = self.guard_dm;
        let k = self.k;
        Ok(MappedRefMut {
            guard_sl,
            guard_dm,
            k,
            v,
        })
    }
}

impl<'a, K: Eq + Hash + Debug, V, T: Debug, S: BuildHasher> Debug for MappedRefMut<'a, K, V, T, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappedRefMut")
            .field("k", &self.k)
            .field("v", &self.v)
            .finish()
    }
}

impl<'a, K: Eq + Hash, V, T, S: BuildHasher> Deref for MappedRefMut<'a, K, V, T, S> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value()
    }
}

impl<'a, K: Eq + Hash, V, T, S: BuildHasher> DerefMut for MappedRefMut<'a, K, V, T, S> {
    fn deref_mut(&mut self) -> &mut T {
        self.value_mut()
    }
}