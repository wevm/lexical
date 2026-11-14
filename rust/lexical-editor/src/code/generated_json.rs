//! `LexicalCodeCoreGeneratedJSON.ts`: the JSON export and import JS
//! generates from `CodeNode`'s and `CodeHighlightNode`'s serialization
//! schemas, written out by hand in the same order and with the same tests.
//! `afterCloneCodeNode` and `afterCloneCodeHighlightNode` are the nodes'
//! `Clone`.

use super::code_highlight_node::CodeHighlightNode;
use super::code_node::CodeNode;
use crate::generated_json::{
    direction_value, element_format_name, export_compact_text_node, export_text_node,
    update_element_node, update_text_node,
};
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

/// An optional string field: the string, or `undefined`.
fn optional(value: &Option<String>) -> Value {
    value.as_deref().map_or(Value::Undefined, Value::from)
}

/// What a setter that normalizes a falsy value to `undefined`
/// (`setLanguage`, `setTheme`, `setHighlightType`) keeps of a JSON field:
/// a non-empty string. Any other value (`null`, `undefined`, `''`, or what
/// the generated code turns into `''`) is `undefined`.
fn truthy_string(json: &Object, key: &str) -> Option<String> {
    json.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// `exportCodeNode`.
pub(super) fn export_code_node(node: &CodeNode) -> Object {
    let e = &node.element;
    let (text_format, text_style) = text_styles(node);
    let mut json = Object::new();
    json.insert("children", Value::Array(Vec::new()));
    json.insert("language", optional(&node.language));
    json.insert("theme", optional(&node.theme));
    json.insert("direction", direction_value(e));
    json.insert("format", element_format_name(e));
    json.insert("indent", e.indent.into());
    json.insert("textFormat", text_format);
    json.insert("textStyle", text_style);
    json.insert("type", node.get_type().into());
    json.insert("version", 1.0.into());
    json
}

/// `exportCompactCodeNode`.
pub(super) fn export_compact_code_node(node: &CodeNode) -> Object {
    let e = &node.element;
    let (text_format, text_style) = text_styles(node);
    let mut json = Object::new();
    json.insert("type", node.get_type().into());
    json.insert("children", Value::Array(Vec::new()));
    if let Some(language) = &node.language {
        json.insert("language", language.as_str().into());
    }
    if let Some(theme) = &node.theme {
        json.insert("theme", theme.as_str().into());
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

/// `updateCodeNode`: the element's fields, then `setLanguage` and
/// `setTheme`.
pub(super) fn update_code_node(node: &mut CodeNode, json: &Object) {
    update_element_node(&mut node.element, json);
    node.language = truthy_string(json, "language");
    node.theme = truthy_string(json, "theme");
}

/// `exportCodeHighlightNode`: `highlightType`, then the text node's fields.
pub(super) fn export_code_highlight_node(node: &CodeHighlightNode) -> Object {
    let mut json = Object::new();
    json.insert("highlightType", optional(&node.highlight_type));
    for (key, value) in export_text_node(node.get_type(), &node.text).iter() {
        json.insert(key, value.clone());
    }
    json
}

/// `exportCompactCodeHighlightNode`: `type`, `highlightType`, then the
/// text node's fields.
pub(super) fn export_compact_code_highlight_node(node: &CodeHighlightNode) -> Object {
    let mut json = Object::new();
    json.insert("type", node.get_type().into());
    if let Some(highlight_type) = &node.highlight_type {
        json.insert("highlightType", highlight_type.as_str().into());
    }
    // `type` is already first; setting it again keeps its place.
    for (key, value) in export_compact_text_node(node.get_type(), &node.text).iter() {
        json.insert(key, value.clone());
    }
    json
}

/// `updateCodeHighlightNode`: the text node's fields, except that `format`
/// goes through `setFormat`, which `CodeHighlightNode` makes a no-op, so a
/// node keeps the format it had; then `setHighlightType`.
pub(super) fn update_code_highlight_node(node: &mut CodeHighlightNode, json: &Object) {
    let format = node.text.format;
    update_text_node(&mut node.text, json);
    node.text.format = format;
    node.highlight_type = truthy_string(json, "highlightType");
}
