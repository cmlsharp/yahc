#![allow(dead_code)]

use crate::unsync::{Consable, Hc, Weak};
use crate::{HasTable, Id};
use std::cell::{Cell, RefCell};

use std::thread::LocalKey as ThreadLocal;

use crate::fxhash::FxHashMap as HashMap;

pub struct Table<T: Consable>(ThreadLocal<InnerTable<T>>);
impl<T: Consable> Table<T> {
    /// # SAFETY
    /// Table<T> should be constructed at most once for any concrete type T. As such new_unchecked
    /// is only intended to be called inside the generate_hashcons macro. The HasTable
    /// implementation ensures that generate_hashcons can only ever be called once per concrete T
    pub const unsafe fn new_unchecked(inner: ThreadLocal<InnerTable<T>>) -> Self {
        Table(inner)
    }

    pub fn gc() -> usize {
        <T as HasTable>::table().0.with(|inner| inner.gc().unwrap())
    }

    pub fn len() -> usize {
        <T as HasTable>::table()
            .0
            .with(|inner| inner.table.borrow().len())
    }

    pub fn for_each<F: FnMut(&T)>(f: F) {
        <T as HasTable>::table().0.with(|inner| {
            inner.table.borrow().keys().for_each(f);
        })
    }

    pub(crate) fn create(t: T) -> Hc<T> {
        <T as HasTable>::table().0.with(|inner| inner.create(t))
    }

    pub(crate) fn add_to_gc(w: Weak<T>) {
        let _ = <T as HasTable>::table().0.try_with(|inner| {
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
        <T as HasTable>::table()
            .0
            .with(|inner| inner.table.borrow_mut().reserve(num_nodes))
    }

    //pub fn gc_hook_add<I: Into<String>, F: Fn(Id) -> Vec<Hc<T>> + 'static>(name: I, f: F) {
    //    <T as HasTable>::table().0.with(|inner| {
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
    //    <T as HasTable>::table().0.with(|inner| {
    //        inner.gc.borrow_mut().hooks.retain(|(s, _)| s != name_ref);
    //    });
    //}

    //pub fn gc_hooks_clear() {
    //    <T as HasTable>::table().0.with(|inner| {
    //        inner.gc.borrow_mut().hooks.clear();
    //    });
    //}
}

//type GcHook<T> = Box<dyn Fn(Id) -> Vec<Hc<T>>>;

struct GcData<T: Consable> {
    to_collect: Vec<Weak<T>>,
    //hooks: Vec<(String, GcHook<T>)>,
}

//struct HooksDebug<'a, T: Consable>(&'a [(String, GcHook<T>)]);
//impl<T: Consable> std::fmt::Debug for HooksDebug<'_, T> {
//    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//        f.debug_list()
//            .entries(self.0.iter().map(|(n, _)| n))
//            .finish()
//    }
//}

impl<T: Consable> std::fmt::Debug for GcData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("GcData")
            .field("to_collect", &self.to_collect)
            // Fix this
            //.field("hooks", &HooksDebug(&self.hooks))
            .finish()
    }
}
impl<T: Consable> Default for GcData<T> {
    fn default() -> Self {
        Self {
            to_collect: Default::default(),
            //hooks: Default::default(),
        }
    }
}

pub struct InnerTable<T: Consable> {
    table: RefCell<HashMap<T, Hc<T>>>,
    gc: RefCell<GcData<T>>,
    next_id: Cell<Id>,
}

impl<T: Consable> Default for InnerTable<T> {
    fn default() -> Self {
        Self {
            table: Default::default(),
            gc: Default::default(),
            next_id: Default::default(),
        }
    }
}

impl<T: Consable> InnerTable<T> {
    fn new() -> Self {
        Self::default()
    }

    fn create(&self, data: T) -> Hc<T> {
        self.table
            .borrow_mut()
            .entry(data)
            .or_insert_with_key(|key| {
                let id = self.next_id.get();
                self.next_id
                    .set(Id(id.0.checked_add(1).expect("id overflow")));
                Hc::new_unchecked(id, key.clone())
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
