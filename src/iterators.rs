use std::{hash::Hash, collections::hash_map::RandomState, marker::PhantomData};

pub struct Iter<'a, I: 'a, P: 'a>
where
    I: Hash + Eq,
    P: Ord,
{
    marker_i: PhantomData<&'a I>,
    marker_p: PhantomData<&'a P>,
}

pub struct IterMut<'a, I: 'a, P: 'a, H: 'a = RandomState>
where
    I: Hash + Eq,
    P: Ord,
{
    marker_i: PhantomData<&'a I>,
    marker_p: PhantomData<&'a P>,
    marker_h: PhantomData<&'a H>,
}
