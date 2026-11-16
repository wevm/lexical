//! Ports of `packages/lexical-list/src/__tests__/unit/InsertListKeepsListState.test.ts`.

use super::support::{build, text_item, update_discrete};
use crate::extension::LexicalEditorWithDispose;
use crate::list::{ListType, create_list_item_node, create_list_node, insert_list, list_extension};
use crate::{Direction, NodeRef, get_root};

fn build_editor() -> LexicalEditorWithDispose {
    build("insert-list-state-host", vec![list_extension().into()])
}

fn root_list() -> NodeRef {
    let node = get_root().get_first_child().unwrap();
    assert!(
        crate::list::is_list_node(&node),
        "expected a ListNode at the root"
    );
    node
}

// upstream: lexical-list/src/__tests__/unit/InsertListKeepsListState.test.ts::$insertList keeps the replaced list state > converting a populated list keeps its direction, format and start
#[test]
fn converting_a_populated_list_keeps_its_direction_format_and_start() {
    let editor = build_editor();
    update_discrete(&editor, || {
        let list = create_list_node(ListType::Number, 4.0);
        list.set_direction(Some(Direction::Rtl));
        list.set_element_format("center");
        list.set_element_style("color: red;");
        list.append(&[text_item("a"), text_item("b")])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&list))?;
        list.get_first_child().unwrap().select_end()?;
        Ok(())
    });
    update_discrete(&editor, || insert_list(ListType::Bullet));
    editor
        .read(|| {
            let list = root_list();
            let data = list.element_data();
            assert_eq!(list.get_list_type(), Some(ListType::Bullet));
            assert_eq!(data.get_direction(), Some(Direction::Rtl));
            assert_eq!(data.get_format_type(), "center");
            assert_eq!(data.get_style(), "color: red;");
            assert_eq!(list.get_list_start(), Some(4.0));
            let texts: Vec<String> = list
                .get_children()
                .iter()
                .map(NodeRef::get_text_content)
                .collect();
            assert_eq!(texts, ["a", "b"]);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/InsertListKeepsListState.test.ts::$insertList keeps the replaced list state > converting from an empty list item keeps the list state
#[test]
fn converting_from_an_empty_list_item_keeps_the_list_state() {
    let editor = build_editor();
    update_discrete(&editor, || {
        let list = create_list_node(ListType::Bullet, 1.0);
        list.set_direction(Some(Direction::Rtl));
        list.set_element_format("right");
        list.append(&[create_list_item_node(None)])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&list))?;
        list.get_first_child().unwrap().select_end()?;
        Ok(())
    });
    update_discrete(&editor, || insert_list(ListType::Number));
    editor
        .read(|| {
            let list = root_list();
            let data = list.element_data();
            assert_eq!(list.get_list_type(), Some(ListType::Number));
            assert_eq!(data.get_direction(), Some(Direction::Rtl));
            assert_eq!(data.get_format_type(), "right");
        })
        .unwrap();
    editor.dispose();
}
