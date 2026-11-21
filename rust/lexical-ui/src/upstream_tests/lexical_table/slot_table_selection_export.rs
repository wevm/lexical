//! Ports of `packages/lexical-table/src/__tests__/unit/SlotTableSelectionExport.test.ts`.
//!
//! `PlainShadowRootNode` is the shared `TestShadowRootNode`, a shadow-root
//! block that renders a `div`.

use crate::clipboard::generate_json_from_selected_nodes;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::generate_html_from_nodes;
use crate::table::{
    TableCellHeaderStates, compute_table_map_skip_cell_check, create_table_cell_node,
    create_table_node, create_table_row_node, create_table_selection_from, table_extension,
};
use crate::upstream_tests::support::{TestShadowRootNode, create_test_shadow_root_node, update};
use crate::{
    InitialEditorStateType, LexicalExtension, NodeClass, create_paragraph_node, create_text_node,
    define_extension, get_root, get_selection, set_selection, set_slot,
};

const CELL_TEXT: &str = "SlotTableTarget";

fn build_editor_with_slotted_table() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![table_extension().into()],
        nodes: vec![NodeClass::of::<TestShadowRootNode>()],
        ..LexicalExtension::<(), (), ()>::new("[slot-table-selection-export]", ())
    })
    .into()])
    .unwrap();
    update(&editor, || {
        let table = create_table_node();
        for r in 0..2 {
            let row = create_table_row_node(None);
            for c in 0..2 {
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node(&format!("{CELL_TEXT}-{r}{c}"))])?;
                let cell = create_table_cell_node(TableCellHeaderStates::NO_STATUS, 1.0, None);
                cell.append(&[paragraph])?;
                row.append(&[cell])?;
            }
            table.append(&[row])?;
        }
        let host = create_paragraph_node();
        let shadow = create_test_shadow_root_node();
        shadow.append(std::slice::from_ref(&table))?;
        set_slot(&host, "media", &shadow)?;
        get_root().append(&[host])?;

        let (map, _, _) = compute_table_map_skip_cell_check(&table, None, None)?;
        let cell = |r: usize, c: usize| map[r][c].as_ref().expect("a cell").cell.clone();
        set_selection(Some(create_table_selection_from(
            &table,
            &cell(0, 0),
            &cell(1, 1),
        )?));
        Ok(())
    });
    editor
}

// upstream: lexical-table/src/__tests__/unit/SlotTableSelectionExport.test.ts::slot frame redirect for a TableSelection > text/html export contains the selected cells
#[test]
fn text_html_export_contains_the_selected_cells() {
    let editor = build_editor_with_slotted_table();
    let html = editor
        .read(|| generate_html_from_nodes(&editor, get_selection().as_ref()))
        .unwrap()
        .unwrap();
    assert!(html.contains(CELL_TEXT), "{html}");
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/SlotTableSelectionExport.test.ts::slot frame redirect for a TableSelection > application/x-lexical-editor export contains the selected cells
#[test]
fn application_x_lexical_editor_export_contains_the_selected_cells() {
    let editor = build_editor_with_slotted_table();
    let nodes = editor
        .read(|| {
            generate_json_from_selected_nodes(&editor, get_selection().as_ref())
                .map(|selected| selected.nodes)
        })
        .unwrap()
        .unwrap();
    let json =
        crate::json::Value::Array(nodes.into_iter().map(crate::json::Value::Object).collect())
            .to_string();
    assert!(json.contains(CELL_TEXT), "{json}");
    editor.dispose();
}
