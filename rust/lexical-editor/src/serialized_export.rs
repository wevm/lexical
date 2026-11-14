//! `LexicalSerializedExport.ts`: which JSON form an export writes.

use crate::json::{Object, Value};
use crate::node::LexicalNode;
use std::cell::Cell;

thread_local! {
    static COMPACT_EXPORT: Cell<bool> = const { Cell::new(false) };
}

/// `$withCompactExport(compact, f)`: runs `f` writing the compact form (or,
/// with `false`, the legacy form) for any export it performs.
pub fn with_compact_export<R>(compact: bool, f: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            COMPACT_EXPORT.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(COMPACT_EXPORT.with(|c| c.replace(compact)));
    f()
}

/// `$isCompactExport()`: whether the export walk in progress writes the
/// compact form.
pub fn is_compact_export() -> bool {
    COMPACT_EXPORT.with(Cell::get)
}

/// `$exportNodeJSON(node)`: one node's JSON in the active form, checked as
/// every export walk checks it. A failed check is a bug in the node's
/// implementation, so it panics.
pub fn export_node_json(node: &dyn LexicalNode) -> Object {
    let serialized_node = node.export_json(is_compact_export());
    if serialized_node.get("type").and_then(Value::as_str) != Some(node.get_type()) {
        panic!(
            "LexicalNode: Node {} does not match the serialized type. Check if .exportJSON() is implemented and it is returning the correct type.",
            node.get_type()
        );
    }
    if node.element().is_some()
        && serialized_node
            .get("children")
            .and_then(Value::as_array)
            .is_none()
    {
        panic!(
            "LexicalNode: Node {} is an element but .exportJSON() does not have a children array.",
            node.get_type()
        );
    }
    serialized_node
}
