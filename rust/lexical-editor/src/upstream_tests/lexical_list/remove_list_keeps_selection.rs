//! Ports of `packages/lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts`.

use super::support::{item, list, update_discrete};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::list::{ListType, create_list_item_node, list_extension, remove_list};
use crate::upstream_tests::support::{TestDecoratorNode, create_test_decorator_node};
use crate::{
    Editor, LexicalError, LexicalExtension, NodeClass, NodeRef, PointType, Which,
    create_range_selection, create_text_node, define_extension, get_root, get_selection,
    set_selection,
};

fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![list_extension().into()],
        nodes: vec![NodeClass::of::<TestDecoratorNode>()],
        ..LexicalExtension::<(), (), ()>::new("remove-list-selection-host", ())
    })
    .into()])
    .unwrap()
}

type NodeOffset = (NodeRef, usize);

fn point_type(node: &NodeRef) -> PointType {
    if node.is_text() {
        PointType::Text
    } else {
        PointType::Element
    }
}

fn select(anchor: NodeOffset, focus: NodeOffset) -> Result<(), LexicalError> {
    let selection = create_range_selection();
    selection.point_set(
        Which::Anchor,
        anchor.0.get_key(),
        anchor.1,
        point_type(&anchor.0),
    )?;
    selection.point_set(
        Which::Focus,
        focus.0.get_key(),
        focus.1,
        point_type(&focus.0),
    )?;
    set_selection(Some(selection));
    Ok(())
}

/// `node` names the node by its text or its type, so that a failure shows
/// where a point went without looking up keys.
fn describe_point(node: &NodeRef, offset: usize, point_type: PointType) -> String {
    let name = if node.is_text() {
        node.get_text_content()
    } else {
        node.get_type().to_owned()
    };
    format!("{}:{name}:{offset}:{}", node.get_key(), point_type.as_str())
}

fn expect_selection(anchor: NodeOffset, focus: NodeOffset) {
    let selection = get_selection().filter(crate::SelectionRef::is_range);
    let selection = selection.expect("Expected a RangeSelection");
    let points: Vec<String> = [Which::Anchor, Which::Focus]
        .into_iter()
        .map(|w| {
            let p = selection.point(w).unwrap();
            describe_point(&p.get_node().unwrap(), p.offset, p.point_type)
        })
        .collect();
    assert_eq!(
        points,
        [
            describe_point(&anchor.0, anchor.1, point_type(&anchor.0)),
            describe_point(&focus.0, focus.1, point_type(&focus.0)),
        ]
    );
}

fn paragraphs() -> Vec<NodeRef> {
    get_root()
        .get_children()
        .into_iter()
        .filter(|n| n.get_type() == "paragraph")
        .collect()
}

/// A list item holding `text` and then an inline decorator.
fn create_item_ending_in_decorator(text: &str) -> NodeRef {
    item(&[create_text_node(text), create_test_decorator_node()])
}

fn remove_list_after(editor: &Editor, set_up: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    update_discrete(editor, || {
        get_root().clear()?;
        set_up()
    });
    update_discrete(editor, remove_list);
}

fn first_child(node: &NodeRef) -> NodeRef {
    node.get_first_child_or_throw().unwrap()
}

// upstream: lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a caret after an inline decorator at the end of the item
#[test]
fn a_caret_after_an_inline_decorator_at_the_end_of_the_item() {
    let editor = build_editor();
    remove_list_after(&editor, || {
        let item = create_item_ending_in_decorator("ab ");
        get_root().append(&[list(ListType::Bullet, std::slice::from_ref(&item))])?;
        select((item.clone(), 2), (item, 2))
    });
    editor
        .read(|| {
            let p = paragraphs()[0].clone();
            expect_selection((p.clone(), 2), (p, 2));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a caret between the text and an inline decorator
#[test]
fn a_caret_between_the_text_and_an_inline_decorator() {
    let editor = build_editor();
    remove_list_after(&editor, || {
        let item = create_item_ending_in_decorator("ab ");
        get_root().append(&[list(ListType::Bullet, std::slice::from_ref(&item))])?;
        select((item.clone(), 1), (item, 1))
    });
    editor
        .read(|| {
            let t = first_child(&paragraphs()[0]);
            expect_selection((t.clone(), 3), (t, 3));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a forward selection with its focus after an inline decorator
#[test]
fn a_forward_selection_with_its_focus_after_an_inline_decorator() {
    let editor = build_editor();
    remove_list_after(&editor, || {
        let item = create_item_ending_in_decorator("ab ");
        get_root().append(&[list(ListType::Bullet, std::slice::from_ref(&item))])?;
        select((first_child(&item), 1), (item, 2))
    });
    editor
        .read(|| {
            let p = paragraphs()[0].clone();
            expect_selection((first_child(&p), 1), (p, 2));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a backward selection with its anchor after an inline decorator
#[test]
fn a_backward_selection_with_its_anchor_after_an_inline_decorator() {
    let editor = build_editor();
    remove_list_after(&editor, || {
        let item = create_item_ending_in_decorator("ab ");
        get_root().append(&[list(ListType::Bullet, std::slice::from_ref(&item))])?;
        select((item.clone(), 2), (first_child(&item), 1))
    });
    editor
        .read(|| {
            let p = paragraphs()[0].clone();
            expect_selection((p.clone(), 2), (first_child(&p), 1));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a selection from one item to the next
#[test]
fn a_selection_from_one_item_to_the_next() {
    let editor = build_editor();
    remove_list_after(&editor, || {
        let first = create_item_ending_in_decorator("ab ");
        let second = create_item_ending_in_decorator("xy ");
        get_root().append(&[list(ListType::Bullet, &[first.clone(), second.clone()])])?;
        select((first, 2), (second, 2))
    });
    editor
        .read(|| {
            let ps = paragraphs();
            expect_selection((ps[0].clone(), 2), (ps[1].clone(), 2));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a caret in the text
#[test]
fn a_caret_in_the_text() {
    let editor = build_editor();
    remove_list_after(&editor, || {
        let item = create_item_ending_in_decorator("ab ");
        get_root().append(&[list(ListType::Bullet, std::slice::from_ref(&item))])?;
        let t = first_child(&item);
        select((t.clone(), 2), (t, 2))
    });
    editor
        .read(|| {
            let t = first_child(&paragraphs()[0]);
            expect_selection((t.clone(), 2), (t, 2));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a caret in an empty nested item
#[test]
fn a_caret_in_an_empty_nested_item() {
    let editor = build_editor();
    remove_list_after(&editor, || {
        let nested = create_list_item_node(None);
        get_root().append(&[list(
            ListType::Bullet,
            &[
                item(&[create_text_node("ab ")]),
                item(&[list(ListType::Bullet, std::slice::from_ref(&nested))]),
            ],
        )])?;
        select((nested.clone(), 0), (nested, 0))
    });
    editor
        .read(|| {
            let p = paragraphs()[1].clone();
            assert_eq!(p.get_children_size(), 0);
            expect_selection((p.clone(), 0), (p, 0));
        })
        .unwrap();
    editor.dispose();
}
