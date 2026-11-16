//! Ports of
//! `packages/lexical-selection/src/__tests__/unit/LexicalComputedStyleSlots.test.ts`.
//!
//! Upstream mounts the editor on a jsdom element, whose `getComputedStyle`
//! gives a style for every rendered element; [`JsdomStyledRoot`] reports
//! one for every node, as Swift's `StyledRootElement` does.

#![cfg(feature = "extension")]

use crate::extension::build_editor_from_extensions;
use crate::selection::{ComputedStyle, get_computed_style_for_parent, is_parent_rtl};
use crate::upstream_tests::support::{
    TestShadowRootNode, create_test_shadow_root_node, update, update_with,
};
use crate::{
    LexicalError, LexicalExtension, NodeClass, RootElement, RootElementRef, create_paragraph_node,
    create_text_node, define_extension, get_node_by_key, get_root, set_slot,
};
use std::rc::Rc;

/// A root element whose rendered elements all have jsdom's default
/// computed style.
struct JsdomStyledRoot;

impl RootElement for JsdomStyledRoot {
    fn computed_style(&self, _key: &str) -> Option<ComputedStyle> {
        Some(ComputedStyle::default())
    }
}

/// `runInEditor($build, $check)`: `build` runs in its own update, so the
/// nodes are reconciled before `check` reads their computed styles.
fn run_in_editor<T: 'static>(
    build: impl FnOnce() -> Result<T, LexicalError> + 'static,
    check: impl FnOnce(T) -> Result<(), LexicalError> + 'static,
) {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        nodes: vec![NodeClass::of::<TestShadowRootNode>()],
        ..LexicalExtension::<(), (), ()>::new("@computed-style-slots-test", ())
    })
    .into()])
    .unwrap();
    let keys = update_with(&editor, build);
    editor
        .set_root_element(Some(RootElementRef(Rc::new(JsdomStyledRoot))))
        .unwrap();
    update(&editor, move || check(keys));
    editor.dispose();
}

/// A paragraph in the `title` slot of a paragraph at the root; its key.
fn bare_block_slot_value() -> Result<String, LexicalError> {
    let host = create_paragraph_node();
    get_root().append(std::slice::from_ref(&host))?;
    let slot_para = create_paragraph_node();
    set_slot(&host, "title", &slot_para)?;
    Ok(slot_para.get_key().to_owned())
}

// upstream: lexical-selection/src/__tests__/unit/LexicalComputedStyleSlots.test.ts::computed styles and named slots > $getComputedStyleForParent returns null for a bare-block slot value
#[test]
fn get_computed_style_for_parent_returns_null_for_a_bare_block_slot_value() {
    run_in_editor(bare_block_slot_value, |slot_para_key| {
        let slot_para = get_node_by_key(&slot_para_key).unwrap();
        assert_eq!(slot_para.get_type(), "paragraph");
        assert_eq!(get_computed_style_for_parent(&slot_para), None);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalComputedStyleSlots.test.ts::computed styles and named slots > $isParentRTL is false for the anchor inside an empty bare-block slot value
#[test]
fn is_parent_rtl_is_false_for_the_anchor_inside_an_empty_bare_block_slot_value() {
    run_in_editor(bare_block_slot_value, |slot_para_key| {
        let slot_para = get_node_by_key(&slot_para_key).unwrap();
        assert_eq!(slot_para.get_type(), "paragraph");
        let selection = slot_para.select(None, None)?;
        let anchor = selection.point(crate::Which::Anchor).unwrap().get_node()?;
        assert!(!is_parent_rtl(&anchor));
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalComputedStyleSlots.test.ts::computed styles and named slots > $getComputedStyleForParent still resolves the parent inside a slot
#[test]
fn get_computed_style_for_parent_still_resolves_the_parent_inside_a_slot() {
    run_in_editor(
        || {
            let host = create_paragraph_node();
            get_root().append(std::slice::from_ref(&host))?;
            let slot_root = create_test_shadow_root_node();
            let slot_para = create_paragraph_node();
            slot_para.append(&[create_text_node("title")])?;
            slot_root.append(std::slice::from_ref(&slot_para))?;
            set_slot(&host, "title", &slot_root)?;
            Ok((
                slot_para.get_key().to_owned(),
                slot_root.get_key().to_owned(),
            ))
        },
        |(slot_para_key, slot_root_key)| {
            let slot_para = get_node_by_key(&slot_para_key).unwrap();
            assert_eq!(slot_para.get_type(), "paragraph");
            // The paragraph is inside the slot but is not the slot value, so
            // it has a parent and resolution is unchanged.
            assert_eq!(slot_para.get_parent().unwrap().get_key(), slot_root_key);
            assert!(get_computed_style_for_parent(&slot_para).is_some());
            Ok(())
        },
    );
}
