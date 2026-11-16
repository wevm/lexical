//! `utils/mergeRegister.ts`.

use crate::Unregister;

/// `mergeRegister(...func)`: one clean-up function that calls each of
/// `funcs` in reverse order, as clean-ups usually need to run in the reverse
/// of the order they were registered.
pub fn merge_register(funcs: Vec<Unregister>) -> Unregister {
    Box::new(move || {
        for func in funcs.into_iter().rev() {
            func();
        }
    })
}
