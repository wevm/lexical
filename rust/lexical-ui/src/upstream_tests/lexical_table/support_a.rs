//! Helpers for group a of the lexical-table test ports.

#![allow(dead_code)]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::table::{
    TableCellHeaderStates, compute_table_map_skip_cell_check, create_table_cell_node,
    create_table_node, create_table_row_node, is_table_node, table_extension, table_nodes,
};
use crate::{
    AnyLexicalExtensionArgument, Editor, InitialEditorStateType, LexicalError, LexicalExtension,
    NodeRef, UpdateOptions, create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::rc::Rc;

/// `{discrete: true}`.
pub(super) fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// `editor.update(fn, {discrete: true})`, which must not fail.
pub(super) fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

/// `editor.read(fn)`.
pub(super) fn read<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read(f).unwrap()
}

/// `editor.read('latest', fn)`.
pub(super) fn read_latest<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor
        .read_with_mode(crate::EditorReadMode::Latest, f)
        .unwrap()
}

/// `buildEditorFromExtensions(defineExtension({dependencies, name, theme}))`.
pub(super) fn build_with(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
    theme: &str,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        theme: Some(
            crate::json::parse(theme)
                .unwrap()
                .as_object()
                .unwrap()
                .clone(),
        ),
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// An editor with `TableExtension` and `theme: {tableScrollableWrapper: ''}`.
pub(super) fn build_table_editor(name: &str) -> LexicalEditorWithDispose {
    build_with(
        name,
        vec![table_extension().into()],
        r#"{"tableScrollableWrapper":""}"#,
    )
}

/// `buildEditorFromExtensions(defineExtension({$initialEditorState, name,
/// nodes: [TableNode, TableRowNode, TableCellNode]}))`: no table transform.
pub(super) fn build_with_table_nodes(
    name: &str,
    initial: impl Fn() -> Result<(), LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        nodes: table_nodes(),
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `$createParagraphNode().append($createTextNode(text))`.
pub(super) fn text_paragraph(text: &str) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)]).unwrap();
    paragraph
}

/// `$createTableCellNode(headerState)` holding `children`.
pub(super) fn cell_with(header_state: i32, children: &[NodeRef]) -> NodeRef {
    let cell = create_table_cell_node(header_state, 1.0, None);
    cell.append(children).unwrap();
    cell
}

/// A cell with no header holding a paragraph of `text`.
pub(super) fn text_cell(text: &str) -> NodeRef {
    cell_with(TableCellHeaderStates::NO_STATUS, &[text_paragraph(text)])
}

/// A cell with a single empty paragraph.
pub(super) fn empty_cell(header_state: i32) -> NodeRef {
    cell_with(header_state, &[create_paragraph_node()])
}

/// `$createTableRowNode().append(...cells)`.
pub(super) fn row(cells: &[NodeRef]) -> NodeRef {
    let row = create_table_row_node(None);
    row.append(cells).unwrap();
    row
}

/// `$createTableNode().append(...rows)`.
pub(super) fn table(rows: &[NodeRef]) -> NodeRef {
    let table = create_table_node();
    table.append(rows).unwrap();
    table
}

/// A table of rows of cells.
pub(super) fn table_of(rows: Vec<Vec<NodeRef>>) -> NodeRef {
    table(&rows.iter().map(|cells| row(cells)).collect::<Vec<_>>())
}

/// The first table among the root's children.
pub(super) fn first_table() -> NodeRef {
    get_root()
        .get_children()
        .into_iter()
        .find(is_table_node)
        .expect("a table")
}

/// The cells of each of the table's rows.
pub(super) fn cells_of(table: &NodeRef) -> Vec<Vec<NodeRef>> {
    table
        .get_children()
        .iter()
        .map(NodeRef::get_children)
        .collect()
}

/// The text of each of the table's cells, in document order.
pub(super) fn cell_texts(table: &NodeRef) -> Vec<Vec<String>> {
    cells_of(table)
        .iter()
        .map(|cells| cells.iter().map(NodeRef::get_text_content).collect())
        .collect()
}

/// `$computeTableMapSkipCellCheck(table, null, null)[0]`.
pub(super) fn grid(table: &NodeRef) -> crate::table::TableMap {
    compute_table_map_skip_cell_check(table, None, None)
        .unwrap()
        .0
}
