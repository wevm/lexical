//! Ports of `packages/lexical-table/src/__tests__/unit/TableFieldLeavesRaggedRows.test.ts`.
//!
//! A table whose rows are ragged (collaborators' concurrent row and column
//! inserts, which each author pads) and an update that changes only the
//! table itself: the short row is left to its author.

use super::support_a::{discrete, empty_cell, first_table, read, row, table, update};
use crate::extension::build_editor_from_extensions;
use crate::rich_text::rich_text_extension;
use crate::table::{
    TableCellHeaderStates, TableConfigOverrides, is_table_row_node, table_extension,
};
use crate::{
    InitialEditorStateType, LexicalExtension, UpdateOptions, config_extension, define_extension,
    get_root,
};

fn widths() -> Vec<usize> {
    first_table()
        .get_children()
        .into_iter()
        .filter(is_table_row_node)
        .map(|row| row.get_children_size())
        .collect()
}

// upstream: lexical-table/src/__tests__/unit/TableFieldLeavesRaggedRows.test.ts::a table-only field change leaves pre-existing short rows alone
#[test]
fn a_table_only_field_change_leaves_pre_existing_short_rows_alone() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            rich_text_extension().into(),
            config_extension(
                &table_extension(),
                [TableConfigOverrides {
                    has_horizontal_scroll: Some(false),
                    ..TableConfigOverrides::default()
                }],
            ),
        ],
        ..LexicalExtension::<(), (), ()>::new("review-9539-table-field", ())
    })
    .into()])
    .unwrap();
    editor
        .update(
            || {
                let rows: Vec<_> = [3, 2, 3]
                    .iter()
                    .map(|&width| {
                        row(&(0..width)
                            .map(|_| empty_cell(TableCellHeaderStates::NO_STATUS))
                            .collect::<Vec<_>>())
                    })
                    .collect();
                get_root().append(&[table(&rows)])
            },
            UpdateOptions {
                skip_transforms: true,
                ..discrete()
            },
        )
        .unwrap();
    assert_eq!(read(&editor, widths), [3, 2, 3]);
    update(&editor, || {
        first_table().set_row_striping(true);
        Ok(())
    });
    assert_eq!(read(&editor, widths), [3, 2, 3]);
    editor.dispose();
}
