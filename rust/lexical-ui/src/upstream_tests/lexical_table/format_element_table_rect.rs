//! Ports of `packages/lexical-table/src/__tests__/unit/FormatElementTableRect.test.ts`.

use super::support_c::{map_cell, root_table, table_with_dimensions};
use crate::extension::build_editor_from_extensions;
use crate::table::{create_table_selection_from, is_table_cell_node, merge_cells, table_extension};
use crate::upstream_tests::support::{mount_in_jsdom, update};
use crate::{
    EditorReadMode, FORMAT_ELEMENT_COMMAND, InitialEditorStateType, LexicalExtension,
    define_extension, get_root, get_selection, set_selection,
};
use std::rc::Rc;

// upstream: lexical-table/src/__tests__/unit/FormatElementTableRect.test.ts::FORMAT_ELEMENT_COMMAND over a table selection > formats every cell the selection contains, including one pulled in by a merge
#[test]
fn formats_every_cell_the_selection_contains_including_one_pulled_in_by_a_merge() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let table = table_with_dimensions(3, 3, false);
            get_root().append(std::slice::from_ref(&table))?;
            // Merge grid columns 0 and 1 of row 1. The merged cell straddles
            // the left edge of the rect the next selection describes, so the
            // rect has to grow to column 0 to contain it.
            let merged = merge_cells(&[map_cell(&table, 1, 0), map_cell(&table, 1, 1)])?;
            assert!(merged.is_some(), "expected the cells to merge");
            Ok(())
        }))),
        dependencies: vec![table_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("format-element-rect-host", ())
    })
    .into()])
    .unwrap();
    // `afterRegistration`: the FORMAT_ELEMENT_COMMAND handler is registered
    // by the table selection observer, which needs a root element.
    mount_in_jsdom(&editor);

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

    editor
        .dispatch_command(&FORMAT_ELEMENT_COMMAND, "center".to_owned())
        .unwrap();

    editor
        .read_with_mode(EditorReadMode::ForceCommit, || {
            let selection = get_selection().expect("expected a selection");
            let selected_cells: Vec<_> = selection
                .get_nodes()
                .unwrap()
                .into_iter()
                .filter(is_table_cell_node)
                .collect();
            assert!(!selected_cells.is_empty());
            let formats: Vec<_> = selected_cells
                .iter()
                .map(|cell| cell.element_data().get_format_type())
                .collect();
            assert_eq!(formats, vec!["center"; selected_cells.len()]);
        })
        .unwrap();
    editor.dispose();
}
