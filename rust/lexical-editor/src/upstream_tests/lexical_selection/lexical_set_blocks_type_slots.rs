//! Ports of
//! `packages/lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use crate::extension::build_editor_from_extensions;
use crate::rich_text::{
    HeadingNode, HeadingTagType, QuoteNode, create_heading_node, create_quote_node, is_heading_node,
};
use crate::selection::{set_blocks_type, wrap_nodes};
use crate::upstream_tests::support::{TestShadowRootNode, create_test_shadow_root_node, update};
use crate::{
    LexicalError, LexicalExtension, NodeClass, create_paragraph_node, create_text_node,
    define_extension, get_root, get_slot, get_slot_names, set_slot,
};

/// `runInEditor(fn)`.
fn run_in_editor(f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        nodes: vec![
            NodeClass::of::<TestShadowRootNode>(),
            NodeClass::of::<HeadingNode>(),
            NodeClass::of::<QuoteNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("@setBlocksType-slots-test", ())
    })
    .into()])
    .unwrap();
    update(&editor, f);
    editor.dispose();
}

fn h2() -> crate::NodeRef {
    create_heading_node(HeadingTagType::H2)
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$setBlocksType and named slots > no-ops on a bare-block slot value
#[test]
fn set_blocks_type_no_ops_on_a_bare_block_slot_value() {
    run_in_editor(|| {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let text = create_text_node("title text");
        let slot_para = create_paragraph_node();
        slot_para.append(std::slice::from_ref(&text))?;
        set_slot(&host, "title", &slot_para)?;
        let selection = text.select(Some(0), Some(0))?;

        set_blocks_type(Some(&selection), h2, None)?;

        let value = get_slot(&host, "title").expect("a slot value");
        assert_eq!(value, slot_para);
        assert_eq!(value.get_type(), "paragraph");
        assert_eq!(value.get_text_content(), "title text");
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$setBlocksType and named slots > no-ops on an empty bare-block slot value
#[test]
fn set_blocks_type_no_ops_on_an_empty_bare_block_slot_value() {
    run_in_editor(|| {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let slot_para = create_paragraph_node();
        set_slot(&host, "title", &slot_para)?;
        let selection = slot_para.select(None, None)?;

        set_blocks_type(Some(&selection), h2, None)?;

        let value = get_slot(&host, "title").expect("a slot value");
        assert_eq!(value, slot_para);
        assert_eq!(value.get_type(), "paragraph");
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$setBlocksType and named slots > converting a slot host does not carry its slots onto the new block
#[test]
fn converting_a_slot_host_does_not_carry_its_slots_onto_the_new_block() {
    run_in_editor(|| {
        let host = create_paragraph_node();
        let body_text = create_text_node("body");
        host.append(std::slice::from_ref(&body_text))?;
        get_root().append(std::slice::from_ref(&host))?;
        let slot_para = create_paragraph_node();
        slot_para.append(&[create_text_node("title")])?;
        set_slot(&host, "title", &slot_para)?;
        let selection = body_text.select(Some(0), Some(0))?;

        set_blocks_type(Some(&selection), h2, None)?;

        let heading = get_root().get_last_child().expect("a last child");
        assert!(is_heading_node(&heading));
        assert_eq!(heading.get_text_content(), "body");
        // The slots did not transfer; they remain on the detached host.
        assert!(get_slot_names(&heading).is_empty());
        assert!(!host.is_attached());
        assert_eq!(get_slot(&host, "title"), Some(slot_para));
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$setBlocksType and named slots > converts blocks inside a shadow-root slot value normally
#[test]
fn converts_blocks_inside_a_shadow_root_slot_value_normally() {
    run_in_editor(|| {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let container = create_test_shadow_root_node();
        let text = create_text_node("quoted");
        let inner_para = create_paragraph_node();
        inner_para.append(std::slice::from_ref(&text))?;
        container.append(&[inner_para])?;
        set_slot(&host, "quote", &container)?;
        let selection = text.select(Some(0), Some(0))?;

        set_blocks_type(Some(&selection), h2, None)?;

        // The container keeps its slot; the block inside it converted.
        let value = get_slot(&host, "quote").expect("a slot value");
        assert_eq!(value, container);
        let child = container.get_first_child().expect("a first child");
        assert!(is_heading_node(&child));
        assert_eq!(child.get_text_content(), "quoted");
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$wrapNodes and named slots > no-ops on a bare-block slot value
#[test]
fn wrap_nodes_no_ops_on_a_bare_block_slot_value() {
    run_in_editor(|| {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let text = create_text_node("title text");
        let slot_para = create_paragraph_node();
        slot_para.append(std::slice::from_ref(&text))?;
        set_slot(&host, "title", &slot_para)?;
        let selection = text.select(Some(0), Some(0))?;

        wrap_nodes(&selection, || create_quote_node(false), None)?;

        let value = get_slot(&host, "title").expect("a slot value");
        assert_eq!(value, slot_para);
        assert_eq!(value.get_type(), "paragraph");
        assert_eq!(value.get_text_content(), "title text");
        Ok(())
    });
}
