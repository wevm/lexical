//! Ports of `packages/lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts`.

use crate::Unregister;
use crate::ref_counted_registry::create_ref_counted_registry;
use std::cell::RefCell;
use std::rc::Rc;

/// `vi.fn()` used as a disposer: a clean-up that counts its calls.
fn counted(calls: &Rc<RefCell<u32>>) -> Unregister {
    let calls = calls.clone();
    Box::new(move || *calls.borrow_mut() += 1)
}

// upstream: lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > activates on the first registration and passes options through
#[test]
fn activates_on_the_first_registration_and_passes_options_through() {
    #[derive(Clone, Debug, PartialEq)]
    struct Options {
        n: i32,
    }
    let activate = Rc::new(RefCell::new(Vec::new()));
    let calls = activate.clone();
    let registry = create_ref_counted_registry(move |key: &String, options: Option<Options>| {
        calls.borrow_mut().push((key.clone(), options));
        Box::new(|| {}) as Unregister
    });
    let _ = registry.register("a".to_owned(), Some(Options { n: 1 }));
    assert_eq!(activate.borrow().len(), 1);
    assert_eq!(
        activate.borrow()[0],
        ("a".to_owned(), Some(Options { n: 1 }))
    );
}

// upstream: lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > reference counts: activates once, disposes only after the last release
#[test]
fn reference_counts_activates_once_disposes_only_after_the_last_release() {
    let dispose = Rc::new(RefCell::new(0));
    let activate = Rc::new(RefCell::new(0));
    let (d, a) = (dispose.clone(), activate.clone());
    let registry = create_ref_counted_registry(move |_: &String, _: Option<()>| {
        *a.borrow_mut() += 1;
        counted(&d)
    });
    let release_a = registry.register("k".to_owned(), None);
    let release_b = registry.register("k".to_owned(), None);
    assert_eq!(*activate.borrow(), 1);
    release_a();
    assert_eq!(*dispose.borrow(), 0);
    release_b();
    assert_eq!(*dispose.borrow(), 1);
}

// upstream: lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > distinct keys activate and dispose independently
#[test]
fn distinct_keys_activate_and_dispose_independently() {
    let dispose_a = Rc::new(RefCell::new(0));
    let dispose_b = Rc::new(RefCell::new(0));
    let (a, b) = (dispose_a.clone(), dispose_b.clone());
    let registry = create_ref_counted_registry(
        move |key: &String, _: Option<()>| {
            if key == "a" { counted(&a) } else { counted(&b) }
        },
    );
    let _ = registry.register("a".to_owned(), None);
    let release_b = registry.register("b".to_owned(), None);
    release_b();
    assert_eq!(*dispose_a.borrow(), 0);
    assert_eq!(*dispose_b.borrow(), 1);
}

// upstream: lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > re-registering a released key creates a fresh activation
#[test]
fn re_registering_a_released_key_creates_a_fresh_activation() {
    let dispose = Rc::new(RefCell::new(0));
    let activate = Rc::new(RefCell::new(0));
    let (d, a) = (dispose.clone(), activate.clone());
    let registry = create_ref_counted_registry(move |_: &String, _: Option<()>| {
        *a.borrow_mut() += 1;
        counted(&d)
    });
    registry.register("k".to_owned(), None)();
    assert_eq!(*dispose.borrow(), 1);
    let _ = registry.register("k".to_owned(), None);
    assert_eq!(*activate.borrow(), 2);
}

// upstream: lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > dispose() tears down every live registration
#[test]
fn dispose_tears_down_every_live_registration() {
    let dispose_a = Rc::new(RefCell::new(0));
    let dispose_b = Rc::new(RefCell::new(0));
    let (a, b) = (dispose_a.clone(), dispose_b.clone());
    let registry = create_ref_counted_registry(
        move |key: &String, _: Option<()>| {
            if key == "a" { counted(&a) } else { counted(&b) }
        },
    );
    let _ = registry.register("a".to_owned(), None);
    let _ = registry.register("b".to_owned(), None);
    let _ = registry.register("b".to_owned(), None);
    registry.dispose();
    assert_eq!(*dispose_a.borrow(), 1);
    assert_eq!(*dispose_b.borrow(), 1);
}

// upstream: lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > a disposer held after registry.dispose() is a no-op
#[test]
fn a_disposer_held_after_registry_dispose_is_a_no_op() {
    let dispose = Rc::new(RefCell::new(0));
    let d = dispose.clone();
    let registry = create_ref_counted_registry(move |_: &String, _: Option<()>| counted(&d));
    let release = registry.register("k".to_owned(), None);
    registry.dispose();
    assert_eq!(*dispose.borrow(), 1);
    release();
    assert_eq!(*dispose.borrow(), 1);
}
