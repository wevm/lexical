//! Ports of `packages/lexical-table/src/__tests__/unit/LexicalTablePlugin.test.tsx`.

use super::support_d::{discrete, read_latest};
use crate::table::{
    INSERT_TABLE_COMMAND, InsertTableCommandPayload, is_table_node, register_table_plugin,
    table_nodes,
};
use crate::{CreateEditorArgs, Editor, create_editor, get_root, set_selection};

/// `beforeEach`: an editor with the table nodes and `registerTablePlugin`.
fn editor() -> Editor {
    let editor = create_editor(CreateEditorArgs {
        namespace: "test".to_owned(),
        nodes: table_nodes(),
        ..CreateEditorArgs::default()
    });
    // The plugin stays registered for the editor's life, as upstream's.
    std::mem::forget(register_table_plugin(&editor, || false).unwrap());
    editor
}

fn payload() -> InsertTableCommandPayload {
    InsertTableCommandPayload {
        columns: "3".to_owned(),
        rows: "2".to_owned(),
        include_headers: None,
    }
}

// upstream: lexical-table/src/__tests__/unit/LexicalTablePlugin.test.tsx::LexicalTablePlugin > INSERT_TABLE_COMMAND inserts a table
#[test]
fn insert_table_command_inserts_a_table() {
    let editor = editor();
    let e = editor.clone();
    editor
        .update(
            move || {
                get_root().select(None, None)?;
                e.dispatch_command(&INSERT_TABLE_COMMAND, payload())?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    read_latest(&editor, || {
        let table = get_root().get_first_child().unwrap();
        assert!(is_table_node(&table), "Expected table node");
        let rows = table.get_children();
        assert_eq!(rows.len(), 2);
        let first_row = &rows[0];
        assert!(first_row.is_element(), "Expected row node");
        assert_eq!(first_row.get_children().len(), 3);
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTablePlugin.test.tsx::LexicalTablePlugin > INSERT_TABLE_COMMAND inserts a table when the editor is blurred
#[test]
fn insert_table_command_inserts_a_table_when_the_editor_is_blurred() {
    let editor = editor();
    editor
        .update(
            || {
                get_root().select(None, None)?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let e = editor.clone();
    editor
        .update(
            move || {
                // Simulate a blur event
                set_selection(None);
                e.dispatch_command(&INSERT_TABLE_COMMAND, payload())?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    read_latest(&editor, || {
        let table = get_root().get_first_child().unwrap();
        assert!(is_table_node(&table));
    });
}
