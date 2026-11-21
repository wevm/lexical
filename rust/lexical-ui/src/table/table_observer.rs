//! The model parts of `LexicalTableObserver.ts`'s `TableObserver`:
//! `$clearText` and `$formatCells`, and the table selection that
//! `$setAnchorCellForSelection` and `$setFocusCellForSelection` make.
//!
//! Not ported: the observer itself, which tracks the rendered table's DOM
//! grid, pointer drags and the selection highlight. The model resolves cells
//! from the table map (rust/PARITY.md).

use super::table_cell_node::is_table_cell_node;
use super::table_node::is_table_node;
use super::table_row_node::is_table_row_node;
use super::table_selection::create_table_selection_from;
use crate::commands::{INSERT_PARAGRAPH_COMMAND, SELECTION_CHANGE_COMMAND};
use crate::error::invariant;
use crate::lexical_selection::{PointType, create_range_selection, get_selection, set_selection};
use crate::lexical_utils::copy_node;
use crate::node::NodeRef;
use crate::nodes::paragraph::{ParagraphNode, create_paragraph_node};
use crate::nodes::text::create_text_node;
use crate::{Editor, LexicalError};

/// `$setAnchorCellForSelection(anchor)` then
/// `$setFocusCellForSelection(focus, true)`: a table selection from the
/// cell `anchor` to the cell `focus` of `table`.
pub(crate) fn set_cells_for_selection(
    table: &NodeRef,
    anchor: &NodeRef,
    focus: &NodeRef,
) -> Result<(), LexicalError> {
    set_selection(Some(create_table_selection_from(table, anchor, focus)?));
    Ok(())
}

/// `TableObserver.$formatCells(type)`: toggles `format_type` on the text
/// of every selected cell, aligned with the first cell's paragraph.
pub(crate) fn format_cells(editor: &Editor, format_type: &str) -> Result<(), LexicalError> {
    let selection = get_selection();
    let Some(selection) = selection.filter(|s| s.is_table()) else {
        invariant!(false, "Expected Table selection");
        unreachable!()
    };
    let format_selection = create_range_selection();
    let cell_nodes: Vec<NodeRef> = selection
        .get_nodes()?
        .into_iter()
        .filter(is_table_cell_node)
        .collect();
    invariant!(!cell_nodes.is_empty(), "No table cells present");
    let align_format_with = cell_nodes[0]
        .get_first_child()
        .filter(|p| p.get_latest().is::<ParagraphNode>())
        .map(|p| f64::from(p.get_format_flags(format_type, None)));
    for cell_node in &cell_nodes {
        let key = cell_node.get_key();
        format_selection.point_set(
            crate::lexical_selection::Which::Anchor,
            key,
            0,
            PointType::Element,
        )?;
        format_selection.point_set(
            crate::lexical_selection::Which::Focus,
            key,
            cell_node.get_children_size(),
            PointType::Element,
        )?;
        format_selection.format_text(format_type, align_format_with)?;
    }
    set_selection(Some(selection));
    editor.dispatch_command(&SELECTION_CHANGE_COMMAND, ())?;
    Ok(())
}

/// `TableObserver.$clearText()`: empties the selected cells, or removes the
/// table when every cell is selected.
pub(crate) fn clear_text(editor: &Editor, table_node: &NodeRef) -> Result<(), LexicalError> {
    invariant!(is_table_node(table_node), "Expected TableNode.");
    let selection = get_selection();
    let Some(selection) = selection.filter(|s| s.is_table()) else {
        invariant!(false, "Expected TableSelection");
        unreachable!()
    };
    let selected_nodes: Vec<NodeRef> = selection
        .get_nodes()?
        .into_iter()
        .filter(is_table_cell_node)
        .collect();
    let first_row = table_node.get_first_child().filter(is_table_row_node);
    let last_row = table_node.get_last_child().filter(is_table_row_node);
    let is_entire_table_selected = match (
        selected_nodes.first(),
        selected_nodes.last(),
        &first_row,
        &last_row,
    ) {
        (Some(first), Some(last), Some(first_row), Some(last_row)) => {
            first_row.get_first_child().as_ref() == Some(first)
                && last_row.get_last_child().as_ref() == Some(last)
        }
        _ => false,
    };
    if is_entire_table_selected {
        table_node.select_previous(None, None)?;
        let parent = table_node.get_parent();
        table_node.remove(false)?;
        if parent.is_some_and(|p| p.is_root() && p.is_empty()) {
            editor.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
        }
        return Ok(());
    }
    for cell_node in &selected_nodes {
        let paragraph_node = match cell_node.get_first_child() {
            Some(first) if first.get_latest().is::<ParagraphNode>() => copy_node(&first),
            _ => create_paragraph_node(),
        };
        paragraph_node.append(&[create_text_node("")])?;
        cell_node.append(std::slice::from_ref(&paragraph_node))?;
        for child in cell_node.get_children() {
            if child != paragraph_node {
                child.remove(false)?;
            }
        }
    }
    set_selection(None);
    editor.dispatch_command(&SELECTION_CHANGE_COMMAND, ())?;
    Ok(())
}
