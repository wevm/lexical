//! `LexicalTablePluginHelpers.ts`: `registerTablePlugin` (INSERT_TABLE_COMMAND,
//! the clipboard, select-all and triple-click handlers and the table
//! integrity transforms) and `registerTableCellUnmergeTransform`.
//! `registerTableSelectionObserver` is in `table_selection_helpers`.

use super::table_cell_node::{TableCellNode, create_table_cell_node, is_table_cell_node};
use super::table_commands::{INSERT_TABLE_COMMAND, InsertTableCommandPayload, dimension};
use super::table_node::{TableNode, is_table_node};
use super::table_row_node::{TableRowNode, is_table_row_node};
use super::table_selection::create_table_selection_from;
use super::table_selection_helpers::find_table_node;
use super::table_utils::{
    compute_table_map, compute_table_map_skip_cell_check, create_table_node_with_dimensions,
    get_node_triplet, insert_table_into_grid, pad_table_rows,
};
use crate::commands::{
    CLICK_COMMAND, InsertClipboardNodesPayload, SELECT_ALL_COMMAND,
    SELECTION_INSERT_CLIPBOARD_NODES_COMMAND,
};
use crate::editor::{COMMAND_PRIORITY_EDITOR, COMMAND_PRIORITY_LOW};
use crate::editor_state::EditorStateInner;
use crate::error::invariant;
use crate::lexical_selection::{SelectionRef, get_selection, set_selection};
use crate::lexical_utils::{get_node_by_key, get_root};
use crate::merge_register::merge_register;
use crate::node::{NodeKey, NodeRef};
use crate::nodes::paragraph::create_paragraph_node;
use crate::selection_insert_nodes::{
    find_matching_parent, get_previous_selection, normalize_selection,
};
use crate::updates::get_active_editor;
use crate::utils::{dfs, insert_first, insert_node_to_nearest_root, unwrap_and_filter_descendants};
use crate::{Editor, LexicalError, Unregister};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

/// `$insertTable`.
fn insert_table(
    payload: &InsertTableCommandPayload,
    has_nested_tables: bool,
) -> Result<bool, LexicalError> {
    let Some(selection) = get_selection().or_else(get_previous_selection) else {
        return Ok(false);
    };
    let Some(range) = selection.range() else {
        return Ok(false);
    };
    // Prevent nested tables by checking if we're already inside a table.
    if !has_nested_tables && find_table_node(&range.anchor.get_node()?).is_some() {
        return Ok(false);
    }
    let table_node = create_table_node_with_dimensions(
        dimension(&payload.rows),
        dimension(&payload.columns),
        payload.include_headers.unwrap_or_default(),
    )?;
    insert_node_to_nearest_root(&table_node)?;
    if let Some(first) = table_node.get_first_descendant()
        && first.is_text()
    {
        first.select(None, None)?;
    }
    Ok(true)
}

/// Per update (its pending editor state), by table, the rows that lost
/// cells in the update (`rowsLost`, see `table_row_transform`).
type RowsLost = (Weak<EditorStateInner>, HashMap<NodeKey, HashSet<NodeKey>>);

thread_local! {
    static ROWS_LOST: RefCell<Option<RowsLost>> = const { RefCell::new(None) };
}

fn pending_state() -> Option<Rc<EditorStateInner>> {
    get_active_editor()?.pending().map(|state| state.0)
}

/// `$rowsLost`: the rows of `table` that lost cells in the running update.
fn rows_lost(table: &NodeRef) -> Option<HashSet<NodeKey>> {
    let pending = pending_state()?;
    ROWS_LOST.with(|cell| {
        let lost = cell.borrow();
        let (state, tables) = lost.as_ref()?;
        if !state.upgrade().is_some_and(|s| Rc::ptr_eq(&s, &pending)) {
            return None;
        }
        tables.get(table.get_key()).cloned()
    })
}

fn add_row_lost(table: &NodeRef, row: &NodeRef) {
    let Some(pending) = pending_state() else {
        return;
    };
    ROWS_LOST.with(|cell| {
        let mut lost = cell.borrow_mut();
        let fresh = !lost
            .as_ref()
            .and_then(|(state, _)| state.upgrade())
            .is_some_and(|s| Rc::ptr_eq(&s, &pending));
        if fresh {
            *lost = Some((Rc::downgrade(&pending), HashMap::new()));
        }
        if let Some((_, tables)) = lost.as_mut() {
            tables
                .entry(table.get_key().to_owned())
                .or_default()
                .insert(row.get_key().to_owned());
        }
    });
}

/// `$tableCellTransform`.
fn table_cell_transform(node: &NodeRef) -> Result<(), LexicalError> {
    let row = node.get_parent().filter(is_table_row_node);
    let Some(row) = row else {
        // TableCellNode must be a child of TableRowNode.
        return node.remove(false);
    };
    if node.is_empty() {
        // TableCellNode should never be empty.
        node.append(&[create_paragraph_node()])?;
    }
    // A cell changed after rows of its table lost cells in this update: the
    // table's transform pads those rows again.
    if let Some(table) = row.get_parent().filter(is_table_node)
        && rows_lost(&table).is_some()
    {
        table.mark_dirty();
    }
    Ok(())
}

/// The children count of `key` in the committed editor state, when it is a
/// table row there (`previous.__size`).
fn previous_row_size(key: &str) -> Option<usize> {
    let editor = get_active_editor()?;
    let state = editor.get_editor_state();
    let map = state.0.node_map.borrow();
    let previous = map.get(key)?;
    if !previous.is::<TableRowNode>() {
        return None;
    }
    previous.element().map(|e| e.size)
}

/// `$tableRowTransform`.
fn table_row_transform(node: &NodeRef) -> Result<(), LexicalError> {
    let Some(table) = node.get_parent().filter(is_table_node) else {
        // TableRowNode must be a child of TableNode.
        return node.remove(false);
    };
    unwrap_and_filter_descendants(node, is_table_cell_node)?;
    // A row that lost cells (a delete across cells removes some) can be
    // shorter than the table: the table's transform pads it. A row that
    // changed after rows lost cells in this update marks the table again.
    if previous_row_size(node.get_key()).is_some_and(|size| node.get_children_size() < size) {
        add_row_lost(&table, node);
        table.mark_dirty();
    } else if rows_lost(&table).is_some() {
        table.mark_dirty();
    }
    Ok(())
}

/// `$sameRows`: whether the running update left the rows of `table` as they
/// were (the same rows in the same order, in a table there before it).
fn same_rows(table: &NodeRef) -> bool {
    let Some(editor) = get_active_editor() else {
        return false;
    };
    let state = editor.get_editor_state();
    let before = state.0.node_map.borrow();
    let Some(previous) = before.get(table.get_key()) else {
        return false;
    };
    if !previous.is::<TableNode>() {
        return false;
    }
    let mut key = previous.element().and_then(|e| e.first.clone());
    for current in table.get_children_keys() {
        if key.as_deref() != Some(current.as_str()) {
            return false;
        }
        key = before.get(&current).and_then(|row| row.base().next.clone());
    }
    key.is_none()
}

/// `$tableTransform`.
fn table_transform(node: &NodeRef) -> Result<(), LexicalError> {
    // TableRowNode is the only valid child for TableNode.
    unwrap_and_filter_descendants(node, is_table_row_node)?;
    // A new table, or one whose rows the update changed, gets all its rows
    // padded; one whose rows are the ones it had, the rows that lost cells.
    let rows = rows_lost(node);
    if !same_rows(node) {
        pad_table_rows(node, |_| true)?;
    } else if let Some(rows) = rows {
        pad_table_rows(node, |row| rows.contains(row.get_key()))?;
    }
    let column_count = node.get_column_count();
    if let Some(col_widths) = node.get_col_widths()
        && col_widths.len() != column_count
    {
        let new_col_widths = if column_count < col_widths.len() {
            Some(col_widths[..column_count].to_vec())
        } else if let Some(&fill) = col_widths.last() {
            // Repeat the last column width.
            let mut widths = col_widths.clone();
            widths.resize(column_count, fill);
            Some(widths)
        } else {
            None
        };
        node.set_col_widths(new_col_widths);
    }
    Ok(())
}

/// `$tableClickCommand`: a triple click in a cell selects its block, not
/// the cell's other blocks.
fn table_click_command(detail: i32, target_key: Option<&str>) -> Result<bool, LexicalError> {
    if detail < 3 {
        return Ok(false);
    }
    let Some(start_node) = target_key.and_then(get_node_by_key) else {
        return Ok(false);
    };
    let Some(block_node) = find_matching_parent(&start_node, |n| n.is_element() && !n.is_inline())
    else {
        return Ok(false);
    };
    if !block_node
        .get_parent()
        .is_some_and(|p| is_table_cell_node(&p))
    {
        return Ok(false);
    }
    let selection = block_node.select(Some(0), None)?;
    normalize_selection(&selection)?;
    Ok(true)
}

/// `$tableSelectAllCommand`: in a table that is the root's only child, a
/// table selection of every cell.
fn table_select_all_command() -> Result<bool, LexicalError> {
    let Some(selection) = get_selection() else {
        return Ok(false);
    };
    let Some(range) = selection.range() else {
        return Ok(false);
    };
    let Some(table_node) = find_table_node(&range.anchor.get_node()?) else {
        return Ok(false);
    };
    let root = get_root();
    if !table_node.get_parent().is_some_and(|p| p == root) || root.get_children_size() != 1 {
        return Ok(false);
    }
    let (table_map, _, _) = compute_table_map_skip_cell_check(&table_node, None, None)?;
    let Some(first) = table_map
        .first()
        .and_then(|row| row.first())
        .cloned()
        .flatten()
    else {
        return Ok(false);
    };
    let Some(last) = table_map
        .last()
        .and_then(|row| row.last())
        .cloned()
        .flatten()
    else {
        return Ok(false);
    };
    let table_selection = create_table_selection_from(&table_node, &first.cell, &last.cell)?;
    set_selection(Some(table_selection));
    Ok(true)
}

/// `$isMultiCellTableSelection`.
fn is_multi_cell_table_selection(selection: &SelectionRef) -> Result<bool, LexicalError> {
    let Some((anchor, focus)) = selection.get_start_end_points() else {
        return Ok(false);
    };
    let (anchor, focus) = (anchor.get_node()?, focus.get_node()?);
    if selection.is_table() {
        return Ok(focus != anchor);
    }
    Ok(selection.is_range() && is_table_cell_node(&anchor) && anchor != focus)
}

/// `$tableSelectionInsertClipboardNodesCommand`.
fn table_selection_insert_clipboard_nodes(
    payload: &InsertClipboardNodesPayload,
    has_nested_tables: bool,
) -> Result<bool, LexicalError> {
    let InsertClipboardNodesPayload { nodes, selection } = payload;
    let has_tables = nodes
        .iter()
        .any(|n| is_table_node(n) || dfs(Some(n), None).iter().any(|d| is_table_node(&d.node)));
    if !has_tables {
        if let Some(table_selection) = selection.table() {
            let _ = table_selection;
            let mut text = String::new();
            let mut last_was_block = false;
            for node in nodes {
                let is_block = node.is_element() && !node.is_inline();
                if !text.is_empty() && (is_block || last_was_block) {
                    text.push('\n');
                }
                text.push_str(&node.get_text_content());
                last_was_block = is_block;
            }
            selection.insert_raw_text(&text)?;
            return Ok(true);
        }
        return Ok(false);
    }
    let is_table_selection = selection.is_table();
    let is_range_selection = selection.is_range();
    let is_selection_inside_of_grid = is_table_selection
        || (is_range_selection
            && selection
                .get_start_end_points()
                .is_some_and(|(anchor, focus)| {
                    let in_cell = |p: &crate::lexical_selection::Point| {
                        p.get_node()
                            .ok()
                            .and_then(|n| find_matching_parent(&n, is_table_cell_node))
                            .is_some()
                    };
                    in_cell(&anchor) && in_cell(&focus)
                }));
    if !is_selection_inside_of_grid {
        // Not pasting in a grid: no special handling required.
        return Ok(false);
    }
    // When pasting just a table, flatten the table on the destination table,
    // even when nested tables are allowed.
    if nodes.len() == 1 && is_table_node(&nodes[0]) {
        return insert_table_into_grid(&nodes[0], selection);
    }
    // If nested tables are enabled, allow pasting a table into a single cell.
    if is_range_selection && has_nested_tables && !is_multi_cell_table_selection(selection)? {
        return Ok(false);
    }
    // A table in the paste and nested tables not allowed: reject the paste.
    Ok(true)
}

/// `registerTablePlugin(editor, {hasNestedTables})`: INSERT_TABLE_COMMAND,
/// the clipboard, select-all and triple-click handlers and the table
/// integrity transforms. `has_nested_tables` is read when a command runs
/// (`hasNestedTables.peek()`).
pub fn register_table_plugin(
    editor: &Editor,
    has_nested_tables: impl Fn() -> bool + 'static,
) -> Result<Unregister, LexicalError> {
    invariant!(
        editor.has_nodes(&["table"]),
        "TablePlugin: TableNode is not registered on editor"
    );
    let has_nested_tables = Rc::new(has_nested_tables);
    let nested = has_nested_tables.clone();
    let weak = editor.weak();
    Ok(merge_register(vec![
        editor.register_command(
            &INSERT_TABLE_COMMAND,
            move |payload, _| insert_table(payload, nested()),
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &SELECTION_INSERT_CLIPBOARD_NODES_COMMAND,
            move |payload, dispatch_editor| {
                if !weak
                    .upgrade()
                    .is_some_and(|e| Rc::ptr_eq(&e, &dispatch_editor.0))
                {
                    return Ok(false);
                }
                table_selection_insert_clipboard_nodes(payload, has_nested_tables())
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &SELECT_ALL_COMMAND,
            |_, _| table_select_all_command(),
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &CLICK_COMMAND,
            |event, _| table_click_command(event.detail, event.target_key.as_deref()),
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_node_transform("table", table_transform),
        editor.register_node_transform("tablerow", table_row_transform),
        editor.register_node_transform("tablecell", table_cell_transform),
    ]))
}

/// `registerTableCellUnmergeTransform`: splits every merged cell, for an
/// editor that doesn't support merged cells.
pub fn register_table_cell_unmerge_transform(editor: &Editor) -> Unregister {
    editor.register_node_transform("tablecell", |node| {
        if node.get_col_span() <= 1.0 && node.get_row_span() <= 1.0 {
            return Ok(());
        }
        // With a row span the whole table is mapped to find where the new
        // cells go; all cells are split at once.
        let (_, _, grid_node) = get_node_triplet(node)?;
        let (grid_map, _, _) = compute_table_map(&grid_node, node, node)?;
        let rows_count = grid_map.len();
        let columns_count = grid_map.first().map_or(0, Vec::len);
        let mut row = grid_node.get_first_child();
        let mut unmerged = Vec::new();
        for (i, map_row) in grid_map.iter().enumerate().take(rows_count) {
            if i != 0 {
                row = row.and_then(|r| r.get_next_sibling());
            }
            let row_node = row.clone().filter(is_table_row_node);
            invariant!(
                row_node.is_some(),
                "Expected TableNode first child to be a RowNode"
            );
            let Some(row) = row_node else { unreachable!() };
            let mut last_row_cell: Option<NodeRef> = None;
            for (j, cell_map) in map_row.iter().enumerate().take(columns_count) {
                let Some(cell_map) = cell_map else {
                    continue;
                };
                let cell = &cell_map.cell;
                if cell_map.start_row == i && cell_map.start_column == j {
                    last_row_cell = Some(cell.clone());
                    unmerged.push(cell.clone());
                } else if cell.get_col_span() > 1.0 || cell.get_row_span() > 1.0 {
                    let header_state = cell
                        .get_latest()
                        .downcast_ref::<TableCellNode>()
                        .map_or(0, TableCellNode::header_state);
                    let new_cell = create_table_cell_node(header_state, 1.0, None);
                    match &last_row_cell {
                        Some(last) => {
                            last.insert_after(&new_cell, true)?;
                        }
                        None => insert_first(&row, &new_cell)?,
                    }
                }
            }
        }
        for cell in unmerged {
            cell.set_col_span(1.0);
            cell.set_row_span(1.0);
        }
        Ok(())
    })
}
