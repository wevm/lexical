//! `LexicalTableUtils.ts`: the table map (`$computeTableMap`), the cell
//! rectangle a selection covers, and the commands that insert, delete,
//! move, merge and unmerge rows, columns and cells.
//!
//! Not ported: `$insertTableRow`, `$insertTableColumn` and
//! `$getTableCellSiblingsFromTableCellNode`, deprecated upstream, which take
//! a rendered table's DOM grid (`TableDOMTable`); their replacements
//! (`$insertTableRowAtNode`, `$insertTableColumnAtNode` and the `AtSelection`
//! forms) are ported.

use super::table_cell_node::{
    TableCellHeaderStates, create_table_cell_node, is_table_cell_node, with_cell,
};
use super::table_commands::InsertTableCommandPayloadHeaders;
use super::table_node::{create_table_node, is_table_node};
use super::table_row_node::{create_table_row_node, is_table_row_node};
use crate::LexicalError;
use crate::error::invariant;
use crate::lexical_selection::{Point, SelectionRef, get_selection};
use crate::lexical_utils::{copy_node, get_writable};
use crate::node::NodeRef;
use crate::nodes::paragraph::{ParagraphNode, create_paragraph_node};
use crate::nodes::text::create_text_node;
use crate::selection_insert_nodes::find_matching_parent;
use std::collections::HashSet;

/// `TableMapValueType`: a cell and the grid position it starts at.
#[derive(Clone, Debug, PartialEq)]
pub struct TableMapValue {
    pub cell: NodeRef,
    pub start_row: usize,
    pub start_column: usize,
}

/// `TableMapType`: for each grid row, the cell that covers each column. A
/// position no cell covers (a ragged table) is `None`, a hole in JS's array.
pub type TableMap = Vec<Vec<Option<TableMapValue>>>;

/// `TableCellRectBoundary`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableCellRectBoundary {
    pub min_column: usize,
    pub min_row: usize,
    pub max_column: usize,
    pub max_row: usize,
}

/// `TableCellRectSpans`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableCellRectSpans {
    pub top_span: i64,
    pub left_span: i64,
    pub right_span: i64,
    pub bottom_span: i64,
}

/// A cell's `__colSpan`, as the integer the map arithmetic uses.
pub(crate) fn col_span(cell: &NodeRef) -> i64 {
    cell.get_col_span() as i64
}

/// A cell's `__rowSpan`.
pub(crate) fn row_span(cell: &NodeRef) -> i64 {
    cell.get_row_span() as i64
}

/// `map[row][column]`, where JS reads a position it expects to be filled
/// (reading a hole throws a `TypeError` there).
pub(crate) fn map_at(
    map: &TableMap,
    row: i64,
    column: i64,
) -> Result<&TableMapValue, LexicalError> {
    usize::try_from(row)
        .ok()
        .and_then(|r| map.get(r))
        .and_then(|r| usize::try_from(column).ok().and_then(|c| r.get(c)))
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            LexicalError::new(format!(
                "table map has no cell at row {row}, column {column}"
            ))
        })
}

/// `map[row][column]`, or `None` where JS's read is `undefined`.
pub(crate) fn map_get(map: &TableMap, row: i64, column: i64) -> Option<&TableMapValue> {
    let r = usize::try_from(row).ok()?;
    let c = usize::try_from(column).ok()?;
    map.get(r)?.get(c)?.as_ref()
}

/// `$createTableNodeWithDimensions(rowCount, columnCount, includeHeaders)`:
/// a table of empty cells, each with a paragraph holding an empty text
/// node, with header rows and columns as `include_headers` says.
pub fn create_table_node_with_dimensions(
    row_count: usize,
    column_count: usize,
    include_headers: InsertTableCommandPayloadHeaders,
) -> Result<NodeRef, LexicalError> {
    let table_node = create_table_node();
    for i_row in 0..row_count {
        let table_row_node = create_table_row_node(None);
        for i_column in 0..column_count {
            let mut header_state = TableCellHeaderStates::NO_STATUS;
            let (rows, columns) = match include_headers {
                InsertTableCommandPayloadHeaders::Both(both) => (both, both),
                InsertTableCommandPayloadHeaders::Each { rows, columns } => (rows, columns),
            };
            if i_row == 0 && rows {
                header_state |= TableCellHeaderStates::ROW;
            }
            if i_column == 0 && columns {
                header_state |= TableCellHeaderStates::COLUMN;
            }
            let table_cell_node = create_table_cell_node(header_state, 1.0, None);
            let paragraph_node = create_paragraph_node();
            paragraph_node.append(&[create_text_node("")])?;
            table_cell_node.append(&[paragraph_node])?;
            table_row_node.append(&[table_cell_node])?;
        }
        table_node.append(&[table_row_node])?;
    }
    Ok(table_node)
}

/// `$getTableCellNodeFromLexicalNode(startingNode)`.
pub fn get_table_cell_node_from_lexical_node(starting_node: &NodeRef) -> Option<NodeRef> {
    find_matching_parent(starting_node, is_table_cell_node)
}

/// `$getTableRowNodeFromTableCellNodeOrThrow(startingNode)`.
pub fn get_table_row_node_from_table_cell_node_or_throw(
    starting_node: &NodeRef,
) -> Result<NodeRef, LexicalError> {
    find_matching_parent(starting_node, is_table_row_node)
        .ok_or_else(|| LexicalError::new("Expected table cell to be inside of table row."))
}

/// `$getTableNodeFromLexicalNodeOrThrow(startingNode)`.
pub fn get_table_node_from_lexical_node_or_throw(
    starting_node: &NodeRef,
) -> Result<NodeRef, LexicalError> {
    find_matching_parent(starting_node, is_table_node)
        .ok_or_else(|| LexicalError::new("Expected table cell to be inside of table."))
}

/// `$getTableRowIndexFromTableCellNode(tableCellNode)`: -1 when not found.
pub fn get_table_row_index_from_table_cell_node(
    table_cell_node: &NodeRef,
) -> Result<i64, LexicalError> {
    let table_row_node = get_table_row_node_from_table_cell_node_or_throw(table_cell_node)?;
    let table_node = get_table_node_from_lexical_node_or_throw(&table_row_node)?;
    Ok(table_node
        .get_children()
        .iter()
        .position(|n| *n == table_row_node)
        .map_or(-1, |i| i as i64))
}

/// `$getTableColumnIndexFromTableCellNode(tableCellNode)`: -1 when not
/// found.
pub fn get_table_column_index_from_table_cell_node(
    table_cell_node: &NodeRef,
) -> Result<i64, LexicalError> {
    let table_row_node = get_table_row_node_from_table_cell_node_or_throw(table_cell_node)?;
    Ok(table_row_node
        .get_children()
        .iter()
        .position(|n| n == table_cell_node)
        .map_or(-1, |i| i as i64))
}

/// `$removeTableRowAtIndex(tableNode, indexToDelete)`.
pub fn remove_table_row_at_index(
    table_node: &NodeRef,
    index_to_delete: usize,
) -> Result<NodeRef, LexicalError> {
    let table_rows = table_node.get_children();
    let Some(target_row_node) = table_rows.get(index_to_delete) else {
        return Err(LexicalError::new(
            "Expected table cell to be inside of table row.",
        ));
    };
    target_row_node.remove(false)?;
    Ok(table_node.clone())
}

/// `getHeaderState(currentState, possibleState)`.
fn get_header_state(current_state: i32, possible_state: i32) -> i32 {
    if current_state == TableCellHeaderStates::BOTH || current_state == possible_state {
        possible_state
    } else {
        TableCellHeaderStates::NO_STATUS
    }
}

/// The selection the `AtSelection` commands act on: a range or table
/// selection (`Expected a RangeSelection or TableSelection` otherwise).
fn range_or_table_selection() -> Result<(SelectionRef, Point, Point), LexicalError> {
    let selection = get_selection();
    let points = selection
        .as_ref()
        .and_then(SelectionRef::get_start_end_points);
    match (selection, points) {
        (Some(selection), Some((anchor, focus))) => Ok((selection, anchor, focus)),
        _ => Err(LexicalError::new(
            "Expected a RangeSelection or TableSelection",
        )),
    }
}

/// `$insertTableRowAtSelection(insertAfter)`: inserts a row after the
/// lower or before the upper of the selection's cells, taking spans into
/// account, and returns it.
pub fn insert_table_row_at_selection(insert_after: bool) -> Result<Option<NodeRef>, LexicalError> {
    let (_, anchor, focus) = range_or_table_selection()?;
    let (anchor_cell, _, _) = get_node_triplet(&anchor.get_node()?)?;
    let (focus_cell, _, grid) = get_node_triplet(&focus.get_node()?)?;
    let (_, focus_cell_map, anchor_cell_map) = compute_table_map(&grid, &focus_cell, &anchor_cell)?;
    let anchor_start_row = anchor_cell_map.start_row as i64;
    let focus_start_row = focus_cell_map.start_row as i64;
    if insert_after {
        insert_table_row_at_node(
            if anchor_start_row + row_span(&anchor_cell) > focus_start_row + row_span(&focus_cell) {
                &anchor_cell
            } else {
                &focus_cell
            },
            true,
        )
    } else {
        insert_table_row_at_node(
            if focus_start_row < anchor_start_row {
                &focus_cell
            } else {
                &anchor_cell
            },
            false,
        )
    }
}

/// `$insertTableRowAtNode(cellNode, insertAfter)`: inserts a row after
/// the last or before the first row the cell spans; cells spanning across
/// the new row grow instead. Returns the new row.
pub fn insert_table_row_at_node(
    cell_node: &NodeRef,
    insert_after: bool,
) -> Result<Option<NodeRef>, LexicalError> {
    let (_, _, grid) = get_node_triplet(cell_node)?;
    // Commands below assume a rectangular grid; repair a ragged one first.
    pad_table_rows(&grid, |_| true)?;
    let (grid_map, cell_map, _) = compute_table_map(&grid, cell_node, cell_node)?;
    let column_count = grid_map.first().map_or(0, Vec::len) as i64;
    let cell_start_row = cell_map.start_row as i64;
    let target_row = if insert_after {
        cell_start_row + row_span(cell_node) - 1
    } else {
        cell_start_row
    };
    let new_row = create_table_row_node(None);
    for i in 0..column_count {
        let value = map_at(&grid_map, target_row, i)?;
        let start_row = value.start_row as i64;
        let cell = &value.cell;
        let owns = if insert_after {
            start_row + row_span(cell) - 1 <= target_row
        } else {
            start_row == target_row
        };
        if owns {
            let header_state =
                get_header_state(cell.get_header_styles(), TableCellHeaderStates::COLUMN);
            let new_cell = create_table_cell_node(header_state, 1.0, None);
            new_cell.append(&[create_paragraph_node()])?;
            new_row.append(&[new_cell])?;
        } else {
            cell.set_row_span(cell.get_row_span() + 1.0);
        }
    }
    let row_node = usize::try_from(target_row)
        .ok()
        .and_then(|r| grid.get_child_at_index(r))
        .filter(is_table_row_node);
    if insert_after {
        let row_node =
            row_node.ok_or_else(|| LexicalError::new("insertAfterEndRow is not a TableRowNode"))?;
        row_node.insert_after(&new_row, true)?;
    } else {
        let row_node = row_node
            .ok_or_else(|| LexicalError::new("insertBeforeStartRow is not a TableRowNode"))?;
        row_node.insert_before(&new_row, true)?;
    }
    Ok(Some(new_row))
}

/// `$insertTableColumnAtSelection(insertAfter)`: inserts a column after
/// the rightmost or before the leftmost of the selection's cells, and
/// returns its first cell.
pub fn insert_table_column_at_selection(
    insert_after: bool,
) -> Result<Option<NodeRef>, LexicalError> {
    let (_, anchor, focus) = range_or_table_selection()?;
    let (anchor_cell, _, _) = get_node_triplet(&anchor.get_node()?)?;
    let (focus_cell, _, grid) = get_node_triplet(&focus.get_node()?)?;
    let (_, focus_cell_map, anchor_cell_map) = compute_table_map(&grid, &focus_cell, &anchor_cell)?;
    let anchor_start_column = anchor_cell_map.start_column as i64;
    let focus_start_column = focus_cell_map.start_column as i64;
    if insert_after {
        insert_table_column_at_node(
            if anchor_start_column + col_span(&anchor_cell)
                > focus_start_column + col_span(&focus_cell)
            {
                &anchor_cell
            } else {
                &focus_cell
            },
            true,
            true,
        )
    } else {
        insert_table_column_at_node(
            if focus_start_column < anchor_start_column {
                &focus_cell
            } else {
                &anchor_cell
            },
            false,
            true,
        )
    }
}

/// `$insertFirst(parent, node)` of `LexicalTableUtils.ts`: before the
/// first child, or appended to an empty parent.
pub(crate) fn insert_first(parent: &NodeRef, node: &NodeRef) -> Result<(), LexicalError> {
    match parent.get_first_child() {
        Some(first_child) => first_child.insert_before(node, true).map(drop),
        None => parent.append(std::slice::from_ref(node)),
    }
}

/// `$insertTableColumnAtNode(cellNode, insertAfter, shouldSetSelection)`:
/// inserts a column after the last or before the first column the cell
/// spans; cells spanning across it grow instead. Returns the first new
/// cell, which the selection moves to when `should_set_selection`.
pub fn insert_table_column_at_node(
    cell_node: &NodeRef,
    insert_after: bool,
    should_set_selection: bool,
) -> Result<Option<NodeRef>, LexicalError> {
    let (_, _, grid) = get_node_triplet(cell_node)?;
    pad_table_rows(&grid, |_| true)?;
    let (grid_map, cell_map, _) = compute_table_map(&grid, cell_node, cell_node)?;
    let row_count = grid_map.len();
    let start_column = cell_map.start_column as i64;
    let insert_after_column = if insert_after {
        start_column + col_span(cell_node) - 1
    } else {
        start_column - 1
    };
    let grid_first_child = grid
        .get_first_child()
        .filter(is_table_row_node)
        .ok_or_else(|| LexicalError::new("Expected firstTable child to be a row"))?;
    let mut first_inserted_cell: Option<NodeRef> = None;
    let mut new_cell = |header_state: i32| -> Result<NodeRef, LexicalError> {
        let cell = create_table_cell_node(header_state, 1.0, None);
        cell.append(&[create_paragraph_node()])?;
        if first_inserted_cell.is_none() {
            first_inserted_cell = Some(cell.clone());
        }
        Ok(cell)
    };
    let mut loop_row = grid_first_child;
    for i in 0..row_count {
        if i != 0 {
            loop_row = loop_row
                .get_next_sibling()
                .filter(is_table_row_node)
                .ok_or_else(|| LexicalError::new("Expected row nextSibling to be a row"))?;
        }
        let current_cell_header_state = map_at(&grid_map, i as i64, insert_after_column.max(0))?
            .cell
            .get_header_styles();
        let header_state = get_header_state(current_cell_header_state, TableCellHeaderStates::ROW);
        if insert_after_column < 0 {
            insert_first(&loop_row, &new_cell(header_state)?)?;
            continue;
        }
        let value = map_at(&grid_map, i as i64, insert_after_column)?;
        let (current_cell, current_start_column) = (&value.cell, value.start_column as i64);
        if current_start_column + col_span(current_cell) - 1 <= insert_after_column {
            // The last cell this row owns at or before the insertion column.
            let mut insert_after_cell: Option<NodeRef> = None;
            let mut column = 0i64;
            while column <= insert_after_column {
                let current_cell_map = map_at(&grid_map, i as i64, column)?;
                if current_cell_map.start_row == i {
                    insert_after_cell = Some(current_cell_map.cell.clone());
                }
                if col_span(&current_cell_map.cell) > 1 {
                    column += col_span(&current_cell_map.cell) - 1;
                }
                column += 1;
            }
            match insert_after_cell {
                None => insert_first(&loop_row, &new_cell(header_state)?)?,
                Some(cell) => {
                    cell.insert_after(&new_cell(header_state)?, true)?;
                }
            }
        } else {
            current_cell.set_col_span(current_cell.get_col_span() + 1.0);
        }
    }
    if let Some(first) = &first_inserted_cell
        && should_set_selection
    {
        move_selection_to_cell(first)?;
    }
    if let Some(col_widths) = grid.get_col_widths() {
        let mut new_col_widths = col_widths;
        let column_index = insert_after_column.max(0) as usize;
        // `splice(columnIndex, 0, newColWidths[columnIndex])`: an index past
        // the end inserts `undefined` at the end, which the JSON writes as
        // `null` and a number field reads as 0.
        let new_width = new_col_widths
            .get(column_index)
            .copied()
            .unwrap_or(f64::NAN);
        let at = column_index.min(new_col_widths.len());
        new_col_widths.insert(at, new_width);
        grid.set_col_widths(Some(new_col_widths));
    }
    Ok(first_inserted_cell)
}

/// `$deleteTableColumn(tableNode, targetIndex)`, deprecated upstream: the
/// cell at `target_index` of every row, without regard to spans.
pub fn delete_table_column(
    table_node: &NodeRef,
    target_index: usize,
) -> Result<NodeRef, LexicalError> {
    for row in table_node.get_children() {
        if is_table_row_node(&row) {
            let children = row.get_children();
            let Some(cell) = children.get(target_index) else {
                return Err(LexicalError::new("Table column target index out of range"));
            };
            cell.remove(false)?;
        }
    }
    Ok(table_node.clone())
}

/// `$deleteTableRowAtSelection()`: deletes the rows the selection's cells
/// span, shortening cells that span out of them and moving down those that
/// start in them; the whole table when every row goes.
pub fn delete_table_row_at_selection() -> Result<(), LexicalError> {
    let (selection, anchor, focus) = range_or_table_selection()?;
    let (anchor, focus) = if selection.is_backward()? {
        (focus.get_node()?, anchor.get_node()?)
    } else {
        (anchor.get_node()?, focus.get_node()?)
    };
    let (anchor_cell, _, grid) = get_node_triplet(&anchor)?;
    let (focus_cell, _, _) = get_node_triplet(&focus)?;
    pad_table_rows(&grid, |_| true)?;
    let (grid_map, anchor_cell_map, focus_cell_map) =
        compute_table_map(&grid, &anchor_cell, &focus_cell)?;
    let anchor_start_row = anchor_cell_map.start_row as i64;
    let focus_start_row = focus_cell_map.start_row as i64;
    let focus_end_row = focus_start_row + row_span(&focus_cell) - 1;
    if grid_map.len() as i64 == focus_end_row - anchor_start_row + 1 {
        // Empty grid. Move the selection out of the table before removing it.
        grid.select_previous(None, None)?;
        grid.remove(false)?;
        return Ok(());
    }
    let column_count = grid_map.first().map_or(0, Vec::len) as i64;
    let next_row = usize::try_from(focus_end_row + 1)
        .ok()
        .and_then(|r| grid_map.get(r));
    let next_row_node = usize::try_from(focus_end_row + 1)
        .ok()
        .and_then(|r| grid.get_child_at_index(r));
    let mut row = focus_end_row;
    while row >= anchor_start_row {
        let mut column = column_count - 1;
        while column >= 0 {
            let value = map_at(&grid_map, row, column)?;
            let (cell, cell_start_row, cell_start_column) = (
                &value.cell,
                value.start_row as i64,
                value.start_column as i64,
            );
            if cell_start_column != column {
                // Don't repeat work for the same cell.
                column -= 1;
                continue;
            }
            // Rows overflowing top or bottom have to be trimmed.
            if cell_start_row < anchor_start_row
                || cell_start_row + row_span(cell) - 1 > focus_end_row
            {
                let intersection_start = cell_start_row.max(anchor_start_row);
                let intersection_end = (row_span(cell) + cell_start_row - 1).min(focus_end_row);
                let overflow_rows_count = if intersection_start <= intersection_end {
                    intersection_end - intersection_start + 1
                } else {
                    0
                };
                cell.set_row_span(cell.get_row_span() - overflow_rows_count as f64);
            }
            // Rows overflowing bottom have to be moved to the next row.
            if cell_start_row >= anchor_start_row
                && cell_start_row + row_span(cell) - 1 > focus_end_row
                && row == focus_end_row
            {
                let next_row_node = next_row_node
                    .as_ref()
                    .filter(|n| is_table_row_node(n))
                    .ok_or_else(|| LexicalError::new("Expected a TableRowNode"))?;
                let next_row = next_row.ok_or_else(|| LexicalError::new("Expected a next row"))?;
                let mut insert_after_cell: Option<NodeRef> = None;
                let mut column_index = 0i64;
                while column_index < column {
                    let current_cell_map = next_row
                        .get(column_index as usize)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| LexicalError::new("table map hole"))?;
                    let current_cell = &current_cell_map.cell;
                    if current_cell_map.start_row as i64 == row + 1 {
                        insert_after_cell = Some(current_cell.clone());
                    }
                    if col_span(current_cell) > 1 {
                        column_index += col_span(current_cell) - 1;
                    }
                    column_index += 1;
                }
                match insert_after_cell {
                    None => insert_first(next_row_node, cell)?,
                    Some(after) => {
                        after.insert_after(cell, true)?;
                    }
                }
            }
            column -= 1;
        }
        let row_node = usize::try_from(row)
            .ok()
            .and_then(|r| grid.get_child_at_index(r))
            .filter(is_table_row_node)
            .ok_or_else(|| {
                LexicalError::new(format!(
                    "Expected TableNode childAtIndex({row}) to be RowNode"
                ))
            })?;
        row_node.remove(false)?;
        row -= 1;
    }
    match next_row {
        Some(next_row) => {
            let cell = next_row
                .first()
                .and_then(Option::as_ref)
                .ok_or_else(|| LexicalError::new("table map hole"))?;
            move_selection_to_cell(&cell.cell)?;
        }
        None => {
            let cell = map_at(&grid_map, anchor_start_row - 1, 0)?;
            move_selection_to_cell(&cell.cell)?;
        }
    }
    Ok(())
}

/// `$deleteTableColumnAtSelection()`: deletes the columns the selection's
/// cells span, narrowing cells that span out of them; the whole table when
/// every column goes.
pub fn delete_table_column_at_selection() -> Result<(), LexicalError> {
    let (_, anchor, focus) = range_or_table_selection()?;
    let (anchor_cell, _, grid) = get_node_triplet(&anchor.get_node()?)?;
    let (focus_cell, _, _) = get_node_triplet(&focus.get_node()?)?;
    pad_table_rows(&grid, |_| true)?;
    let (grid_map, anchor_cell_map, focus_cell_map) =
        compute_table_map(&grid, &anchor_cell, &focus_cell)?;
    let anchor_start_column = anchor_cell_map.start_column as i64;
    let focus_start_row = focus_cell_map.start_row as i64;
    let focus_start_column = focus_cell_map.start_column as i64;
    let start_column = anchor_start_column.min(focus_start_column);
    let end_column = (anchor_start_column + col_span(&anchor_cell) - 1)
        .max(focus_start_column + col_span(&focus_cell) - 1);
    let selected_column_count = end_column - start_column + 1;
    let column_count = grid_map.first().map_or(0, Vec::len) as i64;
    if column_count == end_column - start_column + 1 {
        // Empty grid.
        grid.select_previous(None, None)?;
        grid.remove(false)?;
        return Ok(());
    }
    for row in 0..grid_map.len() as i64 {
        for column in start_column..=end_column {
            let value = map_at(&grid_map, row, column)?;
            let (cell, cell_start_column) = (&value.cell, value.start_column as i64);
            if cell_start_column < start_column {
                if column == start_column {
                    let overflow_left = start_column - cell_start_column;
                    // Overflowing left, possibly right too.
                    cell.set_col_span(
                        cell.get_col_span()
                            - selected_column_count.min(col_span(cell) - overflow_left) as f64,
                    );
                }
            } else if cell_start_column + col_span(cell) - 1 > end_column {
                if column == end_column {
                    // Overflowing right.
                    let in_selected_area = end_column - cell_start_column + 1;
                    cell.set_col_span(cell.get_col_span() - in_selected_area as f64);
                }
            } else {
                cell.remove(false)?;
            }
        }
    }
    let next_column = if anchor_start_column > focus_start_column {
        map_get(
            &grid_map,
            focus_start_row,
            anchor_start_column + col_span(&anchor_cell),
        )
    } else {
        map_get(
            &grid_map,
            focus_start_row,
            focus_start_column + col_span(&focus_cell),
        )
    };
    match next_column {
        Some(next) => move_selection_to_cell(&next.cell)?,
        None => {
            let column = if focus_start_column < anchor_start_column {
                focus_start_column - 1
            } else {
                anchor_start_column - 1
            };
            move_selection_to_cell(&map_at(&grid_map, focus_start_row, column)?.cell)?;
        }
    }
    if let Some(mut col_widths) = grid.get_col_widths() {
        let start = (start_column as usize).min(col_widths.len());
        let end = (start + selected_column_count as usize).min(col_widths.len());
        col_widths.drain(start..end);
        grid.set_col_widths(Some(col_widths));
    }
    Ok(())
}

/// `$moveSelectionToCell(cell)`: the start of the block holding the cell's
/// first descendant.
pub(crate) fn move_selection_to_cell(cell: &NodeRef) -> Result<(), LexicalError> {
    match cell.get_first_descendant() {
        None => cell.select_start().map(drop),
        Some(first_descendant) => first_descendant
            .get_parent_or_throw()?
            .select_start()
            .map(drop),
    }
}

/// `$cellContainsEmptyParagraph(cell)`.
fn cell_contains_empty_paragraph(cell: &NodeRef) -> bool {
    cell.get_children_size() == 1
        && cell
            .get_first_child()
            .is_some_and(|c| c.get_latest().is::<ParagraphNode>() && c.is_empty())
}

/// `$mergeCells(cellNodes)`: makes the top-left cell of the rectangle the
/// cells cover span it, moves the others' content into it and removes
/// them. Returns the merged cell.
pub fn merge_cells(cell_nodes: &[NodeRef]) -> Result<Option<NodeRef>, LexicalError> {
    let Some(first) = cell_nodes.first() else {
        return Ok(None);
    };
    let table_node = get_table_node_from_lexical_node_or_throw(first)?;
    pad_table_rows(&table_node, |_| true)?;
    let (grid_map, _, _) = compute_table_map_skip_cell_check(&table_node, None, None)?;
    let (mut min_row, mut max_row, mut min_col, mut max_col) =
        (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    let mut processed_cells: HashSet<String> = HashSet::new();
    for row in &grid_map {
        for map_cell in row.iter().flatten() {
            let cell_key = map_cell.cell.get_key().to_owned();
            if processed_cells.contains(&cell_key) {
                continue;
            }
            if cell_nodes.contains(&map_cell.cell) {
                processed_cells.insert(cell_key);
                let cell_start_row = map_cell.start_row as i64;
                let cell_start_col = map_cell.start_column as i64;
                // `__rowSpan || 1`.
                let cell_row_span = Some(row_span(&map_cell.cell))
                    .filter(|s| *s != 0)
                    .unwrap_or(1);
                let cell_col_span = Some(col_span(&map_cell.cell))
                    .filter(|s| *s != 0)
                    .unwrap_or(1);
                min_row = min_row.min(cell_start_row);
                max_row = max_row.max(cell_start_row + cell_row_span - 1);
                min_col = min_col.min(cell_start_col);
                max_col = max_col.max(cell_start_col + cell_col_span - 1);
            }
        }
    }
    if min_row == i64::MAX || min_col == i64::MAX {
        return Ok(None);
    }
    let total_row_span = max_row - min_row + 1;
    let total_col_span = max_col - min_col + 1;
    let target_cell = map_at(&grid_map, min_row, min_col)?.cell.clone();
    target_cell.set_col_span(total_col_span as f64);
    target_cell.set_row_span(total_row_span as f64);
    let mut seen_cells: HashSet<String> = HashSet::from([target_cell.get_key().to_owned()]);
    for row in min_row..=max_row {
        for col in min_col..=max_col {
            let Some(map_cell) = map_get(&grid_map, row, col) else {
                continue;
            };
            let current_cell = &map_cell.cell;
            let key = current_cell.get_key().to_owned();
            if seen_cells.insert(key) {
                if !cell_contains_empty_paragraph(current_cell) {
                    // The loop skips the target, so drop its own empty
                    // paragraph before real content lands underneath it.
                    if cell_contains_empty_paragraph(&target_cell) {
                        target_cell.clear()?;
                    }
                    target_cell.append(&current_cell.get_children())?;
                }
                current_cell.remove(false)?;
            }
        }
    }
    if target_cell.get_children_size() == 0 {
        target_cell.append(&[create_paragraph_node()])?;
    }
    Ok(Some(target_cell))
}

/// `$unmergeCell()`: unmerges the cell the selection's anchor is in.
pub fn unmerge_cell() -> Result<(), LexicalError> {
    let (_, anchor, _) = range_or_table_selection()?;
    let cell_node = find_matching_parent(&anchor.get_node()?, is_table_cell_node)
        .ok_or_else(|| LexicalError::new("Expected to find a parent TableCellNode"))?;
    unmerge_cell_node(&cell_node)
}

/// `$unmergeCellNode(cellNode)`: splits a merged cell back into cells that
/// keep its presentation, with header states each row and column had.
pub fn unmerge_cell_node(cell_node: &NodeRef) -> Result<(), LexicalError> {
    let (cell, row, grid) = get_node_triplet(cell_node)?;
    let col_span_ = col_span(&cell);
    let row_span_ = row_span(&cell);
    if col_span_ == 1 && row_span_ == 1 {
        return Ok(());
    }
    pad_table_rows(&grid, |_| true)?;
    let (map, cell_map, _) = compute_table_map(&grid, &cell, &cell)?;
    let start_column = cell_map.start_column as i64;
    let start_row = cell_map.start_row as i64;
    let header_state = cell.get_header_styles();
    let base_col_style = header_state & TableCellHeaderStates::COLUMN;
    let mut col_styles = Vec::new();
    for i in 0..col_span_.max(0) {
        let mut col_style = base_col_style;
        let mut row_idx = 0;
        while col_style != 0 && row_idx < map.len() {
            col_style &= map_at(&map, row_idx as i64, i + start_column)?
                .cell
                .get_header_styles();
            row_idx += 1;
        }
        col_styles.push(col_style);
    }
    let base_row_style = header_state & TableCellHeaderStates::ROW;
    let width = map.first().map_or(0, Vec::len);
    let mut row_styles = Vec::new();
    for i in 0..row_span_.max(0) {
        let mut row_style = base_row_style;
        let mut col_idx = 0;
        while row_style != 0 && col_idx < width {
            row_style &= map_at(&map, i + start_row, col_idx as i64)?
                .cell
                .get_header_styles();
            col_idx += 1;
        }
        row_styles.push(row_style);
    }
    let style = |styles: &[i32], i: i64| styles.get(i as usize).copied().unwrap_or(0);
    let create_split_cell = |header_state: i32| -> Result<NodeRef, LexicalError> {
        let copy = copy_node(&cell);
        copy.set_col_span(1.0);
        copy.set_row_span(1.0);
        copy.set_header_styles(header_state, TableCellHeaderStates::BOTH);
        copy.set_cell_width(None);
        copy.append(&[create_paragraph_node()])?;
        Ok(copy)
    };
    if col_span_ > 1 {
        for i in 1..col_span_ {
            cell.insert_after(
                &create_split_cell(style(&col_styles, i) | style(&row_styles, 0))?,
                true,
            )?;
        }
        cell.set_col_span(1.0);
    }
    if row_span_ > 1 {
        let mut current_row_node: Option<NodeRef> = None;
        for i in 1..row_span_ {
            let current_row = start_row + i;
            current_row_node = current_row_node
                .as_ref()
                .unwrap_or(&row)
                .get_next_sibling()
                .filter(is_table_row_node);
            let row_node = current_row_node
                .clone()
                .ok_or_else(|| LexicalError::new("Expected row next sibling to be a row"))?;
            let mut insert_after_cell: Option<NodeRef> = None;
            let mut column = 0i64;
            while column < start_column {
                let current_cell_map = map_at(&map, current_row, column)?;
                let current_cell = &current_cell_map.cell;
                if current_cell_map.start_row as i64 == current_row {
                    insert_after_cell = Some(current_cell.clone());
                }
                if col_span(current_cell) > 1 {
                    column += col_span(current_cell) - 1;
                }
                column += 1;
            }
            for j in (0..col_span_).rev() {
                let split = create_split_cell(style(&col_styles, j) | style(&row_styles, i))?;
                match &insert_after_cell {
                    None => insert_first(&row_node, &split)?,
                    Some(after) => {
                        after.insert_after(&split, true)?;
                    }
                }
            }
        }
        cell.set_row_span(1.0);
    }
    Ok(())
}

/// `$computeTableMap(tableNode, cellA, cellB)`: the map, with both cells'
/// entries (`Anchor not found in Table` or `Focus not found in Table`
/// otherwise).
pub fn compute_table_map(
    table_node: &NodeRef,
    cell_a: &NodeRef,
    cell_b: &NodeRef,
) -> Result<(TableMap, TableMapValue, TableMapValue), LexicalError> {
    let (table_map, cell_a_value, cell_b_value) =
        compute_table_map_skip_cell_check(table_node, Some(cell_a), Some(cell_b))?;
    let cell_a_value =
        cell_a_value.ok_or_else(|| LexicalError::new("Anchor not found in Table"))?;
    let cell_b_value = cell_b_value.ok_or_else(|| LexicalError::new("Focus not found in Table"))?;
    Ok((table_map, cell_a_value, cell_b_value))
}

/// `$computeTableMapSkipCellCheck(tableNode, cellA, cellB)`: for each
/// grid position the cell covering it, placing each row's cells after the
/// positions cells from rows above span into.
pub fn compute_table_map_skip_cell_check(
    table_node: &NodeRef,
    cell_a: Option<&NodeRef>,
    cell_b: Option<&NodeRef>,
) -> Result<(TableMap, Option<TableMapValue>, Option<TableMapValue>), LexicalError> {
    let mut table_map: TableMap = Vec::new();
    let mut cell_a_value: Option<TableMapValue> = None;
    let mut cell_b_value: Option<TableMapValue> = None;
    fn get_map_row(map: &mut TableMap, i: usize) -> &mut Vec<Option<TableMapValue>> {
        while map.len() <= i {
            map.push(Vec::new());
        }
        &mut map[i]
    }
    let grid_children = table_node.get_children();
    for (row_idx, row) in grid_children.iter().enumerate() {
        invariant!(
            is_table_row_node(row),
            "Expected TableNode children to be TableRowNode"
        );
        get_map_row(&mut table_map, row_idx);
        let mut col_idx = 0usize;
        let mut cell = row.get_first_child();
        while let Some(c) = cell {
            invariant!(
                is_table_cell_node(&c),
                "Expected TableRowNode children to be TableCellNode"
            );
            // Skip past any columns that were merged from a higher row.
            while table_map[row_idx].get(col_idx).is_some_and(Option::is_some) {
                col_idx += 1;
            }
            let value = TableMapValue {
                cell: c.clone(),
                start_column: col_idx,
                start_row: row_idx,
            };
            let row_span = row_span(&c).max(0) as usize;
            let col_span = col_span(&c).max(0) as usize;
            for j in 0..row_span {
                if row_idx + j >= grid_children.len() {
                    // A rowSpan below the last row.
                    break;
                }
                let map_row = get_map_row(&mut table_map, row_idx + j);
                if map_row.len() < col_idx + col_span {
                    map_row.resize(col_idx + col_span, None);
                }
                for i in 0..col_span {
                    map_row[col_idx + i] = Some(value.clone());
                }
            }
            if cell_a_value.is_none() && cell_a == Some(&c) {
                cell_a_value = Some(value.clone());
            }
            if cell_b_value.is_none() && cell_b == Some(&c) {
                cell_b_value = Some(value);
            }
            cell = c.get_next_sibling();
        }
    }
    Ok((table_map, cell_a_value, cell_b_value))
}

/// `$padTableRows(tableNode, $shouldPad)`: appends empty cells to each row
/// `should_pad` accepts that covers fewer grid positions than the widest,
/// in a header row when the row's last cell heads it. Returns whether it
/// added any.
pub fn pad_table_rows(
    table_node: &NodeRef,
    should_pad: impl Fn(&NodeRef) -> bool,
) -> Result<bool, LexicalError> {
    let (grid_map, _, _) = compute_table_map_skip_cell_check(table_node, None, None)?;
    let max_row_length = grid_map.iter().map(Vec::len).max().unwrap_or(0);
    let row_nodes = table_node.get_children();
    let mut changed = false;
    for (i, map_row) in grid_map.iter().enumerate() {
        let Some(row_node) = row_nodes.get(i) else {
            continue;
        };
        if !is_table_row_node(row_node) || !should_pad(row_node) {
            continue;
        }
        let row_length = map_row.iter().filter(|c| c.is_some()).count();
        if row_length >= max_row_length {
            continue;
        }
        let header_state = row_node
            .get_last_child()
            .filter(is_table_cell_node)
            .map_or(TableCellHeaderStates::NO_STATUS, |c| {
                c.get_header_styles() & TableCellHeaderStates::ROW
            });
        for _ in row_length..max_row_length {
            let cell = create_table_cell_node(header_state, 1.0, None);
            cell.append(&[create_paragraph_node()])?;
            row_node.append(&[cell])?;
        }
        changed = true;
    }
    Ok(changed)
}

/// `$getNodeTriplet(source)`: the cell `source` is in (or is), its row and
/// its table.
pub fn get_node_triplet(source: &NodeRef) -> Result<(NodeRef, NodeRef, NodeRef), LexicalError> {
    let cell = if is_table_cell_node(source) {
        source.clone()
    } else {
        find_matching_parent(source, is_table_cell_node)
            .ok_or_else(|| LexicalError::new("Expected to find a parent TableCellNode"))?
    };
    let row = cell
        .get_parent()
        .filter(is_table_row_node)
        .ok_or_else(|| LexicalError::new("Expected TableCellNode to have a parent TableRowNode"))?;
    let grid = row
        .get_parent()
        .filter(is_table_node)
        .ok_or_else(|| LexicalError::new("Expected TableRowNode to have a parent TableNode"))?;
    Ok((cell, row, grid))
}

/// `$getNodeTriplet(point)`.
pub fn get_node_triplet_from_point(
    point: &Point,
) -> Result<(NodeRef, NodeRef, NodeRef), LexicalError> {
    get_node_triplet(&point.get_node()?)
}

/// `$computeTableCellRectSpans(map, boundary)`: the largest spans of the
/// cells along each edge of the rectangle.
pub fn compute_table_cell_rect_spans(
    map: &TableMap,
    boundary: &TableCellRectBoundary,
) -> Result<TableCellRectSpans, LexicalError> {
    let (mut top_span, mut left_span, mut right_span, mut bottom_span) = (1, 1, 1, 1);
    let b = boundary;
    for col in b.min_column..=b.max_column {
        top_span = top_span.max(row_span(&map_at(map, b.min_row as i64, col as i64)?.cell));
        bottom_span = bottom_span.max(row_span(&map_at(map, b.max_row as i64, col as i64)?.cell));
    }
    for row in b.min_row..=b.max_row {
        left_span = left_span.max(col_span(
            &map_at(map, row as i64, b.min_column as i64)?.cell,
        ));
        right_span = right_span.max(col_span(
            &map_at(map, row as i64, b.max_column as i64)?.cell,
        ));
    }
    Ok(TableCellRectSpans {
        top_span,
        left_span,
        right_span,
        bottom_span,
    })
}

/// `$computeTableCellRectBoundary(map, cellAMap, cellBMap)`: the rectangle
/// two cells span, grown until every cell it overlaps is inside it.
pub fn compute_table_cell_rect_boundary(
    map: &TableMap,
    cell_a_map: &TableMapValue,
    cell_b_map: &TableMapValue,
) -> TableCellRectBoundary {
    let (a, b) = (cell_a_map, cell_b_map);
    let mut min_column = a.start_column.min(b.start_column) as i64;
    let mut min_row = a.start_row.min(b.start_row) as i64;
    let mut max_column = (a.start_column as i64 + col_span(&a.cell) - 1)
        .max(b.start_column as i64 + col_span(&b.cell) - 1);
    let mut max_row = (a.start_row as i64 + row_span(&a.cell) - 1)
        .max(b.start_row as i64 + row_span(&b.cell) - 1);
    let width = map.first().map_or(0, Vec::len);
    loop {
        let mut has_changes = false;
        for row in map {
            for cell in row.iter().take(width).flatten() {
                let start_column = cell.start_column as i64;
                let start_row = cell.start_row as i64;
                let cell_end_col = start_column + col_span(&cell.cell) - 1;
                let cell_end_row = start_row + row_span(&cell.cell) - 1;
                let intersects_horizontally =
                    start_column <= max_column && cell_end_col >= min_column;
                let intersects_vertically = start_row <= max_row && cell_end_row >= min_row;
                if intersects_horizontally && intersects_vertically {
                    let new_min_column = min_column.min(start_column);
                    let new_max_column = max_column.max(cell_end_col);
                    let new_min_row = min_row.min(start_row);
                    let new_max_row = max_row.max(cell_end_row);
                    if new_min_column != min_column
                        || new_max_column != max_column
                        || new_min_row != min_row
                        || new_max_row != max_row
                    {
                        min_column = new_min_column;
                        max_column = new_max_column;
                        min_row = new_min_row;
                        max_row = new_max_row;
                        has_changes = true;
                    }
                }
            }
        }
        if !has_changes {
            break;
        }
    }
    TableCellRectBoundary {
        min_column: min_column.max(0) as usize,
        min_row: min_row.max(0) as usize,
        max_column: max_column.max(0) as usize,
        max_row: max_row.max(0) as usize,
    }
}

/// `$isSimpleTable(table)`: rows of the same number of cells, none merged.
pub fn is_simple_table(table: &NodeRef) -> bool {
    let mut columns: Option<usize> = None;
    for row in table.get_children() {
        if !is_table_row_node(&row) {
            return false;
        }
        let size = row.get_children_size();
        if *columns.get_or_insert(size) != size {
            return false;
        }
        for cell in row.get_children() {
            if !is_table_cell_node(&cell)
                || cell.get_row_span() != 1.0
                || cell.get_col_span() != 1.0
            {
                return false;
            }
        }
    }
    columns.unwrap_or(0) > 0
}

/// `$moveTableColumn(tableNode, originColumn, targetColumn)`, in a simple
/// table, with its column widths.
pub fn move_table_column(
    table_node: &NodeRef,
    origin_column: usize,
    target_column: usize,
) -> Result<(), LexicalError> {
    if origin_column == target_column {
        return Ok(());
    }
    let column_count = table_node.get_column_count();
    if origin_column >= column_count || target_column >= column_count {
        return Ok(());
    }
    if !is_simple_table(table_node) {
        return Ok(());
    }
    for row in table_node
        .get_children()
        .iter()
        .filter(|r| is_table_row_node(r))
    {
        let mut cells = row.get_children();
        let moved = cells.remove(origin_column);
        cells.insert(target_column, moved);
        row.splice(0, cells.len(), &cells)?;
    }
    if let Some(mut widths) = table_node.get_col_widths()
        && widths.len() == column_count
    {
        let moved = widths.remove(origin_column);
        widths.insert(target_column, moved);
        table_node.set_col_widths(Some(widths));
    }
    Ok(())
}

/// `$moveTableRow(tableNode, originRow, targetRow)`, in a simple table.
pub fn move_table_row(
    table_node: &NodeRef,
    origin_row: usize,
    target_row: usize,
) -> Result<(), LexicalError> {
    if origin_row == target_row {
        return Ok(());
    }
    let rows: Vec<NodeRef> = table_node
        .get_children()
        .into_iter()
        .filter(is_table_row_node)
        .collect();
    if origin_row >= rows.len() || target_row >= rows.len() {
        return Ok(());
    }
    if !is_simple_table(table_node) {
        return Ok(());
    }
    let (origin, target) = (&rows[origin_row], &rows[target_row]);
    if target_row > origin_row {
        target.insert_after(origin, true)?;
    } else {
        target.insert_before(origin, true)?;
    }
    Ok(())
}

/// `$getTableCellNodeRect(tableCellNode)`'s result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableCellNodeRect {
    pub row_index: usize,
    pub column_index: usize,
    pub row_span: usize,
    pub col_span: usize,
}

/// `$getTableCellNodeRect(tableCellNode)`: the cell's position and spans.
pub fn get_table_cell_node_rect(
    table_cell_node: &NodeRef,
) -> Result<Option<TableCellNodeRect>, LexicalError> {
    let (cell_node, _, grid_node) = get_node_triplet(table_cell_node)?;
    let rows: Vec<NodeRef> = grid_node
        .get_children()
        .into_iter()
        .filter(is_table_row_node)
        .collect();
    let mut matrix: Vec<Vec<bool>> = vec![Vec::new(); rows.len()];
    for (row_index, row) in rows.iter().enumerate() {
        let mut column_index = 0;
        for cell in row.get_children().into_iter().filter(is_table_cell_node) {
            while matrix[row_index]
                .get(column_index)
                .copied()
                .unwrap_or(false)
            {
                column_index += 1;
            }
            let rs = Some(row_span(&cell))
                .filter(|s| *s != 0)
                .unwrap_or(1)
                .max(0) as usize;
            let cs = Some(col_span(&cell))
                .filter(|s| *s != 0)
                .unwrap_or(1)
                .max(0) as usize;
            for i in 0..rs {
                // JS throws past the last row (`cellMatrix[rowIndex + i]` is
                // undefined); a row below is out of the table here.
                let Some(r) = matrix.get_mut(row_index + i) else {
                    return Err(LexicalError::new(
                        "Cannot set properties of undefined (a rowSpan below the table)",
                    ));
                };
                if r.len() < column_index + cs {
                    r.resize(column_index + cs, false);
                }
                for j in 0..cs {
                    r[column_index + j] = true;
                }
            }
            if cell == cell_node {
                return Ok(Some(TableCellNodeRect {
                    row_index,
                    column_index,
                    row_span: rs,
                    col_span: cs,
                }));
            }
            column_index += cs;
        }
    }
    Ok(None)
}

/// `$insertTableIntoGrid(tableNode, selection)`: pastes a table into the
/// table the selection is in, at its anchor cell (into the selected cells
/// for a table selection), growing the grid as needed and taking the
/// pasted cells' merges, background colors, vertical alignment and content.
pub fn insert_table_into_grid(
    table_node: &NodeRef,
    selection: &SelectionRef,
) -> Result<bool, LexicalError> {
    let is_table_sel = selection.is_table();
    let Some((anchor, focus)) = selection.get_start_end_points() else {
        return Ok(false);
    };
    let (anchor_cell_node, _, grid_node) = get_node_triplet_from_point(&anchor)?;
    let Some(focus_cell_node) = find_matching_parent(&focus.get_node()?, is_table_cell_node) else {
        return Ok(false);
    };
    pad_table_rows(&grid_node, |_| true)?;
    let (initial_grid_map, anchor_cell_map, focus_cell_map) =
        compute_table_map(&grid_node, &anchor_cell_node, &focus_cell_node)?;
    let (template_grid_map, _, _) = compute_table_map_skip_cell_check(table_node, None, None)?;
    let initial_row_count = initial_grid_map.len() as i64;
    let initial_col_count = initial_grid_map.first().map_or(0, Vec::len) as i64;

    let mut start_row = anchor_cell_map.start_row as i64;
    let mut start_col = anchor_cell_map.start_column as i64;
    let mut affected_row_count = template_grid_map.len() as i64;
    let mut affected_col_count = template_grid_map.first().map_or(0, Vec::len) as i64;
    if is_table_sel {
        let boundary =
            compute_table_cell_rect_boundary(&initial_grid_map, &anchor_cell_map, &focus_cell_map);
        let selection_row_count = (boundary.max_row - boundary.min_row + 1) as i64;
        let selection_col_count = (boundary.max_column - boundary.min_column + 1) as i64;
        start_row = boundary.min_row as i64;
        start_col = boundary.min_column as i64;
        affected_row_count = affected_row_count.min(selection_row_count);
        affected_col_count = affected_col_count.min(selection_col_count);
    }

    // Step 1: unmerge the merged cells in the affected area.
    let mut did_perform_merge_operations = false;
    let last_row_for_unmerge = initial_row_count.min(start_row + affected_row_count) - 1;
    let last_col_for_unmerge = initial_col_count.min(start_col + affected_col_count) - 1;
    let mut unmerged_keys: HashSet<String> = HashSet::new();
    for row in start_row..=last_row_for_unmerge {
        for col in start_col..=last_col_for_unmerge {
            let cell = map_at(&initial_grid_map, row, col)?.cell.clone();
            if unmerged_keys.contains(cell.get_key()) {
                continue;
            }
            if cell.get_row_span() == 1.0 && cell.get_col_span() == 1.0 {
                continue;
            }
            unmerge_cell_node(&cell)?;
            unmerged_keys.insert(cell.get_key().to_owned());
            did_perform_merge_operations = true;
        }
    }

    get_writable(grid_node.get_key());
    let (mut interim_grid_map, _, _) = compute_table_map_skip_cell_check(&grid_node, None, None)?;

    // Step 2: grow the table.
    let rows_to_insert = affected_row_count - initial_row_count + start_row;
    for _ in 0..rows_to_insert.max(0) {
        let cell = map_at(&interim_grid_map, initial_row_count - 1, 0)?
            .cell
            .clone();
        insert_table_row_at_node(&cell, true)?;
    }
    let cols_to_insert = affected_col_count - initial_col_count + start_col;
    for _ in 0..cols_to_insert.max(0) {
        let cell = map_at(&interim_grid_map, 0, initial_col_count - 1)?
            .cell
            .clone();
        insert_table_column_at_node(&cell, true, false)?;
    }

    get_writable(grid_node.get_key());
    (interim_grid_map, _, _) = compute_table_map_skip_cell_check(&grid_node, None, None)?;

    // Step 3: merge cells and set their content, as the template has them.
    for row in start_row..start_row + affected_row_count {
        for col in start_col..start_col + affected_col_count {
            let template_row = row - start_row;
            let template_col = col - start_col;
            let template_cell_map = map_at(&template_grid_map, template_row, template_col)?;
            if template_cell_map.start_row as i64 != template_row
                || template_cell_map.start_column as i64 != template_col
            {
                continue;
            }
            let template_cell = template_cell_map.cell.clone();
            if template_cell.get_row_span() != 1.0 || template_cell.get_col_span() != 1.0 {
                let mut cells_to_merge = Vec::new();
                let last_row_for_merge =
                    (row + row_span(&template_cell)).min(start_row + affected_row_count) - 1;
                let last_col_for_merge =
                    (col + col_span(&template_cell)).min(start_col + affected_col_count) - 1;
                for r in row..=last_row_for_merge {
                    for c in col..=last_col_for_merge {
                        cells_to_merge.push(map_at(&interim_grid_map, r, c)?.cell.clone());
                    }
                }
                merge_cells(&cells_to_merge)?;
                did_perform_merge_operations = true;
            }
            let cell = map_at(&interim_grid_map, row, col)?.cell.clone();
            if let Some(background_color) = template_cell.get_background_color() {
                cell.set_background_color(Some(&background_color));
            }
            if let Some(vertical_align) = template_cell.get_vertical_align() {
                with_cell(&cell, |c| c.vertical_align = Some(vertical_align));
            }
            let original_children = cell.get_children();
            for child in template_cell.get_children() {
                if child.is_text() {
                    // As in JS: the paragraph is made and the text moves on
                    // into the cell.
                    let paragraph_node = create_paragraph_node();
                    paragraph_node.append(std::slice::from_ref(&child))?;
                }
                cell.append(&[child])?;
            }
            for n in original_children {
                n.remove(false)?;
            }
        }
    }

    if is_table_sel && did_perform_merge_operations {
        // Reset the table selection in case the anchor or focus cell was
        // removed by merges.
        get_writable(grid_node.get_key());
        let (final_grid_map, _, _) = compute_table_map_skip_cell_check(&grid_node, None, None)?;
        let new_anchor = map_at(
            &final_grid_map,
            anchor_cell_map.start_row as i64,
            anchor_cell_map.start_column as i64,
        )?;
        new_anchor.cell.select_end()?;
    }
    Ok(true)
}

/// `$setTableRowIsHeader(tableNode, rowIndex, isHeader)`.
pub fn set_table_row_is_header(
    table_node: &NodeRef,
    row_index: usize,
    is_header: bool,
) -> Result<(), LexicalError> {
    let (grid_map, _, _) = compute_table_map_skip_cell_check(table_node, None, None)?;
    invariant!(
        row_index < grid_map.len(),
        "Table row index {row_index} out of range"
    );
    let header_state = if is_header {
        TableCellHeaderStates::ROW
    } else {
        TableCellHeaderStates::NO_STATUS
    };
    let mut visited: Vec<NodeRef> = Vec::new();
    for map_cell in grid_map[row_index].iter().flatten() {
        if !visited.contains(&map_cell.cell) {
            visited.push(map_cell.cell.clone());
            map_cell
                .cell
                .set_header_styles(header_state, TableCellHeaderStates::ROW);
        }
    }
    Ok(())
}

/// `$setTableColumnIsHeader(tableNode, columnIndex, isHeader)`.
pub fn set_table_column_is_header(
    table_node: &NodeRef,
    column_index: usize,
    is_header: bool,
) -> Result<(), LexicalError> {
    let (grid_map, _, _) = compute_table_map_skip_cell_check(table_node, None, None)?;
    invariant!(
        !grid_map.is_empty() && column_index < grid_map[0].len(),
        "Table column index {column_index} out of range"
    );
    let header_state = if is_header {
        TableCellHeaderStates::COLUMN
    } else {
        TableCellHeaderStates::NO_STATUS
    };
    let mut visited: Vec<NodeRef> = Vec::new();
    for row in &grid_map {
        if let Some(Some(map_cell)) = row.get(column_index)
            && !visited.contains(&map_cell.cell)
        {
            visited.push(map_cell.cell.clone());
            map_cell
                .cell
                .set_header_styles(header_state, TableCellHeaderStates::COLUMN);
        }
    }
    Ok(())
}
