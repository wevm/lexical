//! `LexicalLinkGeneratedJSON.ts`: the JSON export and import JS generates
//! from `LinkNode`'s and `AutoLinkNode`'s serialization schemas, written out
//! by hand in the same order and with the same tests. `afterCloneLinkNode`
//! and `afterCloneAutoLinkNode` are the nodes' `Clone`.

use super::link_node::{AutoLinkNode, LinkNode};
use crate::generated_json::{direction_value, element_format_name, update_element_node};
use crate::json::{Object, Value};
use crate::node::LexicalNode;

/// A nullable string field: the string, or `null`.
fn nullable(value: &Option<String>) -> Value {
    value.as_deref().map_or(Value::Null, Value::from)
}

/// `nullable(stringValue(), {defaultAsNull: true})`: a non-empty string, or
/// `null`.
fn default_as_null(json: &Object, key: &str) -> Option<String> {
    json.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

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

/// `exportLinkNode`, and with `is_unlinked` `exportAutoLinkNode`.
fn export(node: &dyn LexicalNode, link: &LinkNode, is_unlinked: Option<bool>) -> Object {
    let e = node.element().expect("an element");
    let (text_format, text_style) = text_styles(node);
    let mut json = Object::new();
    json.insert("children", Value::Array(Vec::new()));
    if let Some(is_unlinked) = is_unlinked {
        json.insert("isUnlinked", is_unlinked.into());
    }
    json.insert("rel", nullable(&link.rel));
    json.insert("target", nullable(&link.target));
    json.insert("title", nullable(&link.title));
    json.insert("url", link.url.as_str().into());
    json.insert("direction", direction_value(e));
    json.insert("format", element_format_name(e));
    json.insert("indent", e.indent.into());
    json.insert("textFormat", text_format);
    json.insert("textStyle", text_style);
    json.insert("type", node.get_type().into());
    json.insert("version", 1.0.into());
    json
}

/// `exportCompactLinkNode`, and with `is_unlinked`
/// `exportCompactAutoLinkNode`.
fn export_compact(node: &dyn LexicalNode, link: &LinkNode, is_unlinked: Option<bool>) -> Object {
    let e = node.element().expect("an element");
    let (text_format, text_style) = text_styles(node);
    let mut json = Object::new();
    json.insert("type", node.get_type().into());
    json.insert("children", Value::Array(Vec::new()));
    if is_unlinked == Some(true) {
        json.insert("isUnlinked", true.into());
    }
    for (key, value) in [
        ("rel", &link.rel),
        ("target", &link.target),
        ("title", &link.title),
    ] {
        if let Some(value) = value {
            json.insert(key, value.as_str().into());
        }
    }
    if !link.url.is_empty() {
        json.insert("url", link.url.as_str().into());
    }
    if e.dir.is_some() {
        json.insert("direction", direction_value(e));
    }
    let format = element_format_name(e);
    if !format.is_undefined() && format.as_str() != Some("") {
        json.insert("format", format);
    }
    if e.indent != 0.0 {
        json.insert("indent", e.indent.into());
    }
    if !text_format.is_undefined() {
        json.insert("textFormat", text_format);
    }
    if !text_style.is_undefined() {
        json.insert("textStyle", text_style);
    }
    json
}

/// `exportLinkNode`.
pub(super) fn export_link_node(node: &LinkNode) -> Object {
    export(node, node, None)
}

/// `exportCompactLinkNode`.
pub(super) fn export_compact_link_node(node: &LinkNode) -> Object {
    export_compact(node, node, None)
}

/// `updateLinkNode`.
pub(super) fn update_link_node(node: &mut LinkNode, json: &Object) {
    update_element_node(&mut node.element, json);
    node.rel = default_as_null(json, "rel");
    node.target = default_as_null(json, "target");
    node.title = default_as_null(json, "title");
    node.url = json
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
}

/// `exportAutoLinkNode`.
pub(super) fn export_auto_link_node(node: &AutoLinkNode) -> Object {
    export(node, &node.link, Some(node.is_unlinked))
}

/// `exportCompactAutoLinkNode`.
pub(super) fn export_compact_auto_link_node(node: &AutoLinkNode) -> Object {
    export_compact(node, &node.link, Some(node.is_unlinked))
}

/// `updateAutoLinkNode`.
pub(super) fn update_auto_link_node(node: &mut AutoLinkNode, json: &Object) {
    update_link_node(&mut node.link, json);
    node.is_unlinked = json
        .get("isUnlinked")
        .and_then(Value::as_bool)
        .unwrap_or(false);
}
