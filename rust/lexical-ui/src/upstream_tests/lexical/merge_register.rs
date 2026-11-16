//! Ports of `packages/lexical/src/__tests__/unit/mergeRegister.test.ts`.

use crate::merge_register::merge_register;
use std::cell::RefCell;
use std::rc::Rc;

// upstream: lexical/src/__tests__/unit/mergeRegister.test.ts::mergeRegister > calls all of the clean-up functions
#[test]
fn calls_all_of_the_clean_up_functions() {
    let calls = Rc::new(RefCell::new(0));
    let cleanup = || {
        let calls = calls.clone();
        Box::new(move || *calls.borrow_mut() += 1) as crate::Unregister
    };
    merge_register(vec![cleanup(), cleanup()])();
    assert_eq!(*calls.borrow(), 2);
}

// upstream: lexical/src/__tests__/unit/mergeRegister.test.ts::mergeRegister > calls the clean-up functions in reverse order
#[test]
fn calls_the_clean_up_functions_in_reverse_order() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let cleanup = |v: i32| {
        let calls = calls.clone();
        Box::new(move || calls.borrow_mut().push(v)) as crate::Unregister
    };
    merge_register(vec![cleanup(1), cleanup(2)])();
    assert_eq!(*calls.borrow(), [2, 1]);
}
