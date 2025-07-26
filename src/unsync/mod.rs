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

// Trait alias
pub trait Consable: Clone + Debug + Eq + Hash + 'static {}
impl<T> Consable for T where T: Clone + Debug + Eq + Hash + 'static {}

// Trait alias
// trait TableKey<T> = crate::TableKey<Table=Table<T, Self>>
pub trait TableKey<T: Consable>: crate::TableKey<Table = Table<T, Self>> {}
impl<T: Consable, I> TableKey<T> for I where I: crate::TableKey<Table = Table<T, Self>> {}

pub struct Hc<T: Consable, I: TableKey<T>> {
    data: Rc<T>,
    _marker: std::marker::PhantomData<I>,
}

impl<T: Consable, I: TableKey<T>> Hc<T, I> {
    pub fn new(t: T) -> Self {
        <I as crate::TableKey>::Table::create(t)
    }

    fn new_unchecked(data: T) -> Self {
        Hc {
            data: Rc::new(data),
            _marker: std::marker::PhantomData,
        }
    }
    pub fn id(this: &Hc<T, I>) -> Id {
        Id(Rc::as_ptr(&this.data).addr() as u64)
    }

    pub fn downgrade(this: &Hc<T, I>) -> Weak<T, I> {
        Weak {
            data: Rc::downgrade(&this.data),
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

impl<T: Consable, I: TableKey<T>> Drop for Hc<T, I> {
    fn drop(&mut self) {
        //eprintln!("DROPPING");
        //eprintln!("{}:{:?}", Rc::strong_count(&self.data), &self.data);
        // This and the table entry
        if Rc::strong_count(&self.data) == 2 && !std::thread::panicking() {
            <I as crate::TableKey>::Table::add_to_gc(Hc::downgrade(self));
        }
    }
}

impl<T: Consable, I: TableKey<T>> Debug for Hc<T, I> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Hc")
            .field("id", &Hc::id(&self))
            .field("data", &self.data)
            .finish()
    }
}

impl<T: Consable, I: TableKey<T>> Deref for Hc<T, I> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Consable, I: TableKey<T>> Clone for Hc<T, I> {
    fn clone(&self) -> Self {
        Hc {
            data: self.data.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: Consable, I: TableKey<T>> PartialEq for Hc<T, I> {
    fn eq(&self, other: &Self) -> bool {
        Hc::id(self) == Hc::id(other)
    }
}

impl<T: Consable, I: TableKey<T>> Eq for Hc<T, I> {}

impl<T: Consable, I: TableKey<T>> Hash for Hc<T, I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hc::id(self).hash(state)
    }
}

pub struct Weak<T: Consable, I: TableKey<T>> {
    data: std::rc::Weak<T>,
    _marker: std::marker::PhantomData<I>,
}

impl<T: Consable, I: TableKey<T>> Clone for Weak<T, I> {
    fn clone(&self) -> Self {
        Weak {
            data: self.data.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: Consable, I: TableKey<T>> Debug for Weak<T, I> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Weak")
            .field("id", &self.id())
            .field("data", &self.data)
            .finish()
    }
}

impl<T: Consable, I: TableKey<T>> Weak<T, I> {
    pub fn id(&self) -> Id {
        Id(self.data.as_ptr().addr() as u64)
    }

    pub fn upgrade(&self) -> Option<Hc<T, I>> {
        self.data.upgrade().map(|data| Hc {
            data,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn weak_count(this: &Self) -> usize {
        this.data.weak_count()
    }
}

impl<T: Consable, I: TableKey<T>> PartialEq for Weak<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl<T: Consable, I: TableKey<T>> Eq for Weak<T, I> {}

impl<T: Consable, I: TableKey<T>> Hash for Weak<T, I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state)
    }
}

#[macro_export]
macro_rules! generate_hashcons_unsync {
    (mod $mod:ident, $ty:ident) => {
        mod $mod {
            mod inner {
                pub enum LocalKey {}

                thread_local! {
                    static HC_INNER_TABLE: $crate::unsync::table::InnerTable<super::super::$ty, LocalKey> = Default::default();
                }

                // SAFETY:
                // We construct a totally new type LocalKey in this macro, so as long
                // as no one else calls Table::new_unchecked (per its safety contract)
                // this is the only instance of Table<T, LocalKey>
                static HC_TABLE: $crate::unsync::Table<super::super::$ty, LocalKey> =
                    unsafe { $crate::unsync::Table::new_unchecked(HC_INNER_TABLE) };

                impl $crate::TableKey for LocalKey {
                    type Table = $crate::unsync::Table<super::super::$ty, LocalKey>;
                    fn table() -> &'static Self::Table {
                        &HC_TABLE
                    }
                }
            }

            pub type Hc = $crate::unsync::Hc<super::$ty, inner::LocalKey>;
            pub type Table = $crate::unsync::Table<super::$ty, inner::LocalKey>;
            pub type Weak = $crate::unsync::Weak<super::$ty, inner::LocalKey>;
        }
    };
}

#[cfg(test)]
mod tests {
    #[derive(Debug, Clone, Eq, Hash, PartialEq)]
    pub enum LangInner {
        Val(i32),
        Add(Lang, Lang),
    }
    generate_hashcons_unsync!(mod test1, LangInner);
    use test1::Hc as Lang;

    #[test]
    fn test() {
        let add = Lang::new(LangInner::Add(
            Lang::new(LangInner::Val(12)),
            Lang::new(LangInner::Val(13)),
        ));
        drop(add);
        //assert_eq!(<Lang as HasTable>::Table::len(), 2);
        eprintln!("TABLE LEN {}", test1::Table::gc());
        test1::Table::gc();
        eprintln!("TABLE LEN {}", test1::Table::gc());
    }

    // How we'd implement for circ
    mod circ {
        generate_hashcons_unsync!(mod inner, TermInner);
        use inner::Hc;
        pub use inner::Table as TermTable;
        #[derive(Eq, Hash, PartialEq, Clone)]
        pub struct Term(Hc);
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
    }

    #[test]
    fn test2() {
        use circ::Op;
        use circ::{Term, TermTable};
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
