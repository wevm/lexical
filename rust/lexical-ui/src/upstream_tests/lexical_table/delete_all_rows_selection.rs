//! Ports of `packages/lexical-table/src/__tests__/unit/DeleteAllRowsSelection.test.ts`.

use super::support_c::{build_table_editor, map_cell, table_with_dimensions};
use crate::table::{
    create_table_selection_from, delete_table_column_at_selection, delete_table_row_at_selection,
    is_table_node,
};
use crate::upstream_tests::support::update;
use crate::{create_paragraph_node, create_text_node, get_root, get_selection, set_selection};

#[derive(Debug, PartialEq)]
struct Outcome {
    selection_type: String,
    stale_table_selection: bool,
    root_children: usize,
}

/// `deleteWholeTable(kind)`: deletes every row (or column) of a fully
/// selected 2x2 table after a paragraph.
fn delete_whole_table(row: bool) -> Outcome {
    let editor = build_table_editor(
        if row {
            "delete-all-row-host"
        } else {
            "delete-all-column-host"
        },
        Vec::new(),
    );
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("before")])?;
        root.append(&[paragraph])?;
        root.append(&[table_with_dimensions(2, 2, false)])?;
        let table = root.get_child_at_index(1).expect("a child");
        assert!(is_table_node(&table), "expected a TableNode");
        // Every cell selected, so deleting rows/columns empties the table.
        set_selection(Some(create_table_selection_from(
            &table,
            &map_cell(&table, 0, 0),
            &map_cell(&table, 1, 1),
        )?));
        Ok(())
    });
    update(&editor, move || {
        if row {
            delete_table_row_at_selection()
        } else {
            delete_table_column_at_selection()
        }
    });
    let outcome = editor
        .read(|| {
            let selection = get_selection();
            Outcome {
                root_children: get_root().get_children_size(),
                selection_type: match &selection {
                    Some(s) if s.is_range() => "range".to_owned(),
                    Some(s) if s.is_table() => "table".to_owned(),
                    Some(_) => "node".to_owned(),
                    None => "null".to_owned(),
                },
                // A TableSelection left behind by the removed table reports
                // isValid() false, because the cells it points at are gone.
                stale_table_selection: selection
                    .and_then(|s| s.table())
                    .is_some_and(|t| !t.is_valid()),
            }
        })
        .unwrap();
    editor.dispose();
    outcome
}

fn expected() -> Outcome {
    Outcome {
        root_children: 1,
        selection_type: "range".to_owned(),
        stale_table_selection: false,
    }
}

// upstream: lexical-table/src/__tests__/unit/DeleteAllRowsSelection.test.ts::deleting every row of a table > leaves the selection outside the removed table
#[test]
fn leaves_the_selection_outside_the_removed_table() {
    assert_eq!(delete_whole_table(true), expected());
}

// upstream: lexical-table/src/__tests__/unit/DeleteAllRowsSelection.test.ts::deleting every row of a table > matches what deleting every column already does
#[test]
fn matches_what_deleting_every_column_already_does() {
    assert_eq!(delete_whole_table(false), expected());
}
