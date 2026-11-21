//! `LexicalTableSelectionHelpers.ts`: finding a node's cell and table, and
//! the model parts of `applyTableHandlers` and
//! `$handleTableSelectionChangeCommand`, registered by
//! [`register_table_selection_observer`].
//!
//! JS attaches one set of handlers to each table the reconciler rendered;
//! the port registers one set per editor that finds the table from the
//! selection. A cell's coordinates are its index in its row and its row's
//! index in the table, as `getTable` numbers the rendered cells. Up and down
//! leave a cell when the app's caret movement
//! ([`SelectionMovement`](crate::SelectionMovement), by line) lands outside
//! it, where JS compares the caret's and the cell's client rects.
//!
//! Not ported: pointer selection and the highlight, the document's copy and
//! paste listeners, the typeahead menu check, `FOCUS_COMMAND`,
//! `$getTableEdgeCursorPosition` (a DOM caret beside the `<table>` element,
//! which a model selection can't be) and `$fixTableSelectionForSelectedTable`,
//! which read the DOM selection.

use super::table_cell_node::is_table_cell_node;
use super::table_node::is_table_node;
use super::table_observer::{clear_text, format_cells, set_cells_for_selection};
use super::table_row_node::is_table_row_node;
use super::table_selection::create_table_selection_from;
use super::table_utils::{
    TableCellRectBoundary, TableMap, TableMapValue, compute_table_cell_rect_boundary,
    compute_table_cell_rect_spans, compute_table_map, get_node_triplet,
};
use crate::caret::{
    CaretDirection, RootMode, extend_caret_to_range, get_child_caret, get_sibling_caret,
};
use crate::caret_utils::{
    caret_from_point, is_extendable_text_point_caret, normalize_caret, set_point_from_caret,
};
use crate::commands::{
    DELETE_CHARACTER_COMMAND, DELETE_LINE_COMMAND, DELETE_WORD_COMMAND, FORMAT_ELEMENT_COMMAND,
    FORMAT_TEXT_COMMAND, KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND, KEY_ARROW_RIGHT_COMMAND,
    KEY_ARROW_UP_COMMAND, KEY_BACKSPACE_COMMAND, KEY_DELETE_COMMAND, KEY_ESCAPE_COMMAND,
    KEY_TAB_COMMAND, SELECTION_CHANGE_COMMAND,
};
use crate::editor::COMMAND_PRIORITY_HIGH;
use crate::events::KeyboardEvent;
use crate::lexical_selection::{PointType, SelectionRef, Which, get_selection, set_selection};
use crate::lexical_utils::{get_node_by_key, is_root_or_shadow_root};
use crate::merge_register::merge_register;
use crate::node::NodeRef;
use crate::selection_deletion::SelectionGranularity;
use crate::selection_insert_nodes::find_matching_parent;
use crate::{Editor, LexicalError, Unregister};
use std::collections::HashSet;

/// `$findCellNode`: the table cell `node` is in (or is), if any.
pub fn find_cell_node(node: &NodeRef) -> Option<NodeRef> {
    find_matching_parent(node, is_table_cell_node)
}

/// `$findTableNode`: the table `node` is in (or is), if any.
pub fn find_table_node(node: &NodeRef) -> Option<NodeRef> {
    find_matching_parent(node, is_table_node)
}

/// The nodes of a selection's anchor and focus.
fn point_nodes(selection: &SelectionRef) -> Option<(NodeRef, NodeRef)> {
    let (anchor, focus) = selection.get_start_end_points()?;
    Some((anchor.get_node().ok()?, focus.get_node().ok()?))
}

/// `$isSelectionInTable`: whether a range or table selection has both its
/// points inside `table_node`.
pub(crate) fn is_selection_in_table(
    selection: Option<&SelectionRef>,
    table_node: &NodeRef,
) -> bool {
    let Some(selection) = selection.filter(|s| s.is_range() || s.is_table()) else {
        return false;
    };
    point_nodes(selection).is_some_and(|(anchor, focus)| {
        table_node.is_parent_of(&anchor) && table_node.is_parent_of(&focus)
    })
}

/// The table whose handlers a JS editor would run for `selection`: the
/// table of a table selection, or the innermost table holding both points
/// of a range selection.
fn selection_table(selection: Option<&SelectionRef>) -> Option<NodeRef> {
    let selection = selection?;
    if let Some(table) = selection.table() {
        return get_node_by_key(&table.table_key).filter(is_table_node);
    }
    let (anchor, _) = point_nodes(selection)?;
    let mut table = find_table_node(&anchor);
    while let Some(t) = table {
        if is_selection_in_table(Some(selection), &t) {
            return Some(t);
        }
        table = t.get_parent().and_then(|p| find_table_node(&p));
    }
    None
}

/// `stopEvent`.
fn stop_event(event: &KeyboardEvent) {
    event.event.prevent_default();
    event.event.stop_propagation();
}

/// `$findParentTableCellNodeInTable`: the outermost cell of `table_node`
/// holding `node`.
fn find_parent_table_cell_node_in_table(table_node: &NodeRef, node: &NodeRef) -> Option<NodeRef> {
    let mut last_table_cell_node = None;
    let mut current = Some(node.clone());
    while let Some(c) = current {
        if c == *table_node {
            return last_table_cell_node;
        }
        if is_table_cell_node(&c) {
            last_table_cell_node = Some(c.clone());
        }
        current = c.get_parent();
    }
    None
}

/// `$getBlockParentIfFirstNode`.
fn get_block_parent_if_first_node(node: &NodeRef) -> Option<NodeRef> {
    let mut prev = node.clone();
    let mut current = Some(node.clone());
    while let Some(c) = current {
        if c.is_element() {
            if c != prev && c.get_first_child().as_ref() != Some(&prev) {
                return None;
            }
            if !c.is_inline() {
                return Some(c);
            }
        }
        prev = c.clone();
        current = c.get_parent();
    }
    None
}

/// `$selectAdjacentCell`: Tab and Shift+Tab, to the end of the next or
/// previous cell, or past the table.
fn select_adjacent_cell(table_cell_node: &NodeRef, next: bool) -> Result<(), LexicalError> {
    let sibling = |n: &NodeRef| {
        if next {
            n.get_next_sibling()
        } else {
            n.get_previous_sibling()
        }
    };
    let child = |n: &NodeRef| {
        if next {
            n.get_first_child()
        } else {
            n.get_last_child()
        }
    };
    if let Some(s) = sibling(table_cell_node).filter(NodeRef::is_element) {
        s.select_end()?;
        return Ok(());
    }
    let parent_row = find_matching_parent(table_cell_node, is_table_row_node);
    crate::error::invariant!(
        parent_row.is_some(),
        "selectAdjacentCell: Cell not in table row"
    );
    let parent_row = parent_row.unwrap_or_else(|| unreachable!());
    let mut next_row = sibling(&parent_row);
    while let Some(row) = next_row.filter(is_table_row_node) {
        if let Some(c) = child(&row).filter(NodeRef::is_element) {
            c.select_end()?;
            return Ok(());
        }
        next_row = sibling(&row);
    }
    let parent_table = find_matching_parent(&parent_row, is_table_node);
    crate::error::invariant!(
        parent_table.is_some(),
        "selectAdjacentCell: Row not in table"
    );
    let parent_table = parent_table.unwrap_or_else(|| unreachable!());
    if next {
        parent_table.select_next(None, None)?;
    } else {
        parent_table.select_previous(None, None)?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Backward,
    Forward,
    Up,
    Down,
}

/// `getCordsFromCellNode`: the cell's index in its row and its row's index.
fn cords(cell: &NodeRef) -> Option<(usize, usize)> {
    let row = cell.get_parent()?;
    Some((
        cell.get_index_within_parent()?,
        row.get_index_within_parent()?,
    ))
}

/// `getCellNodeFromCordsOrThrow`.
fn cell_from_cords(table: &NodeRef, x: usize, y: usize) -> Result<NodeRef, LexicalError> {
    table
        .get_children()
        .get(y)
        .and_then(|row| row.get_children().get(x).cloned())
        .filter(is_table_cell_node)
        .ok_or_else(|| LexicalError::new(format!("Cell not found at cords {x},{y}")))
}

/// `selectTableNodeInDirection`.
fn select_table_node_in_direction(
    table: &NodeRef,
    x: usize,
    y: usize,
    direction: Direction,
) -> Result<bool, LexicalError> {
    // `getTable`'s grid: as many rows as the table has, as many columns as
    // its last row.
    let rows = table.get_children_size();
    let columns = table.get_last_child().map_or(0, |r| r.get_children_size());
    let select = |cell: NodeRef, from_start: bool| -> Result<(), LexicalError> {
        if from_start {
            cell.select_start()?;
        } else {
            cell.select_end()?;
        }
        Ok(())
    };
    match direction {
        Direction::Backward | Direction::Forward => {
            let forward = direction == Direction::Forward;
            let edge_x = if forward {
                columns.saturating_sub(1)
            } else {
                0
            };
            if x != edge_x {
                let nx = if forward { x + 1 } else { x - 1 };
                select(cell_from_cords(table, nx, y)?, forward)?;
            } else {
                let edge_y = if forward { rows.saturating_sub(1) } else { 0 };
                if y != edge_y {
                    let ny = if forward { y + 1 } else { y - 1 };
                    let nx = if forward {
                        0
                    } else {
                        columns.saturating_sub(1)
                    };
                    select(cell_from_cords(table, nx, ny)?, forward)?;
                } else if forward {
                    table.select_next(None, None)?;
                } else {
                    table.select_previous(None, None)?;
                }
            }
        }
        Direction::Up => {
            if y != 0 {
                select(cell_from_cords(table, x, y - 1)?, false)?;
            } else {
                table.select_previous(None, None)?;
            }
        }
        Direction::Down => {
            if y + 1 != rows {
                select(cell_from_cords(table, x, y + 1)?, true)?;
            } else {
                table.select_next(None, None)?;
            }
        }
    }
    Ok(true)
}

type Corner = (bool, bool); // (is max column, is max row)

fn corner_value(rect: &TableCellRectBoundary, (max_col, max_row): Corner) -> (usize, usize) {
    (
        if max_col {
            rect.max_column
        } else {
            rect.min_column
        },
        if max_row { rect.max_row } else { rect.min_row },
    )
}

/// `getCorner`.
fn get_corner(rect: &TableCellRectBoundary, cell: &TableMapValue) -> Option<Corner> {
    let col_span = cell.cell.get_col_span().max(1.0) as usize;
    let row_span = cell.cell.get_row_span().max(1.0) as usize;
    let max_col = if cell.start_column == rect.min_column {
        false
    } else if cell.start_column + col_span - 1 == rect.max_column {
        true
    } else {
        return None;
    };
    let max_row = if cell.start_row == rect.min_row {
        false
    } else if cell.start_row + row_span - 1 == rect.max_row {
        true
    } else {
        return None;
    };
    Some((max_col, max_row))
}

/// `getAnchorCorner`.
fn get_anchor_corner(
    rect: &TableCellRectBoundary,
    anchor: &TableMapValue,
    focus: &TableMapValue,
) -> Corner {
    if let Some(corner) = get_corner(rect, anchor) {
        return corner;
    }
    if let Some((c, r)) = get_corner(rect, focus) {
        return (!c, !r);
    }
    (false, false)
}

/// `cellAtCornerOrThrow`.
fn cell_at_corner(
    map: &TableMap,
    rect: &TableCellRectBoundary,
    corner: Corner,
) -> Result<TableMapValue, LexicalError> {
    let (col, row) = corner_value(rect, corner);
    map.get(row)
        .and_then(|r| r.get(col).cloned().flatten())
        .ok_or_else(|| {
            LexicalError::new(format!(
                "cellAtCornerOrThrow: {row},{col} missing in tableMap"
            ))
        })
}

/// `$adjustFocusInDirection`: Shift+arrow grows or shrinks a table
/// selection by a cell.
fn adjust_focus_in_direction(
    table: &NodeRef,
    map: &TableMap,
    anchor: &TableMapValue,
    focus: &TableMapValue,
    direction: Direction,
) -> Result<bool, LexicalError> {
    let rect = compute_table_cell_rect_boundary(map, anchor, focus);
    let spans = compute_table_cell_rect_spans(map, &rect)?;
    let anchor_corner = get_anchor_corner(&rect, anchor, focus);
    let (focus_max_col, focus_max_row) = (!anchor_corner.0, !anchor_corner.1);
    let (col, row) = corner_value(&rect, (focus_max_col, focus_max_row));
    let (mut f_col, mut f_row) = (col as isize, row as isize);
    match direction {
        Direction::Forward => {
            f_col += if focus_max_col {
                1
            } else {
                spans.left_span as isize
            }
        }
        Direction::Backward => {
            f_col -= if focus_max_col {
                spans.right_span as isize
            } else {
                1
            }
        }
        Direction::Down => {
            f_row += if focus_max_row {
                1
            } else {
                spans.top_span as isize
            }
        }
        Direction::Up => {
            f_row -= if focus_max_row {
                spans.bottom_span as isize
            } else {
                1
            }
        }
    }
    let Some(new_focus) = usize::try_from(f_row)
        .ok()
        .and_then(|r| map.get(r))
        .and_then(|r| {
            usize::try_from(f_col)
                .ok()
                .and_then(|c| r.get(c).cloned().flatten())
        })
    else {
        return Ok(false);
    };
    // `$extractRectCorners`: the rect may grow to keep merged cells whole.
    let rect = compute_table_cell_rect_boundary(map, anchor, &new_focus);
    let corner = get_anchor_corner(&rect, anchor, &new_focus);
    let final_anchor = cell_at_corner(map, &rect, corner)?;
    let final_focus = cell_at_corner(map, &rect, (!corner.0, !corner.1))?;
    set_cells_for_selection(table, &final_anchor.cell, &final_focus.cell)?;
    Ok(true)
}

/// `$findNextTableCell`.
fn find_next_table_cell(
    initial: &NodeRef,
    direction: CaretDirection,
) -> Result<Option<NodeRef>, LexicalError> {
    let caret = get_sibling_caret(initial, direction);
    for next in extend_caret_to_range(&caret)?.iter_node_carets(RootMode::Root) {
        let origin = next.origin().clone();
        if is_table_cell_node(&origin) {
            if next.is_child_caret() {
                return Ok(Some(origin));
            }
        } else if !is_table_row_node(&origin) {
            break;
        }
    }
    Ok(None)
}

/// `$handleHorizontalArrowKeyRangeSelection`: left and right at the edge of
/// a cell move to the next cell, or with Shift select the cells.
fn handle_horizontal_arrow_key_range_selection(
    event: &KeyboardEvent,
    selection: &SelectionRef,
    extend: bool,
    is_backward: bool,
    table_node: &NodeRef,
) -> Result<bool, LexicalError> {
    let Some(range) = selection.range() else {
        return Ok(false);
    };
    let direction = if is_backward {
        CaretDirection::Previous
    } else {
        CaretDirection::Next
    };
    let initial_focus = caret_from_point(&range.focus, direction)?;
    if is_extendable_text_point_caret(&initial_focus) {
        return Ok(false);
    }
    let mut last_caret = initial_focus.clone();
    for next in extend_caret_to_range(&initial_focus)?.iter_node_carets(RootMode::ShadowRoot) {
        if !(next.is_sibling_caret() && next.origin().is_element()) {
            return Ok(false);
        }
        last_caret = next;
    }
    let Some(anchor_cell) = last_caret.get_parent_at_caret().filter(is_table_cell_node) else {
        return Ok(false);
    };
    let focus_cell = find_next_table_cell(&anchor_cell, last_caret.direction())?;
    let Some(anchor_cell_table) = find_matching_parent(&anchor_cell, is_table_node) else {
        return Ok(false);
    };
    if anchor_cell_table != *table_node {
        return Ok(false);
    }
    match focus_cell {
        None if extend => set_cells_for_selection(table_node, &anchor_cell, &anchor_cell)?,
        None => {
            // `$getTableExitCaret`.
            let sibling = get_sibling_caret(&anchor_cell_table, initial_focus.direction());
            let outer = match crate::caret::get_adjacent_child_caret(Some(&sibling)) {
                Some(adjacent) if adjacent.is_child_caret() => normalize_caret(&adjacent),
                _ => sibling,
            };
            set_point_from_caret(selection, Which::Anchor, &outer)?;
            set_point_from_caret(selection, Which::Focus, &outer)?;
        }
        Some(focus_cell) if extend => {
            set_cells_for_selection(table_node, &anchor_cell, &focus_cell)?
        }
        Some(focus_cell) => {
            let Some(inner) = get_child_caret(&focus_cell, last_caret.direction()) else {
                return Ok(false);
            };
            let inner = normalize_caret(&inner);
            set_point_from_caret(selection, Which::Anchor, &inner)?;
            set_point_from_caret(selection, Which::Focus, &inner)?;
        }
    }
    stop_event(event);
    Ok(true)
}

/// The first and last cells of `table` (`$getNodeTriplet` of its first and
/// last descendants).
fn first_and_last_cells(table: &NodeRef) -> Result<Option<(NodeRef, NodeRef)>, LexicalError> {
    let (Some(first), Some(last)) = (table.get_first_descendant(), table.get_last_descendant())
    else {
        return Ok(None);
    };
    Ok(Some((
        get_node_triplet(&first)?.0,
        get_node_triplet(&last)?.0,
    )))
}

/// The arrows outside a table: Left after a table enters it, and Shift+Up
/// or Down extends into or out of an adjacent table.
fn handle_arrow_key_outside_table(
    event: &KeyboardEvent,
    direction: Direction,
    selection: &SelectionRef,
) -> Result<bool, LexicalError> {
    let Some(range) = selection.range() else {
        return Ok(false);
    };
    let focus_node = range.focus.get_node()?;
    if direction == Direction::Backward {
        if range.focus.offset > 0 {
            return Ok(false);
        }
        let Some(parent) = get_block_parent_if_first_node(&focus_node) else {
            return Ok(false);
        };
        let Some(table) = parent.get_previous_sibling().filter(is_table_node) else {
            return Ok(false);
        };
        stop_event(event);
        if event.shift_key {
            let index = table.get_index_within_parent().unwrap_or(0);
            selection.point_set(
                Which::Focus,
                table.get_parent_or_throw()?.get_key(),
                index,
                PointType::Element,
            )?;
        } else {
            table.select_end()?;
        }
        return Ok(true);
    }
    if !event.shift_key || !(direction == Direction::Up || direction == Direction::Down) {
        return Ok(false);
    }
    let up = direction == Direction::Up;
    let is_backward = selection.is_backward()?;
    let is_table_unselect = !selection.range().is_some_and(|r| r.is_collapsed())
        && ((up && !is_backward) || (!up && is_backward));
    if is_table_unselect {
        let Some(table) = find_table_node(&focus_node) else {
            return Ok(false);
        };
        let Some(sibling) = (if up {
            table.get_previous_sibling()
        } else {
            table.get_next_sibling()
        }) else {
            return Ok(false);
        };
        let mut new_focus = sibling.clone();
        let mut offset = 0;
        if up && sibling.is_element() {
            new_focus = sibling.get_last_child().unwrap_or(sibling);
            offset = if new_focus.is_text() {
                new_focus.get_text_content_size()
            } else {
                0
            };
        }
        let new_selection = selection.clone_selection();
        let point_type = if new_focus.is_text() {
            PointType::Text
        } else {
            PointType::Element
        };
        new_selection.point_set(Which::Focus, new_focus.get_key(), offset, point_type)?;
        set_selection(Some(new_selection));
        stop_event(event);
        return Ok(true);
    }
    if is_root_or_shadow_root(&focus_node) {
        let nodes = selection.get_nodes()?;
        let selected = if up { nodes.last() } else { nodes.first() };
        let Some(selected) = selected else {
            return Ok(false);
        };
        let Some(table) = find_table_node(selected) else {
            return Ok(false);
        };
        if find_parent_table_cell_node_in_table(&table, selected).is_none() {
            return Ok(false);
        }
        let Some((first, last)) = first_and_last_cells(&table)? else {
            return Ok(false);
        };
        set_cells_for_selection(&table, &first, &last)?;
        return Ok(true);
    }
    let mut focus_parent = find_matching_parent(&focus_node, |n| n.is_element() && !n.is_inline());
    if focus_parent.as_ref().is_some_and(is_table_cell_node) {
        focus_parent = focus_parent.and_then(|p| find_matching_parent(&p, is_table_node));
    }
    let Some(focus_parent) = focus_parent else {
        return Ok(false);
    };
    let sibling = if up {
        focus_parent.get_previous_sibling()
    } else {
        focus_parent.get_next_sibling()
    };
    let Some(table) = sibling.filter(is_table_node) else {
        return Ok(false);
    };
    let Some((first, last)) = first_and_last_cells(&table)? else {
        return Ok(false);
    };
    let new_selection = selection.clone_selection();
    let (cell, offset) = if up {
        (&first, 0)
    } else {
        (&last, last.get_children_size())
    };
    new_selection.point_set(Which::Focus, cell.get_key(), offset, PointType::Element)?;
    stop_event(event);
    set_selection(Some(new_selection));
    Ok(true)
}

/// `$handleArrowKey`.
fn handle_arrow_key(
    editor: &Editor,
    event: &KeyboardEvent,
    direction: Direction,
) -> Result<bool, LexicalError> {
    let selection = get_selection();
    let Some(table_node) = selection_table(selection.as_ref()) else {
        return match &selection {
            Some(s) if s.is_range() => handle_arrow_key_outside_table(event, direction, s),
            _ => Ok(false),
        };
    };
    let Some(selection) = selection else {
        return Ok(false);
    };
    if selection.is_range() {
        if matches!(direction, Direction::Backward | Direction::Forward) {
            return handle_horizontal_arrow_key_range_selection(
                event,
                &selection,
                event.shift_key,
                direction == Direction::Backward,
                &table_node,
            );
        }
        let Some(range) = selection.range().filter(|r| r.is_collapsed()) else {
            return Ok(false);
        };
        let anchor_cell = find_cell_node(&range.anchor.get_node()?);
        let focus_cell = find_cell_node(&range.focus.get_node()?);
        let Some(anchor_cell) = anchor_cell.filter(|a| Some(a) == focus_cell.as_ref()) else {
            return Ok(false);
        };
        // Whether the caret leaves the cell: where moving it by a line lands.
        let Some(movement) = editor.selection_movement() else {
            return Ok(false);
        };
        let landed = movement.landed_point(
            &range.focus,
            direction == Direction::Up,
            SelectionGranularity::Line,
        )?;
        let is_exiting = landed
            .and_then(|p| p.get_node().ok())
            .and_then(|n| find_cell_node(&n))
            .is_none_or(|cell| cell != anchor_cell);
        if !is_exiting {
            return Ok(false);
        }
        stop_event(event);
        let Some((x, y)) = cords(&anchor_cell) else {
            return Ok(false);
        };
        if event.shift_key {
            set_cells_for_selection(&table_node, &anchor_cell, &anchor_cell)?;
            return Ok(true);
        }
        return select_table_node_in_direction(&table_node, x, y, direction);
    }
    let Some(table_selection) = selection.table() else {
        return Ok(false);
    };
    if table_selection.table_key != table_node.get_key() {
        return Ok(false);
    }
    let anchor_cell = find_cell_node(&table_selection.anchor.get_node()?);
    let focus_cell = find_cell_node(&table_selection.focus.get_node()?);
    let (Some(anchor_cell), Some(focus_cell)) = (anchor_cell, focus_cell) else {
        return Ok(false);
    };
    stop_event(event);
    if event.shift_key {
        let (map, anchor, focus) = compute_table_map(&table_node, &anchor_cell, &focus_cell)?;
        return adjust_focus_in_direction(&table_node, &map, &anchor, &focus, direction);
    }
    focus_cell.select_end()?;
    Ok(true)
}

/// The Backspace and Delete keys (`$deleteCellHandler`): a selection with
/// one point in a table is moved to the table's edge; a table selection's
/// cells are emptied.
fn delete_cell_handler(
    editor: &Editor,
    event: Option<&KeyboardEvent>,
) -> Result<bool, LexicalError> {
    let Some(selection) = get_selection().filter(|s| s.is_table() || s.is_range()) else {
        return Ok(false);
    };
    let Some((anchor_node, focus_node)) = point_nodes(&selection) else {
        return Ok(false);
    };
    // JS runs this for each table; one with exactly one point inside it
    // is the table being deleted across.
    let mut tables = HashSet::new();
    for node in [&anchor_node, &focus_node] {
        let mut table = find_table_node(node);
        while let Some(t) = table {
            table = t.get_parent().and_then(|p| find_table_node(&p));
            tables.insert(t);
        }
    }
    for table in &tables {
        let anchor_inside = table.is_parent_of(&anchor_node);
        let focus_inside = table.is_parent_of(&focus_node);
        if anchor_inside != focus_inside {
            let (table_which, outer_which) = if anchor_inside {
                (Which::Anchor, Which::Focus)
            } else {
                (Which::Focus, Which::Anchor)
            };
            let (Some(table_point), Some(outer)) =
                (selection.point(table_which), selection.point(outer_which))
            else {
                return Ok(false);
            };
            let new_selection = if table_point.is_before(&outer)? {
                table.select_previous(None, None)?
            } else {
                table.select_next(None, None)?
            };
            new_selection.point_set(outer_which, &outer.key, outer.offset, outer.point_type)?;
            return Ok(false);
        }
    }
    let Some(table_node) = selection_table(Some(&selection)) else {
        return Ok(false);
    };
    if selection.is_table() {
        if let Some(event) = event {
            event.event.prevent_default();
            event.event.stop_propagation();
        }
        clear_text(editor, &table_node)?;
        return Ok(true);
    }
    Ok(false)
}

/// `$deleteTextHandler`: deleting a table selection empties its cells.
fn delete_text_handler(editor: &Editor) -> Result<bool, LexicalError> {
    let selection = get_selection();
    let Some(table_node) = selection_table(selection.as_ref()) else {
        return Ok(false);
    };
    if selection.is_some_and(|s| s.is_table()) {
        clear_text(editor, &table_node)?;
        return Ok(true);
    }
    Ok(false)
}

/// FORMAT_ELEMENT_COMMAND for a table selection: the whole table's format
/// when it covers every cell, otherwise each cell's and its blocks'.
fn format_element_handler(format_type: &str) -> Result<bool, LexicalError> {
    let selection = get_selection();
    let Some(selection) = selection.filter(|s| s.is_table()) else {
        return Ok(false);
    };
    let Some(table_node) = selection_table(Some(&selection)) else {
        return Ok(false);
    };
    let Some((anchor_node, focus_node)) = point_nodes(&selection) else {
        return Ok(false);
    };
    if !is_table_cell_node(&anchor_node) || !is_table_cell_node(&focus_node) {
        return Ok(false);
    }
    let (map, anchor, focus) = compute_table_map(&table_node, &anchor_node, &focus_node)?;
    let rect = compute_table_cell_rect_boundary(&map, &anchor, &focus);
    let columns = map.first().map_or(0, Vec::len);
    if rect.min_row == 0
        && rect.min_column == 0
        && rect.max_row + 1 == map.len()
        && rect.max_column + 1 == columns
    {
        table_node.set_element_format(format_type);
        return Ok(true);
    }
    let mut visited = HashSet::new();
    for row in &map[rect.min_row..=rect.max_row] {
        for value in row[rect.min_column..=rect.max_column].iter().flatten() {
            let cell = &value.cell;
            if !visited.insert(cell.clone()) {
                continue;
            }
            cell.set_element_format(format_type);
            for child in cell.get_children() {
                if child.is_element() && !child.is_inline() {
                    child.set_element_format(format_type);
                }
            }
        }
    }
    Ok(true)
}

/// `$fixRangeSelectionForSelectedTable`, without its touch cases: a range
/// selection with one point in a table covers the whole table, and one
/// across cells of a table becomes a table selection.
fn fix_range_selection_for_selected_table(selection: &SelectionRef) -> Result<(), LexicalError> {
    let Some((anchor_node, focus_node)) = point_nodes(selection) else {
        return Ok(());
    };
    let anchor_cell = find_cell_node(&anchor_node);
    let focus_cell = find_cell_node(&focus_node);
    let anchor_table = anchor_cell.as_ref().and_then(find_table_node);
    let focus_table = focus_cell.as_ref().and_then(find_table_node);
    let is_backward = selection.is_backward()?;
    let should_move_focus = focus_table
        .as_ref()
        .is_some_and(|f| anchor_table.as_ref().is_none_or(|a| a.is_parent_of(f)));
    let should_move_anchor = anchor_table
        .as_ref()
        .is_some_and(|a| focus_table.as_ref().is_none_or(|f| f.is_parent_of(a)));
    let ends =
        |table: &NodeRef, cell: &NodeRef| -> Result<Option<(NodeRef, NodeRef)>, LexicalError> {
            let (map, _, _) = compute_table_map(table, cell, cell)?;
            let first = map.first().and_then(|r| r.first().cloned().flatten());
            let last = map.last().and_then(|r| r.last().cloned().flatten());
            Ok(first.zip(last).map(|(f, l)| (f.cell, l.cell)))
        };
    if should_move_focus {
        let (Some(table), Some(cell)) = (&focus_table, &focus_cell) else {
            return Ok(());
        };
        let Some((first, last)) = ends(table, cell)? else {
            return Ok(());
        };
        let new_selection = selection.clone_selection();
        let (cell, offset) = if is_backward {
            (&first, 0)
        } else {
            (&last, last.get_children_size())
        };
        new_selection.point_set(Which::Focus, cell.get_key(), offset, PointType::Element)?;
        set_selection(Some(new_selection));
    } else if should_move_anchor {
        let (Some(table), Some(cell)) = (&anchor_table, &anchor_cell) else {
            return Ok(());
        };
        let Some((first, last)) = ends(table, cell)? else {
            return Ok(());
        };
        let new_selection = selection.clone_selection();
        let (cell, offset) = if is_backward {
            (&last, last.get_children_size())
        } else {
            (&first, 0)
        };
        new_selection.point_set(Which::Anchor, cell.get_key(), offset, PointType::Element)?;
        set_selection(Some(new_selection));
    } else if let (Some(anchor_cell), Some(focus_cell), Some(table)) =
        (&anchor_cell, &focus_cell, &anchor_table)
        && anchor_table == focus_table
        && anchor_cell != focus_cell
    {
        set_selection(Some(create_table_selection_from(
            table,
            anchor_cell,
            focus_cell,
        )?));
    }
    Ok(())
}

/// `registerTableSelectionObserver(editor, hasTabHandler)`: the model parts
/// of `applyTableHandlers` (the arrows, Escape, Backspace and Delete, the
/// delete commands, FORMAT_TEXT_COMMAND and FORMAT_ELEMENT_COMMAND, and Tab
/// with `has_tab_handler`) and of `$handleTableSelectionChangeCommand`.
pub fn register_table_selection_observer(editor: &Editor, has_tab_handler: bool) -> Unregister {
    let mut unregisters = Vec::new();
    for (command, direction) in [
        (&KEY_ARROW_DOWN_COMMAND, Direction::Down),
        (&KEY_ARROW_UP_COMMAND, Direction::Up),
        (&KEY_ARROW_LEFT_COMMAND, Direction::Backward),
        (&KEY_ARROW_RIGHT_COMMAND, Direction::Forward),
    ] {
        unregisters.push(editor.register_command(
            command,
            move |event, editor| handle_arrow_key(editor, event, direction),
            COMMAND_PRIORITY_HIGH,
        ));
    }
    unregisters.push(editor.register_command(
        &KEY_ESCAPE_COMMAND,
        |event, _| {
            let selection = get_selection();
            let Some(selection) = selection.filter(|s| s.is_table()) else {
                return Ok(false);
            };
            let Some(table) = selection_table(Some(&selection)) else {
                return Ok(false);
            };
            let Some((_, focus)) = point_nodes(&selection) else {
                return Ok(false);
            };
            if let Some(cell) = find_parent_table_cell_node_in_table(&table, &focus) {
                stop_event(event);
                cell.select_end()?;
                return Ok(true);
            }
            Ok(false)
        },
        COMMAND_PRIORITY_HIGH,
    ));
    unregisters.push(editor.register_command(
        &DELETE_CHARACTER_COMMAND,
        |_, editor| delete_text_handler(editor),
        COMMAND_PRIORITY_HIGH,
    ));
    unregisters.push(editor.register_command(
        &DELETE_WORD_COMMAND,
        |_, editor| delete_text_handler(editor),
        COMMAND_PRIORITY_HIGH,
    ));
    unregisters.push(editor.register_command(
        &DELETE_LINE_COMMAND,
        |_, editor| delete_text_handler(editor),
        COMMAND_PRIORITY_HIGH,
    ));
    for command in [&KEY_BACKSPACE_COMMAND, &KEY_DELETE_COMMAND] {
        unregisters.push(editor.register_command(
            command,
            |event, editor| delete_cell_handler(editor, Some(event)),
            COMMAND_PRIORITY_HIGH,
        ));
    }
    #[cfg(feature = "clipboard")]
    unregisters.push(editor.register_command(
        &crate::commands::CUT_COMMAND,
        |event, editor| {
            // JS registers this for each rendered table, so it runs only
            // while the document has one.
            if crate::lexical_utils::nodes_of_type("table").is_empty() {
                return Ok(false);
            }
            let Some(selection) = get_selection() else {
                return Ok(false);
            };
            if !(selection.is_table() || selection.is_range()) {
                return Ok(false);
            }
            // Copying to the clipboard is async in JS, so the data is
            // captured before it is deleted.
            if let Some(event) =
                crate::events::ClipboardOrKeyboardEvent::clipboard_event(event.as_ref())
            {
                let data = crate::clipboard::get_clipboard_data_from_selection(Some(&selection))?;
                event.prevent_default();
                if let Some(clipboard_data) = &event.clipboard_data {
                    crate::clipboard::set_lexical_clipboard_data_transfer(clipboard_data, &data);
                }
            }
            let intercepted = delete_cell_handler(editor, None)?;
            if selection.is_range() {
                selection.remove_text()?;
                return Ok(true);
            }
            Ok(intercepted)
        },
        COMMAND_PRIORITY_HIGH,
    ));
    unregisters.push(editor.register_command(
        &FORMAT_TEXT_COMMAND,
        |payload, editor| {
            let selection = get_selection();
            if selection_table(selection.as_ref()).is_none()
                || !selection.is_some_and(|s| s.is_table())
            {
                return Ok(false);
            }
            format_cells(editor, payload)?;
            Ok(true)
        },
        COMMAND_PRIORITY_HIGH,
    ));
    unregisters.push(editor.register_command(
        &FORMAT_ELEMENT_COMMAND,
        |format_type, _| format_element_handler(format_type),
        COMMAND_PRIORITY_HIGH,
    ));
    if has_tab_handler {
        unregisters.push(editor.register_command(
            &KEY_TAB_COMMAND,
            |event, _| {
                let selection = get_selection();
                let Some(selection) =
                    selection.filter(|s| s.range().is_some_and(|r| r.is_collapsed()))
                else {
                    return Ok(false);
                };
                let Some(table) = selection_table(Some(&selection)) else {
                    return Ok(false);
                };
                let Some((anchor, _)) = point_nodes(&selection) else {
                    return Ok(false);
                };
                let Some(cell) =
                    find_cell_node(&anchor).filter(|c| find_table_node(c).as_ref() == Some(&table))
                else {
                    return Ok(false);
                };
                stop_event(event);
                select_adjacent_cell(&cell, !event.shift_key)?;
                Ok(true)
            },
            COMMAND_PRIORITY_HIGH,
        ));
    }
    unregisters.push(editor.register_command(
        &SELECTION_CHANGE_COMMAND,
        |_, editor| {
            // A JS editor without a root element renders no table to
            // observe.
            if editor.get_root_element().is_none() {
                return Ok(false);
            }
            if let Some(selection) = get_selection().filter(|s| s.is_range()) {
                fix_range_selection_for_selected_table(&selection)?;
            }
            Ok(false)
        },
        COMMAND_PRIORITY_HIGH,
    ));
    merge_register(unregisters)
}
