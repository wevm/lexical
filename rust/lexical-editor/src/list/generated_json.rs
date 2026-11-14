//! `LexicalListGeneratedJSON.ts`: the JSON export and import JS generates
//! from `ListNode`'s and `ListItemNode`'s serialization schemas, written out
//! by hand in the same order and with the same tests.

use super::list_item_node::ListItemNode;
use super::list_node::{ListNode, ListType};
use crate::generated_json::{direction_value, element_format_name, num, update_element_node};
use crate::json::{Object, Value};
use crate::node::LexicalNode;

/// The `textFormat` and `textStyle` an element writes: each only when it is
/// set and `shouldSerializeTextStyles()` allows it, otherwise `undefined`.
fn text_styles(node: &dyn LexicalNode) -> (Value, Value) {
    let e = node.element().expect("an element");
    let should_serialize_text_styles =
        (e.text_format != 0.0 || !e.text_style.is_empty()) && node.should_serialize_text_styles();
    let text_format = if e.text_format != 0.0 && should_serialize_text_styles {
        e.text_format.into()
    } else {
        Value::Undefined
    };
    let text_style = if !e.text_style.is_empty() && should_serialize_text_styles {
        e.text_style.as_str().into()
    } else {
        Value::Undefined
    };
    (text_format, text_style)
}

/// The compact form's `direction`, `format`, `textFormat` and `textStyle`,
/// each left out at its default, in the generated order. `indent` (between
/// `format` and `textFormat`) is the caller's.
fn insert_compact_element(json: &mut Object, node: &dyn LexicalNode, indent: Option<f64>) {
    let e = node.element().expect("an element");
    let (text_format, text_style) = text_styles(node);
    if e.dir.is_some() {
        json.insert("direction", direction_value(e));
    }
    let format = element_format_name(e);
    if !format.is_undefined() && format.as_str() != Some("") {
        json.insert("format", format);
    }
    if let Some(indent) = indent.filter(|i| *i != 0.0) {
        json.insert("indent", indent.into());
    }
    if !text_format.is_undefined() {
        json.insert("textFormat", text_format);
    }
    if !text_style.is_undefined() {
        json.insert("textStyle", text_style);
    }
}

/// `exportListNode`.
pub(super) fn export_list_node(node: &ListNode) -> Object {
    let e = node.element().expect("an element");
    let (text_format, text_style) = text_styles(node);
    let mut json = Object::new();
    json.insert("children", Value::Array(Vec::new()));
    json.insert("listType", node.list_type.as_str().into());
    json.insert("start", node.start.into());
    json.insert("tag", node.list_type.tag().into());
    json.insert("direction", direction_value(e));
    json.insert("format", element_format_name(e));
    json.insert("indent", e.indent.into());
    json.insert("textFormat", text_format);
    json.insert("textStyle", text_style);
    json.insert("type", node.get_type().into());
    json.insert("version", 1.0.into());
    json
}

/// `exportCompactListNode`.
pub(super) fn export_compact_list_node(node: &ListNode) -> Object {
    let mut json = Object::new();
    json.insert("type", node.get_type().into());
    json.insert("children", Value::Array(Vec::new()));
    if node.list_type != ListType::Number {
        json.insert("listType", node.list_type.as_str().into());
    }
    if node.start != 1.0 {
        json.insert("start", node.start.into());
    }
    let indent = node.element().expect("an element").indent;
    insert_compact_element(&mut json, node, Some(indent));
    json
}

/// `updateListNode`: `listType` reads the legacy tags (`LIST_LISTTYPE_ALIAS`,
/// `ol` and `ul`) as the types they stand for, and anything else unknown as
/// `number`.
pub(super) fn update_list_node(node: &mut ListNode, json: &Object) {
    update_element_node(&mut node.element, json);
    node.list_type = match json.get("listType").and_then(Value::as_str) {
        Some("ol") => ListType::Number,
        Some("ul") => ListType::Bullet,
        Some(list_type) => ListType::parse(list_type).unwrap_or_default(),
        None => ListType::Number,
    };
    node.start = num(json.get("start"), 1.0);
}

/// `exportListItemNode`, with what `getChecked()` and `getIndent()` return,
/// which depend on the item's place in the tree.
pub(super) fn export_list_item_node(
    node: &ListItemNode,
    checked: Option<bool>,
    indent: f64,
) -> Object {
    let e = node.element().expect("an element");
    let (text_format, text_style) = text_styles(node);
    let mut json = Object::new();
    json.insert("children", Value::Array(Vec::new()));
    json.insert("checked", checked.map_or(Value::Undefined, Value::from));
    json.insert("indent", indent.into());
    json.insert("value", node.value.into());
    json.insert("direction", direction_value(e));
    json.insert("format", element_format_name(e));
    json.insert("textFormat", text_format);
    json.insert("textStyle", text_style);
    json.insert("type", node.get_type().into());
    json.insert("version", 1.0.into());
    json
}

/// `exportCompactListItemNode`.
pub(super) fn export_compact_list_item_node(
    node: &ListItemNode,
    checked: Option<bool>,
    indent: f64,
) -> Object {
    let mut json = Object::new();
    json.insert("type", node.get_type().into());
    json.insert("children", Value::Array(Vec::new()));
    if let Some(checked) = checked {
        json.insert("checked", checked.into());
    }
    if indent != 0.0 {
        json.insert("indent", indent.into());
    }
    if node.value != 1.0 {
        json.insert("value", node.value.into());
    }
    insert_compact_element(&mut json, node, None);
    json
}

/// `updateListItemNode`. JS applies `indent` with `setIndent`, which nests
/// or unwraps the item in lists; on the new node `importJSON` gives it (no
/// parent, so no lists) that changes nothing, which is all this port calls
/// it on, so `indent` keeps the node's own. `checked` is kept when the JSON
/// has it, as a boolean or `false`.
pub(super) fn update_list_item_node(node: &mut ListItemNode, json: &Object) {
    let indent = node.element.indent;
    update_element_node(&mut node.element, json);
    node.element.indent = indent;
    node.checked = match json.get("checked") {
        None | Some(Value::Undefined) => None,
        Some(Value::Bool(checked)) => Some(*checked),
        Some(_) => Some(false),
    };
    node.value = num(json.get("value"), 1.0);
}
