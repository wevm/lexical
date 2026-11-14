//! `LexicalMarkGeneratedJSON.ts`: the JSON export and import JS generates
//! from `MarkNode`'s serialization schema, written out by hand in the same
//! order. `afterCloneMarkNode` is the node's `Clone`.

use super::mark_node::MarkNode;
use crate::generated_json::{
    export_compact_element_node, export_element_node, update_element_node,
};
use crate::json::{Object, Value};

/// `json` with `ids` after `children`, where the generated export writes it
/// (the compact form only when there are ids).
fn with_ids(json: Object, ids: &[String], compact: bool) -> Object {
    let mut out = Object::new();
    for (key, value) in json.iter() {
        out.insert(key, value.clone());
        if key == "children" && !(compact && ids.is_empty()) {
            out.insert(
                "ids",
                Value::Array(ids.iter().map(|id| id.as_str().into()).collect()),
            );
        }
    }
    out
}

/// `exportMarkNode`.
pub(super) fn export_mark_node(node: &MarkNode) -> Object {
    with_ids(export_element_node(node), &node.ids, false)
}

/// `exportCompactMarkNode`.
pub(super) fn export_compact_mark_node(node: &MarkNode) -> Object {
    with_ids(export_compact_element_node(node), &node.ids, true)
}

/// `updateMarkNode`: an `ids` that is not an array is `[]`, and an entry
/// that is not a string is `''` (`arrayValue(stringValue())`).
pub(super) fn update_mark_node(node: &mut MarkNode, json: &Object) {
    update_element_node(&mut node.element, json);
    node.ids = json
        .get("ids")
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .map(|id| id.as_str().unwrap_or("").to_owned())
                .collect()
        })
        .unwrap_or_default();
}
