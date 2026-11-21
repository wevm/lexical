//! Ports of `packages/lexical-table/src/__tests__/unit/ClearTextKeepsParagraphState.test.ts`.

use super::support_c::{
    build_mounted_table_editor, key_event, map_cell, root_table, table_with_dimensions,
};
use crate::nodes::paragraph::ParagraphNode;
use crate::table::{create_table_selection_from, is_table_cell_node};
use crate::upstream_tests::support::update;
use crate::{
    Direction, EditorReadMode, KEY_BACKSPACE_COMMAND, NodeRef, create_text_node, get_root,
    set_selection,
};

/// `$firstRowParagraphState()`: each first-row cell's paragraph direction,
/// format, style and the cell's text.
fn first_row_paragraph_state() -> Vec<(Option<Direction>, String, String, String)> {
    let table = root_table();
    let first_row = table.get_first_child().expect("a row");
    first_row
        .get_children()
        .into_iter()
        .filter(is_table_cell_node)
        .map(|cell: NodeRef| {
            let paragraph = cell.get_first_child().expect("a paragraph");
            assert!(
                paragraph.get_latest().is::<ParagraphNode>(),
                "expected a ParagraphNode in a cell"
            );
            let data = paragraph.element_data();
            (
                data.get_direction(),
                data.get_format_type().to_owned(),
                data.get_style().to_owned(),
                cell.get_text_content(),
            )
        })
        .collect()
}

// upstream: lexical-table/src/__tests__/unit/ClearTextKeepsParagraphState.test.ts::clearing a table selection > keeps the paragraph format, style and direction of each cell
#[test]
fn keeps_the_paragraph_format_style_and_direction_of_each_cell() {
    let editor = build_mounted_table_editor("clear-text-host");
    update(&editor, || {
        let table = table_with_dimensions(2, 2, false);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&table))?;
        for column in 0..2 {
            let paragraph = map_cell(&table, 0, column)
                .get_first_child()
                .expect("a paragraph");
            paragraph.set_element_format("center");
            paragraph.set_direction(Some(Direction::Rtl));
            paragraph.set_element_style("line-height: 2;");
            paragraph.append(&[create_text_node("text")])?;
        }
        set_selection(Some(create_table_selection_from(
            &table,
            &map_cell(&table, 0, 0),
            &map_cell(&table, 0, 1),
        )?));
        Ok(())
    });

    // Backspace over a multi-cell selection routes to `$clearText`. The
    // selection covers only the first row, so the table itself survives.
    let e = (*editor).clone();
    update(&editor, move || {
        e.dispatch_command(&KEY_BACKSPACE_COMMAND, key_event("Backspace", false))?;
        Ok(())
    });

    let state = editor
        .read_with_mode(EditorReadMode::Latest, first_row_paragraph_state)
        .unwrap();
    let cell = (
        Some(Direction::Rtl),
        "center".to_owned(),
        "line-height: 2;".to_owned(),
        String::new(),
    );
    assert_eq!(state, vec![cell.clone(), cell]);
    editor.dispose();
}
