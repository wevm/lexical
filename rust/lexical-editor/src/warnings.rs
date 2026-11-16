//! `console.warn` in the JS implementation: the developer warnings Lexical
//! prints where it recovers from a misuse, as Swift's `consoleWarn` and
//! `withWarningHandler` (`Support/Warnings.swift`). An app or a test reads
//! them by installing a handler on its thread; without one they are written
//! to standard error.

use std::cell::RefCell;
use std::rc::Rc;

/// What receives the warnings.
type Handler = Rc<dyn Fn(&str)>;

thread_local! {
    static WARNING_HANDLER: RefCell<Option<Handler>> = const { RefCell::new(None) };
}

/// `console.warn(message)`: hands the warning to the handler
/// [`with_warning_handler`] installed, or writes it to standard error.
pub(crate) fn console_warn(message: &str) {
    let handler = WARNING_HANDLER.with(|h| h.borrow().clone());
    match handler {
        Some(handler) => handler(message),
        None => eprintln!("lexical: {message}"),
    }
}

/// `warnOnlyOnce(message)()` (`@lexical/internal/warnOnlyOnce`): the first
/// call on a thread warns; later ones with the same message don't.
#[cfg_attr(not(any(feature = "code", feature = "selection")), allow(dead_code))]
pub(crate) fn warn_only_once(message: &'static str) {
    thread_local! {
        static WARNED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    }
    let first = WARNED.with(|w| {
        let mut w = w.borrow_mut();
        let first = !w.contains(&message);
        if first {
            w.push(message);
        }
        first
    });
    if first {
        console_warn(message);
    }
}

/// Runs `f` with `handler` receiving, on this thread, the warnings JS
/// writes with `console.warn`, and restores the previous handler after,
/// even when `f` panics.
pub fn with_warning_handler<R>(handler: impl Fn(&str) + 'static, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Handler>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            WARNING_HANDLER.with(|h| *h.borrow_mut() = previous);
        }
    }
    let handler: Handler = Rc::new(handler);
    let previous = WARNING_HANDLER.with(|h| h.borrow_mut().replace(handler));
    let _restore = Restore(previous);
    f()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn a_handler_receives_warnings_and_is_restored() {
        let outer = Rc::new(RefCell::new(Vec::<String>::new()));
        let inner = Rc::new(RefCell::new(Vec::<String>::new()));
        let (o, i) = (outer.clone(), inner.clone());
        with_warning_handler(
            move |m| o.borrow_mut().push(m.to_owned()),
            || {
                console_warn("one");
                with_warning_handler(
                    move |m| i.borrow_mut().push(m.to_owned()),
                    || {
                        console_warn("two");
                    },
                );
                console_warn("three");
            },
        );
        assert_eq!(*outer.borrow(), ["one", "three"]);
        assert_eq!(*inner.borrow(), ["two"]);
    }
}
