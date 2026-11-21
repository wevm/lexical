//! Helpers for group c of the lexical-table test ports.

#![allow(dead_code)]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::table::{
    InsertTableCommandPayloadHeaders, compute_table_map_skip_cell_check,
    create_table_node_with_dimensions, is_table_node, table_extension, table_nodes,
};
use crate::upstream_tests::support::{initialize_unit_test, mount_in_jsdom};
use crate::{
    AnyLexicalExtensionArgument, CreateEditorArgs, Editor, KeyboardEvent, LexicalExtension,
    NodeRef, define_extension, get_root,
};
use std::rc::Rc;

/// `buildEditorFromExtensions(defineExtension({dependencies:
/// [TableExtension, ...more], name}))`.
pub(super) fn build_table_editor(
    name: &str,
    more: Vec<AnyLexicalExtensionArgument>,
) -> LexicalEditorWithDispose {
    let mut dependencies = vec![table_extension().into()];
    dependencies.extend(more);
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// [`build_table_editor`], then `editor.setRootElement(container)`.
pub(super) fn build_mounted_table_editor(name: &str) -> LexicalEditorWithDispose {
    let editor = build_table_editor(name, Vec::new());
    mount_in_jsdom(&editor);
    editor
}

/// `initializeUnitTest`'s editor, whose `DEFAULT_NODES` hold the table
/// nodes.
pub(super) fn initialize_table_unit_test() -> Editor {
    initialize_unit_test(CreateEditorArgs {
        nodes: table_nodes(),
        ..CreateEditorArgs::default()
    })
}

/// `new KeyboardEvent('keydown', {key, shiftKey})`.
pub(super) fn key_event(key: &str, shift_key: bool) -> Rc<KeyboardEvent> {
    Rc::new(KeyboardEvent {
        key: key.to_owned(),
        shift_key,
        ..KeyboardEvent::default()
    })
}

/// `$createTableNodeWithDimensions(rows, columns, includeHeaders)`.
pub(super) fn table_with_dimensions(rows: usize, columns: usize, headers: bool) -> NodeRef {
    create_table_node_with_dimensions(
        rows,
        columns,
        InsertTableCommandPayloadHeaders::Both(headers),
    )
    .unwrap()
}

/// The cell at `map[row][column]` of
/// `$computeTableMapSkipCellCheck(table, null, null)`.
pub(super) fn map_cell(table: &NodeRef, row: usize, column: usize) -> NodeRef {
    let (map, _, _) = compute_table_map_skip_cell_check(table, None, None).unwrap();
    map[row][column].as_ref().expect("a cell").cell.clone()
}

/// The root's first child, asserted to be a `TableNode`.
pub(super) fn root_table() -> NodeRef {
    let table = get_root().get_first_child().expect("a child at the root");
    assert!(is_table_node(&table), "expected a TableNode at the root");
    table
}

/// The text of each row's cells.
pub(super) fn cell_texts(table: &NodeRef) -> Vec<Vec<String>> {
    table
        .get_children()
        .iter()
        .map(|row| {
            row.get_children()
                .iter()
                .map(NodeRef::get_text_content)
                .collect()
        })
        .collect()
}
