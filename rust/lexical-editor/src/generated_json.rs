//! `LexicalGeneratedJSON.ts`: the JSON export and import JS generates from
//! the core nodes' serialization schemas, written out by hand here in the
//! same order and with the same tests. ParagraphNode's generated code is the
//! same as ElementNode's, and RootNode uses ElementNode's.

use crate::constants::{
    DETAIL_TYPE_TO_DETAIL, ELEMENT_TYPE_TO_FORMAT, TEXT_MODE_TO_TYPE, TEXT_TYPE_TO_FORMAT, lookup,
    name_of,
};
use crate::json::{Object, Value};
use crate::node::LexicalNode;
use crate::nodes::element::{Direction, ElementData};
use crate::nodes::text::TextData;

/// `JSON_NUMBER`: the JSON number grammar, anchored.
fn is_json_number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - start
    };
    if b.get(i) == Some(&b'-') {
        i += 1;
    }
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => {
            digits(&mut i);
        }
        _ => return false,
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        if digits(&mut i) == 0 {
            return false;
        }
    }
    if let Some(b'e' | b'E') = b.get(i) {
        i += 1;
        if let Some(b'+' | b'-') = b.get(i) {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return false;
        }
    }
    i == b.len()
}

/// `num(v, d)`: a finite number, or a string in the JSON number grammar
/// naming one; otherwise `d`.
pub(crate) fn num(v: Option<&Value>, d: f64) -> f64 {
    match v {
        Some(Value::Number(n)) => {
            if n.is_finite() {
                *n
            } else {
                d
            }
        }
        Some(Value::String(s)) if is_json_number(s) => {
            let n: f64 = s.parse().expect("JSON number");
            if n.is_finite() { n } else { d }
        }
        _ => d,
    }
}

/// `numC(v, d, min, max, integer)`.
pub(crate) fn num_c(v: Option<&Value>, d: f64, min: f64, max: f64, integer: bool) -> f64 {
    let n = num(v, d);
    if n >= min && n <= max && (!integer || n.fract() == 0.0) {
        n
    } else {
        d
    }
}

fn string_or(v: Option<&Value>, d: &str) -> String {
    v.and_then(Value::as_str).unwrap_or(d).to_owned()
}

fn element_format_name(e: &ElementData) -> Value {
    name_of(ELEMENT_TYPE_TO_FORMAT, e.format).map_or(Value::Undefined, Value::from)
}

fn direction_value(e: &ElementData) -> Value {
    e.dir.map_or(Value::Null, |d| d.as_str().into())
}

/// `exportElementNode`.
pub(crate) fn export_element_node(node: &dyn LexicalNode) -> Object {
    let e = node.element().expect("an element");
    let text_format = e.text_format;
    let text_style = &e.text_style;
    let should_serialize_text_styles =
        (text_format != 0.0 || !text_style.is_empty()) && node.should_serialize_text_styles();
    let mut json = Object::new();
    json.insert("children", Value::Array(Vec::new()));
    json.insert("direction", direction_value(e));
    json.insert("format", element_format_name(e));
    json.insert("indent", e.indent.into());
    json.insert(
        "textFormat",
        if text_format != 0.0 && should_serialize_text_styles {
            text_format.into()
        } else {
            Value::Undefined
        },
    );
    json.insert(
        "textStyle",
        if !text_style.is_empty() && should_serialize_text_styles {
            text_style.as_str().into()
        } else {
            Value::Undefined
        },
    );
    json.insert("type", node.get_type().into());
    json.insert("version", 1.0.into());
    json
}

/// `exportCompactElementNode`.
pub(crate) fn export_compact_element_node(node: &dyn LexicalNode) -> Object {
    let e = node.element().expect("an element");
    let text_format = e.text_format;
    let text_style = &e.text_style;
    let should_serialize_text_styles =
        (text_format != 0.0 || !text_style.is_empty()) && node.should_serialize_text_styles();
    let mut json = Object::new();
    json.insert("type", node.get_type().into());
    json.insert("children", Value::Array(Vec::new()));
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
    if text_format != 0.0 && should_serialize_text_styles {
        json.insert("textFormat", text_format.into());
    }
    if !text_style.is_empty() && should_serialize_text_styles {
        json.insert("textStyle", text_style.as_str().into());
    }
    json
}

/// `updateElementNode`.
pub(crate) fn update_element_node(e: &mut ElementData, json: &Object) {
    e.dir = match json.get("direction").and_then(Value::as_str) {
        Some("ltr") => Some(Direction::Ltr),
        Some("rtl") => Some(Direction::Rtl),
        _ => None,
    };
    e.format = json
        .get("format")
        .and_then(Value::as_str)
        .and_then(|f| lookup(ELEMENT_TYPE_TO_FORMAT, f))
        .unwrap_or(0);
    e.indent = num_c(json.get("indent"), 0.0, 0.0, f64::INFINITY, true);
    e.text_format = num(json.get("textFormat"), 0.0);
    e.text_style = string_or(json.get("textStyle"), "");
}

fn text_mode_name(t: &TextData) -> Value {
    name_of(TEXT_MODE_TO_TYPE, t.mode).map_or(Value::Undefined, Value::from)
}

/// `exportTextNode`.
pub(crate) fn export_text_node(node_type: &str, t: &TextData) -> Object {
    let mut json = Object::new();
    json.insert("detail", t.detail.into());
    json.insert("format", t.format.into());
    json.insert("mode", text_mode_name(t));
    json.insert("style", t.style.as_str().into());
    json.insert("text", t.text.as_str().into());
    json.insert("type", node_type.into());
    json.insert("version", 1.0.into());
    json
}

/// `exportCompactTextNode`.
pub(crate) fn export_compact_text_node(node_type: &str, t: &TextData) -> Object {
    let mut json = Object::new();
    json.insert("type", node_type.into());
    if t.detail != 0.0 {
        json.insert("detail", t.detail.into());
    }
    if t.format != 0.0 {
        json.insert("format", t.format.into());
    }
    let mode = text_mode_name(t);
    if !mode.is_undefined() && mode.as_str() != Some("normal") {
        json.insert("mode", mode);
    }
    if !t.style.is_empty() {
        json.insert("style", t.style.as_str().into());
    }
    if !t.text.is_empty() {
        json.insert("text", t.text.as_str().into());
    }
    json
}

/// A number or one of the names an alias table maps to a number.
fn aliased(table: &[(&str, i32)], v: Option<&Value>) -> f64 {
    match v.and_then(Value::as_str).and_then(|s| lookup(table, s)) {
        Some(n) => n as f64,
        None => num(v, 0.0),
    }
}

/// `updateTextNode`.
pub(crate) fn update_text_node(t: &mut TextData, json: &Object) {
    t.detail = aliased(DETAIL_TYPE_TO_DETAIL, json.get("detail"));
    t.format = aliased(TEXT_TYPE_TO_FORMAT, json.get("format"));
    t.mode = json
        .get("mode")
        .and_then(Value::as_str)
        .and_then(|m| lookup(TEXT_MODE_TO_TYPE, m))
        .unwrap_or(0);
    t.style = string_or(json.get("style"), "");
    t.text = string_or(json.get("text"), "");
}

/// `exportLineBreakNode`.
pub(crate) fn export_line_break_node(node_type: &str) -> Object {
    let mut json = Object::new();
    json.insert("type", node_type.into());
    json.insert("version", 1.0.into());
    json
}

/// `exportCompactLineBreakNode`.
pub(crate) fn export_compact_line_break_node(node_type: &str) -> Object {
    let mut json = Object::new();
    json.insert("type", node_type.into());
    json
}

/// `exportTabNode`. A tab's mode table holds only `normal`.
pub(crate) fn export_tab_node(node_type: &str, t: &TextData) -> Object {
    let mut json = Object::new();
    json.insert("detail", t.detail.into());
    json.insert(
        "mode",
        if t.mode == 0 {
            "normal".into()
        } else {
            Value::Undefined
        },
    );
    json.insert("text", t.text.as_str().into());
    json.insert("format", t.format.into());
    json.insert("style", t.style.as_str().into());
    json.insert("type", node_type.into());
    json.insert("version", 1.0.into());
    json
}

/// `exportCompactTabNode`.
pub(crate) fn export_compact_tab_node(node_type: &str, t: &TextData) -> Object {
    let mut json = Object::new();
    json.insert("type", node_type.into());
    if t.format != 0.0 {
        json.insert("format", t.format.into());
    }
    if !t.style.is_empty() {
        json.insert("style", t.style.as_str().into());
    }
    json
}

/// `updateTabNode`: a tab's detail, mode and text are fixed.
pub(crate) fn update_tab_node(t: &mut TextData, json: &Object) {
    t.format = aliased(TEXT_TYPE_TO_FORMAT, json.get("format"));
    t.style = string_or(json.get("style"), "");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_reads_what_js_reads() {
        let s = |s: &str| Some(Value::String(s.to_owned()));
        // Each `num(v, 7)` in JS.
        let cases: [(Option<Value>, f64); 9] = [
            (Some(Value::Number(1.5)), 1.5),
            (Some(Value::Number(f64::INFINITY)), 7.0),
            (s("12"), 12.0),
            (s("-1e2"), -100.0),
            (s("0x10"), 7.0),
            (s(""), 7.0),
            (s(" 1"), 7.0),
            (s("1e400"), 7.0),
            (None, 7.0),
        ];
        for (v, want) in cases {
            assert_eq!(num(v.as_ref(), 7.0), want, "{v:?}");
        }
        assert_eq!(num_c(Some(&1.5.into()), 0.0, 0.0, f64::INFINITY, true), 0.0);
        assert_eq!(
            num_c(Some(&(-1.0).into()), 0.0, 0.0, f64::INFINITY, true),
            0.0
        );
        assert_eq!(
            num_c(Some(&1e300.into()), 0.0, 0.0, f64::INFINITY, true),
            1e300
        );
    }
}
