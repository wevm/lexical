//! Ports of `packages/lexical-text/src/__tests__/unit/registerLexicalTextEntity.test.ts`.

use super::support::{entity_class, regex_match};
use crate::text::register_lexical_text_entity;
use crate::upstream_tests::support::{initialize_unit_test, update};
use crate::{
    CreateEditorArgs, Editor, NodeClass, apply_node_replacement, create_paragraph_node,
    create_text_node, get_root,
};

entity_class!(TestEntityNode, "test-entity");

/// `$createTestEntityNode(text)`.
fn create_test_entity_node(text: &str) -> crate::NodeRef {
    apply_node_replacement(&TestEntityNode::new_node(text)).unwrap()
}

fn editor() -> Editor {
    initialize_unit_test(CreateEditorArgs {
        namespace: "test".into(),
        nodes: vec![NodeClass::of::<TestEntityNode>()],
        ..CreateEditorArgs::default()
    })
}

fn register(editor: &Editor) {
    let _ = register_lexical_text_entity(editor, regex_match(r"#\w+", ""), "test-entity", |n| {
        create_test_entity_node(&n.get_text_content())
    });
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntity.test.ts::registerLexicalTextEntity > preserves style and detail when promoting simple text to an entity
#[test]
fn preserves_style_and_detail_when_promoting_simple_text_to_an_entity() {
    let editor = editor();
    register(&editor);
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let text = create_text_node("#lexical");
        text.set_format_type("bold");
        text.set_style("color: red");
        text.set_detail_type("directionless")?;
        paragraph.append(&[text])?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    editor.get_editor_state().read(|| {
        let node = get_root().get_first_descendant().unwrap();
        assert_eq!(node.get_type(), "test-entity");
        assert_eq!(node.get_text_content(), "#lexical");
        assert!(node.has_format("bold"));
        assert_eq!(node.text_data().get_style(), "color: red");
        assert!(node.is_directionless());
    });
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntity.test.ts::registerLexicalTextEntity > preserves style and detail when reverting an entity to simple text
#[test]
fn preserves_style_and_detail_when_reverting_an_entity_to_simple_text() {
    let editor = editor();
    register(&editor);
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("#lexical")])?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    update(&editor, || {
        let node = get_root().get_first_descendant().unwrap();
        assert_eq!(node.get_type(), "test-entity");
        node.set_format_type("bold");
        node.set_style("color: red");
        node.set_detail_type("directionless")
    });
    // Break the match so the entity reverts to a plain TextNode.
    update(&editor, || {
        let node = get_root().get_first_descendant().unwrap();
        node.set_text_content("lexical");
        Ok(())
    });
    editor.get_editor_state().read(|| {
        let node = get_root().get_first_descendant().unwrap();
        assert_ne!(node.get_type(), "test-entity");
        assert_eq!(node.get_text_content(), "lexical");
        assert!(node.has_format("bold"));
        assert_eq!(node.text_data().get_style(), "color: red");
        assert!(node.is_directionless());
    });
}
