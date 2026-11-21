//! Ports of `packages/lexical-table/src/__tests__/unit/TableImportExtension.test.ts`.

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::generate_nodes_from_dom_via_extension;
use crate::table::{is_table_cell_node, is_table_node, is_table_row_node, table_extension};
use crate::upstream_tests::lexical_table::support_b::theme;
use crate::upstream_tests::support::update;
use crate::{LexicalExtension, NodeRef, define_extension, get_root};

/// `buildEditor()`: `TableExtension` registers its own import rules (and
/// the shared `CoreImportExtension` baseline).
fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![table_extension().into()],
        theme: Some(theme(r#"{"tableScrollableWrapper":""}"#)),
        ..LexicalExtension::<(), (), ()>::new("table-host", ())
    })
    .into()])
    .unwrap()
}

/// `importInto(editor, html)`.
fn import_into(editor: &LexicalEditorWithDispose, html: &str) {
    let html = format!("<!doctype html><html><body>{html}</body></html>");
    update(editor, move || {
        let dom = crate::dom::parse_from_string(&html);
        let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
        let root = get_root();
        root.clear()?;
        root.append(&nodes)
    });
}

/// `$rootTable()`.
fn root_table() -> NodeRef {
    let t = get_root().get_first_child().expect("a child");
    assert!(is_table_node(&t), "expected TableNode at root");
    t
}

/// `$rows(table)`.
fn rows(table: &NodeRef) -> Vec<NodeRef> {
    table
        .get_children()
        .into_iter()
        .filter(is_table_row_node)
        .collect()
}

/// `$cells(row)`.
fn cells(row: &NodeRef) -> Vec<NodeRef> {
    row.get_children()
        .into_iter()
        .filter(is_table_cell_node)
        .collect()
}

// upstream: lexical-table/src/__tests__/unit/TableImportExtension.test.ts::TableImportExtension > basic 2x2 table imports with correct structure
#[test]
fn basic_2x2_table_imports_with_correct_structure() {
    let editor = build_editor();
    import_into(
        &editor,
        "<table><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>",
    );
    editor
        .read(|| {
            let rows = rows(&root_table());
            assert_eq!(rows.len(), 2);
            let first_row_cells = cells(&rows[0]);
            assert_eq!(first_row_cells.len(), 2);
            assert_eq!(first_row_cells[0].get_text_content(), "a");
            assert_eq!(first_row_cells[1].get_text_content(), "b");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/TableImportExtension.test.ts::TableImportExtension > <th scope="col"> → header cell
#[test]
fn th_scope_col_header_cell() {
    let editor = build_editor();
    import_into(&editor, r#"<table><tr><th scope="col">H</th></tr></table>"#);
    editor
        .read(|| {
            let cell = cells(&rows(&root_table())[0])[0].clone();
            assert!(cell.has_header());
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/TableImportExtension.test.ts::TableImportExtension > table picks up <thead>/<tbody> rows via $descendantsMatching
#[test]
fn table_picks_up_thead_tbody_rows_via_descendants_matching() {
    let editor = build_editor();
    import_into(
        &editor,
        "<table><thead><tr><th>H</th></tr></thead><tbody><tr><td>B</td></tr></tbody></table>",
    );
    editor
        .read(|| {
            let rows = rows(&root_table());
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].get_first_child().unwrap().get_text_content(), "H");
            assert_eq!(rows[1].get_first_child().unwrap().get_text_content(), "B");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/TableImportExtension.test.ts::TableImportExtension > row picks up cells via $descendantsMatching
#[test]
fn row_picks_up_cells_via_descendants_matching() {
    let editor = build_editor();
    import_into(&editor, "<table><tr><td>a</td></tr></table>");
    editor
        .read(|| {
            let cell = cells(&rows(&root_table())[0])[0].clone();
            assert_eq!(cell.get_text_content(), "a");
        })
        .unwrap();
    editor.dispose();
}
