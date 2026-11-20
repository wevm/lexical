//! `TableImportExtension.ts`: the `DOMImportExtension` rules for tables
//! (`@experimental` upstream), which `TableExtension` contributes, and the
//! `TableSchema` and `TableRowSchema` child schemas.

use super::constants::parse_pixel_value;
use super::dom_helpers::{cell_index, col_span, row_index, row_span};
use super::table_cell_node::{
    TableCellHeaderStates, create_table_cell_node, is_table_cell_node, is_valid_vertical_align,
    with_cell,
};
use super::table_node::create_table_node;
use super::table_row_node::{create_table_row_node, is_table_row_node};
use crate::LexicalError;
use crate::constants::{IS_BOLD, IS_ITALIC, IS_STRIKETHROUGH, IS_UNDERLINE};
use crate::dom::DomNode;
use crate::html::import::sel::is_element_of_tag;
use crate::html::import::{
    ChildSchema, DomImportRule, ImportChildrenOpts, OnReject, context_value, define_import_rule,
    import_text_format, import_text_style, propagate_text_align_to_block_children, sel,
};
use crate::node::NodeRef;
use crate::nodes::line_break::LineBreakNode;
use crate::nodes::paragraph::create_paragraph_node;
use crate::utils::descendants_matching;
use std::rc::Rc;

/// `cellTextFormatMask(style)`: the format bits a cell's own style sets.
fn cell_text_format_mask(el: &DomNode) -> i32 {
    let style = el.style();
    let mut mask = 0;
    let font_weight = style.get_property_value("font-weight");
    if font_weight == "700" || font_weight == "bold" {
        mask |= IS_BOLD;
    }
    if style.get_property_value("font-style") == "italic" {
        mask |= IS_ITALIC;
    }
    let decoration = style.get_property_value("text-decoration");
    let decoration: Vec<&str> = decoration.split(' ').collect();
    if decoration.contains(&"underline") {
        mask |= IS_UNDERLINE;
    }
    if decoration.contains(&"line-through") {
        mask |= IS_STRIKETHROUGH;
    }
    mask
}

/// `$isInlineElementOrDecoratorNode(node) || $isTextNode(node) ||
/// $isLineBreakNode(node)`.
fn is_inline_content(node: &NodeRef) -> bool {
    let latest = node.get_latest();
    latest.is::<LineBreakNode>()
        || latest.text().is_some()
        || ((latest.element().is_some() || latest.is_decorator()) && latest.is_inline())
}

/// `$packageCellChildren(children)`: runs of inline children wrapped in
/// paragraphs, a paragraph holding only a line break emptied, and an empty
/// cell given a paragraph.
fn package_cell_children(children: Vec<NodeRef>) -> Result<Vec<NodeRef>, LexicalError> {
    let mut result = Vec::new();
    let mut paragraph: Option<NodeRef> = None;
    let flush_single_line_break = |paragraph: &Option<NodeRef>| -> Result<(), LexicalError> {
        if let Some(paragraph) = paragraph
            && let Some(first) = paragraph.get_first_child()
            && first.get_latest().is::<LineBreakNode>()
            && paragraph.get_children_size() == 1
        {
            first.remove(false)?;
        }
        Ok(())
    };
    for child in children {
        if is_inline_content(&child) {
            match &paragraph {
                Some(p) => p.append(&[child])?,
                None => {
                    let p = create_paragraph_node();
                    p.append(&[child])?;
                    result.push(p.clone());
                    paragraph = Some(p);
                }
            }
        } else {
            flush_single_line_break(&paragraph)?;
            paragraph = None;
            result.push(child);
        }
    }
    flush_single_line_break(&paragraph)?;
    if result.is_empty() {
        result.push(create_paragraph_node());
    }
    Ok(result)
}

/// `/^\d+$/`.
fn is_digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

/// `TableRule`.
fn table_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/table/table"),
        sel::tag(&["table"]),
        |ctx, el, _| {
            let node = create_table_node();
            if el.has_attribute("data-lexical-row-striping") {
                node.set_row_striping(true);
            }
            if el.has_attribute("data-lexical-frozen-column") {
                node.set_frozen_columns(1.0);
            }
            if el.has_attribute("data-lexical-frozen-row") {
                node.set_frozen_rows(1.0);
            }
            // `:scope > colgroup`, then its `:scope > col`.
            if let Some(col_group) = el
                .children()
                .into_iter()
                .find(|c| is_element_of_tag(c, "colgroup"))
            {
                let mut columns = Some(Vec::new());
                for col in col_group
                    .children()
                    .into_iter()
                    .filter(|c| is_element_of_tag(c, "col"))
                {
                    let style_width = col.style().get_property_value("width");
                    let width = match parse_pixel_value(&style_width) {
                        Some(width) => width,
                        None => {
                            let attr = col.get_attribute("width").unwrap_or_default();
                            if !is_digits(&attr) {
                                columns = None;
                                break;
                            }
                            attr.parse().unwrap_or(0.0)
                        }
                    };
                    if let Some(columns) = columns.as_mut() {
                        columns.push(width);
                    }
                }
                if let Some(columns) = columns {
                    node.set_col_widths(Some(columns));
                }
            }
            let children = ctx.import_children(el, &ImportChildrenOpts::default())?;
            node.splice(0, 0, &descendants_matching(&children, is_table_row_node))?;
            Ok(vec![node])
        },
    )
}

/// `TableRowRule`.
fn table_row_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/table/tr"),
        sel::tag(&["tr"]),
        |ctx, el, _| {
            let height = parse_pixel_value(&el.style().get_property_value("height"));
            let node = create_table_row_node(height);
            let children = ctx.import_children(el, &ImportChildrenOpts::default())?;
            node.splice(0, 0, &descendants_matching(&children, is_table_cell_node))?;
            Ok(vec![node])
        },
    )
}

/// `TableCellRule`.
fn table_cell_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/table/cell"),
        sel::tag(&["td", "th"]),
        |ctx, el, _| {
            let is_header = is_element_of_tag(el, "th");
            let style = el.style();
            let width = parse_pixel_value(&style.get_property_value("width"));
            let mut header_state = TableCellHeaderStates::NO_STATUS;
            if is_header {
                match el.get_attribute("scope").as_deref() {
                    Some("col") => header_state = TableCellHeaderStates::COLUMN,
                    Some("row") => header_state = TableCellHeaderStates::ROW,
                    _ => {
                        let parent_row = el.parent_element().filter(|r| is_element_of_tag(r, "tr"));
                        let is_in_header_row = parent_row.as_ref().is_some_and(|row| {
                            row.parent_element()
                                .is_some_and(|section| is_element_of_tag(&section, "thead"))
                                || row_index(row) == 0
                        });
                        if is_in_header_row {
                            header_state |= TableCellHeaderStates::ROW;
                        }
                        if cell_index(el) == 0 {
                            header_state |= TableCellHeaderStates::COLUMN;
                        }
                        if header_state == TableCellHeaderStates::NO_STATUS {
                            header_state = TableCellHeaderStates::ROW;
                        }
                    }
                }
            }
            let cell = create_table_cell_node(header_state, col_span(el), width);
            let rows = row_span(el);
            with_cell(&cell, |c| c.row_span = rows);
            let background_color = style.get_property_value("background-color");
            if !background_color.is_empty() {
                with_cell(&cell, |c| c.background_color = Some(background_color));
            }
            let vertical_align = style.get_property_value("vertical-align");
            if is_valid_vertical_align(Some(&vertical_align)) {
                with_cell(&cell, |c| c.vertical_align = Some(vertical_align));
            }
            let format_cfg = import_text_format();
            let inherited_format = ctx.get(&format_cfg);
            let cell_format = inherited_format | cell_text_format_mask(el);
            let style_cfg = import_text_style();
            let inherited_style = ctx.get(&style_cfg);
            let color = style.get_property_value("color");
            let mut branch_context = Vec::new();
            if cell_format != inherited_format {
                branch_context.push(context_value(&format_cfg, cell_format));
            }
            if !color.is_empty() {
                let mut cell_style = inherited_style;
                cell_style.insert("color", color);
                branch_context.push(context_value(&style_cfg, cell_style));
            }
            let packaged = package_cell_children(ctx.import_children(
                el,
                &ImportChildrenOpts {
                    context: Some(branch_context),
                    ..ImportChildrenOpts::default()
                },
            )?)?;
            let children = if is_header {
                packaged
            } else {
                propagate_text_align_to_block_children(packaged, Some(el))
            };
            cell.splice(0, 0, &children)?;
            Ok(vec![cell])
        },
    )
}

/// `TableSchema` (`@experimental`): accepts only `TableRowNode` children;
/// a run of cells is wrapped in a new row, and other runs dropped.
pub fn table_schema() -> ChildSchema {
    ChildSchema {
        name: Some("TableSchema".to_owned()),
        accepts: Rc::new(|child, _| is_table_row_node(child)),
        package_run: Some(Rc::new(|run, _, _| {
            if !run.iter().all(is_table_cell_node) {
                return Ok(Vec::new());
            }
            let row = create_table_row_node(None);
            row.splice(0, 0, &run)?;
            Ok(vec![row])
        })),
        on_reject: OnReject::default(),
        finalize: None,
    }
}

/// `TableRowSchema` (`@experimental`): accepts only `TableCellNode`
/// children.
pub fn table_row_schema() -> ChildSchema {
    ChildSchema {
        name: Some("TableRowSchema".to_owned()),
        accepts: Rc::new(|child, _| is_table_cell_node(child)),
        package_run: None,
        on_reject: OnReject::default(),
        finalize: None,
    }
}

/// `TableImportRules` (`@experimental`): the import rules for `TableNode`,
/// `TableRowNode` and `TableCellNode`, which `TableExtension` contributes
/// with `CoreImportExtension`.
pub fn table_import_rules() -> Vec<DomImportRule> {
    vec![table_rule(), table_row_rule(), table_cell_rule()]
}
