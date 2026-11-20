//! Ports of `packages/lexical-table/src/__tests__/unit/LexicalTableRowNode.test.ts`.

use super::support_b::{outer_html, theme_config};
use crate::table::{create_table_row_node, table_nodes};
use crate::upstream_tests::support::{initialize_unit_test, panics, update};
use crate::{CreateEditorArgs, EditorConfig};

fn editor_config() -> EditorConfig {
    theme_config(r#"{"tableRow":"test-table-row-class"}"#)
}

fn editor() -> crate::Editor {
    initialize_unit_test(CreateEditorArgs {
        nodes: table_nodes(),
        ..CreateEditorArgs::default()
    })
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableRowNode.test.ts::LexicalTableRowNode tests > TableRowNode.constructor
#[test]
fn table_row_node_constructor() {
    let editor = editor();
    update(&editor, || {
        let row_node = create_table_row_node(None);
        assert_eq!(row_node.get_type(), "tablerow");
        Ok(())
    });
    assert!(panics(|| create_table_row_node(None)));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableRowNode.test.ts::LexicalTableRowNode tests > TableRowNode.createDOM()
#[test]
fn table_row_node_create_dom() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let row_node = create_table_row_node(None);
        assert_eq!(
            outer_html(&row_node, &editor_config(), &e),
            r#"<tr class="test-table-row-class"></tr>"#
        );
        let row_with_custom_height_node = create_table_row_node(Some(36.0));
        assert_eq!(
            outer_html(&row_with_custom_height_node, &editor_config(), &e),
            r#"<tr style="height: 36px;" class="test-table-row-class"></tr>"#
        );
        Ok(())
    });
}
