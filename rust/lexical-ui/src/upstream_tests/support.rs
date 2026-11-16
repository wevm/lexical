//! Helpers shared by the ported upstream tests.

use crate::json::Value;

/// Vitest's `toEqual` on JSON: objects compare by content whatever their
/// key order, and a property whose value is `undefined` counts as absent.
pub(crate) fn json_to_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            let defined = |o: &crate::json::Object| {
                o.iter()
                    .filter(|(_, v)| !v.is_undefined())
                    .map(|(k, _)| k.to_owned())
                    .collect::<std::collections::BTreeSet<_>>()
            };
            defined(a) == defined(b)
                && a.iter()
                    .filter(|(_, v)| !v.is_undefined())
                    .all(|(k, v)| json_to_equal(v, b.get(k).expect("a defined key")))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| json_to_equal(x, y))
        }
        _ => a == b,
    }
}

/// The message of a caught panic.
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default()
}

use crate::{Editor, LexicalError, NodeRef, RootElement, RootElementRef, UpdateOptions, get_root};
use std::cell::RefCell;
use std::rc::Rc;

/// A root element with no view. JS's unit tests mount their editors on a
/// `contenteditable` div, and an editor with a root element reconciles.
pub(crate) struct TestRootElement;

impl RootElement for TestRootElement {}

/// `editor.setRootElement(container)`, with a [`TestRootElement`].
pub(crate) fn set_test_root_element(editor: &Editor) {
    editor
        .set_root_element(Some(RootElementRef(Rc::new(TestRootElement))))
        .unwrap();
}

/// `editor.update(fn, {discrete: true})`, which panics if the update fails.
/// JS's test editors throw from `onError`; the unit tests `await` their
/// updates, which commits them.
pub(crate) fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
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

/// [`update`], returning what `f` returns.
pub(crate) fn update_with<R: 'static>(
    editor: &Editor,
    f: impl FnOnce() -> Result<R, LexicalError> + 'static,
) -> R {
    let out = Rc::new(RefCell::new(None));
    let slot = out.clone();
    update(editor, move || {
        *slot.borrow_mut() = Some(f()?);
        Ok(())
    });
    out.take().expect("the update ran")
}

/// The committed document's shape, which stands for the HTML upstream reads
/// from the mounted editor: `<type>...</type>` for an element, `[text]` for a
/// text node, `<br>` for a line break and `<type/>` for any other node.
pub(crate) fn shape(editor: &Editor) -> String {
    editor
        .read_with_mode(crate::EditorReadMode::Latest, || {
            get_root().get_children().iter().map(shape_of).collect()
        })
        .unwrap()
}

/// [`shape`] of one node.
pub(crate) fn shape_of(node: &NodeRef) -> String {
    if node.is_element() {
        let children: String = node.get_children().iter().map(shape_of).collect();
        format!("<{t}>{children}</{t}>", t = node.get_type())
    } else if node.is_text() {
        format!("[{}]", node.get_text_content())
    } else if node.get_type() == "linebreak" {
        "<br>".to_owned()
    } else {
        format!("<{}/>", node.get_type())
    }
}

/// Whether `f` panics, as a JS call that throws (for a node read outside an
/// update or a read, where a `NodeRef` panics).
pub(crate) fn panics<R>(f: impl FnOnce() -> R) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err()
}

// The test nodes of `packages/lexical/src/__tests__/utils/index.tsx`.

use crate::nodes::element::ElementData;
use crate::nodes::text::TextData;
use crate::{CreateEditorArgs, LexicalNode, NodeBase, NodeClass};

/// `TestElementNode` (`test_block`): a block element that renders a `div`.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestElementNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for TestElementNode {
    fn get_type(&self) -> &'static str {
        "test_block"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("div"))
    }
}

/// `$createTestElementNode()`.
pub(crate) fn create_test_element_node() -> NodeRef {
    crate::lexical_utils::set_node_key(Box::new(TestElementNode::default()))
}

/// `TestInlineElementNode` (`test_inline_block`): an inline element that
/// can't be empty and renders an `a`.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestInlineElementNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for TestInlineElementNode {
    fn get_type(&self) -> &'static str {
        "test_inline_block"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
    fn is_inline(&self) -> bool {
        true
    }
    fn can_be_empty(&self) -> bool {
        false
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("a"))
    }
}

/// `$createTestInlineElementNode()`.
pub(crate) fn create_test_inline_element_node() -> NodeRef {
    crate::lexical_utils::set_node_key(Box::new(TestInlineElementNode::default()))
}

/// `TestSegmentedNode` (`test_segmented`): a `TextNode` subclass.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestSegmentedNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for TestSegmentedNode {
    fn get_type(&self) -> &'static str {
        "test_segmented"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
}

/// `$createTestSegmentedNode(text)`: `new TestSegmentedNode(text)` in
/// segmented mode.
pub(crate) fn create_test_segmented_node(text: &str) -> NodeRef {
    let node = TestSegmentedNode {
        text: TextData {
            text: text.to_owned(),
            ..TextData::default()
        },
        ..TestSegmentedNode::default()
    };
    let node = crate::lexical_utils::set_node_key(Box::new(node));
    node.set_mode("segmented").unwrap();
    node
}

/// `TestDecoratorNode` (`test_decorator`): an inline decorator whose text
/// is `Hello world`.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestDecoratorNode {
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
}

/// `$createTestDecoratorNode()`.
pub(crate) fn create_test_decorator_node() -> NodeRef {
    crate::lexical_utils::set_node_key(Box::new(TestDecoratorNode::default()))
}

/// `createTestEditor(config)`: `DEFAULT_NODES` (those the build has) and
/// `config.nodes`.
pub(crate) fn create_test_editor(mut args: CreateEditorArgs) -> Editor {
    #[allow(unused_mut)]
    let mut nodes = vec![
        NodeClass::of::<TestElementNode>(),
        NodeClass::of::<TestSegmentedNode>(),
        NodeClass::of::<TestDecoratorNode>(),
        NodeClass::of::<TestInlineElementNode>(),
    ];
    #[cfg(feature = "rich-text")]
    nodes.extend([
        NodeClass::of::<crate::rich_text::HeadingNode>(),
        NodeClass::of::<crate::rich_text::QuoteNode>(),
    ]);
    #[cfg(feature = "list")]
    nodes.extend([
        NodeClass::of::<crate::list::ListNode>(),
        NodeClass::of::<crate::list::ListItemNode>(),
    ]);
    #[cfg(feature = "link")]
    nodes.extend([
        NodeClass::of::<crate::link::LinkNode>(),
        NodeClass::of::<crate::link::AutoLinkNode>(),
    ]);
    nodes.append(&mut args.nodes);
    args.nodes = nodes;
    crate::create_editor(args)
}

/// `initializeUnitTest`'s editor: [`create_test_editor`] with
/// `{namespace: 'test', theme: {}}` (or `args`), on a root element.
pub(crate) fn initialize_unit_test(args: CreateEditorArgs) -> Editor {
    crate::reset_random_key();
    let editor = create_test_editor(args);
    set_test_root_element(&editor);
    editor
}

/// The window of a JS unit test's mounted editor: jsdom's, installed after
/// Lexical loaded, so it gives no DOM selection and `Selection.modify`
/// stops after its model step (where an editor with no window throws).
#[cfg_attr(not(feature = "extension"), allow(dead_code))]
pub(crate) struct JsdomWindow;

impl crate::SelectionMovement for JsdomWindow {
    fn landed_point(
        &self,
        _from: &crate::Point,
        _is_backward: bool,
        _granularity: crate::SelectionGranularity,
    ) -> Result<Option<crate::Point>, LexicalError> {
        Ok(None)
    }

    fn has_dom_selection(&self) -> bool {
        false
    }
}

/// `editor.setRootElement(container)` in a JS unit test: a
/// [`TestRootElement`] and the [`JsdomWindow`].
#[cfg_attr(not(feature = "extension"), allow(dead_code))]
pub(crate) fn mount_in_jsdom(editor: &Editor) {
    editor.set_selection_movement(Some(Rc::new(JsdomWindow)));
    set_test_root_element(editor);
}

/// `TestShadowRootNode` (`test_shadow_root`): a shadow root element.
#[cfg_attr(not(feature = "extension"), allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub(crate) struct TestShadowRootNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for TestShadowRootNode {
    fn get_type(&self) -> &'static str {
        "test_shadow_root"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
    fn is_shadow_root(&self) -> bool {
        true
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("div"))
    }
}

/// `$createTestShadowRootNode()`.
#[cfg_attr(not(feature = "extension"), allow(dead_code))]
pub(crate) fn create_test_shadow_root_node() -> NodeRef {
    crate::create::<TestShadowRootNode>()
}

/// `TestDecoratorNode` with its `setIsInline` (`test_decorator`), for the
/// editors a test builds from extensions: inline unless
/// [`set_is_inline`]`(false)`, and its text is `Hello world`.
#[cfg_attr(not(feature = "extension"), allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub(crate) struct TestBlockableDecoratorNode {
    base: NodeBase,
    block: bool,
}

impl LexicalNode for TestBlockableDecoratorNode {
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
    fn is_inline(&self) -> bool {
        !self.block
    }
    fn get_text_content(&self) -> String {
        "Hello world".to_owned()
    }
    fn decorate(&self, _editor: &Editor) -> Option<Rc<dyn std::any::Any>> {
        Some(Rc::new("Hello world"))
    }
}

/// `$createTestDecoratorNode()`, as a [`TestBlockableDecoratorNode`].
#[cfg_attr(not(feature = "extension"), allow(dead_code))]
pub(crate) fn create_test_blockable_decorator_node() -> NodeRef {
    crate::create::<TestBlockableDecoratorNode>()
}

/// `testDecorator.setIsInline(inline)`.
#[cfg_attr(not(feature = "extension"), allow(dead_code))]
pub(crate) fn set_is_inline(node: &NodeRef, inline: bool) -> NodeRef {
    crate::lexical_utils::with_writable(node.get_key(), |n| {
        n.as_any_mut()
            .downcast_mut::<TestBlockableDecoratorNode>()
            .expect("a TestBlockableDecoratorNode")
            .block = !inline;
    });
    node.clone()
}

/// `$createTestDecoratorNode().setIsInline(inline)`, a
/// [`TestBlockableDecoratorNode`].
#[cfg_attr(not(feature = "rich-text"), allow(dead_code))] // used by feature-gated ports
pub(crate) fn create_test_decorator_node_inline(inline: bool) -> NodeRef {
    set_is_inline(&create_test_blockable_decorator_node(), inline)
}

/// The active selection, which must be a range selection.
#[cfg_attr(not(feature = "rich-text"), allow(dead_code))] // used by feature-gated ports
pub(crate) fn range_selection() -> crate::SelectionRef {
    let selection = crate::get_selection().filter(crate::SelectionRef::is_range);
    selection.expect("Expected RangeSelection")
}
