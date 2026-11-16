//! Ports of `packages/lexical-html/src/__tests__/unit/SlotNodeSelectionExport.test.ts`.
//!
//! `PlainShadowRootNode` is the shared [`TestShadowRootNode`]; only its
//! type name differs.

#![cfg(feature = "extension")]

use super::super::support::{
    TestShadowRootNode, create_test_shadow_root_node, update, update_with,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::generate_html_from_nodes;
use crate::{
    InitialEditorStateType, LexicalExtension, NodeClass, PointType, Which, create_node_selection,
    create_paragraph_node, create_range_selection, create_text_node, define_extension, get_root,
    get_selection, set_selection, set_slot,
};

const TARGET_TEXT: &str = "SlotNodeTarget";

/// `buildEditorWithSlottedParagraph()`: the editor and the keys of the
/// slotted paragraph and its text.
fn build_editor_with_slotted_paragraph() -> (LexicalEditorWithDispose, String, String) {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        nodes: vec![NodeClass::of::<TestShadowRootNode>()],
        ..LexicalExtension::<(), (), ()>::new("[slot-node-selection-html]", ())
    })
    .into()])
    .unwrap();
    let (paragraph, text) = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = create_test_shadow_root_node();
        let text = create_text_node(TARGET_TEXT);
        let inner_paragraph = create_paragraph_node();
        inner_paragraph.append(std::slice::from_ref(&text))?;
        slot.append(std::slice::from_ref(&inner_paragraph))?;
        set_slot(&host, "media", &slot)?;
        get_root().append(&[host])?;
        Ok((
            inner_paragraph.get_key().to_owned(),
            text.get_key().to_owned(),
        ))
    });
    (editor, paragraph, text)
}

fn selection_html(editor: &LexicalEditorWithDispose) -> String {
    editor
        .read(|| generate_html_from_nodes(editor, get_selection().as_ref()))
        .unwrap()
        .unwrap()
}

// upstream: lexical-html/src/__tests__/unit/SlotNodeSelectionExport.test.ts::$generateHtmlFromNodes slot frame redirect > a NodeSelection inside a slot exports the node, not empty HTML
#[test]
fn a_node_selection_inside_a_slot_exports_the_node() {
    let (editor, paragraph, _) = build_editor_with_slotted_paragraph();
    update(&editor, move || {
        assert!(crate::get_node_by_key(&paragraph).is_some());
        let node_selection = create_node_selection();
        node_selection.add(&paragraph);
        set_selection(Some(node_selection));
        Ok(())
    });
    assert!(selection_html(&editor).contains(TARGET_TEXT));
}

// upstream: lexical-html/src/__tests__/unit/SlotNodeSelectionExport.test.ts::$generateHtmlFromNodes slot frame redirect > a RangeSelection inside a slot still exports the node
#[test]
fn a_range_selection_inside_a_slot_still_exports_the_node() {
    let (editor, _, text) = build_editor_with_slotted_paragraph();
    update(&editor, move || {
        let range_selection = create_range_selection();
        range_selection.point_set(Which::Anchor, &text, 0, PointType::Text)?;
        range_selection.point_set(
            Which::Focus,
            &text,
            crate::utf16::len(TARGET_TEXT),
            PointType::Text,
        )?;
        set_selection(Some(range_selection));
        Ok(())
    });
    assert!(selection_html(&editor).contains(TARGET_TEXT));
}
