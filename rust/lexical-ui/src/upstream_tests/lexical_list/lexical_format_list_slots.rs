//! Ports of `packages/lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts`.
//!
//! A named-slot value has no `__parent` (its up-link is `__slotHost`), so it
//! is not eligible for generic list conversion: the slot assignment is
//! managed by the node or extension that owns the slot (`$setBlocksType` has
//! the same rule).

use super::support::{discrete, list};
use crate::extension::build_editor_from_extensions;
use crate::list::{
    ListItemNode, ListNode, ListType, create_list_item_node, insert_list, is_list_node, remove_list,
};
use crate::{
    LexicalError, LexicalExtension, NodeClass, NodeRef, create_paragraph_node, create_text_node,
    define_extension, get_root, get_slot, set_slot,
};

fn run_in_editor(f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        nodes: vec![NodeClass::of::<ListNode>(), NodeClass::of::<ListItemNode>()],
        ..LexicalExtension::<(), (), ()>::new("@formatList-slots-test", ())
    })
    .into()])
    .unwrap();
    editor.update(f, discrete()).unwrap();
    editor.dispose();
}

/// `{host, list, text}`: a paragraph holding a bullet list in its `items`
/// slot, whose one item holds `item`.
fn create_host_with_slotted_list() -> Result<(NodeRef, NodeRef, NodeRef), LexicalError> {
    let host = create_paragraph_node();
    get_root().append(std::slice::from_ref(&host))?;
    let text = create_text_node("item");
    let list = list(
        ListType::Bullet,
        &[super::support::item(std::slice::from_ref(&text))],
    );
    set_slot(&host, "items", &list)?;
    Ok((host, list, text))
}

fn is_paragraph(node: &NodeRef) -> bool {
    node.get_type() == "paragraph"
}

// upstream: lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$insertList and named slots > no-ops on a non-empty bare-block slot value
#[test]
fn insert_list_no_ops_on_a_non_empty_bare_block_slot_value() {
    run_in_editor(|| {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let text = create_text_node("title text");
        let slot_para = create_paragraph_node();
        slot_para.append(std::slice::from_ref(&text))?;
        set_slot(&host, "title", &slot_para)?;
        text.select(Some(0), Some(0))?;

        insert_list(ListType::Bullet)?;

        let value = get_slot(&host, "title").expect("a slot value");
        assert_eq!(value.get_key(), slot_para.get_key());
        assert!(is_paragraph(&value));
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$insertList and named slots > no-ops on an empty bare-block slot value
#[test]
fn insert_list_no_ops_on_an_empty_bare_block_slot_value() {
    run_in_editor(|| {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let slot_para = create_paragraph_node();
        set_slot(&host, "title", &slot_para)?;
        slot_para.select(None, None)?;

        insert_list(ListType::Bullet)?;

        let value = get_slot(&host, "title").expect("a slot value");
        assert_eq!(value.get_key(), slot_para.get_key());
        assert!(is_paragraph(&value));
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$insertList and named slots > does not change the type of a slotted list
#[test]
fn insert_list_does_not_change_the_type_of_a_slotted_list() {
    run_in_editor(|| {
        let (host, list, text) = create_host_with_slotted_list()?;
        text.select(Some(0), Some(0))?;

        insert_list(ListType::Number)?;

        let value = get_slot(&host, "items").expect("a slot value");
        assert_eq!(value.get_key(), list.get_key());
        assert!(is_list_node(&value));
        assert_eq!(list.get_list_type(), Some(ListType::Bullet));
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$insertList and named slots > does not change the type of a slotted list from an empty item
#[test]
fn insert_list_does_not_change_the_type_of_a_slotted_list_from_an_empty_item() {
    run_in_editor(|| {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let item = create_list_item_node(None);
        let list = list(ListType::Bullet, std::slice::from_ref(&item));
        set_slot(&host, "items", &list)?;
        item.select(None, None)?;

        insert_list(ListType::Number)?;

        let value = get_slot(&host, "items").expect("a slot value");
        assert_eq!(value.get_key(), list.get_key());
        assert_eq!(list.get_list_type(), Some(ListType::Bullet));
        assert!(item.is_attached());
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$removeList and named slots > no-ops on a slotted list
#[test]
fn remove_list_no_ops_on_a_slotted_list() {
    run_in_editor(|| {
        let (host, list, text) = create_host_with_slotted_list()?;
        text.select(Some(0), Some(0))?;

        remove_list()?;

        let value = get_slot(&host, "items").expect("a slot value");
        assert_eq!(value.get_key(), list.get_key());
        assert!(is_list_node(&value));
        assert_eq!(list.get_text_content(), "item");
        Ok(())
    });
}
