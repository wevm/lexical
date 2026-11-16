//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/LexicalTabNode.test.ts`.

use crate::rich_text::register_rich_text;
use crate::upstream_tests::support::{initialize_unit_test, range_selection, update};
use crate::{
    CreateEditorArgs, Editor, FORMAT_TEXT_COMMAND, INSERT_TAB_COMMAND, create_paragraph_node,
    get_root,
};

/// The two updates both tests start with: a paragraph, `ab` typed into it
/// and selected.
fn type_ab_and_select_it(editor: &Editor) {
    update(editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select(None, None)?;
        Ok(())
    });
    update(editor, || {
        range_selection().insert_text("ab")?;
        let text_node = get_root().get_first_descendant().unwrap();
        assert!(text_node.is_text());
        text_node.select(Some(0), Some(2))?;
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalTabNode.test.ts::LexicalTabNode tests > INSERT_TAB_COMMAND applies selection format and style to TabNode
#[test]
fn insert_tab_command_applies_selection_format_and_style_to_tab_node() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let _unregister = register_rich_text(&editor);
    type_ab_and_select_it(&editor);
    editor
        .dispatch_command(&FORMAT_TEXT_COMMAND, "bold".to_owned())
        .unwrap();
    editor.dispatch_command(&INSERT_TAB_COMMAND, ()).unwrap();
    editor
        .read(|| {
            let nodes = get_root().get_all_text_nodes();
            let tab_node = nodes.iter().find(|n| n.get_type() == "tab").expect("a tab");
            assert_eq!(tab_node.text_data().get_format(), 1.0);
        })
        .unwrap();
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalTabNode.test.ts::LexicalTabNode tests > format preserved when typing between tabs inserted in bold text
#[test]
fn format_preserved_when_typing_between_tabs_inserted_in_bold_text() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let _unregister = register_rich_text(&editor);
    type_ab_and_select_it(&editor);
    editor
        .dispatch_command(&FORMAT_TEXT_COMMAND, "bold".to_owned())
        .unwrap();
    update(&editor, || {
        let text_node = get_root().get_first_descendant().unwrap();
        assert!(text_node.is_text());
        text_node.select(Some(1), Some(1))?;
        Ok(())
    });
    editor.dispatch_command(&INSERT_TAB_COMMAND, ()).unwrap();
    editor.dispatch_command(&INSERT_TAB_COMMAND, ()).unwrap();
    update(&editor, || {
        let tab_nodes: Vec<_> = get_root()
            .get_all_text_nodes()
            .into_iter()
            .filter(|n| n.get_type() == "tab")
            .collect();
        assert_eq!(tab_nodes.len(), 2);
        tab_nodes[1].select_start()?;
        Ok(())
    });
    update(&editor, || range_selection().insert_text("x"));
    editor
        .read(|| {
            let nodes = get_root().get_all_text_nodes();
            let x_node = nodes
                .iter()
                .find(|n| n.get_text_content() == "x")
                .expect("x");
            assert_eq!(x_node.text_data().get_format(), 1.0);
        })
        .unwrap();
}
