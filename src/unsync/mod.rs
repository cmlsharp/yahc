#![allow(dead_code)]

mod table;
use table::Table;

use crate::Id;
//use debug_cell::RefCell;
//use std::cell::Cell;
use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::rc::Rc;

use crate::HasTable;

pub trait Consable: HasTable<Table = Table<Self>> + Clone + Debug + Eq + Hash + 'static {}
impl<T> Consable for T where T: HasTable<Table = Table<Self>> + Clone + Debug + Eq + Hash + 'static {}

// If we didn't have a garbage-collecting table,
// we could avoid having a separate Id and could just use
// Rc::as_ptr(data).addr() as the identifier.
// For strong pointers this would be fine, (who cares if we re-use an identifier of a node that's
// gone), but weak pointers also have an id and they _could_ stick around. If so, you could have
// two structurally unequal Weak<T>s which compare and hash equally, violating the hashconsing
// guarantee. UGH!
//
// Could choose to make Hc's single wide by instead having them contain Rc<(T,Id)>.
// Trade-off: Every copy is now just 8-bytes instead of 16 (at the cost of 8 more bytes on the heap).
// But accessing the id now costs a pointer dereference. Either way, Weak would still need {data,
// id}.
pub struct Hc<T: Consable> {
    data: Rc<T>,
    id: Id,
}

impl<T: Consable> Hc<T> {
    pub fn new(t: T) -> Self {
        <T as HasTable>::Table::create(t)
    }

    fn new_unchecked(id: Id, data: T) -> Self {
        Hc {
            id,
            data: Rc::new(data),
        }
    }
    pub fn id(hc: &Hc<T>) -> Id {
        hc.id
    }

    pub fn downgrade(hc: &Hc<T>) -> Weak<T> {
        Weak {
            data: Rc::downgrade(&hc.data),
            id: hc.id,
        }
    }
    pub fn strong_count(this: &Self) -> usize {
        Rc::strong_count(&this.data)
    }

    pub fn weak_count(this: &Self) -> usize {
        Rc::weak_count(&this.data)
    }
}

impl<T: Consable> Drop for Hc<T> {
    fn drop(&mut self) {
        //eprintln!("DROPPING");
        //eprintln!("{}:{:?}", Rc::strong_count(&self.data), &self.data);
        if Rc::strong_count(&self.data) == 2 && !std::thread::panicking() {
            <T as HasTable>::Table::add_to_gc(Hc::downgrade(self));
        }
    }
}

impl<T: Consable> Debug for Hc<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Hc")
            .field("id", &self.id)
            .field("data", &self.data)
            .finish()
    }
}

impl<T: Consable> Deref for Hc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Consable> Clone for Hc<T> {
    fn clone(&self) -> Self {
        Hc {
            id: self.id,
            data: self.data.clone(),
        }
    }
}

impl<T: Consable> PartialEq for Hc<T> {
    fn eq(&self, other: &Self) -> bool {
        Hc::id(self) == Hc::id(other)
    }
}

impl<T: Consable> Eq for Hc<T> {}

impl<T: Consable> Hash for Hc<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hc::id(self).hash(state)
    }
}

pub struct Weak<T: Consable> {
    data: std::rc::Weak<T>,
    id: Id,
}

impl<T: Consable> Clone for Weak<T> {
    fn clone(&self) -> Self {
        Weak {
            data: self.data.clone(),
            id: self.id,
        }
    }
}

impl<T: Consable> Debug for Weak<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Weak")
            .field("id", &self.id)
            .field("data", &self.data)
            .finish()
    }
}

impl<T: Consable> Weak<T> {
    pub fn id(&self) -> Id {
        self.id
    }

    pub fn upgrade(&self) -> Option<Hc<T>> {
        self.data.upgrade().map(|data| Hc {
            data,
            id: self.id(),
        })
    }

    pub fn weak_count(this: &Self) -> usize {
        this.data.weak_count()
    }
}

impl<T: Consable> PartialEq for Weak<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl<T: Consable> Eq for Weak<T> {}

impl<T: Consable> Hash for Weak<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state)
    }
}

#[macro_export]
macro_rules! generate_hashcons_unsync {
    ($ty:ident) => {
        thread_local! {
            static HC_INNER_TABLE: $crate::unsync::table::InnerTable<$ty> = Default::default();
        }

        // SAFETY:
        // HasTable is implemented alongside this so we can be sure
        // we are the only inhabitant of type Table<$ty> as long
        // as no one else calls new_unchecked which is the invariant.
        static HC_TABLE: $crate::unsync::Table<$ty> =
            unsafe { $crate::unsync::Table::new_unchecked(HC_INNER_TABLE) };

        impl $crate::HasTable for $ty {
            type Table = $crate::unsync::Table<$ty>;
            fn table() -> &'static Self::Table {
                &HC_TABLE
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    mod test1 {
        generate_hashcons_unsync!(Lang);
        use super::Hc;
        #[derive(Debug, Clone, Eq, Hash, PartialEq)]
        pub enum Lang {
            Val(i32),
            Add(Hc<Lang>, Hc<Lang>),
        }
    }

    #[test]
    fn test() {
        use test1::Lang;
        let add = Hc::new(Lang::Add(Hc::new(Lang::Val(12)), Hc::new(Lang::Val(13))));
        drop(add);
        //assert_eq!(<Lang as HasTable>::Table::len(), 2);
        eprintln!("TABLE LEN {}", <Lang as HasTable>::Table::len());
        <Lang as HasTable>::Table::gc();
        eprintln!("TABLE LEN {}", <Lang as HasTable>::Table::len());
    }

    mod test2 {
        use super::Hc;
        generate_hashcons_unsync!(TermInner);
        #[derive(Eq, Hash, PartialEq, Clone)]
        pub struct Term(Hc<TermInner>);
        impl std::ops::Deref for Term {
            type Target = TermInner;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl std::fmt::Debug for Term {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{:?}", &*self.0)
            }
        }
        impl Term {
            pub fn new<I: Into<Box<[Term]>>>(op: Op, cs: I) -> Self {
                Term(Hc::new(TermInner { op, cs: cs.into() }))
            }
        }
        #[derive(Eq, Hash, PartialEq, Debug, Clone, Copy)]
        pub enum Op {
            Add,
            Val(i32),
        }
        #[derive(Eq, Hash, PartialEq, Clone)]
        pub struct TermInner {
            op: Op,
            cs: Box<[Term]>,
        }
        impl std::fmt::Debug for TermInner {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                // Lie about our name
                f.debug_struct("Term")
                    .field("op", &self.op)
                    .field("cs", &self.cs)
                    .finish()
            }
        }
        impl TermInner {
            pub fn op(&self) -> Op {
                self.op
            }
            pub fn cs(&self) -> &[Term] {
                &self.cs
            }
        }
        pub type TermTable = super::Table<TermInner>;
    }

    #[test]
    fn test2() {
        use test2::Op;
        use test2::{Term, TermTable};
        let term1 = Term::new(
            Op::Add,
            vec![Term::new(Op::Val(3), vec![]), Term::new(Op::Val(4), vec![])],
        );
        assert_eq!(TermTable::len(), 3);
        let term2 = Term::new(
            Op::Add,
            vec![Term::new(Op::Val(3), vec![]), Term::new(Op::Val(4), vec![])],
        );
        assert_eq!(TermTable::len(), 3);
        drop(term1);
        assert_eq!(TermTable::len(), 3);
        TermTable::gc();
        assert_eq!(TermTable::len(), 3);
        drop(term2);
        assert_eq!(TermTable::len(), 3);
        TermTable::gc();
        assert_eq!(TermTable::len(), 0);
    }
}
