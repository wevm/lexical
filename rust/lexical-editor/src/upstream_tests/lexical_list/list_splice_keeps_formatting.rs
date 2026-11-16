//! Ports of `packages/lexical-list/src/__tests__/unit/ListSpliceKeepsFormatting.test.ts`.

use super::support::{build, list, text_item, update_discrete};
use crate::extension::LexicalEditorWithDispose;
use crate::link::{LinkAttributes, create_link_node, is_link_node, link_extension};
use crate::list::{ListType, is_list_item_node, is_list_node, list_extension};
use crate::{NodeRef, create_paragraph_node, create_text_node, get_root};

fn build_editor() -> LexicalEditorWithDispose {
    build(
        "list-splice-host",
        vec![list_extension().into(), link_extension().into()],
    )
}

fn spliced_item_children() -> Vec<NodeRef> {
    let list = get_root().get_first_child().unwrap();
    assert!(is_list_node(&list), "expected a ListNode at the root");
    let item = list.get_child_at_index(1).unwrap();
    assert!(is_list_item_node(&item), "expected a ListItemNode");
    item.get_children()
}

// upstream: lexical-list/src/__tests__/unit/ListSpliceKeepsFormatting.test.ts::ListNode.splice converting a block into a list item > keeps the text formats of the block it unwraps
#[test]
fn keeps_the_text_formats_of_the_block_it_unwraps() {
    let editor = build_editor();
    update_discrete(&editor, || {
        let list = list(ListType::Bullet, &[text_item("A"), text_item("C")]);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&list))?;
        let paragraph = create_paragraph_node();
        let bold = create_text_node("bold");
        bold.toggle_format("bold");
        let styled = create_text_node("styled");
        styled.set_style("color: red;");
        paragraph.append(&[bold, styled])?;
        list.splice(1, 0, &[paragraph])
    });
    editor
        .read(|| {
            let children = spliced_item_children();
            let texts: Vec<String> = children.iter().map(NodeRef::get_text_content).collect();
            assert_eq!(texts, ["bold", "styled"]);
            assert!(children[0].is_text(), "expected a TextNode");
            assert!(children[1].is_text(), "expected a TextNode");
            assert!(children[0].has_format("bold"));
            assert_eq!(children[1].text_data().get_style(), "color: red;");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/ListSpliceKeepsFormatting.test.ts::ListNode.splice converting a block into a list item > keeps an inline node of the block it unwraps
#[test]
fn keeps_an_inline_node_of_the_block_it_unwraps() {
    let editor = build_editor();
    update_discrete(&editor, || {
        let list = list(ListType::Bullet, &[text_item("A"), text_item("C")]);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&list))?;
        let paragraph = create_paragraph_node();
        let link = create_link_node("https://lexical.dev/", &LinkAttributes::default());
        link.append(&[create_text_node("link")])?;
        paragraph.append(&[link])?;
        list.splice(1, 0, &[paragraph])
    });
    editor
        .read(|| {
            let children = spliced_item_children();
            assert_eq!(children.len(), 1);
            assert!(
                is_link_node(&children[0]),
                "expected the LinkNode to survive"
            );
            assert_eq!(
                children[0].get_url().as_deref(),
                Some("https://lexical.dev/")
            );
            assert_eq!(children[0].get_text_content(), "link");
        })
        .unwrap();
    editor.dispose();
}
