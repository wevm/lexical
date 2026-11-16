//! Ports of
//! `packages/lexical-clipboard/src/__tests__/unit/MarkClipboardJSON.test.ts`.

#![cfg(feature = "mark")]

use crate::clipboard::{generate_json_from_selected_nodes, generate_nodes_from_serialized_nodes};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::json::Value;
use crate::mark::{MarkNode, create_mark_node, is_mark_node};
use crate::rich_text::rich_text_extension;
use crate::upstream_tests::support::update;
use crate::{
    InitialEditorStateType, LexicalError, LexicalExtension, NodeClass, NodeRef,
    create_paragraph_node, create_text_node, define_extension, get_root, get_selection, select_all,
};
use std::rc::Rc;

/// `buildEditorFromExtensions(extension)`: a paragraph holding a mark
/// `comment-1` around `marked`.
fn build() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let mark = create_mark_node(&["comment-1".to_owned()]);
            mark.append(&[create_text_node("marked")])?;
            let paragraph = create_paragraph_node();
            paragraph.append(&[mark])?;
            let root = get_root();
            root.clear()?;
            root.append(&[paragraph])?;
            Ok(())
        }))),
        dependencies: vec![rich_text_extension().into()],
        nodes: vec![NodeClass::of::<MarkNode>()],
        ..LexicalExtension::<(), (), ()>::new("[mark-clipboard]", ())
    })
    .into()])
    .unwrap()
}

/// The first mark node in document order.
fn first_mark(nodes: &[NodeRef]) -> Option<NodeRef> {
    let mut stack: Vec<NodeRef> = nodes.to_vec();
    while !stack.is_empty() {
        let node = stack.remove(0);
        if is_mark_node(&node) {
            return Some(node);
        }
        if node.is_element() {
            let children = node.get_children();
            stack.splice(0..0, children);
        }
    }
    None
}

// upstream: lexical-clipboard/src/__tests__/unit/MarkClipboardJSON.test.ts::$generateJSONFromSelectedNodes with a MarkNode > keeps the MarkNode in the clone payload
#[test]
fn keeps_the_mark_node_in_the_clone_payload() {
    let editor = build();
    let ed = (*editor).clone();
    update(&editor, move || {
        assert!(first_mark(&rebuilt_of(&ed)?).is_some());
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/MarkClipboardJSON.test.ts::$generateJSONFromSelectedNodes with a MarkNode > keeps the mark ids on the round trip
#[test]
fn keeps_the_mark_ids_on_the_round_trip() {
    let editor = build();
    let ed = (*editor).clone();
    update(&editor, move || {
        let mark = first_mark(&rebuilt_of(&ed)?).expect("expected a MarkNode");
        assert_eq!(mark.get_ids(), Some(vec!["comment-1".to_owned()]));
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/MarkClipboardJSON.test.ts::$generateJSONFromSelectedNodes with a MarkNode > still carries the marked text
#[test]
fn still_carries_the_marked_text() {
    let editor = build();
    let ed = (*editor).clone();
    update(&editor, move || {
        let text: String = rebuilt_of(&ed)?
            .iter()
            .map(NodeRef::get_text_content)
            .collect();
        assert!(text.contains("marked"));
        Ok(())
    });
    editor.dispose();
}

/// `$selectAll()`, then the nodes rebuilt from the selection's JSON.
fn rebuilt_of(editor: &crate::Editor) -> Result<Vec<NodeRef>, LexicalError> {
    select_all()?;
    let nodes = generate_json_from_selected_nodes(editor, get_selection().as_ref())?.nodes;
    let nodes: Vec<Value> = nodes.into_iter().map(Value::from).collect();
    generate_nodes_from_serialized_nodes(&nodes)
}
