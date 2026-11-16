//! Ports of `packages/lexical-list/src/__tests__/unit/Issue7032Repro.test.ts`.

use super::support::{build, text_item, update_discrete};
use crate::extension::LexicalEditorWithDispose;
use crate::list::{ListType, create_list_node, is_list_item_node, is_list_node, list_extension};
use crate::{NodeRef, create_paragraph_node, create_text_node, get_root};

fn build_editor() -> LexicalEditorWithDispose {
    build("issue-7032-host", vec![list_extension().into()])
}

fn list_at(index: usize) -> NodeRef {
    let node = get_root().get_child_at_index(index);
    let node = node.filter(is_list_node);
    node.unwrap_or_else(|| panic!("expected a ListNode at root index {index}"))
}

fn item_values(list: &NodeRef) -> Vec<f64> {
    list.get_children()
        .iter()
        .filter(|n| is_list_item_node(n))
        .map(|item| item.get_list_item_value().unwrap())
        .collect()
}

fn set_list(
    list_type: ListType,
    start: f64,
    texts: &'static [&'static str],
) -> impl FnOnce() -> Result<(), crate::LexicalError> {
    move || {
        let list = create_list_node(list_type, start);
        for text in texts {
            list.append(&[text_item(text)])?;
        }
        get_root().clear()?;
        get_root().append(&[list])
    }
}

fn break_paragraph() -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node("break")]).unwrap();
    paragraph
}

// upstream: lexical-list/src/__tests__/unit/Issue7032Repro.test.ts::Issue #7032: splitting a numbered list restarts its numbering > inserting a block between list items continues the numbering
#[test]
fn inserting_a_block_between_list_items_continues_the_numbering() {
    let editor = build_editor();
    update_discrete(&editor, set_list(ListType::Number, 1.0, &["a", "b", "c"]));
    update_discrete(&editor, || {
        let first = list_at(0).get_first_child().unwrap();
        assert!(is_list_item_node(&first), "expected a ListItemNode");
        // Inserting a non-list node between two items splits the list around
        // it — the page-break case from issue #7032.
        first.insert_after(&break_paragraph(), true)?;
        Ok(())
    });
    editor
        .read(|| {
            let types: Vec<&str> = get_root()
                .get_children()
                .iter()
                .map(NodeRef::get_type)
                .collect();
            assert_eq!(types, ["list", "paragraph", "list"]);
            assert_eq!(item_values(&list_at(0)), [1.0]);
            let tail = list_at(2);
            assert_eq!(tail.get_list_start(), Some(2.0));
            assert_eq!(item_values(&tail), [2.0, 3.0]);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/Issue7032Repro.test.ts::Issue #7032: splitting a numbered list restarts its numbering > a list that does not start at 1 continues from the split point
#[test]
fn a_list_that_does_not_start_at_1_continues_from_the_split_point() {
    let editor = build_editor();
    update_discrete(&editor, set_list(ListType::Number, 5.0, &["a", "b", "c"]));
    update_discrete(&editor, || {
        let second = list_at(0).get_children()[1].clone();
        assert!(is_list_item_node(&second), "expected a ListItemNode");
        second.insert_after(&break_paragraph(), true)?;
        Ok(())
    });
    editor
        .read(|| {
            assert_eq!(item_values(&list_at(0)), [5.0, 6.0]);
            assert_eq!(list_at(2).get_list_start(), Some(7.0));
            assert_eq!(item_values(&list_at(2)), [7.0]);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/Issue7032Repro.test.ts::Issue #7032: splitting a numbered list restarts its numbering > a bullet list keeps its start when it is split
#[test]
fn a_bullet_list_keeps_its_start_when_it_is_split() {
    let editor = build_editor();
    update_discrete(&editor, set_list(ListType::Bullet, 1.0, &["a", "b"]));
    update_discrete(&editor, || {
        let first = list_at(0).get_first_child().unwrap();
        assert!(is_list_item_node(&first), "expected a ListItemNode");
        first.insert_after(&break_paragraph(), true)?;
        Ok(())
    });
    editor
        .read(|| assert_eq!(list_at(2).get_list_start(), Some(1.0)))
        .unwrap();
    editor.dispose();
}
