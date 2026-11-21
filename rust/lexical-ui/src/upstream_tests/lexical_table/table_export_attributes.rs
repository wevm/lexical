//! Ports of `packages/lexical-table/src/__tests__/unit/TableExportAttributes.test.ts`.

use super::support_b::build_table_editor;
use crate::html::generate_html_from_nodes;
use crate::table::{
    InsertTableCommandPayloadHeaders, create_table_node_with_dimensions, is_table_node,
};
use crate::upstream_tests::support::update;
use crate::{Direction, LexicalError, NodeRef, get_root};

/// `exportTable(hasHorizontalScroll, $configureTable)`: the HTML of a
/// one-cell table set up by `configure_table`.
fn export_table(
    has_horizontal_scroll: bool,
    configure_table: impl FnOnce(&NodeRef) -> Result<(), LexicalError> + 'static,
) -> String {
    let editor = build_table_editor(
        "table-export-attributes-host",
        has_horizontal_scroll,
        r#"{"tableScrollableWrapper":"scroll-wrapper"}"#,
        Vec::new(),
    );
    update(&editor, move || {
        let table =
            create_table_node_with_dimensions(1, 1, InsertTableCommandPayloadHeaders::Both(false))?;
        configure_table(&table)?;
        let root = get_root();
        root.clear()?;
        root.append(&[table])
    });
    let e = (*editor).clone();
    let html = editor
        .read(move || {
            let table = get_root().get_first_child().expect("a child");
            assert!(is_table_node(&table), "expected a TableNode at the root");
            generate_html_from_nodes(&e, None)
        })
        .unwrap()
        .unwrap();
    editor.dispose();
    html
}

fn rtl(table: &NodeRef) -> Result<(), LexicalError> {
    table.set_direction(Some(Direction::Rtl));
    Ok(())
}

fn indent_2(table: &NodeRef) -> Result<(), LexicalError> {
    table.set_indent(2.0)
}

// upstream: lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM direction > exports dir with scrollable tables active
#[test]
fn exports_dir_with_scrollable_tables_active() {
    let html = export_table(true, rtl);
    assert!(html.contains(r#"<table dir="rtl">"#), "{html}");
}

// upstream: lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM direction > exports dir without scrollable tables
#[test]
fn exports_dir_without_scrollable_tables() {
    let html = export_table(false, rtl);
    assert!(html.contains(r#"<table dir="rtl">"#), "{html}");
}

// upstream: lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM indent > exports the indent with scrollable tables active
#[test]
fn exports_the_indent_with_scrollable_tables_active() {
    let html = export_table(true, indent_2);
    assert!(html.contains(r#"data-lexical-indent="2""#), "{html}");
    assert!(html.contains("padding-inline-start: 80px"), "{html}");
}

// upstream: lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM indent > exports the indent without scrollable tables
#[test]
fn exports_the_indent_without_scrollable_tables() {
    let html = export_table(false, indent_2);
    assert!(html.contains(r#"data-lexical-indent="2""#), "{html}");
    assert!(html.contains("padding-inline-start: 80px"), "{html}");
}

// upstream: lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM indent > writes nothing for an unindented table
#[test]
fn writes_nothing_for_an_unindented_table() {
    let html = export_table(true, |_| Ok(()));
    assert!(!html.contains("data-lexical-indent"), "{html}");
}
