//! A cache from terms that does not retain its keys.

use fxhash::FxHashMap as HashMap;

use crate::{HasTable, Weak, WeakOf};

/// A cache from terms that does not retain its keys.
#[derive(Clone, Default)]
pub struct CacheOf<D: HasTable, V> {
    inner: HashMap<WeakOf<D>, V>,
}

impl<D: HasTable, V> CacheOf<D, V> {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            inner: HashMap::default(),
        }
    }
    /// Create an empty cache with room for `n` items before allocation.
    pub fn with_capacity(n: usize) -> Self {
        Self {
            inner: HashMap::with_capacity_and_hasher(n, fxhash::FxBuildHasher::default()),
        }
    }
    /// Remove entries with free'd keys.
    pub fn collect(&mut self) {
        self.inner.retain(|k, _| k.upgrade().is_some());
    }
}

impl<D: HasTable, V> std::ops::Deref for CacheOf<D, V> {
    type Target = HashMap<WeakOf<D>, V>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<D: HasTable, V> std::ops::DerefMut for CacheOf<D, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
