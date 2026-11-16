//! Ports of `packages/lexical/src/__tests__/unit/HandleTextDrop.test.ts`.
//!
//! Upstream mocks `caretFromPoint` to return a DOM point in the mounted
//! editor. A Rust editor asks its root element for the caret at the drop
//! point ([`RootElement::caret_from_point`]), so the root element here
//! returns the caret each DOM point stands for: a DOM text node and offset
//! is a text point caret, and an element and child offset is
//! `$getChildCaretAtIndex(element, offset, 'next')`.

#![cfg(feature = "clipboard")]

use super::super::support::{create_test_editor, create_test_inline_element_node};
use crate::caret::{CaretDirection, PointCaret, get_text_point_caret};
use crate::caret_utils::get_child_caret_at_index;
use crate::clipboard::{
    get_clipboard_data_from_selection, handle_plain_text_drop, handle_rich_text_drop,
    set_lexical_clipboard_data_transfer, write_drag_source_to_data_transfer,
};
use crate::json::Value;
use crate::lexical_utils::set_node_key;
use crate::node::NodeBase;
use crate::{
    CreateEditorArgs, DataTransfer, DragEvent, Editor, LexicalError, LexicalNode, NodeClass,
    NodeRef, PointType, RootElement, RootElementRef, SelectionRef, UpdateOptions, Which,
    create_paragraph_node, create_range_selection, create_text_node, get_node_by_key, get_root,
    get_selection, set_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

/// A DOM point upstream's `caretFromPoint` returns, by the Lexical node
/// rendered there.
#[derive(Clone)]
enum DropPoint {
    /// The DOM text of a text node, and an offset in it.
    Text(String, usize),
    /// The DOM element of an element node, and a child offset.
    Element(String, usize),
}

impl DropPoint {
    fn caret(&self) -> PointCaret {
        match self {
            DropPoint::Text(key, offset) => {
                let text = get_node_by_key(key).expect("the text node");
                get_text_point_caret(&text, CaretDirection::Next, *offset).unwrap()
            }
            DropPoint::Element(key, offset) => {
                let element = get_node_by_key(key).expect("the element");
                get_child_caret_at_index(&element, *offset, CaretDirection::Next)
            }
        }
    }
}

thread_local! {
    /// `caretFromPointState.current`.
    static CARET_FROM_POINT: RefCell<Option<DropPoint>> = const { RefCell::new(None) };
}

/// `setCaretFromPoint(node, offset)`.
fn set_caret_from_point(point: DropPoint) {
    CARET_FROM_POINT.with(|c| *c.borrow_mut() = Some(point));
}

/// The mounted root element, whose `caretFromPoint` is the mocked one.
struct DropRootElement;

impl RootElement for DropRootElement {
    fn caret_from_point(&self, _x: f64, _y: f64) -> Option<PointCaret> {
        CARET_FROM_POINT
            .with(|c| c.borrow().clone())
            .map(|point| point.caret())
    }
}

/// `TestDecoratorNode` (`test_decorator`) as upstream's test utilities
/// define it, with the `createDOM` and `exportDOM` the clipboard's HTML
/// export of a selection reaches (the shared test node has neither).
#[derive(Clone, Debug, Default)]
struct TestDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for TestDecoratorNode {
    fn get_type(&self) -> &'static str {
        "test_decorator"
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
    fn get_text_content(&self) -> String {
        "Hello world".to_owned()
    }
    fn decorate(&self, _editor: &Editor) -> Option<Rc<dyn std::any::Any>> {
        Some(Rc::new("Hello world"))
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("span"))
    }
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        _this: &NodeRef,
        _editor: &Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        Ok(crate::lexical_dom::DomExportOutput {
            element: Some(crate::lexical_dom::get_document().create_element("test-decorator")),
            ..Default::default()
        })
    }
}

/// `$createTestDecoratorNode()`.
fn create_test_decorator_node() -> NodeRef {
    set_node_key(Box::new(TestDecoratorNode::default()))
}

/// `initializeUnitTest`'s editor, mounted on a root element whose
/// `caretFromPoint` resolves nothing until a test sets a point
/// (`beforeEach`).
fn drop_editor() -> Editor {
    CARET_FROM_POINT.with(|c| *c.borrow_mut() = None);
    let editor = create_test_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<TestDecoratorNode>()],
        ..CreateEditorArgs::default()
    });
    editor
        .set_root_element(Some(RootElementRef(Rc::new(DropRootElement))))
        .unwrap();
    editor
}

/// `createDropEvent()`: a `drop` event at (0, 0) carrying a new
/// `DataTransfer`. Upstream spies on `preventDefault`; here the event's
/// `default_prevented` says whether it was called.
fn create_drop_event() -> (DragEvent, Rc<DataTransfer>) {
    let data_transfer = Rc::new(DataTransfer::new());
    (
        DragEvent::new("drop", Some(data_transfer.clone())),
        data_transfer,
    )
}

/// `$markActiveSelectionAsDragSource(dataTransfer, editor)`.
fn mark_active_selection_as_drag_source(data_transfer: &DataTransfer, editor: &Editor) {
    if get_selection()
        .and_then(|s| s.range())
        .is_some_and(|r| !r.is_collapsed())
    {
        write_drag_source_to_data_transfer(data_transfer, editor);
    }
}

/// `setLexicalClipboardDataTransfer(dataTransfer,
/// $getClipboardDataFromSelection())` and
/// `$writeDragSourceToDataTransfer(dataTransfer, editor)`, as DRAGSTART
/// writes them.
fn write_drag_start(data_transfer: &DataTransfer, editor: &Editor) -> Result<(), LexicalError> {
    set_lexical_clipboard_data_transfer(
        data_transfer,
        &get_clipboard_data_from_selection(get_selection().as_ref())?,
    );
    write_drag_source_to_data_transfer(data_transfer, editor);
    Ok(())
}

/// `await editor.update(fn)`: a committed update.
fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

/// `$setSelection` of a range selection from `(anchor key, offset)` to
/// `(focus key, offset)` in text nodes.
fn select_text(anchor: (&str, usize), focus: (&str, usize)) -> Result<(), LexicalError> {
    let selection = create_range_selection();
    selection.point_set(Which::Anchor, anchor.0, anchor.1, PointType::Text)?;
    selection.point_set(Which::Focus, focus.0, focus.1, PointType::Text)?;
    set_selection(Some(selection));
    Ok(())
}

fn root_text(editor: &Editor) -> String {
    editor.read(|| get_root().get_text_content()).unwrap()
}

/// A paragraph holding `Hello foo world`, with `foo` selected from
/// `anchor` to `focus`; the text node's key.
fn hello_foo_world(editor: &Editor, anchor: usize, focus: usize) -> String {
    let key = Rc::new(RefCell::new(String::new()));
    let k = key.clone();
    update(editor, move || {
        let paragraph = create_paragraph_node();
        let text = create_text_node("Hello foo world");
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        *k.borrow_mut() = text.get_key().to_owned();
        select_text((text.get_key(), anchor), (text.get_key(), focus))
    });
    key.take()
}

/// Drops `text/plain` data `plain` at `point` with the drag marker, as
/// `$handleRichTextDrop`; whether it was handled and the default prevented.
fn drop_plain_text(editor: &Editor, point: DropPoint, plain: &'static str) -> (bool, bool) {
    let editor2 = editor.clone();
    let out = Rc::new(RefCell::new((false, false)));
    let o = out.clone();
    update(editor, move || {
        set_caret_from_point(point);
        let (event, data_transfer) = create_drop_event();
        data_transfer.set_data("text/plain", plain);
        mark_active_selection_as_drag_source(&data_transfer, &editor2);
        let handled = handle_rich_text_drop(&event, &editor2)?;
        *o.borrow_mut() = (handled, event.default_prevented());
        Ok(())
    });
    out.take()
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > moves a selected word later within the same TextNode
#[test]
fn moves_a_selected_word_later_within_the_same_text_node() {
    let editor = drop_editor();
    let text_key = hello_foo_world(&editor, 6, 9);
    // Drop position at offset 15 ("Hello foo world|"), past the source's end.
    let (handled, prevented) = drop_plain_text(&editor, DropPoint::Text(text_key, 15), "foo");
    assert!(handled);
    assert!(prevented);
    assert_eq!(root_text(&editor), "Hello  worldfoo");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > moves a selected word earlier within the same TextNode
#[test]
fn moves_a_selected_word_earlier_within_the_same_text_node() {
    let editor = drop_editor();
    let text_key = hello_foo_world(&editor, 6, 9);
    // Drop position at offset 0 (before the source).
    drop_plain_text(&editor, DropPoint::Text(text_key, 0), "foo");
    assert_eq!(root_text(&editor), "fooHello  world");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > no-op when the drop point is inside the source range
#[test]
fn no_op_when_the_drop_point_is_inside_the_source_range() {
    let editor = drop_editor();
    let text_key = hello_foo_world(&editor, 6, 9);
    // Drop inside the selected "foo" range.
    let (handled, prevented) = drop_plain_text(&editor, DropPoint::Text(text_key, 7), "foo");
    assert!(handled);
    assert!(prevented);
    assert_eq!(root_text(&editor), "Hello foo world");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > keeps the dragged text when dropped on the end edge of the source range
#[test]
fn keeps_the_dragged_text_when_dropped_on_the_end_edge_of_the_source_range() {
    let editor = drop_editor();
    let keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let k = keys.clone();
    update(&editor, move || {
        let bold = create_text_node("Hello");
        bold.toggle_format("bold");
        let rest = create_text_node(" world");
        let paragraph = create_paragraph_node();
        paragraph.append(&[bold.clone(), rest.clone()])?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        k.borrow_mut()
            .extend([bold.get_key().to_owned(), rest.get_key().to_owned()]);
        bold.select(Some(0), Some(5))?;
        Ok(())
    });
    let keys = keys.take();
    // Drop at the end of "Hello", then at the start of " world".
    for (key, offset) in [(&keys[0], 5), (&keys[1], 0)] {
        let (handled, _) = drop_plain_text(&editor, DropPoint::Text(key.clone(), offset), "Hello");
        assert!(handled);
        assert_eq!(root_text(&editor), "Hello world");
    }
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > moves a selection across TextNodes in the same block
#[test]
fn moves_a_selection_across_text_nodes_in_the_same_block() {
    let editor = drop_editor();
    let dest_key = Rc::new(RefCell::new(String::new()));
    let d = dest_key.clone();
    update(&editor, move || {
        let paragraph = create_paragraph_node();
        let source = create_text_node("source ");
        source.toggle_format("bold");
        let dest = create_text_node("destination");
        paragraph.append(&[source.clone(), dest.clone()])?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        *d.borrow_mut() = dest.get_key().to_owned();
        select_text((source.get_key(), 0), (source.get_key(), 6))
    });
    // Drop just after the "t" of "destination" (offset 4 into "destination").
    drop_plain_text(&editor, DropPoint::Text(dest_key.take(), 4), "source");
    // " " from the source TextNode is preserved where it was, and "source"
    // is inserted into the destination TextNode at offset 4.
    assert_eq!(root_text(&editor), " destsourceination");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > returns false for an external drag (no marker), letting the browser handle it
#[test]
fn returns_false_for_an_external_drag() {
    let editor = drop_editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("Hello world")])?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    let out = Rc::new(RefCell::new((true, true)));
    let o = out.clone();
    let editor2 = editor.clone();
    update(&editor, move || {
        let (event, data_transfer) = create_drop_event();
        data_transfer.set_data("text/plain", "brave ");
        // No drag-source marker — this represents an external drag. The
        // handler should bail so the browser's native drag-drop flow (which
        // fires beforeinput insertFromDrop on the destination) takes over.
        let handled = handle_rich_text_drop(&event, &editor2)?;
        *o.borrow_mut() = (handled, event.default_prevented());
        Ok(())
    });
    let (handled, prevented) = out.take();
    assert!(!handled);
    assert!(!prevented);
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > returns false when caretFromPoint cannot resolve a location
#[test]
fn returns_false_when_caret_from_point_cannot_resolve_a_location() {
    let editor = drop_editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("Hello")])?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    let out = Rc::new(RefCell::new((true, true)));
    let o = out.clone();
    let editor2 = editor.clone();
    update(&editor, move || {
        let (event, _) = create_drop_event();
        let handled = handle_rich_text_drop(&event, &editor2)?;
        *o.borrow_mut() = (handled, event.default_prevented());
        Ok(())
    });
    let (handled, prevented) = out.take();
    assert!(!handled);
    assert!(!prevented);
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > handles a backward (right-to-left) source selection
#[test]
fn handles_a_backward_source_selection() {
    let editor = drop_editor();
    // Backward selection: anchor > focus in "foo".
    let text_key = hello_foo_world(&editor, 9, 6);
    editor
        .read(|| {
            let current = get_selection().filter(SelectionRef::is_range);
            assert!(
                current.is_some_and(|r| r.is_backward().unwrap()),
                "expected backward selection"
            );
        })
        .unwrap();
    drop_plain_text(&editor, DropPoint::Text(text_key, 15), "foo");
    assert_eq!(root_text(&editor), "Hello  worldfoo");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > preserves DecoratorNodes in the source when moved to a new location
#[test]
fn preserves_decorator_nodes_in_the_source_when_moved_to_a_new_location() {
    let editor = drop_editor();
    let target_key = Rc::new(RefCell::new(String::new()));
    let t = target_key.clone();
    update(&editor, move || {
        let root = get_root();
        root.clear()?;
        // Source paragraph: "before" + DecoratorNode + "after"
        let source_paragraph = create_paragraph_node();
        let before_text = create_text_node("before");
        let decorator = create_test_decorator_node();
        let after_text = create_text_node("after");
        source_paragraph.append(&[before_text.clone(), decorator, after_text.clone()])?;
        // Target paragraph (drop destination)
        let target_paragraph = create_paragraph_node();
        let target_text = create_text_node("target");
        target_paragraph.append(std::slice::from_ref(&target_text))?;
        *t.borrow_mut() = target_text.get_key().to_owned();
        root.append(&[source_paragraph, target_paragraph])?;
        // Select from start of "before" to end of "after" — covers the decorator.
        select_text((before_text.get_key(), 0), (after_text.get_key(), 5))
    });
    // Populate the DataTransfer the same way DRAGSTART does — with Lexical's
    // own serialization so custom nodes survive the drop, plus a drag-source
    // marker so the handler treats it as a same-editor move.
    let target_key = target_key.take();
    let editor2 = editor.clone();
    update(&editor, move || {
        let selection = get_selection().filter(SelectionRef::is_range);
        assert!(selection.is_some(), "expected range selection");
        set_caret_from_point(DropPoint::Text(target_key, 6)); // drop at end of "target"
        let (event, data_transfer) = create_drop_event();
        set_lexical_clipboard_data_transfer(
            &data_transfer,
            &get_clipboard_data_from_selection(selection.as_ref())?,
        );
        write_drag_source_to_data_transfer(&data_transfer, &editor2);
        handle_rich_text_drop(&event, &editor2)?;
        Ok(())
    });
    editor
        .read(|| {
            let top_level_children = get_root().get_children();
            // Two paragraphs at the top level: the (now-empty) source and the
            // target (now containing the moved content).
            assert_eq!(top_level_children.len(), 2);
            let decorators_of = |node: &NodeRef| {
                node.get_children()
                    .iter()
                    .filter(|child| child.is_decorator())
                    .count()
            };
            // Exactly one decorator in the whole tree (preserved, not duplicated).
            assert_eq!(
                top_level_children.iter().map(decorators_of).sum::<usize>(),
                1
            );
            // The decorator must live under the target paragraph, not the source.
            let source_paragraph = &top_level_children[0];
            let target_paragraph = &top_level_children[1];
            assert!(source_paragraph.is_element(), "expected source paragraph");
            assert!(target_paragraph.is_element(), "expected target paragraph");
            assert_eq!(decorators_of(source_paragraph), 0);
            assert_eq!(decorators_of(target_paragraph), 1);
            // TestDecoratorNode.getTextContent() is 'Hello world', so the
            // decorator appears between "before" and "after" in textContent.
            assert_eq!(source_paragraph.get_text_content(), "");
            assert_eq!(
                target_paragraph.get_text_content(),
                "targetbeforeHello worldafter"
            );
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > no-op when drop is inside a multi-node source range
#[test]
fn no_op_when_drop_is_inside_a_multi_node_source_range() {
    let editor = drop_editor();
    let inner_key = Rc::new(RefCell::new(String::new()));
    let i = inner_key.clone();
    update(&editor, move || {
        let paragraph = create_paragraph_node();
        let t1 = create_text_node("one");
        let t2 = create_text_node("two");
        t2.toggle_format("bold");
        let t3 = create_text_node("three");
        paragraph.append(&[t1.clone(), t2.clone(), t3.clone()])?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        *i.borrow_mut() = t2.get_key().to_owned();
        // Selection spans t1..t3 (all three text nodes).
        select_text((t1.get_key(), 0), (t3.get_key(), 5))
    });
    // Drop INSIDE the "two" text node, which is strictly between the
    // source's anchor and focus.
    let (handled, prevented) =
        drop_plain_text(&editor, DropPoint::Text(inner_key.take(), 1), "onetwothree");
    assert!(handled);
    assert!(prevented);
    assert_eq!(root_text(&editor), "onetwothree");
}

// Drop boundaries (`describe.each` over the rich and plain text handlers,
// then `backward`).

type DropHandler = fn(&DragEvent, &Editor) -> Result<bool, LexicalError>;

/// Selects the whole of `source`, forward or backward
/// (`source.select(backward ? 5 : 0, backward ? 0 : 5)`).
fn select_whole(source: &NodeRef, backward: bool) -> Result<(), LexicalError> {
    let (anchor, focus) = if backward { (5, 0) } else { (0, 5) };
    source.select(Some(anchor), Some(focus))?;
    Ok(())
}

/// The selection `start..end` in text nodes, made backward by setting
/// `start` on the focus.
fn select_range(
    start: (&str, usize),
    end: (&str, usize),
    backward: bool,
) -> Result<(), LexicalError> {
    if backward {
        select_text(end, start)
    } else {
        select_text(start, end)
    }
}

/// Drops the selection's own clipboard data at `point`, with the drag
/// marker; whether it was handled and the default prevented.
fn drop_selection(editor: &Editor, handler: DropHandler, point: DropPoint) -> (bool, bool) {
    let editor2 = editor.clone();
    let out = Rc::new(RefCell::new((false, false)));
    let o = out.clone();
    update(editor, move || {
        set_caret_from_point(point);
        let (event, data_transfer) = create_drop_event();
        write_drag_start(&data_transfer, &editor2)?;
        let handled = handler(&event, &editor2)?;
        *o.borrow_mut() = (handled, event.default_prevented());
        Ok(())
    });
    out.take()
}

/// Drops at `point` and expects the drop to be taken (and, where upstream
/// checks it, its default prevented) but the editor state, the selection
/// and the nodes `keys` to be unchanged.
fn expect_drop_keeps_state(
    editor: &Editor,
    handler: DropHandler,
    point: DropPoint,
    keys: Vec<String>,
    expect_prevented: bool,
) {
    let original: Value = editor.get_editor_state().to_json(false);
    let selection = editor
        .read(|| get_selection().unwrap().clone_selection())
        .unwrap();
    let (handled, prevented) = drop_selection(editor, handler, point);
    assert!(handled);
    if expect_prevented {
        assert!(prevented);
    }
    assert_eq!(editor.get_editor_state().to_json(false), original);
    editor
        .read(|| {
            assert!(get_selection().unwrap().get().is(&selection.get()));
            for key in &keys {
                assert_eq!(get_node_by_key(key).map(|n| n.is_attached()), Some(true));
            }
        })
        .unwrap();
}

/// `keeps a token unchanged at %s`.
fn keeps_a_token_unchanged(handler: DropHandler, backward: bool, position: &str) {
    let editor = drop_editor();
    let keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let k = keys.clone();
    update(&editor, move || {
        let before = create_text_node("before ");
        let source = create_text_node("Hello");
        source.set_mode("token")?;
        let after = create_text_node(" world");
        let paragraph = create_paragraph_node();
        paragraph.append(&[before.clone(), source.clone(), after.clone()])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        k.borrow_mut()
            .extend([&paragraph, &before, &source, &after].map(|n| n.get_key().to_owned()));
        select_whole(&source, backward)
    });
    let keys = keys.take();
    let [paragraph, before, source, after] = [0, 1, 2, 3].map(|i| keys[i].clone());
    let point = match position {
        "element end" => DropPoint::Element(paragraph, 2),
        "element start" => DropPoint::Element(paragraph, 1),
        "next start" => DropPoint::Text(after, 0),
        "previous end" => DropPoint::Text(before, 7),
        "text end" => DropPoint::Text(source, 5),
        "text middle" => DropPoint::Text(source, 2),
        "text start" => DropPoint::Text(source, 0),
        _ => unreachable!(),
    };
    expect_drop_keeps_state(&editor, handler, point, keys, true);
}

/// `keeps nested inline wrappers unchanged at %s`.
fn keeps_nested_inline_wrappers_unchanged(handler: DropHandler, backward: bool, position: &str) {
    let editor = drop_editor();
    let keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let k = keys.clone();
    update(&editor, move || {
        let before = create_text_node("before ");
        let source = create_text_node("Hello");
        let after = create_text_node(" world");
        let inner = create_test_inline_element_node();
        inner.append(std::slice::from_ref(&source))?;
        let outer = create_test_inline_element_node();
        outer.append(std::slice::from_ref(&inner))?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[before.clone(), outer.clone(), after.clone()])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        k.borrow_mut()
            .extend([&before, &after, &paragraph, &inner, &outer].map(|n| n.get_key().to_owned()));
        select_whole(&source, backward)
    });
    let keys = keys.take();
    let [before, after, paragraph] = [0, 1, 2].map(|i| keys[i].clone());
    let point = match position {
        "element end" => DropPoint::Element(paragraph, 2),
        "element start" => DropPoint::Element(paragraph, 1),
        "next start" => DropPoint::Text(after, 0),
        "previous end" => DropPoint::Text(before, 7),
        _ => unreachable!(),
    };
    expect_drop_keeps_state(&editor, handler, point, keys[3..].to_vec(), false);
}

/// `keeps a decorator in a multi-node range at the adjacent end edge`.
fn keeps_a_decorator_at_the_adjacent_end_edge(handler: DropHandler, backward: bool) {
    let editor = drop_editor();
    let keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let k = keys.clone();
    update(&editor, move || {
        let first = create_text_node("before");
        let decorator = create_test_decorator_node();
        let last = create_text_node("after");
        last.set_mode("token")?;
        let after = create_text_node(" rest");
        k.borrow_mut()
            .extend([after.get_key().to_owned(), decorator.get_key().to_owned()]);
        let paragraph = create_paragraph_node();
        paragraph.append(&[first.clone(), decorator, last.clone(), after])?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        select_range((first.get_key(), 0), (last.get_key(), 5), backward)
    });
    let keys = keys.take();
    expect_drop_keeps_state(
        &editor,
        handler,
        DropPoint::Text(keys[0].clone(), 0),
        vec![keys[1].clone()],
        false,
    );
}

/// `keeps a %s destination after a cross-paragraph deletion`.
fn keeps_a_destination_after_a_cross_paragraph_deletion(
    handler: DropHandler,
    backward: bool,
    position: &str,
) {
    let editor = drop_editor();
    let keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let k = keys.clone();
    update(&editor, move || {
        let first = create_text_node("abc");
        let last = create_text_node("def");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&last))?;
        k.borrow_mut()
            .extend([last.get_key().to_owned(), paragraph.get_key().to_owned()]);
        let first_paragraph = create_paragraph_node();
        first_paragraph.append(std::slice::from_ref(&first))?;
        get_root().clear()?;
        get_root().append(&[first_paragraph, paragraph])?;
        select_range((first.get_key(), 1), (last.get_key(), 2), backward)
    });
    let keys = keys.take();
    let point = match position {
        "paragraph" => DropPoint::Element(keys[1].clone(), 1),
        "root" => DropPoint::Element("root".to_owned(), 2),
        "text" => DropPoint::Text(keys[0].clone(), 3),
        _ => unreachable!(),
    };
    let (handled, prevented) = drop_selection(&editor, handler, point);
    assert!(handled);
    assert!(prevented);
    let is_rich = std::ptr::fn_addr_eq(handler, handle_rich_text_drop as DropHandler);
    editor
        .read(|| {
            // The unselected f must precede the moved bc/de, even when its
            // former paragraph was the destination caret's origin.
            assert_eq!(get_root().get_text_content().replace('\n', ""), "afbcde");
            assert!(get_selection().unwrap().range().unwrap().is_collapsed());
            if position != "root" {
                let children: Vec<String> = get_root()
                    .get_children()
                    .iter()
                    .map(NodeRef::get_text_content)
                    .collect();
                if is_rich {
                    assert_eq!(children, ["afbc", "de"]);
                } else {
                    assert_eq!(children, ["afbc\nde"]);
                }
            }
        })
        .unwrap();
}

// $handleRichTextDrop across editors

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleRichTextDrop across editors > cancelled cross-editor drag leaves both editors untouched
#[test]
fn cancelled_cross_editor_drag_leaves_both_editors_untouched() {
    let source_editor = drop_editor();
    let dest_editor = drop_editor();
    update(&source_editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("source-content")])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        let text = paragraph.get_first_child_or_throw()?;
        select_text((text.get_key(), 0), (text.get_key(), 6))
    });
    update(&dest_editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("destination")])?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    // No drop event ever fires — there is no DataTransfer to read, so the
    // marker-based design has nothing to act on. Both editors are unchanged.
    assert_eq!(root_text(&source_editor), "source-content");
    assert_eq!(root_text(&dest_editor), "destination");
}

// The drop boundary cases, one test per upstream case.

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at text start
#[test]
fn rich_backward_false_keeps_a_token_unchanged_at_text_start() {
    keeps_a_token_unchanged(handle_rich_text_drop, false, "text start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at text middle
#[test]
fn rich_backward_false_keeps_a_token_unchanged_at_text_middle() {
    keeps_a_token_unchanged(handle_rich_text_drop, false, "text middle");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at text end
#[test]
fn rich_backward_false_keeps_a_token_unchanged_at_text_end() {
    keeps_a_token_unchanged(handle_rich_text_drop, false, "text end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at previous end
#[test]
fn rich_backward_false_keeps_a_token_unchanged_at_previous_end() {
    keeps_a_token_unchanged(handle_rich_text_drop, false, "previous end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at next start
#[test]
fn rich_backward_false_keeps_a_token_unchanged_at_next_start() {
    keeps_a_token_unchanged(handle_rich_text_drop, false, "next start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at element start
#[test]
fn rich_backward_false_keeps_a_token_unchanged_at_element_start() {
    keeps_a_token_unchanged(handle_rich_text_drop, false, "element start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at element end
#[test]
fn rich_backward_false_keeps_a_token_unchanged_at_element_end() {
    keeps_a_token_unchanged(handle_rich_text_drop, false, "element end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps nested inline wrappers unchanged at previous end
#[test]
fn rich_backward_false_keeps_nested_inline_wrappers_unchanged_at_previous_end() {
    keeps_nested_inline_wrappers_unchanged(handle_rich_text_drop, false, "previous end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps nested inline wrappers unchanged at next start
#[test]
fn rich_backward_false_keeps_nested_inline_wrappers_unchanged_at_next_start() {
    keeps_nested_inline_wrappers_unchanged(handle_rich_text_drop, false, "next start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps nested inline wrappers unchanged at element start
#[test]
fn rich_backward_false_keeps_nested_inline_wrappers_unchanged_at_element_start() {
    keeps_nested_inline_wrappers_unchanged(handle_rich_text_drop, false, "element start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps nested inline wrappers unchanged at element end
#[test]
fn rich_backward_false_keeps_nested_inline_wrappers_unchanged_at_element_end() {
    keeps_nested_inline_wrappers_unchanged(handle_rich_text_drop, false, "element end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a decorator in a multi-node range at the adjacent end edge
#[test]
fn rich_backward_false_keeps_a_decorator_in_a_multi_node_range_at_the_adjacent_end_edge() {
    keeps_a_decorator_at_the_adjacent_end_edge(handle_rich_text_drop, false);
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a text destination after a cross-paragraph deletion
#[test]
fn rich_backward_false_keeps_a_text_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_rich_text_drop, false, "text");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a paragraph destination after a cross-paragraph deletion
#[test]
fn rich_backward_false_keeps_a_paragraph_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_rich_text_drop, false, "paragraph");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a root destination after a cross-paragraph deletion
#[test]
fn rich_backward_false_keeps_a_root_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_rich_text_drop, false, "root");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at text start
#[test]
fn rich_backward_true_keeps_a_token_unchanged_at_text_start() {
    keeps_a_token_unchanged(handle_rich_text_drop, true, "text start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at text middle
#[test]
fn rich_backward_true_keeps_a_token_unchanged_at_text_middle() {
    keeps_a_token_unchanged(handle_rich_text_drop, true, "text middle");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at text end
#[test]
fn rich_backward_true_keeps_a_token_unchanged_at_text_end() {
    keeps_a_token_unchanged(handle_rich_text_drop, true, "text end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at previous end
#[test]
fn rich_backward_true_keeps_a_token_unchanged_at_previous_end() {
    keeps_a_token_unchanged(handle_rich_text_drop, true, "previous end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at next start
#[test]
fn rich_backward_true_keeps_a_token_unchanged_at_next_start() {
    keeps_a_token_unchanged(handle_rich_text_drop, true, "next start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at element start
#[test]
fn rich_backward_true_keeps_a_token_unchanged_at_element_start() {
    keeps_a_token_unchanged(handle_rich_text_drop, true, "element start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at element end
#[test]
fn rich_backward_true_keeps_a_token_unchanged_at_element_end() {
    keeps_a_token_unchanged(handle_rich_text_drop, true, "element end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps nested inline wrappers unchanged at previous end
#[test]
fn rich_backward_true_keeps_nested_inline_wrappers_unchanged_at_previous_end() {
    keeps_nested_inline_wrappers_unchanged(handle_rich_text_drop, true, "previous end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps nested inline wrappers unchanged at next start
#[test]
fn rich_backward_true_keeps_nested_inline_wrappers_unchanged_at_next_start() {
    keeps_nested_inline_wrappers_unchanged(handle_rich_text_drop, true, "next start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps nested inline wrappers unchanged at element start
#[test]
fn rich_backward_true_keeps_nested_inline_wrappers_unchanged_at_element_start() {
    keeps_nested_inline_wrappers_unchanged(handle_rich_text_drop, true, "element start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps nested inline wrappers unchanged at element end
#[test]
fn rich_backward_true_keeps_nested_inline_wrappers_unchanged_at_element_end() {
    keeps_nested_inline_wrappers_unchanged(handle_rich_text_drop, true, "element end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a decorator in a multi-node range at the adjacent end edge
#[test]
fn rich_backward_true_keeps_a_decorator_in_a_multi_node_range_at_the_adjacent_end_edge() {
    keeps_a_decorator_at_the_adjacent_end_edge(handle_rich_text_drop, true);
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a text destination after a cross-paragraph deletion
#[test]
fn rich_backward_true_keeps_a_text_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_rich_text_drop, true, "text");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a paragraph destination after a cross-paragraph deletion
#[test]
fn rich_backward_true_keeps_a_paragraph_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_rich_text_drop, true, "paragraph");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a root destination after a cross-paragraph deletion
#[test]
fn rich_backward_true_keeps_a_root_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_rich_text_drop, true, "root");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at text start
#[test]
fn plain_backward_false_keeps_a_token_unchanged_at_text_start() {
    keeps_a_token_unchanged(handle_plain_text_drop, false, "text start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at text middle
#[test]
fn plain_backward_false_keeps_a_token_unchanged_at_text_middle() {
    keeps_a_token_unchanged(handle_plain_text_drop, false, "text middle");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at text end
#[test]
fn plain_backward_false_keeps_a_token_unchanged_at_text_end() {
    keeps_a_token_unchanged(handle_plain_text_drop, false, "text end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at previous end
#[test]
fn plain_backward_false_keeps_a_token_unchanged_at_previous_end() {
    keeps_a_token_unchanged(handle_plain_text_drop, false, "previous end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at next start
#[test]
fn plain_backward_false_keeps_a_token_unchanged_at_next_start() {
    keeps_a_token_unchanged(handle_plain_text_drop, false, "next start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at element start
#[test]
fn plain_backward_false_keeps_a_token_unchanged_at_element_start() {
    keeps_a_token_unchanged(handle_plain_text_drop, false, "element start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at element end
#[test]
fn plain_backward_false_keeps_a_token_unchanged_at_element_end() {
    keeps_a_token_unchanged(handle_plain_text_drop, false, "element end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps nested inline wrappers unchanged at previous end
#[test]
fn plain_backward_false_keeps_nested_inline_wrappers_unchanged_at_previous_end() {
    keeps_nested_inline_wrappers_unchanged(handle_plain_text_drop, false, "previous end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps nested inline wrappers unchanged at next start
#[test]
fn plain_backward_false_keeps_nested_inline_wrappers_unchanged_at_next_start() {
    keeps_nested_inline_wrappers_unchanged(handle_plain_text_drop, false, "next start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps nested inline wrappers unchanged at element start
#[test]
fn plain_backward_false_keeps_nested_inline_wrappers_unchanged_at_element_start() {
    keeps_nested_inline_wrappers_unchanged(handle_plain_text_drop, false, "element start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps nested inline wrappers unchanged at element end
#[test]
fn plain_backward_false_keeps_nested_inline_wrappers_unchanged_at_element_end() {
    keeps_nested_inline_wrappers_unchanged(handle_plain_text_drop, false, "element end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a decorator in a multi-node range at the adjacent end edge
#[test]
fn plain_backward_false_keeps_a_decorator_in_a_multi_node_range_at_the_adjacent_end_edge() {
    keeps_a_decorator_at_the_adjacent_end_edge(handle_plain_text_drop, false);
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a text destination after a cross-paragraph deletion
#[test]
fn plain_backward_false_keeps_a_text_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_plain_text_drop, false, "text");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a paragraph destination after a cross-paragraph deletion
#[test]
fn plain_backward_false_keeps_a_paragraph_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(
        handle_plain_text_drop,
        false,
        "paragraph",
    );
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a root destination after a cross-paragraph deletion
#[test]
fn plain_backward_false_keeps_a_root_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_plain_text_drop, false, "root");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at text start
#[test]
fn plain_backward_true_keeps_a_token_unchanged_at_text_start() {
    keeps_a_token_unchanged(handle_plain_text_drop, true, "text start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at text middle
#[test]
fn plain_backward_true_keeps_a_token_unchanged_at_text_middle() {
    keeps_a_token_unchanged(handle_plain_text_drop, true, "text middle");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at text end
#[test]
fn plain_backward_true_keeps_a_token_unchanged_at_text_end() {
    keeps_a_token_unchanged(handle_plain_text_drop, true, "text end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at previous end
#[test]
fn plain_backward_true_keeps_a_token_unchanged_at_previous_end() {
    keeps_a_token_unchanged(handle_plain_text_drop, true, "previous end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at next start
#[test]
fn plain_backward_true_keeps_a_token_unchanged_at_next_start() {
    keeps_a_token_unchanged(handle_plain_text_drop, true, "next start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at element start
#[test]
fn plain_backward_true_keeps_a_token_unchanged_at_element_start() {
    keeps_a_token_unchanged(handle_plain_text_drop, true, "element start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at element end
#[test]
fn plain_backward_true_keeps_a_token_unchanged_at_element_end() {
    keeps_a_token_unchanged(handle_plain_text_drop, true, "element end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps nested inline wrappers unchanged at previous end
#[test]
fn plain_backward_true_keeps_nested_inline_wrappers_unchanged_at_previous_end() {
    keeps_nested_inline_wrappers_unchanged(handle_plain_text_drop, true, "previous end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps nested inline wrappers unchanged at next start
#[test]
fn plain_backward_true_keeps_nested_inline_wrappers_unchanged_at_next_start() {
    keeps_nested_inline_wrappers_unchanged(handle_plain_text_drop, true, "next start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps nested inline wrappers unchanged at element start
#[test]
fn plain_backward_true_keeps_nested_inline_wrappers_unchanged_at_element_start() {
    keeps_nested_inline_wrappers_unchanged(handle_plain_text_drop, true, "element start");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps nested inline wrappers unchanged at element end
#[test]
fn plain_backward_true_keeps_nested_inline_wrappers_unchanged_at_element_end() {
    keeps_nested_inline_wrappers_unchanged(handle_plain_text_drop, true, "element end");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a decorator in a multi-node range at the adjacent end edge
#[test]
fn plain_backward_true_keeps_a_decorator_in_a_multi_node_range_at_the_adjacent_end_edge() {
    keeps_a_decorator_at_the_adjacent_end_edge(handle_plain_text_drop, true);
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a text destination after a cross-paragraph deletion
#[test]
fn plain_backward_true_keeps_a_text_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_plain_text_drop, true, "text");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a paragraph destination after a cross-paragraph deletion
#[test]
fn plain_backward_true_keeps_a_paragraph_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_plain_text_drop, true, "paragraph");
}

// upstream: lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a root destination after a cross-paragraph deletion
#[test]
fn plain_backward_true_keeps_a_root_destination_after_a_cross_paragraph_deletion() {
    keeps_a_destination_after_a_cross_paragraph_deletion(handle_plain_text_drop, true, "root");
}
