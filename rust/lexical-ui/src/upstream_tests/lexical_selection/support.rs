//! Helpers shared by the ports of `packages/lexical-selection`'s unit tests:
//! the selection test utils (`packages/lexical-selection/src/__tests__/utils`)
//! and the DOM a JS test reads from its mounted editor.

#![allow(dead_code)]

use crate::{
    Editor, LexicalError, LexicalNode, NodeRef, Point, PointType, Which, create_text_node,
    get_selection,
};
use std::rc::Rc;

/// `$setAnchorPoint(point)`: sets the anchor of the range selection,
/// selecting a detached text node first when there is none.
pub(crate) fn set_anchor_point(
    key: &str,
    offset: usize,
    point_type: PointType,
) -> Result<(), LexicalError> {
    set_point(Which::Anchor, key, offset, point_type)
}

/// `$setFocusPoint(point)`.
pub(crate) fn set_focus_point(
    key: &str,
    offset: usize,
    point_type: PointType,
) -> Result<(), LexicalError> {
    set_point(Which::Focus, key, offset, point_type)
}

fn set_point(
    which: Which,
    key: &str,
    offset: usize,
    point_type: PointType,
) -> Result<(), LexicalError> {
    match get_selection().filter(crate::SelectionRef::is_range) {
        Some(selection) => selection.point_set(which, key, offset, point_type),
        None => {
            let dummy = create_text_node("");
            dummy.select(None, None)?;
            set_point(which, key, offset, point_type)
        }
    }
}

/// A point as `key:offset:type`, standing in for
/// `expect(point).toEqual(expect.objectContaining({key, offset, type}))`.
pub(crate) fn point_string(point: &Point) -> String {
    format!(
        "{}:{}:{}",
        point.key,
        point.offset,
        point.point_type.as_str()
    )
}

/// The points of a range selection, as [`point_string`]s.
pub(crate) fn points_of(selection: &crate::SelectionRef) -> (String, String) {
    (
        point_string(&selection.point(Which::Anchor).expect("a range selection")),
        point_string(&selection.point(Which::Focus).expect("a range selection")),
    )
}

/// `new TextNode(text, key)` put in the pending node map by hand, as
/// `$createParagraphWithNodes` does.
pub(crate) fn create_text_node_with_key(text: &str, key: &str) -> NodeRef {
    let mut node: Box<dyn LexicalNode> = Box::new(crate::nodes::text::TextNode::default());
    node.text_mut().expect("a text node").text = text.to_owned();
    let editor_state = crate::updates::get_active_editor_state();
    node.base_mut().key = key.to_owned();
    node.base_mut().written_in = editor_state.0.id;
    editor_state
        .0
        .node_map
        .borrow_mut()
        .insert(key, Rc::from(node));
    if let Some(editor) = crate::updates::get_active_editor() {
        editor.mark_dirty(key, false);
    }
    NodeRef::new(key)
}

/// `$createParagraphWithNodes(editor, nodes)`: a paragraph of text nodes
/// with the given keys, `(key, text, mergeable)`.
pub(crate) fn create_paragraph_with_nodes(nodes: &[(&str, &str, bool)]) -> NodeRef {
    let paragraph = crate::create_paragraph_node();
    for &(key, text, mergeable) in nodes {
        let text_node = create_text_node_with_key(text, key);
        if !mergeable {
            text_node.toggle_unmergeable();
        }
        paragraph.append(&[text_node]).unwrap();
    }
    paragraph
}

/// The DOM the reconciler mounts for the committed document, as the root
/// element's `innerHTML`: each node's `createDOM`, with the attributes
/// `$createNode` adds (`data-lexical-text`; `data-lexical-decorator` and
/// `contenteditable`; `dir`, `auto` on a block in the root or a shadow
/// root) and the managed `<br>` of a block that is empty or ends in a line
/// break or an inline decorator.
#[cfg(feature = "html")]
pub(crate) fn rendered_html(editor: &Editor) -> String {
    editor
        .read_with_mode(crate::EditorReadMode::Latest, || {
            let container = crate::lexical_dom::get_document().create_element("div");
            for child in crate::get_root().get_children() {
                container.append_child(&render_dom(&child, editor));
            }
            container.inner_html()
        })
        .unwrap()
}

#[cfg(feature = "html")]
fn render_dom(node: &NodeRef, editor: &Editor) -> crate::dom::DomNode {
    let dom = node
        .get_latest()
        .create_dom(node, editor.config(), editor)
        .unwrap();
    if node.is_text() {
        dom.set_attribute("data-lexical-text", "true");
    } else if node.get_latest().is_decorator() {
        dom.set_attribute("data-lexical-decorator", "true");
        dom.set_attribute("contenteditable", "false");
    }
    if !node.is_element() {
        return dom;
    }
    if let Some(direction) = node.element_data().get_direction() {
        dom.set_attribute("dir", direction.as_str());
    } else if let Some(parent) = node.get_parent()
        && crate::is_root_or_shadow_root(&parent)
        && parent.element_data().get_direction().is_none()
    {
        dom.set_attribute("dir", "auto");
    }
    // `setElementIndent`.
    let indent = node.element_data().get_indent();
    if indent != 0.0 {
        dom.style().set_property(
            "padding-inline-start",
            &format!(
                "calc({} * var(--lexical-indent-base-value, 40px))",
                crate::json::number_to_string(indent)
            ),
            "",
        );
    }
    let children = node.get_children();
    for child in &children {
        dom.append_child(&render_dom(child, editor));
    }
    if !node.is_inline() {
        let managed = match children.last() {
            None => true,
            Some(last) => {
                last.get_type() == "linebreak"
                    || (last.get_latest().is_decorator() && last.is_inline())
            }
        };
        if managed {
            let br = crate::lexical_dom::get_document().create_element("br");
            br.set_attribute("data-lexical-managed-linebreak", "true");
            dom.append_child(&br);
        }
    }
    dom
}

/// A test decorator that renders a `span` and is a block, as upstream's
/// `TestDecoratorNode` is with `isInline` mocked to return `false`.
#[derive(Clone, Debug, Default)]
pub(crate) struct BlockTestDecoratorNode {
    base: crate::NodeBase,
}

impl LexicalNode for BlockTestDecoratorNode {
    fn get_type(&self) -> &'static str {
        "test_decorator"
    }
    fn base(&self) -> &crate::NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut crate::NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        false
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
}
