#![allow(dead_code)]

use crate::Id;
use crate::unsync::{Consable, Hc, Weak};
use std::cell::{Cell, RefCell};

use std::thread::LocalKey as ThreadLocal;

use crate::fxhash::FxHashMap as HashMap;
use crate::unsync::TableKey;
use std::marker::PhantomData;

pub struct Table<T: Consable, I: TableKey<T>>(ThreadLocal<InnerTable<T, I>>, PhantomData<I>);
impl<T: Consable, I: TableKey<T>> Table<T, I> {
    /// # SAFETY
    /// Table<T, I> should be constructed at most once for any concrete type T. As such new_unchecked
    /// is only intended to be called inside the generate_hashcons macro. The HasTable
    /// implementation ensures that generate_hashcons can only ever be called once per concrete T
    pub const unsafe fn new_unchecked(inner: ThreadLocal<InnerTable<T, I>>) -> Self {
        Table(inner, PhantomData)
    }

    pub fn gc() -> usize {
        <I as crate::TableKey>::table()
            .0
            .with(|inner| inner.gc().unwrap())
    }

    pub fn len() -> usize {
        <I as crate::TableKey>::table()
            .0
            .with(|inner| inner.table.borrow().len())
    }

    pub fn for_each<F: FnMut(&T)>(f: F) {
        <I as crate::TableKey>::table().0.with(|inner| {
            inner.table.borrow().keys().for_each(f);
        })
    }

    pub(crate) fn create(t: T) -> Hc<T, I> {
        <I as crate::TableKey>::table()
            .0
            .with(|inner| inner.create(t))
    }

    pub(crate) fn add_to_gc(w: Weak<T, I>) {
        let _ = <I as crate::TableKey>::table().0.try_with(|inner| {
            //inner.gc.borrow_mut().to_collect.push(w);
            inner
                .gc
                .try_borrow_mut()
                .unwrap_or_else(|_| panic!("Failed to add to gc queue"))
                .to_collect
                .push(w);
        });
    }

    pub fn reserve(num_nodes: usize) {
        <I as crate::TableKey>::table()
            .0
            .with(|inner| inner.table.borrow_mut().reserve(num_nodes))
    }

    //pub fn gc_hook_add<I: Into<String>, F: Fn(Id) -> Vec<Hc<T,I>> + 'static>(name: I, f: F) {
    //    <I as crate::TableKey>::table().0.with(|inner| {
    //        let hooks = &mut inner.gc.borrow_mut().hooks;
    //        let name = name.into();
    //        assert!(
    //            hooks.iter().all(|(s, _)| s != &name),
    //            "Already a hook for '{name}'"
    //        );
    //        hooks.push((name, Box::new(f)))
    //    })
    //}

    //pub fn gc_hook_remove<I: AsRef<str>>(name: I) {
    //    let name_ref = name.as_ref();
    //    <I as crate::TableKey>::table().0.with(|inner| {
    //        inner.gc.borrow_mut().hooks.retain(|(s, _)| s != name_ref);
    //    });
    //}

    //pub fn gc_hooks_clear() {
    //    <I as crate::TableKey>::table().0.with(|inner| {
    //        inner.gc.borrow_mut().hooks.clear();
    //    });
    //}
}

//type GcHook<T> = Box<dyn Fn(Id) -> Vec<Hc<T,I>>>;

struct GcData<T: Consable, I: TableKey<T>> {
    to_collect: Vec<Weak<T, I>>,
    //hooks: Vec<(String, GcHook<T>)>,
}

//struct HooksDebug<'a, T: Consable, I: TableKey<T>>(&'a [(String, GcHook<T>)]);
//impl<T: Consable, I: TableKey<T>> std::fmt::Debug for HooksDebug<'_, T> {
//    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//        f.debug_list()
//            .entries(self.0.iter().map(|(n, _)| n))
//            .finish()
//    }
//}

impl<T: Consable, I: TableKey<T>> std::fmt::Debug for GcData<T, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("GcData")
            .field("to_collect", &self.to_collect)
            // Fix this
            //.field("hooks", &HooksDebug(&self.hooks))
            .finish()
    }
}
impl<T: Consable, I: TableKey<T>> Default for GcData<T, I> {
    fn default() -> Self {
        Self {
            to_collect: Default::default(),
            //hooks: Default::default(),
        }
    }
}

pub struct InnerTable<T: Consable, I: TableKey<T>> {
    table: RefCell<HashMap<T, Hc<T, I>>>,
    gc: RefCell<GcData<T, I>>,
}

impl<T: Consable, I: TableKey<T>> Default for InnerTable<T, I> {
    fn default() -> Self {
        Self {
            table: Default::default(),
            gc: Default::default(),
        }
    }
}

impl<T: Consable, I: TableKey<T>> InnerTable<T, I> {
    fn new() -> Self {
        Self::default()
    }

    fn create(&self, data: T) -> Hc<T, I> {
        self.table
            .borrow_mut()
            .entry(data)
            .or_insert_with_key(|key| {
                Hc::new_unchecked(key.clone())
            })
            .clone()
    }

    fn gc(&self) -> Option<usize> {
        if std::thread::panicking() {
            return None;
        }

        let mut table = self.table.borrow_mut();
        let mut collected = 0;
        loop {
            let Some(t) = ({
                // Putting gc.borrow_mut in its own scope so the guard is dropped by the time any
                // hc destructors run
                self.gc.borrow_mut().to_collect.pop()
            }) else {
                break;
            };

            if t.data.strong_count() != 1 {
                continue;
            }

            collected += 1;

            // Need rc to drop before hc, otherwise hc's ref count will be 2 when it drops
            // and it'll re-add all its children to the queue
            // Once hc is dropped, Hc::drop is called on hc as well as all of its children
            // Hence their destructors might cause more elements to be pushed to gc.to_collect
            std::mem::drop({
                let rc = t.data.upgrade().expect("missing from table");
                table.remove(&*rc).expect("missing from table")
            });
            //let hooks_len = { self.gc.borrow().hooks.len() };
            //for i in 0..hooks_len {
            //    // Note gc hooks should probably not drop any Hcs. Currently this would result in a
            //    // panic. The alternative is to just silently not garbage collect terms which is
            //    // not ideal.
            //    for c in {(self.gc.borrow().hooks[i].1)(id)}.into_iter() {
            //    };

            //}
        }
        Some(collected)
    }

    fn print_gc_queue(&self) {
        for i in self
            .gc
            .borrow()
            .to_collect
            .iter()
            .filter_map(|w| w.data.upgrade())
        {
            eprintln!("TO_COLLECT: {:?}", &i)
        }
    }
}
