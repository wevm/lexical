//! Ports of `packages/lexical-table/src/__tests__/unit/ShiftArrowOffCornerAnchor.test.ts`.

use super::support_c::{
    build_mounted_table_editor, key_event, map_cell, root_table, table_with_dimensions,
};
use crate::table::{
    compute_table_map_skip_cell_check, create_table_selection_from, is_table_cell_node, merge_cells,
};
use crate::upstream_tests::support::update;
use crate::{
    EditorReadMode, KEY_ARROW_DOWN_COMMAND, UpdateOptions, get_root, get_selection, set_selection,
};

/// `$selectionRect()`: where the anchor and focus cells start, how many
/// cells the selection covers, and whether it is valid.
fn selection_rect() -> (String, usize, String, bool) {
    let selection = get_selection().filter(|s| s.is_table());
    let selection = selection.expect("expected a TableSelection");
    let table = selection.table().expect("a table selection");
    let (map, _, _) = compute_table_map_skip_cell_check(&root_table(), None, None).unwrap();
    let start_of = |key: &str| {
        for value in map.iter().flatten().flatten() {
            if value.cell.get_key() == key {
                return format!("{},{}", value.start_row, value.start_column);
            }
        }
        format!("{key} is not in the table")
    };
    (
        start_of(&table.anchor.key),
        selection
            .get_nodes()
            .unwrap()
            .into_iter()
            .filter(is_table_cell_node)
            .count(),
        start_of(&table.focus.key),
        table.is_valid(),
    )
}

// upstream: lexical-table/src/__tests__/unit/ShiftArrowOffCornerAnchor.test.ts::Shift+Arrow with an anchor that is not on a rect corner > extends the selection instead of throwing
#[test]
fn extends_the_selection_instead_of_throwing() {
    let editor = build_mounted_table_editor("shift-arrow-corner-host");
    update(&editor, || {
        let table = table_with_dimensions(3, 3, false);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&table))?;
        // Merge grid columns 0 and 1 of row 1. This cell straddles the left
        // edge of the rect selected below, so the rect grows out to column
        // 0, past the anchor, which then sits on no corner of it.
        let merged = merge_cells(&[map_cell(&table, 1, 0), map_cell(&table, 1, 1)])?;
        assert!(merged.is_some(), "expected the cells to merge");
        Ok(())
    });
    update(&editor, || {
        let table = root_table();
        // Anchor at (row 0, column 1), focus at (row 1, column 2).
        set_selection(Some(create_table_selection_from(
            &table,
            &map_cell(&table, 0, 1),
            &map_cell(&table, 1, 2),
        )?));
        Ok(())
    });

    let e = (*editor).clone();
    let result = editor.update(
        move || {
            e.dispatch_command(&KEY_ARROW_DOWN_COMMAND, key_event("ArrowDown", true))?;
            Ok(())
        },
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        },
    );
    assert!(result.is_ok(), "{result:?}");

    // The anchor was not on a corner, so getAnchorCorner falls back to the
    // corner opposite the focus, top-left, and ArrowDown grows the rect down
    // to the last row. The merged cell keeps the rect out at column 0, so
    // the result is the whole table: 9 grid positions, 8 distinct cells.
    let rect = editor
        .read_with_mode(EditorReadMode::Latest, selection_rect)
        .unwrap();
    assert_eq!(rect, ("0,0".to_owned(), 8, "2,2".to_owned(), true));
    editor.dispose();
}
