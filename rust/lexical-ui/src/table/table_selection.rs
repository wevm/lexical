//! `LexicalTableSelection.ts`: `TableSelection`, `$isTableSelection`,
//! `$createTableSelection`, `$createTableSelectionFrom` and
//! `$visitRecursively`. A table selection is the core's
//! `Selection::Table`, read and changed through `SelectionRef` as the other
//! selections are.

use super::table_cell_node::{TableCellHeaderStates, create_table_cell_node, is_table_cell_node};
use super::table_node::{create_table_node, is_table_node};
use super::table_row_node::{create_table_row_node, is_table_row_node};
use super::table_selection_helpers::find_table_node;
use super::table_utils::{
    compute_table_cell_rect_boundary, compute_table_map, get_table_cell_node_rect,
    insert_table_into_grid, map_at,
};
use crate::LexicalError;
use crate::constants::{TEXT_TYPE_TO_FORMAT, lookup};
use crate::error::invariant;
use crate::lexical_selection::{Point, PointType, Selection, SelectionRef, get_selection};
use crate::lexical_utils::get_node_by_key;
use crate::node::{NodeKey, NodeRef};
use crate::nodes::paragraph::{ParagraphNode, create_paragraph_node};
use crate::nodes::text::create_text_node;
use crate::selection_insert_nodes::find_matching_parent;
use crate::updates::is_currently_read_only_mode;

/// `TableSelection`'s fields: the table, and the anchor and focus cells as
/// element points.
#[derive(Clone, Debug)]
pub struct TableSelection {
    pub table_key: NodeKey,
    pub anchor: Point,
    pub focus: Point,
    pub(crate) dirty: bool,
    /// `_cachedNodes`.
    pub(crate) cached_nodes: Option<Vec<NodeRef>>,
}

/// `TableSelectionShape`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableSelectionShape {
    pub from_x: usize,
    pub from_y: usize,
    pub to_x: usize,
    pub to_y: usize,
}

impl TableSelection {
    /// `new TableSelection(tableKey, anchor, focus)`.
    pub fn new(table_key: impl Into<NodeKey>, anchor: Point, focus: Point) -> Self {
        Self {
            table_key: table_key.into(),
            anchor,
            focus,
            dirty: false,
            cached_nodes: None,
        }
    }

    /// `is(selection)`.
    pub fn is(&self, other: &TableSelection) -> bool {
        self.table_key == other.table_key
            && self.anchor == other.anchor
            && self.focus == other.focus
    }

    /// `set(tableKey, anchorCellKey, focusCellKey)`.
    pub fn set(&mut self, table_key: &str, anchor_cell_key: &str, focus_cell_key: &str) {
        self.dirty = self.dirty
            || table_key != self.table_key
            || anchor_cell_key != self.anchor.key
            || focus_cell_key != self.focus.key;
        self.table_key = table_key.to_owned();
        self.anchor.key = anchor_cell_key.to_owned();
        self.focus.key = focus_cell_key.to_owned();
        self.cached_nodes = None;
    }

    /// `isValid()`: element points on nodes that exist, in a table.
    pub fn is_valid(&self) -> bool {
        if self.table_key == "root"
            || self.anchor.key == "root"
            || self.anchor.point_type != PointType::Element
            || self.focus.key == "root"
            || self.focus.point_type != PointType::Element
        {
            return false;
        }
        get_node_by_key(&self.table_key).is_some()
            && get_node_by_key(&self.anchor.key).is_some()
            && get_node_by_key(&self.focus.key).is_some()
    }
}

/// `$getCellNodes(tableSelection)`: the anchor and focus cells and their
/// table.
struct CellNodes {
    anchor_cell: NodeRef,
    anchor_table: NodeRef,
    focus_cell: NodeRef,
}

fn get_cell_nodes(selection: &TableSelection) -> Result<CellNodes, LexicalError> {
    let resolve = |k: &str, point: &Point| -> Result<(NodeRef, NodeRef), LexicalError> {
        let node = point.get_node()?;
        let cell_node = find_matching_parent(&node, is_table_cell_node);
        invariant!(
            cell_node.is_some(),
            "Expected TableSelection {k} to be (or a child of) TableCellNode, got key {} of type {}",
            node.get_key(),
            node.get_type()
        );
        let cell_node = cell_node.expect("checked");
        let row_node = cell_node.get_parent().filter(is_table_row_node);
        invariant!(
            row_node.is_some(),
            "Expected TableSelection {k} cell parent to be a TableRowNode"
        );
        let table_node = row_node
            .expect("checked")
            .get_parent()
            .filter(is_table_node);
        invariant!(
            table_node.is_some(),
            "Expected TableSelection {k} row parent to be a TableNode"
        );
        Ok((cell_node, table_node.expect("checked")))
    };
    let (anchor_cell, anchor_table) = resolve("anchor", &selection.anchor)?;
    let (focus_cell, focus_table) = resolve("focus", &selection.focus)?;
    invariant!(
        anchor_table == focus_table,
        "Expected TableSelection anchor and focus to be in the same table"
    );
    Ok(CellNodes {
        anchor_cell,
        anchor_table,
        focus_cell,
    })
}

/// `$isTableSelection(selection)`.
pub fn is_table_selection(selection: Option<&SelectionRef>) -> bool {
    selection.is_some_and(SelectionRef::is_table)
}

/// `$createTableSelection()`, deprecated upstream: a table selection at
/// the root, which [`TableSelection::is_valid`] rejects.
pub fn create_table_selection() -> SelectionRef {
    SelectionRef::new(Selection::Table(TableSelection::new(
        "root",
        Point::new("root", 0, PointType::Element),
        Point::new("root", 0, PointType::Element),
    )))
}

/// `$createTableSelectionFrom(tableNode, anchorCell, focusCell)`: a table
/// selection from `anchor_cell` to `focus_cell`, a clone of the current one
/// when it is a table selection. It checks, as JS does in DEV builds, that
/// the table is attached and holds both cells.
pub fn create_table_selection_from(
    table_node: &NodeRef,
    anchor_cell: &NodeRef,
    focus_cell: &NodeRef,
) -> Result<SelectionRef, LexicalError> {
    let table_node_key = table_node.get_key();
    invariant!(
        table_node.is_attached(),
        "$createTableSelectionFrom: tableNode {table_node_key} is not attached"
    );
    invariant!(
        find_table_node(anchor_cell).as_ref() == Some(table_node),
        "$createTableSelectionFrom: anchorCell {} is not in table {table_node_key}",
        anchor_cell.get_key()
    );
    invariant!(
        find_table_node(focus_cell).as_ref() == Some(table_node),
        "$createTableSelectionFrom: focusCell {} is not in table {table_node_key}",
        focus_cell.get_key()
    );
    let prev_selection = get_selection();
    let next_selection = match prev_selection.filter(SelectionRef::is_table) {
        Some(prev) => prev.clone_selection(),
        None => create_table_selection(),
    };
    next_selection
        .with_table(|t| t.set(table_node_key, anchor_cell.get_key(), focus_cell.get_key()));
    Ok(next_selection)
}

/// `$visitRecursively(node, $visit)`: depth first, not into the children of
/// a node `visit` returns false for.
pub fn visit_recursively(node: &NodeRef, mut visit: impl FnMut(&NodeRef) -> bool) {
    let mut stack: Vec<Vec<NodeRef>> = vec![vec![node.clone()]];
    while let Some(current_array) = stack.last_mut() {
        match current_array.pop() {
            None => {
                stack.pop();
            }
            Some(current_node) => {
                if visit(&current_node) && current_node.is_element() {
                    stack.push(current_node.get_children());
                }
            }
        }
    }
}

/// `TableSelection.getNodes()`: the table, then each row and the cells of
/// the selected rectangle with their descendants, once each.
pub(crate) fn get_nodes(selection: &SelectionRef) -> Result<Vec<NodeRef>, LexicalError> {
    let table = selection.table().expect("a table selection");
    if !table.is_valid() {
        return Ok(Vec::new());
    }
    if let Some(cached) = table.cached_nodes {
        return Ok(cached);
    }
    let CellNodes {
        anchor_table: table_node,
        anchor_cell,
        focus_cell,
    } = get_cell_nodes(&table)?;
    let focus_cell_grid = focus_cell.get_parents().get(1).cloned();
    if focus_cell_grid.as_ref() != Some(&table_node) {
        if !table_node.is_parent_of(&focus_cell) {
            // The focus is on a higher grid level than the anchor.
            let grid_parent = table_node.get_parent();
            invariant!(
                grid_parent.is_some(),
                "Expected gridParent to have a parent"
            );
            let grid_parent = grid_parent.expect("checked");
            selection.with_table(|t| {
                t.set(
                    &table.table_key,
                    grid_parent.get_key(),
                    focus_cell.get_key(),
                )
            });
        } else {
            // The anchor is on a higher grid level than the focus.
            let focus_cell_parent = focus_cell_grid.and_then(|g| g.get_parent());
            invariant!(
                focus_cell_parent.is_some(),
                "Expected focusCellParent to have a parent"
            );
            let focus_cell_parent = focus_cell_parent.expect("checked");
            selection.with_table(|t| {
                t.set(
                    &table.table_key,
                    focus_cell.get_key(),
                    focus_cell_parent.get_key(),
                )
            });
        }
        return get_nodes(selection);
    }
    let (map, cell_a_map, cell_b_map) = compute_table_map(&table_node, &anchor_cell, &focus_cell)?;
    let b = compute_table_cell_rect_boundary(&map, &cell_a_map, &cell_b_map);
    let mut nodes: Vec<NodeRef> = vec![table_node.clone()];
    let mut seen: std::collections::HashSet<String> =
        std::collections::HashSet::from([table_node.get_key().to_owned()]);
    let mut last_row: Option<NodeRef> = None;
    for i in b.min_row..=b.max_row {
        for j in b.min_column..=b.max_column {
            let cell = map_at(&map, i as i64, j as i64)?.cell.clone();
            let current_row = cell.get_parent().filter(is_table_row_node);
            invariant!(
                current_row.is_some(),
                "Expected TableCellNode parent to be a TableRowNode"
            );
            let current_row = current_row.expect("checked");
            if last_row.as_ref() != Some(&current_row) {
                // `nodeMap.set` keeps a key's first place.
                if seen.insert(current_row.get_key().to_owned()) {
                    nodes.push(current_row.clone());
                }
                last_row = Some(current_row);
            }
            if !seen.contains(cell.get_key()) {
                visit_recursively(&cell, |child| {
                    if seen.insert(child.get_key().to_owned()) {
                        nodes.push(child.clone());
                    }
                    true
                });
            }
        }
    }
    if !is_currently_read_only_mode() {
        selection.with_table(|t| t.cached_nodes = Some(nodes.clone()));
    }
    Ok(nodes)
}

/// `TableSelection.getTextContent()`: the selected cells' text, a tab
/// between the cells of a row and a line break after each row.
pub(crate) fn get_text_content(selection: &SelectionRef) -> Result<String, LexicalError> {
    let nodes: Vec<NodeRef> = get_nodes(selection)?
        .into_iter()
        .filter(is_table_cell_node)
        .collect();
    let mut text_content = String::new();
    for (i, node) in nodes.iter().enumerate() {
        let row = node.get_parent();
        let next_row = nodes.get(i + 1).and_then(NodeRef::get_parent);
        text_content.push_str(&node.get_text_content());
        text_content.push(if next_row != row { '\n' } else { '\t' });
    }
    Ok(text_content)
}

/// `TableSelection.insertRawText(text)`: the text as tab-separated
/// values, pasted into the grid at the anchor cell.
pub(crate) fn insert_raw_text(selection: &SelectionRef, text: &str) -> Result<(), LexicalError> {
    if text.is_empty() {
        return Ok(());
    }
    let table = selection.table().expect("a table selection");
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    let table_node = create_table_node();
    for line in trimmed.split('\n') {
        let row_node = create_table_row_node(None);
        for cell_text in line.split('\t') {
            let cell_node = create_table_cell_node(TableCellHeaderStates::NO_STATUS, 1.0, None);
            let paragraph = create_paragraph_node();
            if !cell_text.is_empty() {
                paragraph.append(&[create_text_node(cell_text)])?;
            }
            cell_node.append(&[paragraph])?;
            row_node.append(&[cell_node])?;
        }
        table_node.append(&[row_node])?;
    }
    let CellNodes { anchor_cell, .. } = get_cell_nodes(&table)?;
    let range_selection = anchor_cell.select(Some(0), Some(anchor_cell.get_children_size()))?;
    insert_table_into_grid(&table_node, &range_selection)?;
    Ok(())
}

/// `TableSelection.insertNodes(nodes)`: into the focus cell, replacing its
/// content.
pub(crate) fn insert_nodes(
    selection: &SelectionRef,
    nodes: Vec<NodeRef>,
) -> Result<(), LexicalError> {
    let table = selection.table().expect("a table selection");
    let focus_node = table.focus.get_node()?;
    invariant!(
        focus_node.is_element(),
        "Expected TableSelection focus to be an ElementNode"
    );
    let range = focus_node.select(Some(0), Some(focus_node.get_children_size()))?;
    crate::selection_insert_nodes::normalize_selection(&range)?;
    range.insert_nodes(nodes)
}

impl SelectionRef {
    /// `TableSelection.hasFormat(type)`: whether the first paragraph of any
    /// selected cell has the format. False for any other selection.
    pub fn table_has_format(&self, format_type: &str) -> Result<bool, LexicalError> {
        if !self.is_table() {
            return Ok(false);
        }
        let mut format = 0i32;
        for cell in get_nodes(self)?.iter().filter(|n| is_table_cell_node(n)) {
            if let Some(paragraph) = cell
                .get_first_child()
                .filter(|p| p.get_latest().is::<ParagraphNode>())
            {
                format |= crate::nodes::text::to_int32(paragraph.element_data().get_text_format());
            }
        }
        let flag = lookup(TEXT_TYPE_TO_FORMAT, format_type).unwrap_or(0);
        Ok(format & flag != 0)
    }

    /// `TableSelection.getShape()`, deprecated upstream: the rectangle of the
    /// anchor and focus cells' positions.
    pub fn get_table_shape(&self) -> Result<Option<TableSelectionShape>, LexicalError> {
        let Some(table) = self.table() else {
            return Ok(None);
        };
        let CellNodes {
            anchor_cell,
            focus_cell,
            ..
        } = get_cell_nodes(&table)?;
        let a = get_table_cell_node_rect(&anchor_cell)?
            .ok_or_else(|| LexicalError::new("getCellRect: expected to find AnchorNode"))?;
        let f = get_table_cell_node_rect(&focus_cell)?
            .ok_or_else(|| LexicalError::new("getCellRect: expected to find focusCellNode"))?;
        let start_x = a.column_index.min(f.column_index);
        let stop_x = (a.column_index + a.col_span - 1).max(f.column_index + f.col_span - 1);
        let start_y = a.row_index.min(f.row_index);
        let stop_y = (a.row_index + a.row_span - 1).max(f.row_index + f.row_span - 1);
        Ok(Some(TableSelectionShape {
            from_x: start_x.min(stop_x),
            from_y: start_y.min(stop_y),
            to_x: start_x.max(stop_x),
            to_y: start_y.max(stop_y),
        }))
    }
}
