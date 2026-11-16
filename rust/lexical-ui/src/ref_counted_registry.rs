//! `LexicalRefCountedRegistry.ts`: registrations reference counted by key.

use crate::Unregister;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What a key's activation returned, and its live registrations' tokens.
struct Entry {
    holders: Vec<u64>,
    dispose: Option<Unregister>,
}

/// `activate(key, options)`.
type Activate<K, O> = Box<dyn Fn(&K, Option<O>) -> Unregister>;

struct Inner<K, O> {
    activate: Activate<K, O>,
    /// By key, in the order first registered (JS's `Map`).
    entries: RefCell<Vec<(K, Entry)>>,
    next_token: Cell<u64>,
}

/// `RefCountedRegistry<Key, Options>`: the first registration of a key runs
/// the activation; what it returned runs once the last registration of
/// that key is released. A handle; cloning it shares the registry.
pub struct RefCountedRegistry<K, O = ()>(Rc<Inner<K, O>>);

impl<K, O> Clone for RefCountedRegistry<K, O> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// `createRefCountedRegistry(activate)`.
pub fn create_ref_counted_registry<K: PartialEq + Clone + 'static, O: 'static>(
    activate: impl Fn(&K, Option<O>) -> Unregister + 'static,
) -> RefCountedRegistry<K, O> {
    RefCountedRegistry(Rc::new(Inner {
        activate: Box::new(activate),
        entries: RefCell::new(Vec::new()),
        next_token: Cell::new(0),
    }))
}

impl<K: PartialEq + Clone + 'static, O: 'static> RefCountedRegistry<K, O> {
    /// `register(key, options)`: registers `key` and returns its disposer.
    /// `options` are read only by the activating (first) registration of
    /// the key. A stale disposer (one called after the registry was
    /// disposed or the key registered again) does nothing.
    pub fn register(&self, key: K, options: Option<O>) -> Unregister {
        let exists = self.0.entries.borrow().iter().any(|(k, _)| *k == key);
        if !exists {
            // The activation may register other keys, so it runs without
            // the entries borrowed.
            let dispose = (self.0.activate)(&key, options);
            self.0.entries.borrow_mut().push((
                key.clone(),
                Entry {
                    holders: Vec::new(),
                    dispose: Some(dispose),
                },
            ));
        }
        let token = self.0.next_token.replace(self.0.next_token.get() + 1);
        if let Some((_, entry)) = self
            .0
            .entries
            .borrow_mut()
            .iter_mut()
            .find(|(k, _)| *k == key)
        {
            entry.holders.push(token);
        }
        let weak = Rc::downgrade(&self.0);
        Box::new(move || {
            let Some(inner) = weak.upgrade() else { return };
            let dispose = {
                let mut entries = inner.entries.borrow_mut();
                let Some(index) = entries.iter().position(|(k, _)| *k == key) else {
                    return;
                };
                let entry = &mut entries[index].1;
                let Some(at) = entry.holders.iter().position(|t| *t == token) else {
                    return;
                };
                entry.holders.remove(at);
                if !entry.holders.is_empty() {
                    return;
                }
                entries.remove(index).1.dispose
            };
            if let Some(dispose) = dispose {
                dispose();
            }
        })
    }

    /// `dispose()`: disposes every live registration and clears the
    /// registry.
    pub fn dispose(&self) {
        let entries = std::mem::take(&mut *self.0.entries.borrow_mut());
        for (_, entry) in entries {
            if let Some(dispose) = entry.dispose {
                dispose();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(log: &Rc<RefCell<Vec<String>>>) -> RefCountedRegistry<&'static str, u32> {
        let log = log.clone();
        create_ref_counted_registry(move |key: &&'static str, options: Option<u32>| {
            log.borrow_mut().push(format!("activate {key} {options:?}"));
            let log = log.clone();
            let key = *key;
            Box::new(move || log.borrow_mut().push(format!("dispose {key}")))
        })
    }

    #[test]
    fn the_last_release_disposes_the_activation() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let registry = registry(&log);
        let a = registry.register("a", Some(1));
        // Later options are ignored while the key is live.
        let b = registry.register("a", Some(2));
        a();
        assert_eq!(*log.borrow(), ["activate a Some(1)"]);
        b();
        assert_eq!(*log.borrow(), ["activate a Some(1)", "dispose a"]);
        // Registering again activates again, with the new options.
        let c = registry.register("a", Some(3));
        c();
        assert_eq!(
            *log.borrow(),
            [
                "activate a Some(1)",
                "dispose a",
                "activate a Some(3)",
                "dispose a"
            ]
        );
    }

    #[test]
    fn a_stale_release_does_nothing() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let registry = registry(&log);
        let stale = registry.register("a", None);
        let other = registry.register("b", None);
        registry.dispose();
        assert_eq!(
            *log.borrow(),
            [
                "activate a None",
                "activate b None",
                "dispose a",
                "dispose b"
            ]
        );
        let live = registry.register("a", None);
        stale();
        other();
        assert_eq!(log.borrow().len(), 5);
        live();
        assert_eq!(log.borrow().last().unwrap(), "dispose a");
    }
}
