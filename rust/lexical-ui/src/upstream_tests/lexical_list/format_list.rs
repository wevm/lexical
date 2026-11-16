//! Ports of `packages/lexical-list/src/__tests__/unit/formatList.test.ts`,
//! less the tests of `ExtendedTestListNode` and `ExtendedTestListItemNode`,
//! subclasses of `ListNode` and `ListItemNode` (rust/parity/skips.json), and
//! the one that needs `@lexical/table`.

use super::support::{build, discrete, item, list, text_item};
use crate::extension::LexicalEditorWithDispose;
use crate::list::{
    ListType, RegisterListOptions, create_list_item_node, handle_indent, insert_list,
    is_list_item_node, is_list_node, list_extension, register_list,
};
use crate::nodes::line_break::create_line_break_node;
use crate::upstream_tests::support::{create_test_decorator_node, initialize_unit_test, update};
use crate::{
    CreateEditorArgs, Editor, INSERT_PARAGRAPH_COMMAND, LexicalError, NodeRef, PointType,
    create_paragraph_node, create_text_node, get_node_by_key, get_root, get_selection,
    nodes_of_type, select_all,
};
use std::cell::RefCell;
use std::rc::Rc;

fn editor() -> Editor {
    initialize_unit_test(CreateEditorArgs::default())
}

/// `$createNestedListItem(listType, ...texts)`: an item holding a nested
/// list of `list_type`, one item per text.
fn create_nested_list_item(list_type: ListType, texts: &[&str]) -> NodeRef {
    let items: Vec<NodeRef> = texts.iter().map(|t| text_item(t)).collect();
    item(&[list(list_type, &items)])
}

/// `$getNestedList(node)`: the nested list an item holds.
fn get_nested_list(node: &NodeRef) -> NodeRef {
    assert!(is_list_item_node(node));
    let list = node.get_first_child().unwrap();
    assert!(is_list_node(&list));
    list
}

fn texts(list: &NodeRef) -> Vec<String> {
    list.get_children()
        .iter()
        .map(NodeRef::get_text_content)
        .collect()
}

fn read(editor: &Editor, f: impl FnOnce()) {
    editor.read(f).unwrap();
}

// insertList

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::insertList > inserting with empty root selection
#[test]
fn inserting_with_empty_root_selection() {
    let editor = editor();
    update(&editor, || get_root().select(None, None).map(|_| ()));
    update(&editor, || insert_list(ListType::Number));
    read(&editor, || {
        let root = get_root();
        assert_eq!(root.get_children_size(), 1);
        assert!(is_list_node(&root.get_first_child_or_throw().unwrap()));
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::insertList > inserting in root selection with existing child
#[test]
fn inserting_in_root_selection_with_existing_child() {
    let editor = editor();
    update(&editor, || {
        get_root().select(None, None)?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("hello")])?;
        get_root().append(&[paragraph])
    });
    update(&editor, || insert_list(ListType::Number));
    read(&editor, || {
        let root = get_root();
        assert_eq!(root.get_children_size(), 1);
        assert!(is_list_node(&root.get_first_child_or_throw().unwrap()));
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::insertList > formatting empty list items
#[test]
fn formatting_empty_list_items() {
    let editor = editor();
    update(&editor, || {
        get_root().append(&[list(
            ListType::Bullet,
            &[
                text_item("Level 1"),
                item(&[list(ListType::Bullet, &[create_list_item_node(None)])]),
            ],
        )])
    });
    update(&editor, || {
        select_all()?;
        insert_list(ListType::Number)
    });
    read(&editor, || {
        let lists: Vec<NodeRef> = nodes_of_type("list")
            .into_iter()
            .filter(|n| n.get_list_type() == Some(ListType::Number))
            .collect();
        assert_eq!(lists.len(), 2);
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::insertList > preserves element-anchored selection when converting paragraph with linebreak to list
#[test]
fn preserves_element_anchored_selection_when_converting_paragraph_with_linebreak_to_list() {
    let editor = editor();
    // Create a paragraph with text followed by a linebreak node and select
    // the paragraph
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("Item 1"), create_line_break_node()])?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select(None, None)?;
        Ok(())
    });
    // Convert to list
    update(&editor, || insert_list(ListType::Bullet));
    // Verify the list was created and selection is correctly preserved
    read(&editor, || {
        let selection = get_selection();
        let list = get_root().get_first_child_or_throw().unwrap();
        assert!(is_list_node(&list));
        let list_item = list.get_first_child_or_throw().unwrap();
        assert!(is_list_item_node(&list_item));
        // Verify selection is valid and anchored to the listItem, not the list
        if let Some(selection) = selection.filter(crate::SelectionRef::is_range) {
            for which in [crate::Which::Anchor, crate::Which::Focus] {
                let point = selection.point(which).unwrap();
                assert_eq!(point.key, list_item.get_key());
                assert_eq!(point.offset, 2);
                assert_eq!(point.point_type, PointType::Element);
            }
        }
    });
}

// $handleListInsertParagraph

fn list_editor(rich_text: bool) -> Editor {
    let editor = editor();
    if rich_text {
        let _ = crate::rich_text::register_rich_text(&editor);
    }
    let _ = register_list(&editor, RegisterListOptions::default());
    editor
}

fn dispatch_insert_paragraph() {
    crate::get_editor()
        .dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())
        .unwrap();
}

/// The key of a node made in an update, for reading after it.
fn key_cell() -> Rc<RefCell<String>> {
    Rc::new(RefCell::new(String::new()))
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::$handleListInsertParagraph > exits list when list item is completely empty
#[test]
fn exits_list_when_list_item_is_completely_empty() {
    let editor = list_editor(false);
    let key = key_cell();
    let k = key.clone();
    update(&editor, move || {
        let empty = create_list_item_node(None);
        *k.borrow_mut() = empty.get_key().to_string();
        get_root().append(&[list(ListType::Bullet, &[text_item("item"), empty.clone()])])?;
        empty.select(None, None)?;
        dispatch_insert_paragraph();
        Ok(())
    });
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 2);
        assert!(is_list_node(&children[0]));
        assert_eq!(children[1].get_type(), "paragraph");
        assert_eq!(children[0].get_children_size(), 1);
        assert!(get_node_by_key(&key.borrow()).is_none());
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::$handleListInsertParagraph > exits list when list item contains only whitespace
#[test]
fn exits_list_when_list_item_contains_only_whitespace() {
    let editor = list_editor(false);
    let key = key_cell();
    let k = key.clone();
    update(&editor, move || {
        let whitespace = create_text_node(" ");
        let item = item(std::slice::from_ref(&whitespace));
        *k.borrow_mut() = item.get_key().to_string();
        get_root().append(&[list(ListType::Bullet, &[text_item("item"), item])])?;
        whitespace.select(None, None)?;
        dispatch_insert_paragraph();
        Ok(())
    });
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 2);
        assert!(is_list_node(&children[0]));
        assert_eq!(children[1].get_type(), "paragraph");
        assert_eq!(children[0].get_children_size(), 1);
        assert!(get_node_by_key(&key.borrow()).is_none());
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::$handleListInsertParagraph > extends list when list item contains non-whitespace content
#[test]
fn extends_list_when_list_item_contains_non_whitespace_content() {
    let editor = list_editor(true);
    update(&editor, || {
        let text = create_text_node("item 2");
        get_root().append(&[list(
            ListType::Bullet,
            &[text_item("item 1"), item(std::slice::from_ref(&text))],
        )])?;
        text.select_end()?;
        dispatch_insert_paragraph();
        Ok(())
    });
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 1);
        assert!(is_list_node(&children[0]));
        assert_eq!(children[0].get_children_size(), 3);
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::$handleListInsertParagraph > extends list when list item contains a decorator node
#[test]
fn extends_list_when_list_item_contains_a_decorator_node() {
    let editor = list_editor(true);
    update(&editor, || {
        let item2 = item(&[create_test_decorator_node()]);
        get_root().append(&[list(
            ListType::Bullet,
            &[text_item("item 1"), item2.clone()],
        )])?;
        item2.select(None, None)?;
        dispatch_insert_paragraph();
        Ok(())
    });
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 1);
        assert!(is_list_node(&children[0]));
        assert_eq!(children[0].get_children_size(), 3);
    });
}

// $handleIndent

/// A bullet list of an item with a nested list of `before`, the item
/// `middle`, and an item with a nested list of `after`; `$handleIndent` on
/// `middle`. Returns the root list's children.
fn indent_middle(
    root_type: ListType,
    before: (ListType, &'static [&'static str]),
    after: impl FnOnce() -> NodeRef + 'static,
) -> Editor {
    let editor = editor();
    let middle = key_cell();
    let m = middle.clone();
    update(&editor, move || {
        let middle_item = text_item("middle");
        *m.borrow_mut() = middle_item.get_key().to_string();
        get_root().append(&[list(
            root_type,
            &[
                create_nested_list_item(before.0, before.1),
                middle_item,
                after(),
            ],
        )])
    });
    update(&editor, move || {
        handle_indent(&get_node_by_key(&middle.borrow()).unwrap())
    });
    editor
}

fn root_list_children() -> Vec<NodeRef> {
    let list = get_root().get_first_child().unwrap();
    assert!(is_list_node(&list));
    list.get_children()
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::$handleIndent > merges the surrounding sublists when they are the same type
#[test]
fn merges_the_surrounding_sublists_when_they_are_the_same_type() {
    let editor = indent_middle(
        ListType::Bullet,
        (ListType::Bullet, &["first child"]),
        || create_nested_list_item(ListType::Bullet, &["second child"]),
    );
    read(&editor, || {
        let children = root_list_children();
        assert_eq!(children.len(), 1);
        let sublist = get_nested_list(&children[0]);
        assert_eq!(sublist.get_list_type(), Some(ListType::Bullet));
        assert_eq!(texts(&sublist), ["first child", "middle", "second child"]);
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::$handleIndent > does not merge the surrounding sublists of a different listType
#[test]
fn does_not_merge_the_surrounding_sublists_of_a_different_list_type() {
    let editor = indent_middle(
        ListType::Bullet,
        (ListType::Bullet, &["bullet child"]),
        || create_nested_list_item(ListType::Number, &["number one", "number two"]),
    );
    read(&editor, || {
        let children = root_list_children();
        // the numbered sublist survives with its type and its children
        let number_sublist = get_nested_list(children.last().unwrap());
        assert_eq!(number_sublist.get_list_type(), Some(ListType::Number));
        assert_eq!(texts(&number_sublist), ["number one", "number two"]);
        assert_eq!(children.len(), 2);
        // the item is still indented into the preceding sublist
        let bullet_sublist = get_nested_list(&children[0]);
        assert_eq!(bullet_sublist.get_list_type(), Some(ListType::Bullet));
        assert_eq!(texts(&bullet_sublist), ["bullet child", "middle"]);
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::$handleIndent > does not merge the surrounding sublists of a different listType, the other way round
#[test]
fn does_not_merge_the_surrounding_sublists_of_a_different_list_type_the_other_way_round() {
    let editor = indent_middle(
        ListType::Number,
        (ListType::Number, &["number one"]),
        || create_nested_list_item(ListType::Bullet, &["bullet one", "bullet two"]),
    );
    read(&editor, || {
        let children = root_list_children();
        let bullet_sublist = get_nested_list(children.last().unwrap());
        assert_eq!(bullet_sublist.get_list_type(), Some(ListType::Bullet));
        assert_eq!(texts(&bullet_sublist), ["bullet one", "bullet two"]);
        assert_eq!(children.len(), 2);
        let number_sublist = get_nested_list(&children[0]);
        assert_eq!(number_sublist.get_list_type(), Some(ListType::Number));
        assert_eq!(texts(&number_sublist), ["number one", "middle"]);
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::$handleIndent > keeps the checked state of a following check sublist
#[test]
fn keeps_the_checked_state_of_a_following_check_sublist() {
    let editor = indent_middle(
        ListType::Bullet,
        (ListType::Bullet, &["bullet child"]),
        || {
            let check_item = create_list_item_node(None);
            check_item.set_checked(Some(true));
            check_item.append(&[create_text_node("done")]).unwrap();
            item(&[list(ListType::Check, &[check_item])])
        },
    );
    read(&editor, || {
        let children = root_list_children();
        // a bullet list swallowing this one would strip checked, because
        // updateChildrenListItemValue clears it outside a check list
        let check_sublist = get_nested_list(children.last().unwrap());
        assert_eq!(check_sublist.get_list_type(), Some(ListType::Check));
        let check_item = check_sublist.get_first_child().unwrap();
        assert!(is_list_item_node(&check_item));
        assert_eq!(check_item.get_checked(), Some(true));
        assert_eq!(children.len(), 2);
    });
}

// ListItemNode.collapseAtStart

fn collapse_editor() -> LexicalEditorWithDispose {
    build("[root]", vec![list_extension().into()])
}

/// Builds a document with `make`, which returns the item to collapse, then
/// calls `collapseAtStart` with a selection at its start.
fn collapse(editor: &Editor, make: impl FnOnce() -> Result<NodeRef, LexicalError> + 'static) {
    editor
        .update(
            || {
                get_root().clear()?;
                let item = make()?;
                let selection = item.select(Some(0), Some(0))?;
                assert!(selection.is_range(), "Expected RangeSelection");
                item.collapse_at_start(&selection)?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > top-level single item converts to paragraph
#[test]
fn top_level_single_item_converts_to_paragraph() {
    let editor = collapse_editor();
    collapse(&editor, || {
        let item = text_item("hello");
        get_root().append(&[list(ListType::Bullet, std::slice::from_ref(&item))])?;
        Ok(item)
    });
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 1);
        assert_eq!(
            children[0].get_type(),
            "paragraph",
            "Expected ParagraphNode"
        );
        assert_eq!(children[0].get_text_content(), "hello");
    });
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > top-level first item extracts as paragraph, keeps remaining list
#[test]
fn top_level_first_item_extracts_as_paragraph_keeps_remaining_list() {
    let editor = collapse_editor();
    collapse(&editor, || {
        let first = text_item("first");
        get_root().append(&[list(
            ListType::Bullet,
            &[first.clone(), text_item("second")],
        )])?;
        Ok(first)
    });
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 2);
        assert_eq!(
            children[0].get_type(),
            "paragraph",
            "Expected ParagraphNode"
        );
        assert_eq!(children[0].get_text_content(), "first");
        assert!(is_list_node(&children[1]), "Expected ListNode");
        assert_eq!(children[1].get_children_size(), 1);
    });
    editor.dispose();
}

/// A bullet list of `parent` and an item holding a nested list of `nested`;
/// collapses the first nested item and returns its key.
fn collapse_nested(editor: &Editor, nested: fn() -> Vec<NodeRef>) -> String {
    let key = key_cell();
    let k = key.clone();
    collapse(editor, move || {
        let nested = nested();
        let first = nested[0].clone();
        *k.borrow_mut() = first.get_key().to_string();
        get_root().append(&[list(
            ListType::Bullet,
            &[
                text_item("parent"),
                item(&[list(ListType::Bullet, &nested)]),
            ],
        )])?;
        Ok(first)
    });
    key.take()
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > indented item outdents instead of converting to paragraph
#[test]
fn indented_item_outdents_instead_of_converting_to_paragraph() {
    let editor = collapse_editor();
    let key = collapse_nested(&editor, || vec![text_item("child")]);
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 1);
        assert!(is_list_node(&children[0]), "Expected ListNode");
        assert_eq!(children[0].get_children_size(), 2);
        let outdented = get_node_by_key(&key).unwrap();
        assert!(is_list_item_node(&outdented), "Expected ListItemNode");
        assert_eq!(outdented.get_text_content(), "child");
    });
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > indented item with siblings outdents without breaking list
#[test]
fn indented_item_with_siblings_outdents_without_breaking_list() {
    let editor = collapse_editor();
    let key = collapse_nested(&editor, || vec![text_item("child1"), text_item("child2")]);
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 1);
        assert!(is_list_node(&children[0]), "Expected ListNode");
        let outdented = get_node_by_key(&key).unwrap();
        assert!(is_list_item_node(&outdented), "Expected ListItemNode");
        assert_eq!(outdented.get_text_content(), "child1");
        assert!(
            children[0]
                .get_children()
                .iter()
                .any(|item| is_list_item_node(item) && item.get_text_content().contains("child2"))
        );
    });
    editor.dispose();
}

/// A list of `first`, the collapsed item and `third`, in a list of
/// `list_type`.
fn collapse_middle(editor: &Editor, list_type: ListType, middle: fn() -> NodeRef) {
    collapse(editor, move || {
        let item2 = middle();
        get_root().append(&[list(
            list_type,
            &[text_item("first"), item2.clone(), text_item("third")],
        )])?;
        Ok(item2)
    });
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > middle item converts to paragraph and splits list
#[test]
fn middle_item_converts_to_paragraph_and_splits_list() {
    let editor = collapse_editor();
    collapse_middle(&editor, ListType::Bullet, || text_item("second"));
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 3);
        assert!(is_list_node(&children[0]), "Expected first ListNode");
        assert_eq!(children[0].get_children_size(), 1);
        assert_eq!(children[0].get_text_content(), "first");
        assert_eq!(
            children[1].get_type(),
            "paragraph",
            "Expected ParagraphNode"
        );
        assert_eq!(children[1].get_text_content(), "second");
        assert!(is_list_node(&children[2]), "Expected second ListNode");
        assert_eq!(children[2].get_children_size(), 1);
        assert_eq!(children[2].get_text_content(), "third");
    });
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > split preserves list type
#[test]
fn split_preserves_list_type() {
    let editor = collapse_editor();
    collapse_middle(&editor, ListType::Number, || text_item("second"));
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 3);
        assert_eq!(children[0].get_list_type(), Some(ListType::Number));
        assert_eq!(children[2].get_list_type(), Some(ListType::Number));
    });
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > last item converts to paragraph without splitting
#[test]
fn last_item_converts_to_paragraph_without_splitting() {
    let editor = collapse_editor();
    collapse(&editor, || {
        let item2 = text_item("second");
        get_root().append(&[list(ListType::Bullet, &[text_item("first"), item2.clone()])])?;
        Ok(item2)
    });
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 2);
        assert!(is_list_node(&children[0]), "Expected ListNode");
        assert_eq!(children[0].get_children_size(), 1);
        assert_eq!(children[0].get_text_content(), "first");
        assert_eq!(
            children[1].get_type(),
            "paragraph",
            "Expected ParagraphNode"
        );
        assert_eq!(children[1].get_text_content(), "second");
    });
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > empty middle item converts to empty paragraph and splits list
#[test]
fn empty_middle_item_converts_to_empty_paragraph_and_splits_list() {
    let editor = collapse_editor();
    collapse_middle(&editor, ListType::Bullet, || create_list_item_node(None));
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 3);
        assert!(is_list_node(&children[0]), "Expected ListNode");
        assert_eq!(children[0].get_children_size(), 1);
        assert_eq!(
            children[0]
                .get_first_child_or_throw()
                .unwrap()
                .get_text_content(),
            "first"
        );
        assert_eq!(
            children[1].get_type(),
            "paragraph",
            "Expected ParagraphNode"
        );
        assert_eq!(children[1].get_text_content(), "");
        assert!(is_list_node(&children[2]), "Expected ListNode");
        assert_eq!(children[2].get_children_size(), 1);
        assert_eq!(
            children[2]
                .get_first_child_or_throw()
                .unwrap()
                .get_text_content(),
            "third"
        );
        let selection = get_selection().filter(crate::SelectionRef::is_range);
        assert!(
            selection
                .expect("Expected RangeSelection")
                .range()
                .unwrap()
                .is_collapsed()
        );
    });
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > empty indented item outdents
#[test]
fn empty_indented_item_outdents() {
    let editor = collapse_editor();
    let key = collapse_nested(&editor, || vec![create_list_item_node(None)]);
    read(&editor, || {
        let children = get_root().get_children();
        assert_eq!(children.len(), 1);
        assert!(is_list_node(&children[0]), "Expected ListNode");
        let outdented = get_node_by_key(&key).unwrap();
        assert!(is_list_item_node(&outdented), "Expected ListItemNode");
        assert_eq!(outdented.get_indent().unwrap(), 0.0);
    });
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/formatList.test.ts::mergeNextSiblingListIfSameType > does not merge nested sublists of a different listType
#[test]
fn does_not_merge_nested_sublists_of_a_different_list_type() {
    let editor = editor();
    editor
        .update(
            || {
                let list1 = list(
                    ListType::Bullet,
                    &[
                        text_item("a"),
                        create_nested_list_item(ListType::Number, &["one"]),
                    ],
                );
                let list2 = list(
                    ListType::Bullet,
                    &[
                        create_nested_list_item(ListType::Bullet, &["two"]),
                        text_item("b"),
                    ],
                );
                get_root().clear()?;
                get_root().append(&[list1, list2])
            },
            discrete(),
        )
        .unwrap();
    read(&editor, || {
        let list = get_root().get_children()[0].clone();
        assert!(is_list_node(&list), "Expected ListNode");
        // The two bullet lists merge, but their sublists stay separate.
        let sublist_types: Vec<Option<ListType>> = list
            .get_children()
            .iter()
            .filter(|c| is_list_item_node(c))
            .filter_map(NodeRef::get_first_child)
            .filter(is_list_node)
            .map(|s| s.get_list_type())
            .collect();
        assert_eq!(
            sublist_types,
            [Some(ListType::Number), Some(ListType::Bullet)]
        );
    });
}
