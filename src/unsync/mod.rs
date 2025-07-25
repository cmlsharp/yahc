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

pub trait Consable: Clone + Debug + Eq + Hash + 'static {}
impl<T> Consable for T where T: Clone + Debug + Eq + Hash + 'static {}

// Trait alias
pub trait TableId<T: Consable>: crate::TableId<Table=Table<T, Self>> {}
impl<T: Consable,I> TableId<T> for I where I: crate::TableId<Table=Table<T, Self>> {}

// If we didn't have weak pointers
// we could avoid having a separate Id and could just use
// Rc::as_ptr(data).addr() as the identifier. But nodes can be freed.
// For strong pointers this would be fine, (who cares if we re-use an identifier of a node that we
// couldn't ever compare against anyway), but weak pointers also have an id and they _could_ stick
// around. If so, you could have two structurally unequal Weak<T,I>s which compare and hash equally,
// violating the hashconsing guarantee. Annoying.
//
// Could choose to make Hc's 8-bytes by instead having them contain Rc<(T,Id)>.
// Obviously Hc is meant to be cheaply clonable so memory savings could be significant. But its
// heap memory and accessing id now costs a pointer dereference. Either way, Weak would still need {data,
// id}.
pub struct Hc<T: Consable, I: TableId<T>> {
    data: Rc<T>,
    id: Id,
    _marker: std::marker::PhantomData<I>,
}

impl<T: Consable, I: TableId<T>> Hc<T,I> {
    pub fn new(t: T) -> Self {
        <I as crate::TableId>::Table::create(t)
    }

    fn new_unchecked(id: Id, data: T) -> Self {
        Hc {
            id,
            data: Rc::new(data),
            _marker: std::marker::PhantomData,
        }
    }
    pub fn id(this: &Hc<T,I>) -> Id {
        this.id
    }

    pub fn downgrade(this: &Hc<T,I>) -> Weak<T,I> {
        Weak {
            data: Rc::downgrade(&this.data),
            id: this.id,
            _marker: std::marker::PhantomData,
        }
    }
    pub fn strong_count(this: &Self) -> usize {
        Rc::strong_count(&this.data)
    }

    pub fn weak_count(this: &Self) -> usize {
        Rc::weak_count(&this.data)
    }
}

impl<T: Consable, I: TableId<T>> Drop for Hc<T,I> {
    fn drop(&mut self) {
        //eprintln!("DROPPING");
        //eprintln!("{}:{:?}", Rc::strong_count(&self.data), &self.data);
        // This and the table entry
        if Rc::strong_count(&self.data) == 2 && !std::thread::panicking() {
            <I as crate::TableId>::Table::add_to_gc(Hc::downgrade(self));
        }
    }
}

impl<T: Consable, I: TableId<T>> Debug for Hc<T,I> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Hc")
            .field("id", &self.id)
            .field("data", &self.data)
            .finish()
    }
}

impl<T: Consable, I: TableId<T>> Deref for Hc<T,I> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Consable, I: TableId<T>> Clone for Hc<T,I> {
    fn clone(&self) -> Self {
        Hc {
            id: self.id,
            data: self.data.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: Consable, I: TableId<T>> PartialEq for Hc<T,I> {
    fn eq(&self, other: &Self) -> bool {
        Hc::id(self) == Hc::id(other)
    }
}

impl<T: Consable, I: TableId<T>> Eq for Hc<T,I> {}

impl<T: Consable, I: TableId<T>> Hash for Hc<T,I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hc::id(self).hash(state)
    }
}

pub struct Weak<T: Consable, I: TableId<T>> {
    data: std::rc::Weak<T>,
    id: Id,
    _marker: std::marker::PhantomData<I>,
}

impl<T: Consable, I: TableId<T>> Clone for Weak<T,I> {
    fn clone(&self) -> Self {
        Weak {
            data: self.data.clone(),
            id: self.id,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: Consable, I: TableId<T>> Debug for Weak<T,I> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Weak")
            .field("id", &self.id)
            .field("data", &self.data)
            .finish()
    }
}

impl<T: Consable, I: TableId<T>> Weak<T,I> {
    pub fn id(&self) -> Id {
        self.id
    }

    pub fn upgrade(&self) -> Option<Hc<T,I>> {
        self.data.upgrade().map(|data| Hc {
            data,
            id: self.id(),
            _marker: std::marker::PhantomData,
        })
    }

    pub fn weak_count(this: &Self) -> usize {
        this.data.weak_count()
    }
}

impl<T: Consable, I: TableId<T>> PartialEq for Weak<T,I> {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl<T: Consable, I: TableId<T>> Eq for Weak<T,I> {}

impl<T: Consable, I: TableId<T>> Hash for Weak<T,I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state)
    }
}

#[macro_export]
macro_rules! generate_hashcons_unsync {
    ($ty:ident) => {
        pub enum LocalId {}

        thread_local! {
            static HC_INNER_TABLE: $crate::unsync::table::InnerTable<$ty, LocalId> = Default::default();
        }

        // SAFETY:
        // We construct a totally new type LocalId in this macro, so as long
        // as no one else calls Table::new_unchecked (per its safety contract)
        // this is the only instance of Table<T, LocalId>
        static HC_TABLE: $crate::unsync::Table<$ty, LocalId> =
            unsafe { $crate::unsync::Table::new_unchecked(HC_INNER_TABLE) };

        impl $crate::TableId for LocalId {
            type Table = $crate::unsync::Table<$ty, LocalId>;
            fn table() -> &'static Self::Table {
                &HC_TABLE
            }
        }
        // The <T> here is kinda superfluous, these are fixed to a single type type T
        // But Hc<T> looks nice and reminds the user to construct via e.g. Hc::new()
        pub type Hc<T> = $crate::unsync::Hc<T, LocalId>;
        pub type Table<T> = $crate::unsync::Table<T, LocalId>;
        pub type Weak<T> = $crate::unsync::Weak<T, LocalId>;
    };
}

#[cfg(test)]
mod tests {
    mod test1 {
        generate_hashcons_unsync!(Lang);
        #[derive(Debug, Clone, Eq, Hash, PartialEq)]
        pub enum Lang {
            Val(i32),
            Add(Hc<Lang>, Hc<Lang>),
        }
    }

    #[test]
    fn test() {
        use test1::{Lang, Hc};
        let add = Hc::new(Lang::Add(Hc::new(Lang::Val(12)), Hc::new(Lang::Val(13))));
        drop(add);
        //assert_eq!(<Lang as HasTable>::Table::len(), 2);
        eprintln!("TABLE LEN {}", test1::Table::gc());
        test1::Table::gc();
        eprintln!("TABLE LEN {}", test1::Table::gc());
    }

    mod test2 {
        mod inner { 
            use super::TermInner;
            generate_hashcons_unsync!(TermInner);
        }
        use inner::{Hc, Table};
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
        pub type TermTable = Table<TermInner>;
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
