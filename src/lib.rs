//pub mod cache;
pub mod unsync;
pub use fxhash;

use std::cmp::{Eq, Ord, PartialEq, PartialOrd};

pub trait HasTable: Sized {
    type Table;
    fn table() -> &'static Self::Table;
}

//pub trait Table<T>
//where T: HasTable<Table=Self>
//{
//    type Hc;//: Hc<T, Table=Self>;
//    fn create(t: T) -> Self::Hc;
//    fn gc() -> usize;
//    fn len() -> usize;
//    fn reserve(n: usize);
//    fn for_each<F: FnMut(&T)>(f: F);
//    //fn with_debug_info(f: impl FnOnce(&dyn std::fmt::Debug));
//}

//pub trait Weak<T> : Clone
//where T: HasTable<Table=<Self::Hc as Hc<T>>::Table>
//{
//    type Hc: Hc<T, Weak=Self>;
//    fn id(&self) -> Id;
//    fn upgrade(&self) -> Option<Self::Hc>;
//}
//
//pub trait Hc<T>: Deref<Target=T> + Clone
//where T: HasTable<Table=Self::Table>
//{
//    type Table: Table<T, Hc=Self>;
//    type Weak: Weak<T, Hc=Self>;
//    fn strong_count(this: &Self) -> usize;
//    fn downgrade(this: &Self) -> Self::Weak;
//    fn id(this: &Self) -> Id;
//    fn new(this: T) -> Self {
//        Self::Table::create(this)
//    }
//}

/// A unique term ID.
#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub struct Id(pub u64);

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id{}", self.0)
    }
}

impl std::fmt::Debug for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id{}", self.0)
    }
}

mod hash {
    use super::Id;
    use std::hash::{Hash, Hasher};

    // 64 bit primes
    const PRIME_1: u64 = 15124035408605323001;
    const PRIME_2: u64 = 15133577374253939647;

    impl Hash for Id {
        fn hash<H: Hasher>(&self, state: &mut H) {
            let id_hash = self.0.wrapping_mul(PRIME_1).wrapping_add(PRIME_2);
            state.write_u64(id_hash);
        }
    }
}
