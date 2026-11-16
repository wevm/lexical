//! `watchedSignal.ts`.

use super::signals::{Signal, SignalOptions, SignalValue, signal_with_options};
use crate::Unregister;
use std::cell::RefCell;
use std::rc::Rc;

/// `watchedSignal(getSnapshot, register)`: a signal that subscribes to an
/// external store while something watches it, as React's
/// `useSyncExternalStore` does. When it gains its first subscriber it takes
/// the store's value (`get_snapshot`) and calls `register`, which keeps it
/// up to date; when it loses its last, it calls what `register` returned.
pub fn watched_signal<T: SignalValue>(
    get_snapshot: impl Fn() -> T + 'static,
    register: impl Fn(&Signal<T>) -> Unregister + 'static,
) -> Signal<T> {
    let dispose: Rc<RefCell<Option<Unregister>>> = Rc::new(RefCell::new(None));
    let get_snapshot = Rc::new(get_snapshot);
    let initial = get_snapshot();
    let unwatched_dispose = dispose.clone();
    signal_with_options(
        initial,
        SignalOptions {
            unwatched: Some(Rc::new(move |_| {
                let dispose = unwatched_dispose.borrow_mut().take();
                if let Some(dispose) = dispose {
                    dispose();
                }
            })),
            watched: Some(Rc::new(move |this| {
                this.set_value(get_snapshot());
                *dispose.borrow_mut() = Some(register(this));
            })),
        },
    )
}
