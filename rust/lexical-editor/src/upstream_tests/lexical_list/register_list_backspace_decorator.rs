//! Ports of `packages/lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts`.
//!
//! `TestDecoratorNode` with `setIsInline` is the shared
//! `TestBlockableDecoratorNode`; `IsolatedTestDecoratorNode`, which
//! upstream subclasses from it, is a node of its own here with the same
//! type, block and `isIsolated()`.

use super::support::discrete;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::list::{
    ListType, create_list_item_node, create_list_node, is_list_node, list_extension,
};
use crate::upstream_tests::support::{
    TestBlockableDecoratorNode, create_test_decorator_node_inline,
};
use crate::{
    Editor, Event, KEY_BACKSPACE_COMMAND, KeyboardEvent, LexicalExtension, LexicalNode, NodeBase,
    NodeClass, NodeRef, create_paragraph_node, create_text_node, define_extension, get_root,
    get_selection,
};
use std::cell::Cell;
use std::rc::Rc;

/// `IsolatedTestDecoratorNode`: a block `TestDecoratorNode` that is
/// isolated.
#[derive(Clone, Debug, Default)]
struct IsolatedTestDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for IsolatedTestDecoratorNode {
    fn get_type(&self) -> &'static str {
        "isolated_test_decorator"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        false
    }
    fn is_isolated(&self) -> bool {
        true
    }
    fn get_text_content(&self) -> String {
        "Hello world".to_owned()
    }
    fn decorate(&self, _editor: &Editor) -> Option<Rc<dyn std::any::Any>> {
        Some(Rc::new("Hello world"))
    }
}

/// `testExtension`.
fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![list_extension().into()],
        nodes: vec![
            NodeClass::of::<TestBlockableDecoratorNode>(),
            NodeClass::of::<IsolatedTestDecoratorNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

/// `backspaceEvent()`.
fn backspace_event() -> Rc<KeyboardEvent> {
    Rc::new(KeyboardEvent {
        event: Event::new("keydown"),
        key: "Backspace".to_owned(),
        ..KeyboardEvent::default()
    })
}

fn decorator(inline: bool) -> NodeRef {
    create_test_decorator_node_inline(inline)
}

fn text_item(t: &str) -> NodeRef {
    let item = create_list_item_node(None);
    item.append(&[create_text_node(t)]).unwrap();
    item
}

fn list(list_type: ListType, items: &[NodeRef]) -> NodeRef {
    let list = create_list_node(list_type, 1.0);
    list.append(items).unwrap();
    list
}

fn is_paragraph(node: &NodeRef) -> bool {
    node.get_type() == "paragraph"
}

fn is_test_decorator(node: &NodeRef) -> bool {
    node.get_type() == "test_decorator"
}

/// Builds the root with `setup` (which returns the root's children and
/// selects), dispatches `KEY_BACKSPACE_COMMAND` with `event` in the same
/// update, and returns whether it was handled.
fn backspace(
    editor: &Editor,
    event: Rc<KeyboardEvent>,
    setup: impl FnOnce() -> Result<(), crate::LexicalError> + 'static,
) -> bool {
    let handled = Rc::new(Cell::new(false));
    let out = handled.clone();
    let inner = editor.clone();
    editor
        .update(
            move || {
                get_root().clear()?;
                setup()?;
                out.set(inner.dispatch_command(&KEY_BACKSPACE_COMMAND, event)?);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    handled.get()
}

/// The `preserves the decorator and demotes the first list item` table.
fn demotes_the_first_list_item(list_type: ListType, inline: bool) {
    let editor = build_editor();
    let event = backspace_event();
    let handled = backspace(&editor, event.clone(), move || {
        let first_item = text_item("first");
        get_root().append(&[
            decorator(inline),
            list(list_type, &[first_item.clone(), text_item("second")]),
        ])?;
        first_item.select(Some(0), Some(0)).map(|_| ())
    });
    assert!(handled);
    assert!(event.event.default_prevented());
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 3);
            if inline {
                assert!(
                    is_paragraph(&children[0]),
                    "Expected leading ParagraphNode wrapping inline TestDecoratorNode"
                );
                let wrapped: Vec<bool> = children[0]
                    .get_children()
                    .iter()
                    .map(is_test_decorator)
                    .collect();
                assert_eq!(wrapped, [true]);
            } else {
                assert!(
                    is_test_decorator(&children[0]),
                    "Expected leading TestDecoratorNode"
                );
            }
            let paragraph = &children[1];
            assert!(is_paragraph(paragraph), "Expected demoted ParagraphNode");
            assert_eq!(paragraph.get_text_content(), "first");
            let tail = &children[2];
            assert!(is_list_node(tail), "Expected trailing ListNode");
            assert_eq!(tail.get_list_type(), Some(list_type));
            assert_eq!(tail.get_children_size(), 1);
            assert_eq!(tail.get_text_content(), "second");
            let selection = get_selection().unwrap();
            let range = selection.range().expect("Expected RangeSelection");
            assert!(range.is_collapsed());
            assert_eq!(
                range.anchor.key,
                paragraph.get_first_child_or_throw().unwrap().get_key()
            );
            assert_eq!(range.anchor.offset, 0);
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > block decorator + bullet list
#[test]
fn block_decorator_and_bullet_list() {
    demotes_the_first_list_item(ListType::Bullet, false);
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > block decorator + numbered list
#[test]
fn block_decorator_and_numbered_list() {
    demotes_the_first_list_item(ListType::Number, false);
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > block decorator + check list
#[test]
fn block_decorator_and_check_list() {
    demotes_the_first_list_item(ListType::Check, false);
}

/// Upstream's id is the test name as vitest truncates it.
// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > inline keyboard-selectable decorator + …
#[test]
fn inline_keyboard_selectable_decorator_and_bullet_list() {
    demotes_the_first_list_item(ListType::Bullet, true);
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > block decorator + single-item list — list is removed
#[test]
fn block_decorator_and_single_item_list_list_is_removed() {
    let editor = build_editor();
    let event = backspace_event();
    let handled = backspace(&editor, event.clone(), || {
        let only_item = text_item("only");
        get_root().append(&[
            decorator(false),
            list(ListType::Bullet, std::slice::from_ref(&only_item)),
        ])?;
        only_item.select(Some(0), Some(0)).map(|_| ())
    });
    assert!(handled);
    assert!(event.event.default_prevented());
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert!(
                is_test_decorator(&children[0]),
                "Expected leading TestDecoratorNode"
            );
            assert!(is_paragraph(&children[1]), "Expected demoted ParagraphNode");
            assert_eq!(children[1].get_text_content(), "only");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > two adjacent block decorators + list — both preserved
#[test]
fn two_adjacent_block_decorators_and_list_both_preserved() {
    let editor = build_editor();
    let event = backspace_event();
    let handled = backspace(&editor, event.clone(), || {
        let hello_item = text_item("hello");
        get_root().append(&[
            decorator(false),
            decorator(false),
            list(ListType::Bullet, std::slice::from_ref(&hello_item)),
        ])?;
        hello_item.select(Some(0), Some(0)).map(|_| ())
    });
    assert!(handled);
    assert!(event.event.default_prevented());
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 3);
            assert!(
                is_test_decorator(&children[0]),
                "Expected first TestDecoratorNode"
            );
            assert!(
                is_test_decorator(&children[1]),
                "Expected second TestDecoratorNode"
            );
            assert!(is_paragraph(&children[2]), "Expected demoted ParagraphNode");
            assert_eq!(children[2].get_text_content(), "hello");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > no decorator before the list
#[test]
fn no_decorator_before_the_list() {
    let editor = build_editor();
    let handled = backspace(&editor, backspace_event(), || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("intro")])?;
        let first_item = text_item("first");
        get_root().append(&[
            paragraph,
            list(ListType::Bullet, std::slice::from_ref(&first_item)),
        ])?;
        first_item.select(Some(0), Some(0)).map(|_| ())
    });
    assert!(handled);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert!(is_paragraph(&children[0]), "Expected intro ParagraphNode");
            assert_eq!(children[0].get_text_content(), "intro");
            assert!(
                is_paragraph(&children[1]),
                "Expected converted ParagraphNode"
            );
            assert_eq!(children[1].get_text_content(), "first");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > caret on the second list item — converts to paragraph via collapseAtStart
#[test]
fn caret_on_the_second_list_item_converts_to_paragraph() {
    let editor = build_editor();
    let handled = backspace(&editor, backspace_event(), || {
        let second_item = text_item("second");
        get_root().append(&[
            decorator(false),
            list(ListType::Bullet, &[text_item("first"), second_item.clone()]),
        ])?;
        second_item.select(Some(0), Some(0)).map(|_| ())
    });
    assert!(handled);
    editor
        .read(|| {
            let children = get_root().get_children();
            let list = &children[1];
            assert!(is_list_node(list), "Expected ListNode at index 1");
            assert_eq!(list.get_children_size(), 1);
            assert!(
                is_paragraph(&children[2]),
                "Expected converted ParagraphNode"
            );
            assert_eq!(children[2].get_text_content(), "second");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > caret in the middle of the first list item
#[test]
fn caret_in_the_middle_of_the_first_list_item() {
    let editor = build_editor();
    let handled = backspace(&editor, backspace_event(), || {
        let text = create_text_node("first");
        let item = create_list_item_node(None);
        item.append(std::slice::from_ref(&text))?;
        get_root().append(&[decorator(false), list(ListType::Bullet, &[item])])?;
        text.select(Some(2), Some(2)).map(|_| ())
    });
    assert!(!handled);
    editor
        .read(|| {
            assert!(
                get_root()
                    .get_child_at_index(1)
                    .is_some_and(|n| is_list_node(&n)),
                "Expected ListNode at index 1"
            );
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > nested list — outdents inner item
#[test]
fn nested_list_outdents_inner_item() {
    let editor = build_editor();
    let handled = backspace(&editor, backspace_event(), || {
        let inner_li = text_item("nested");
        let inner_list = list(ListType::Bullet, std::slice::from_ref(&inner_li));
        let outer_li = create_list_item_node(None);
        outer_li.append(&[inner_list])?;
        get_root().append(&[decorator(false), list(ListType::Bullet, &[outer_li])])?;
        inner_li.select(Some(0), Some(0)).map(|_| ())
    });
    assert!(handled);
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > isolated decorator before the list
#[test]
fn isolated_decorator_before_the_list() {
    let editor = build_editor();
    let handled = backspace(&editor, backspace_event(), || {
        let first_item = text_item("first");
        get_root().append(&[
            crate::create::<IsolatedTestDecoratorNode>(),
            list(ListType::Bullet, std::slice::from_ref(&first_item)),
        ])?;
        first_item.select(Some(0), Some(0)).map(|_| ())
    });
    assert!(handled);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].get_type(), "isolated_test_decorator");
            assert!(
                is_paragraph(&children[1]),
                "Expected converted ParagraphNode"
            );
            assert_eq!(children[1].get_text_content(), "first");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > empty first list item — converts to empty paragraph
#[test]
fn empty_first_list_item_converts_to_empty_paragraph() {
    let editor = build_editor();
    let handled = backspace(&editor, backspace_event(), || {
        let empty_item = create_list_item_node(None);
        get_root().append(&[
            decorator(false),
            list(ListType::Bullet, &[empty_item.clone(), text_item("second")]),
        ])?;
        empty_item.select(None, None).map(|_| ())
    });
    assert!(handled);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 3);
            assert!(is_test_decorator(&children[0]));
            assert!(is_paragraph(&children[1]), "Expected empty ParagraphNode");
            assert_eq!(children[1].get_text_content(), "");
            assert!(is_list_node(&children[2]), "Expected remaining ListNode");
            assert_eq!(
                children[2]
                    .get_first_child_or_throw()
                    .unwrap()
                    .get_text_content(),
                "second"
            );
        })
        .unwrap();
}
