//! Ports of `packages/lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts`.

use crate::CreateEditorArgs;
use crate::json::Value;
use crate::nodes::line_break::create_line_break_node;
use crate::upstream_tests::support::{initialize_unit_test, update};

// upstream: lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.constructor
#[test]
fn line_break_node_constructor() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let line_break_node = create_line_break_node();
        assert_eq!(line_break_node.get_type(), "linebreak");
        assert_eq!(line_break_node.get_text_content(), "\n");
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.exportJSON() should return and object conforming to the expected schema
#[test]
fn line_break_node_export_json() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let node = create_line_break_node();
        assert_eq!(
            Value::Object(node.export_json(false)).to_string(),
            r#"{"type":"linebreak","version":1}"#
        );
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.createDOM()
#[cfg(feature = "html")]
#[test]
fn line_break_node_create_dom() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let e = editor.clone();
    update(&editor, move || {
        let line_break_node = create_line_break_node();
        let element = line_break_node.get_latest().create_dom(
            &line_break_node,
            &crate::EditorConfig::default(),
            &e,
        )?;
        assert_eq!(element.outer_html(), "<br>");
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.updateDOM()
#[test]
fn line_break_node_update_dom() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let line_break_node = create_line_break_node().get_latest();
        assert!(!line_break_node.update_dom(&*line_break_node));
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.$isLineBreakNode()
#[test]
fn is_line_break_node() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let line_break_node = create_line_break_node();
        assert!(
            line_break_node
                .get_latest()
                .downcast_ref::<crate::nodes::line_break::LineBreakNode>()
                .is_some()
        );
        Ok(())
    });
}
