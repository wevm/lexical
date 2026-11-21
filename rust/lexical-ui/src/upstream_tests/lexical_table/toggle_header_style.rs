//! Ports of `packages/lexical-table/src/__tests__/unit/ToggleHeaderStyle.test.ts`.

use super::support_a::{build_with, update};
use crate::table::{TableCellHeaderStates, create_table_cell_node, table_extension};
use std::cell::Cell;
use std::rc::Rc;

const NO_STATUS: i32 = TableCellHeaderStates::NO_STATUS;
const ROW: i32 = TableCellHeaderStates::ROW;
const COLUMN: i32 = TableCellHeaderStates::COLUMN;
const BOTH: i32 = TableCellHeaderStates::BOTH;

/// `toggle(from, toToggle)`: the header styles of a cell created with
/// `from` once `to_toggle` is toggled.
fn toggle(from: i32, to_toggle: i32) -> i32 {
    let editor = build_with(
        "toggle-host",
        vec![table_extension().into()],
        r#"{"tableScrollableWrapper":"table-scrollable-wrapper"}"#,
    );
    let result = Rc::new(Cell::new(NO_STATUS));
    let out = result.clone();
    update(&editor, move || {
        let cell = create_table_cell_node(from, 1.0, None);
        cell.toggle_header_style(to_toggle);
        out.set(cell.get_header_styles());
        Ok(())
    });
    editor.dispose();
    result.get()
}

// upstream: lexical-table/src/__tests__/unit/ToggleHeaderStyle.test.ts::TableCellNode.toggleHeaderStyle > stays inside the enum when toggling BOTH
#[test]
fn stays_inside_the_enum_when_toggling_both() {
    // Toggling a state the cell does not already have in full adds it; a cell
    // that has all of it loses it.
    assert_eq!(toggle(ROW, BOTH), BOTH);
    assert_eq!(toggle(COLUMN, BOTH), BOTH);
    assert_eq!(toggle(NO_STATUS, BOTH), BOTH);
    assert_eq!(toggle(BOTH, BOTH), NO_STATUS);
}

// upstream: lexical-table/src/__tests__/unit/ToggleHeaderStyle.test.ts::TableCellNode.toggleHeaderStyle > single-bit toggles are unchanged
#[test]
fn single_bit_toggles_are_unchanged() {
    assert_eq!(toggle(NO_STATUS, ROW), ROW);
    assert_eq!(toggle(ROW, ROW), NO_STATUS);
    assert_eq!(toggle(COLUMN, ROW), BOTH);
    assert_eq!(toggle(BOTH, ROW), COLUMN);
    assert_eq!(toggle(NO_STATUS, COLUMN), COLUMN);
    assert_eq!(toggle(COLUMN, COLUMN), NO_STATUS);
    assert_eq!(toggle(ROW, COLUMN), BOTH);
    assert_eq!(toggle(BOTH, COLUMN), ROW);
}
