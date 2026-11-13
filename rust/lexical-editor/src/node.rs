//! `LexicalNode.ts`: the node trait every node implements, and [`NodeRef`],
//! the handle through which code outside a node reads and changes one.

use crate::caret_tree;
use crate::nodes::element::{self, ElementData};
use crate::nodes::root::RootNode;
use crate::nodes::text::TextData;
use crate::updates::get_active_editor_state;
use crate::utils::{get_node_by_key_impl, with_writable};
use crate::{LexicalError, json::Object};
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// A node's key in its editor state's node map. Keys are strings, as in JS,
/// from a per-thread counter (`generateRandomKey`).
pub type NodeKey = String;

/// The fields every node has: its key and its links to its parent and
/// siblings (`__key`, `__parent`, `__prev`, `__next`).
#[derive(Clone, Debug, Default)]
pub struct NodeBase {
    pub(crate) key: NodeKey,
    pub(crate) parent: Option<NodeKey>,
    pub(crate) prev: Option<NodeKey>,
    pub(crate) next: Option<NodeKey>,
}

/// What every node implements. JS's class hierarchy (`LexicalNode`,
/// `ElementNode`, `TextNode`, ...) becomes this trait plus the data each base
/// class adds ([`ElementData`], [`TextData`]); a node that extends
/// `ElementNode` returns its `ElementData` from [`element`](Self::element).
///
/// A node is a value: an editor state holds each one behind an `Rc`, and a
/// change clones it first unless nothing else shares it (`getWritable`), so
/// `Clone` is the JS `clone` plus `afterCloneFrom`.
///
/// Methods that JS subclasses override call `super`; here the base class's
/// behavior is a free function in that class's module (such as
/// `nodes::element::get_text_content`) that an override can call.
pub trait LexicalNode: NodeClone + fmt::Debug + 'static {
    /// `getType()`: the type the node serializes as.
    fn get_type(&self) -> &'static str;

    fn base(&self) -> &NodeBase;
    fn base_mut(&mut self) -> &mut NodeBase;

    /// The `ElementNode` fields, for a node that extends it.
    fn element(&self) -> Option<&ElementData> {
        None
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        None
    }

    /// The `TextNode` fields, for a node that extends it.
    fn text(&self) -> Option<&TextData> {
        None
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        None
    }

    /// `isInline()`: false for an element, true for text.
    fn is_inline(&self) -> bool {
        self.text().is_some()
    }

    /// `ElementNode.isShadowRoot()`.
    fn is_shadow_root(&self) -> bool {
        false
    }

    /// `getTextContent()`.
    fn get_text_content(&self) -> String {
        if let Some(text) = self.text() {
            return text.text.clone();
        }
        if self.element().is_some() {
            return element::get_text_content(self.as_node());
        }
        String::new()
    }

    /// `ElementNode.shouldSerializeTextStyles()`.
    fn should_serialize_text_styles(&self) -> bool {
        element::should_serialize_text_styles(self.as_node())
    }

    /// `exportJSON(compact)`: the node's own properties, with an empty
    /// `children` array for an element (the export walk fills it).
    fn export_json(&self, compact: bool) -> Object;

    /// `updateFromJSON(json)` on a writable node: applies every property the
    /// node's schema declares, a missing one as its default.
    fn update_from_json(&mut self, json: &Object);
}

/// The parts of [`LexicalNode`] every implementation writes the same way,
/// implemented for any `Clone` node.
pub trait NodeClone {
    fn clone_node(&self) -> Box<dyn LexicalNode>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_node(&self) -> &dyn LexicalNode;
}

impl<T: LexicalNode + Clone> NodeClone for T {
    fn clone_node(&self) -> Box<dyn LexicalNode> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_node(&self) -> &dyn LexicalNode {
        self
    }
}

impl dyn LexicalNode {
    pub fn get_key(&self) -> &str {
        &self.base().key
    }

    /// Whether the node is a `T` (JS `instanceof`).
    pub fn is<T: LexicalNode>(&self) -> bool {
        self.as_any().is::<T>()
    }

    pub fn downcast_ref<T: LexicalNode>(&self) -> Option<&T> {
        self.as_any().downcast_ref()
    }

    pub fn downcast_mut<T: LexicalNode>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut()
    }
}

/// A node by its key, resolved in the active editor state on every call, so
/// it always reads the latest version, as JS node methods do through
/// `getLatest()`. Like a JS node reference, it can only be used inside
/// [`Editor::read`](crate::Editor::read) or
/// [`EditorState::read`](crate::EditorState::read), and panics elsewhere.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeRef {
    key: NodeKey,
}

impl NodeRef {
    pub(crate) fn new(key: impl Into<NodeKey>) -> Self {
        Self { key: key.into() }
    }

    pub fn get_key(&self) -> &str {
        &self.key
    }

    /// `getLatest()`: the node's version in the active editor state.
    pub fn get_latest(&self) -> Rc<dyn LexicalNode> {
        get_node_by_key_impl(&get_active_editor_state(), &self.key).unwrap_or_else(|| {
            panic!(
                "Lexical node does not exist in active editor state. Avoid using the same node references between nested closures from editorState.read/editor.update."
            )
        })
    }

    pub fn get_type(&self) -> &'static str {
        self.get_latest().get_type()
    }

    /// `$isElementNode(node)`.
    pub fn is_element(&self) -> bool {
        self.get_latest().element().is_some()
    }

    /// `$isTextNode(node)`.
    pub fn is_text(&self) -> bool {
        self.get_latest().text().is_some()
    }

    /// `$isRootNode(node)`.
    pub fn is_root(&self) -> bool {
        self.get_latest().is::<RootNode>()
    }

    pub fn is_inline(&self) -> bool {
        self.get_latest().is_inline()
    }

    pub fn get_parent(&self) -> Option<NodeRef> {
        self.get_latest().base().parent.clone().map(NodeRef::new)
    }

    pub fn get_previous_sibling(&self) -> Option<NodeRef> {
        self.get_latest().base().prev.clone().map(NodeRef::new)
    }

    pub fn get_next_sibling(&self) -> Option<NodeRef> {
        self.get_latest().base().next.clone().map(NodeRef::new)
    }

    pub fn get_first_child(&self) -> Option<NodeRef> {
        let node = self.get_latest();
        node.element()?.first.clone().map(NodeRef::new)
    }

    pub fn get_last_child(&self) -> Option<NodeRef> {
        let node = self.get_latest();
        node.element()?.last.clone().map(NodeRef::new)
    }

    pub fn get_children_size(&self) -> usize {
        self.get_latest().element().map_or(0, |e| e.size)
    }

    /// `getChildren()`; empty for a node that isn't an element.
    pub fn get_children(&self) -> Vec<NodeRef> {
        let mut children = Vec::with_capacity(self.get_children_size());
        let mut child = self.get_first_child();
        while let Some(c) = child {
            child = c.get_next_sibling();
            children.push(c);
        }
        children
    }

    pub fn get_text_content(&self) -> String {
        self.get_latest().get_text_content()
    }

    /// `exportJSON(compact)`.
    pub fn export_json(&self, compact: bool) -> Object {
        self.get_latest().export_json(compact)
    }

    /// `append(...nodes)`, the insertion half of `ElementNode.splice` (with
    /// `RootNode.splice`'s check), until the update API is ported.
    pub(crate) fn append(&self, nodes: &[NodeRef]) -> Result<(), LexicalError> {
        caret_tree::append(self, nodes)
    }

    /// `updateFromJSON(json)`.
    pub(crate) fn update_from_json(&self, json: &Object) {
        with_writable(&self.key, |node| node.update_from_json(json));
    }
}
