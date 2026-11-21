//! Helpers for group d of the lexical-table test ports.

#![allow(dead_code)]

use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::table::{
    TableOutput, compute_table_map_skip_cell_check, create_table_cell_node, is_table_node,
    table_extension,
};
use crate::{
    AnyLexicalExtensionArgument, Editor, EditorReadMode, InitialEditorStateType, LexicalExtension,
    NodeRef, UpdateOptions, create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::rc::Rc;

/// `{discrete: true}`.
pub(crate) fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// `{discrete: true, skipTransforms: true}`.
pub(crate) fn discrete_skip_transforms() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        skip_transforms: true,
        ..UpdateOptions::default()
    }
}

/// `buildEditorFromExtensions(defineExtension({$initialEditorState: null,
/// dependencies, name}))`.
pub(crate) fn build_null_initial(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `buildEditorFromExtensions(defineExtension({dependencies:
/// [TableExtension], name: 'table-test'}))`, `LexicalTableExtension.test.ts`'s
/// `beforeEach`.
pub(crate) fn build_table_test_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![table_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("table-test", ())
    })
    .into()])
    .unwrap()
}

/// `getExtensionDependencyFromEditor(editor, TableExtension).output`.
pub(crate) fn table_output(editor: &Editor) -> Rc<TableOutput> {
    get_extension_dependency_from_editor(editor, &table_extension())
        .unwrap()
        .output
        .unwrap()
}

/// `editor.read('latest', f)`.
pub(crate) fn read_latest<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read_with_mode(EditorReadMode::Latest, f).unwrap()
}

/// `$createParagraphNode().append($createTextNode(text))`.
pub(crate) fn text_paragraph(text: &str) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)]).unwrap();
    paragraph
}

/// `$createTableCellNode(headerState).append(...children)`.
pub(crate) fn cell(header_state: i32, children: &[NodeRef]) -> NodeRef {
    let cell = create_table_cell_node(header_state, 1.0, None);
    cell.append(children).unwrap();
    cell
}

/// The root's first child, a `TableNode`.
pub(crate) fn root_table() -> NodeRef {
    let table = get_root().get_first_child();
    let table = table.filter(is_table_node);
    table.expect("expected a TableNode at the root")
}

/// `$cellsPerRow()`: each row's children count, joined with commas.
pub(crate) fn cells_per_row() -> String {
    root_table()
        .get_children()
        .iter()
        .filter(|row| crate::table::is_table_row_node(row))
        .map(|row| row.get_children_size().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// `$computeTableMapSkipCellCheck(table, null, null)[0]`'s row widths.
pub(crate) fn map_widths(table: &NodeRef) -> Vec<usize> {
    let (map, _, _) = compute_table_map_skip_cell_check(table, None, None).unwrap();
    map.iter().map(Vec::len).collect()
}
