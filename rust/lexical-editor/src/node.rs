//! `LexicalNode.ts`: the node trait every node implements, and [`NodeRef`],
//! the handle through which code outside a node reads and changes one.

use crate::caret_tree::{
    detach_node_with_selection, error_on_insert_text_node_on_root, insert_node_between,
    insert_sibling, select_adjacent_node,
};
use crate::error::invariant;
use crate::nodes::element::{self, ElementData, is_root_or_shadow_root};
use crate::nodes::root::RootNode;
use crate::nodes::text::TextData;
use crate::selection::{
    PointType, SelectionRef, Which, get_selection, maybe_move_children_selection_to_parent,
    move_selection_point_to_end, move_selection_point_to_sibling, set_selection,
};
use crate::updates::{error_on_read_only, get_active_editor_state};
use crate::utils::{get_node_by_key_impl, get_writable, mutate, with_writable};
use crate::{LexicalError, json::Object, utf16};
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

    /// `ElementNode.canBeEmpty()`: whether the element stays when its last
    /// child is removed.
    fn can_be_empty(&self) -> bool {
        true
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

    /// `updateFromJSON(json)`.
    pub(crate) fn update_from_json(&self, json: &Object) {
        with_writable(&self.key, |node| node.update_from_json(json));
    }

    /// `getParentOrThrow()`.
    pub fn get_parent_or_throw(&self) -> Result<NodeRef, LexicalError> {
        self.get_parent().ok_or_else(|| {
            LexicalError::new(format!("Expected node {} to have a parent.", self.key))
        })
    }

    /// `isAttached()`: whether the node's parents lead to the root.
    pub fn is_attached(&self) -> bool {
        let editor_state = get_active_editor_state();
        let node_map = editor_state.0.node_map.borrow();
        let mut key = self.key.as_str();
        loop {
            if key == "root" {
                return true;
            }
            match node_map.get(key).and_then(|n| n.base().parent.as_deref()) {
                Some(parent) => key = parent,
                None => return false,
            }
        }
    }

    /// `getIndexWithinParent()`; `None` where JS returns -1.
    pub fn get_index_within_parent(&self) -> Option<usize> {
        let parent = self.get_parent()?;
        let mut node = parent.get_first_child();
        let mut index = 0;
        while let Some(n) = node {
            if &n == self {
                return Some(index);
            }
            index += 1;
            node = n.get_next_sibling();
        }
        None
    }

    /// `getTextContentSize()`: the text content's length in UTF-16 code
    /// units.
    pub fn get_text_content_size(&self) -> usize {
        if self.is_element() {
            return self.get_element_text_content_size();
        }
        utf16::len(&self.get_text_content())
    }

    /// `ElementNode.canBeEmpty()`.
    pub fn can_be_empty(&self) -> bool {
        self.get_latest().can_be_empty()
    }

    /// `isSelected()` for a node selection. Range selections come with
    /// `getNodes`.
    fn is_selected_by_node_selection(&self, selection: &SelectionRef) -> bool {
        selection.with_node(|n| n.has(&self.key)).unwrap_or(false)
    }

    /// `markDirty()`: marks the node dirty, so transforms run on it.
    pub fn mark_dirty(&self) {
        get_writable(&self.key);
    }

    /// `remove(preserveEmptyParent)`.
    pub fn remove(&self, preserve_empty_parent: bool) -> Result<(), LexicalError> {
        invariant!(!self.is_root(), "remove: cannot be called on root nodes");
        remove_node(self, true, preserve_empty_parent)
    }

    /// `replace(replaceWith, includeChildren)`: puts `replace_with` where
    /// this node is, with this node's children when `include_children`.
    pub fn replace(
        &self,
        replace_with: &NodeRef,
        include_children: bool,
    ) -> Result<NodeRef, LexicalError> {
        invariant!(!self.is_root(), "replace: cannot be called on root nodes");
        error_on_read_only();
        let selection = get_selection().map(|s| s.clone_selection());
        error_on_insert_text_node_on_root(self, replace_with)?;
        let to_replace_key = self.key.clone();
        get_writable(replace_with.get_key());
        let parent = self.get_parent_or_throw()?;
        get_writable(parent.get_key());
        let size = parent.get_children_size();
        let replace_with_old_parent = replace_with.get_parent();
        let range = selection.as_ref().filter(|s| s.is_range());
        detach_node_with_selection(replace_with, range)?;
        let prev_sibling = self.get_previous_sibling();
        let next_sibling = self.get_next_sibling();
        remove_node(self, false, true)?;
        if let Some(p) = &prev_sibling {
            get_writable(p.get_key());
        }
        if let Some(n) = &next_sibling {
            get_writable(n.get_key());
        }
        insert_node_between(
            parent.get_key(),
            replace_with.get_key(),
            prev_sibling.as_ref().map(NodeRef::get_key),
            next_sibling.as_ref().map(NodeRef::get_key),
        );
        // When replace_with was already a child of the same parent, two
        // children became one.
        let size = if replace_with_old_parent.as_ref() == Some(&parent) {
            size - 1
        } else {
            size
        };
        mutate(parent.get_key(), |p| {
            p.element_mut().expect("a parent is an element").size = size
        });
        let mut prev_size_before_children_transfer = 0;
        if include_children {
            invariant!(
                self.is_element() && replace_with.is_element(),
                "includeChildren should only be true for ElementNodes"
            );
            prev_size_before_children_transfer = replace_with.get_children_size();
            replace_with.splice(prev_size_before_children_transfer, 0, &self.get_children())?;
        }
        if let Some(selection) = range {
            set_selection(Some(selection.clone()));
            for which in [Which::Anchor, Which::Focus] {
                let point = selection.point(which).expect("a range selection");
                if point.key == to_replace_key {
                    if include_children && point.point_type == PointType::Element {
                        selection.point_set(
                            which,
                            replace_with.get_key(),
                            prev_size_before_children_transfer + point.offset,
                            PointType::Element,
                        )?;
                    } else {
                        move_selection_point_to_end(selection, which, replace_with)?;
                    }
                }
            }
        }
        Ok(replace_with.clone())
    }

    /// `insertAfter(nodeToInsert, restoreSelection)`.
    pub fn insert_after(
        &self,
        node_to_insert: &NodeRef,
        restore_selection: bool,
    ) -> Result<NodeRef, LexicalError> {
        invariant!(
            !self.is_root(),
            "insertAfter: cannot be called on root nodes"
        );
        insert_sibling(self, true, node_to_insert, restore_selection)
    }

    /// `insertBefore(nodeToInsert, restoreSelection)`.
    pub fn insert_before(
        &self,
        node_to_insert: &NodeRef,
        restore_selection: bool,
    ) -> Result<NodeRef, LexicalError> {
        invariant!(
            !self.is_root(),
            "insertBefore: cannot be called on root nodes"
        );
        insert_sibling(self, false, node_to_insert, restore_selection)
    }

    /// `select(anchorOffset, focusOffset)` of a text node or an element;
    /// `None` is JS's `undefined` (the end).
    pub fn select(
        &self,
        anchor_offset: Option<usize>,
        focus_offset: Option<usize>,
    ) -> Result<SelectionRef, LexicalError> {
        if self.is_text() {
            self.select_text(anchor_offset, focus_offset)
        } else if self.is_element() {
            self.select_element(anchor_offset, focus_offset)
        } else {
            Err(LexicalError::new(format!(
                "select: node {} is not a text node or an element",
                self.key
            )))
        }
    }

    /// `selectStart()`.
    pub fn select_start(&self) -> Result<SelectionRef, LexicalError> {
        if self.is_text() {
            self.select(Some(0), Some(0))
        } else if self.is_element() {
            match self.get_first_descendant() {
                Some(first) => first.select_start(),
                None => self.select(None, None),
            }
        } else {
            self.select_previous(None, None)
        }
    }

    /// `selectEnd()`.
    pub fn select_end(&self) -> Result<SelectionRef, LexicalError> {
        if self.is_text() {
            let size = self.get_text_content_size();
            self.select(Some(size), Some(size))
        } else if self.is_element() {
            match self.get_last_descendant() {
                Some(last) => last.select_end(),
                None => self.select(None, None),
            }
        } else {
            self.select_next(Some(0), Some(0))
        }
    }

    /// `selectPrevious(anchorOffset, focusOffset)`.
    pub fn select_previous(
        &self,
        anchor_offset: Option<usize>,
        focus_offset: Option<usize>,
    ) -> Result<SelectionRef, LexicalError> {
        select_adjacent_node(self, false, anchor_offset, focus_offset)
    }

    /// `selectNext(anchorOffset, focusOffset)`.
    pub fn select_next(
        &self,
        anchor_offset: Option<usize>,
        focus_offset: Option<usize>,
    ) -> Result<SelectionRef, LexicalError> {
        select_adjacent_node(self, true, anchor_offset, focus_offset)
    }
}

/// `$removeNode(nodeToRemove, restoreSelection, preserveEmptyParent)`.
pub(crate) fn remove_node(
    node_to_remove: &NodeRef,
    restore_selection: bool,
    preserve_empty_parent: bool,
) -> Result<(), LexicalError> {
    error_on_read_only();
    let key = node_to_remove.get_key().to_owned();
    let Some(parent) = node_to_remove.get_parent() else {
        return Ok(());
    };
    let selection = maybe_move_children_selection_to_parent(node_to_remove)?;
    let mut selection_moved = false;
    match &selection {
        Some(range) if range.is_range() && restore_selection => {
            for which in [Which::Anchor, Which::Focus] {
                if range.point(which).expect("a range selection").key == key {
                    move_selection_point_to_sibling(
                        range,
                        which,
                        node_to_remove,
                        &parent,
                        node_to_remove.get_previous_sibling().as_ref(),
                        node_to_remove.get_next_sibling().as_ref(),
                    )?;
                    selection_moved = true;
                }
            }
        }
        Some(nodes)
            if nodes.is_node()
                && restore_selection
                && node_to_remove.is_selected_by_node_selection(nodes) =>
        {
            node_to_remove.select_previous(None, None)?;
        }
        _ => {}
    }

    get_writable(&key);
    let range = selection
        .as_ref()
        .filter(|s| restore_selection && !selection_moved && s.is_range());
    detach_node_with_selection(node_to_remove, range)?;

    if !preserve_empty_parent
        && !is_root_or_shadow_root(&*parent.get_latest())
        && !parent.can_be_empty()
        && parent.is_empty()
    {
        remove_node(&parent, restore_selection, false)?;
    }
    if restore_selection && selection.is_some() && parent.is_root() && parent.is_empty() {
        parent.select_end()?;
    }
    Ok(())
}
