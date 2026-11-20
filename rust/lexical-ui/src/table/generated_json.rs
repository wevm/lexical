//! `LexicalTableGeneratedJSON.ts`: the JSON export and import JS generates
//! from `TableNode`'s, `TableRowNode`'s and `TableCellNode`'s serialization
//! schemas, written out by hand in the same order. The `afterClone`
//! functions are the nodes' `Clone`.

use super::table_cell_node::{TableCellHeaderStates, TableCellNode};
use super::table_node::TableNode;
use super::table_row_node::TableRowNode;
use crate::generated_json::{
    direction_value, element_format_name, num, num_c, update_element_node,
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

/// The legacy form's `direction`, `format`, `indent`, `textFormat`,
/// `textStyle`, `type` and `version`, after the node's own fields.
fn insert_element(json: &mut Object, node: &dyn LexicalNode) {
    let e = node.element().expect("an element");
    let (text_format, text_style) = text_styles(node);
    json.insert("direction", direction_value(e));
    json.insert("format", element_format_name(e));
    json.insert("indent", e.indent.into());
    json.insert("textFormat", text_format);
    json.insert("textStyle", text_style);
    json.insert("type", node.get_type().into());
    json.insert("version", 1.0.into());
}

/// The compact form's `direction`, `format`, `indent`, `textFormat` and
/// `textStyle`, each left out at its default.
fn insert_compact_element(json: &mut Object, node: &dyn LexicalNode) {
    let e = node.element().expect("an element");
    let (text_format, text_style) = text_styles(node);
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
}

fn compact_start(node: &dyn LexicalNode) -> Object {
    let mut json = Object::new();
    json.insert("type", node.get_type().into());
    json.insert("children", Value::Array(Vec::new()));
    json
}

fn col_widths_value(col_widths: &Option<Vec<f64>>) -> Value {
    col_widths.as_ref().map_or(Value::Undefined, |widths| {
        Value::Array(widths.iter().map(|w| Value::Number(*w)).collect())
    })
}

/// `exportTableNode`.
pub(super) fn export_table_node(node: &TableNode) -> Object {
    let mut json = Object::new();
    json.insert("children", Value::Array(Vec::new()));
    json.insert("colWidths", col_widths_value(&node.col_widths));
    json.insert(
        "frozenColumnCount",
        node.serialized_frozen_column_count()
            .map_or(Value::Undefined, Value::from),
    );
    json.insert(
        "frozenRowCount",
        node.serialized_frozen_row_count()
            .map_or(Value::Undefined, Value::from),
    );
    json.insert(
        "rowStriping",
        node.serialized_row_striping()
            .map_or(Value::Undefined, Value::from),
    );
    insert_element(&mut json, node);
    json
}

/// `exportCompactTableNode`.
pub(super) fn export_compact_table_node(node: &TableNode) -> Object {
    let mut json = compact_start(node);
    if node.col_widths.is_some() {
        json.insert("colWidths", col_widths_value(&node.col_widths));
    }
    if let Some(count) = node.serialized_frozen_column_count() {
        json.insert("frozenColumnCount", count.into());
    }
    if let Some(count) = node.serialized_frozen_row_count() {
        json.insert("frozenRowCount", count.into());
    }
    if node.serialized_row_striping().is_some() {
        json.insert("rowStriping", true.into());
    }
    insert_compact_element(&mut json, node);
    json
}

/// `updateTableNode`: a `colWidths` that is not an array is `[]`, and an
/// entry that is not a number is 0.
pub(super) fn update_table_node(node: &mut TableNode, json: &Object) {
    update_element_node(&mut node.element, json);
    node.col_widths = match json.get("colWidths") {
        None | Some(Value::Undefined) => None,
        Some(Value::Array(widths)) => Some(widths.iter().map(|w| num(Some(w), 0.0)).collect()),
        Some(_) => Some(Vec::new()),
    };
    node.frozen_column_count = num(json.get("frozenColumnCount"), 0.0);
    node.frozen_row_count = num(json.get("frozenRowCount"), 0.0);
    node.row_striping = json
        .get("rowStriping")
        .and_then(Value::as_bool)
        .unwrap_or(false);
}

/// `exportTableRowNode`.
pub(super) fn export_table_row_node(node: &TableRowNode) -> Object {
    let mut json = Object::new();
    json.insert("children", Value::Array(Vec::new()));
    json.insert("height", node.height.map_or(Value::Undefined, Value::from));
    insert_element(&mut json, node);
    json
}

/// `exportCompactTableRowNode`.
pub(super) fn export_compact_table_row_node(node: &TableRowNode) -> Object {
    let mut json = compact_start(node);
    if let Some(height) = node.height {
        json.insert("height", height.into());
    }
    insert_compact_element(&mut json, node);
    json
}

/// `updateTableRowNode`.
pub(super) fn update_table_row_node(node: &mut TableRowNode, json: &Object) {
    update_element_node(&mut node.element, json);
    node.height = match json.get("height") {
        None | Some(Value::Undefined) => None,
        height => Some(num(height, 0.0)),
    };
}

/// `exportTableCellNode`.
pub(super) fn export_table_cell_node(node: &TableCellNode) -> Object {
    let mut json = Object::new();
    json.insert("children", Value::Array(Vec::new()));
    json.insert(
        "backgroundColor",
        node.background_color
            .as_deref()
            .map_or(Value::Null, Value::from),
    );
    json.insert("colSpan", node.col_span.into());
    json.insert("headerState", f64::from(node.header_state).into());
    json.insert("rowSpan", node.row_span.into());
    json.insert(
        "verticalAlign",
        node.serialized_vertical_align()
            .map_or(Value::Undefined, Value::from),
    );
    json.insert("width", node.width.map_or(Value::Undefined, Value::from));
    insert_element(&mut json, node);
    json
}

/// `exportCompactTableCellNode`.
pub(super) fn export_compact_table_cell_node(node: &TableCellNode) -> Object {
    let mut json = compact_start(node);
    if let Some(background_color) = &node.background_color {
        json.insert("backgroundColor", background_color.as_str().into());
    }
    if node.col_span != 1.0 {
        json.insert("colSpan", node.col_span.into());
    }
    if node.header_state != 0 {
        json.insert("headerState", f64::from(node.header_state).into());
    }
    if node.row_span != 1.0 {
        json.insert("rowSpan", node.row_span.into());
    }
    if let Some(vertical_align) = node.serialized_vertical_align() {
        json.insert("verticalAlign", vertical_align.into());
    }
    if let Some(width) = node.width {
        json.insert("width", width.into());
    }
    insert_compact_element(&mut json, node);
    json
}

/// `updateTableCellNode`.
pub(super) fn update_table_cell_node(node: &mut TableCellNode, json: &Object) {
    update_element_node(&mut node.element, json);
    node.background_color = json
        .get("backgroundColor")
        .and_then(Value::as_str)
        .filter(|c| !c.is_empty())
        .map(str::to_owned);
    node.col_span = num_c(json.get("colSpan"), 1.0, 1.0, f64::INFINITY, true);
    // `setHeaderStyles(num(json.headerState, 0))`, with its default mask.
    let header_state = crate::nodes::text::to_int32(num(json.get("headerState"), 0.0));
    let mask = TableCellHeaderStates::BOTH;
    node.header_state = (header_state & mask) | (node.header_state & !mask);
    node.row_span = num_c(json.get("rowSpan"), 1.0, 1.0, f64::INFINITY, true);
    node.vertical_align = match json.get("verticalAlign").and_then(Value::as_str) {
        Some(v @ ("middle" | "bottom")) => Some(v.to_owned()),
        _ => None,
    };
    node.width = match json.get("width") {
        None | Some(Value::Undefined) => None,
        width => Some(num(width, 0.0)).filter(|w| *w != 0.0),
    };
}
