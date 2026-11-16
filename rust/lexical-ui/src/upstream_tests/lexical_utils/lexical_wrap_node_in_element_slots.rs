//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/LexicalWrapNodeInElementSlots.test.ts`.

#![cfg(feature = "extension")]

use crate::extension::build_editor_from_extensions;
use crate::upstream_tests::support::{TestShadowRootNode, create_test_shadow_root_node};
use crate::utils::wrap_node_in_element;
use crate::{
    InitialEditorStateType, LexicalExtension, NodeClass, UpdateOptions, create_paragraph_node,
    create_text_node, define_extension, get_root, get_slot, get_slot_host, get_slot_names,
    set_slot,
};
use std::rc::Rc;

// upstream: lexical-utils/src/__tests__/unit/LexicalWrapNodeInElementSlots.test.ts::$wrapNodeInElement and named slots > wrapping a slot host keeps its slots on the host
#[test]
fn wrapping_a_slot_host_keeps_its_slots_on_the_host() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let host = create_paragraph_node();
            let slot = create_test_shadow_root_node();
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("Title")])?;
            slot.append(&[paragraph])?;
            get_root().append(std::slice::from_ref(&host))?;
            set_slot(&host, "title", &slot)?;
            Ok(())
        }))),
        nodes: vec![NodeClass::of::<TestShadowRootNode>()],
        ..LexicalExtension::<(), (), ()>::new("[wrap-slots]", ())
    })
    .into()])
    .unwrap();

    editor
        .update(
            || {
                let host = get_root().get_first_child_or_throw()?;
                let wrapper = wrap_node_in_element(&host, create_test_shadow_root_node)?;
                // The wrapped host keeps its slot; the wrapper gains none.
                assert!(wrapper.get_first_child().unwrap() == host);
                assert_eq!(get_slot_names(&wrapper), Vec::<String>::new());
                assert!(get_slot(&host, "title").is_some());
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();

    editor
        .read(|| {
            let wrapper = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(wrapper.get_type(), "test_shadow_root");
            let host = wrapper.get_first_child_or_throw().unwrap();
            assert_eq!(host.get_type(), "paragraph");
            assert!(host.is_attached());
            let slot = get_slot(&host, "title").expect("the slot");
            assert_eq!(slot.get_text_content(), "Title");
            assert!(get_slot_host(&slot).unwrap() == host);
        })
        .unwrap();
}
