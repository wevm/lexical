//! `LexicalNode.ts`: the node trait every node implements, and [`NodeRef`],
//! the handle through which code outside a node reads and changes one.

use crate::caret_tree::{
    detach_node_with_selection, error_on_insert_text_node_on_root, insert_node_between,
    insert_sibling, select_adjacent_node,
};
use crate::error::invariant;
use crate::lexical_selection::{
    PointType, SelectionRef, Which, get_selection, maybe_move_children_selection_to_parent,
    move_selection_point_to_end, move_selection_point_to_sibling, set_selection,
};
use crate::lexical_utils::{get_node_by_key_impl, get_writable, mutate, with_writable};
use crate::nodes::element::{self, ElementData, is_root_or_shadow_root};
use crate::nodes::root::RootNode;
use crate::nodes::text::TextData;
use crate::updates::{error_on_read_only, get_active_editor_state};
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

    /// `ElementNode.canIndent()`: whether indenting the selection changes
    /// this block's indent.
    fn can_indent(&self) -> bool {
        true
    }

    /// `ElementNode.canMergeWhenEmpty()`: whether the element, when empty,
    /// takes the children of the first block `insertNodes` inserts.
    fn can_merge_when_empty(&self) -> bool {
        false
    }

    /// `isParentRequired()`: whether the node needs a parent of a certain
    /// kind ([`create_parent_element_node`](Self::create_parent_element_node)).
    fn is_parent_required(&self) -> bool {
        false
    }

    /// `createParentElementNode()`: a new parent for the node, a paragraph
    /// by default.
    fn create_parent_element_node(&self) -> NodeRef {
        crate::nodes::paragraph::create_paragraph_node()
    }

    /// `ElementNode.insertNewAfter(selection, restoreSelection)` on the
    /// element `this` (this node's handle): inserts the block that splitting
    /// the element creates after it, and returns it. `None` (the default)
    /// means the element can't be split.
    fn insert_new_after(
        &self,
        this: &NodeRef,
        selection: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        let _ = (this, selection, restore_selection);
        Ok(None)
    }

    /// `ElementNode.collapseAtStart(selection)` on the element `this`: what
    /// a backward deletion at the element's start does instead of merging
    /// it. `true` means the element handled it. `ElementNode` returns false.
    fn collapse_at_start(
        &self,
        this: &NodeRef,
        selection: &SelectionRef,
    ) -> Result<bool, LexicalError> {
        let _ = (this, selection);
        Ok(false)
    }

    /// `ElementNode.append(...nodesToAppend)` on the element `this`: inserts
    /// `nodes` after its last child, through [`splice`](Self::splice).
    fn append(&self, this: &NodeRef, nodes: &[NodeRef]) -> Result<(), LexicalError> {
        this.splice(this.get_children_size(), 0, nodes)
    }

    /// `ElementNode.splice(start, deleteCount, nodesToInsert)` on the
    /// element `this`; the default is `ElementNode`'s
    /// ([`element::splice`]).
    fn splice(
        &self,
        this: &NodeRef,
        start: usize,
        delete_count: usize,
        nodes_to_insert: &[NodeRef],
    ) -> Result<(), LexicalError> {
        element::splice(this, start, delete_count, nodes_to_insert)
    }

    /// `replace(replaceWith, includeChildren)` on `this`; the default is
    /// `LexicalNode`'s (`node::replace`).
    fn replace(
        &self,
        this: &NodeRef,
        replace_with: &NodeRef,
        include_children: bool,
    ) -> Result<NodeRef, LexicalError> {
        replace(this, replace_with, include_children)
    }

    /// `insertAfter(nodeToInsert, restoreSelection)` on `this`; the default
    /// is `LexicalNode`'s (`node::insert_after`).
    fn insert_after(
        &self,
        this: &NodeRef,
        node_to_insert: &NodeRef,
        restore_selection: bool,
    ) -> Result<NodeRef, LexicalError> {
        insert_after(this, node_to_insert, restore_selection)
    }

    /// `remove(preserveEmptyParent)` on `this`; the default is
    /// `LexicalNode`'s (`node::remove`).
    fn remove(&self, this: &NodeRef, preserve_empty_parent: bool) -> Result<(), LexicalError> {
        remove(this, preserve_empty_parent)
    }

    /// `ElementNode.getIndent()` on the element `this`: its `__indent`.
    fn get_indent(&self, this: &NodeRef) -> Result<f64, LexicalError> {
        let _ = this;
        Ok(self.element().map_or(0.0, |e| e.indent))
    }

    /// `ElementNode.setIndent(indentLevel)` on the element `this`: sets its
    /// `__indent`.
    fn set_indent(&self, this: &NodeRef, indent: f64) -> Result<(), LexicalError> {
        with_writable(this.get_key(), |n| {
            n.element_mut().expect("an element").indent = indent
        });
        Ok(())
    }

    /// The `$transform` of the node's `$config`, which the editor runs on a
    /// dirty node before the transforms registered for its type (and before
    /// `ElementNode`'s own, which comes later in the class chain). None by
    /// default.
    fn transform(&self, this: &NodeRef) -> Result<(), LexicalError> {
        let _ = this;
        Ok(())
    }

    /// `resetOnCopyNodeFrom(originalNode)` on `this`, the copy `$copyNode`
    /// just made of `original`. `LexicalNode`'s resets NodeState, which
    /// isn't ported, so the default does nothing.
    fn reset_on_copy_node_from(&self, this: &NodeRef, original: &NodeRef) {
        let _ = (this, original);
    }

    /// `ElementNode.extractWithChild(child, selection, destination)` on the
    /// element `this`: whether copying `child` (to `"clone"` or `"html"`)
    /// takes this element with it. `ElementNode` returns false. Nothing in
    /// the core calls it; `@lexical/html`'s copy does, and the overrides of
    /// `ParagraphNode` and `HeadingNode` come with it.
    fn extract_with_child(
        &self,
        this: &NodeRef,
        child: &NodeRef,
        selection: Option<&SelectionRef>,
        destination: &str,
    ) -> Result<bool, LexicalError> {
        let _ = (this, child, selection, destination);
        Ok(false)
    }

    /// `TextNode.isTextEntity()`: whether `registerLexicalTextEntity` (the
    /// `text` feature) treats the text node as an entity. False by default.
    fn is_text_entity(&self) -> bool {
        false
    }

    /// `ElementNode.excludeFromCopy(destination)`: whether copying (to
    /// `"clone"` or `"html"`) leaves this element out and keeps its
    /// children. `ElementNode` returns false. Nothing in the core calls it;
    /// `@lexical/clipboard`'s and `@lexical/html`'s copies do.
    fn exclude_from_copy(&self, destination: &str) -> bool {
        let _ = destination;
        false
    }

    /// `TextNode.setFormat(format)` on the text node `this`: sets its
    /// `__format`, after `getWritable()`.
    fn set_format(&self, this: &NodeRef, format: f64) {
        with_writable(this.get_key(), |n| {
            n.text_mut().expect("a text node").format = format
        });
    }

    /// `canInsertTextBefore()` (`TextNode` and `ElementNode`): whether text
    /// typed at the node's start goes into it.
    fn can_insert_text_before(&self) -> bool {
        true
    }

    /// `canInsertTextAfter()` (`TextNode` and `ElementNode`): whether text
    /// typed at the node's end goes into it.
    fn can_insert_text_after(&self) -> bool {
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

    /// `getParents()`: the ancestors, nearest first.
    pub fn get_parents(&self) -> Vec<NodeRef> {
        let mut parents = Vec::new();
        let mut node = self.get_parent();
        while let Some(n) = node {
            node = n.get_parent();
            parents.push(n);
        }
        parents
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

    /// `ElementNode.canIndent()`.
    pub fn can_indent(&self) -> bool {
        self.get_latest().can_indent()
    }

    /// `ElementNode.canMergeWhenEmpty()`.
    pub fn can_merge_when_empty(&self) -> bool {
        self.get_latest().can_merge_when_empty()
    }

    /// `isParentRequired()`.
    pub fn is_parent_required(&self) -> bool {
        self.get_latest().is_parent_required()
    }

    /// `createParentElementNode()`.
    pub fn create_parent_element_node(&self) -> NodeRef {
        self.get_latest().create_parent_element_node()
    }

    /// `ElementNode.insertNewAfter(selection, restoreSelection)`.
    pub fn insert_new_after(
        &self,
        selection: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        self.get_latest()
            .insert_new_after(self, selection, restore_selection)
    }

    /// `ElementNode.collapseAtStart(selection)`.
    pub fn collapse_at_start(&self, selection: &SelectionRef) -> Result<bool, LexicalError> {
        self.get_latest().collapse_at_start(self, selection)
    }

    /// `ElementNode.extractWithChild(child, selection, destination)`.
    pub fn extract_with_child(
        &self,
        child: &NodeRef,
        selection: Option<&SelectionRef>,
        destination: &str,
    ) -> Result<bool, LexicalError> {
        self.get_latest()
            .extract_with_child(self, child, selection, destination)
    }

    /// `TextNode.isTextEntity()`.
    pub fn is_text_entity(&self) -> bool {
        self.get_latest().is_text_entity()
    }

    /// `ElementNode.excludeFromCopy(destination)`.
    pub fn exclude_from_copy(&self, destination: &str) -> bool {
        self.get_latest().exclude_from_copy(destination)
    }

    /// `isParentOf(targetNode)`: whether this node is an ancestor of
    /// `target`.
    pub fn is_parent_of(&self, target: &NodeRef) -> bool {
        if self == target {
            return false;
        }
        let mut node = target.get_parent();
        while let Some(n) = node {
            if &n == self {
                return true;
            }
            node = n.get_parent();
        }
        false
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
        self.get_latest().remove(self, preserve_empty_parent)
    }

    /// `replace(replaceWith, includeChildren)`: puts `replace_with` where
    /// this node is, with this node's children when `include_children`.
    pub fn replace(
        &self,
        replace_with: &NodeRef,
        include_children: bool,
    ) -> Result<NodeRef, LexicalError> {
        self.get_latest()
            .replace(self, replace_with, include_children)
    }

    /// `insertAfter(nodeToInsert, restoreSelection)`.
    pub fn insert_after(
        &self,
        node_to_insert: &NodeRef,
        restore_selection: bool,
    ) -> Result<NodeRef, LexicalError> {
        self.get_latest()
            .insert_after(self, node_to_insert, restore_selection)
    }

    /// `ElementNode.getIndent()`.
    pub fn get_indent(&self) -> Result<f64, LexicalError> {
        self.get_latest().get_indent(self)
    }

    /// `ElementNode.setIndent(indentLevel)`.
    pub fn set_indent(&self, indent: f64) -> Result<(), LexicalError> {
        self.get_latest().set_indent(self, indent)
    }

    /// `getPreviousSiblings()`: the siblings before this node, in document
    /// order.
    pub fn get_previous_siblings(&self) -> Vec<NodeRef> {
        let mut siblings = Vec::new();
        let mut node = self.get_previous_sibling();
        while let Some(n) = node {
            node = n.get_previous_sibling();
            siblings.push(n);
        }
        siblings.reverse();
        siblings
    }

    /// `getNextSiblings()`: the siblings after this node, in document order.
    pub fn get_next_siblings(&self) -> Vec<NodeRef> {
        let mut siblings = Vec::new();
        let mut node = self.get_next_sibling();
        while let Some(n) = node {
            node = n.get_next_sibling();
            siblings.push(n);
        }
        siblings
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

/// `LexicalNode.remove(preserveEmptyParent)`, which nodes that override it
/// call as `super.remove`.
pub(crate) fn remove(this: &NodeRef, preserve_empty_parent: bool) -> Result<(), LexicalError> {
    invariant!(!this.is_root(), "remove: cannot be called on root nodes");
    remove_node(this, true, preserve_empty_parent)
}

/// `LexicalNode.replace(replaceWith, includeChildren)`, which nodes that
/// override it call as `super.replace`.
pub(crate) fn replace(
    this: &NodeRef,
    replace_with: &NodeRef,
    include_children: bool,
) -> Result<NodeRef, LexicalError> {
    invariant!(!this.is_root(), "replace: cannot be called on root nodes");
    error_on_read_only();
    let selection = get_selection().map(|s| s.clone_selection());
    error_on_insert_text_node_on_root(this, replace_with)?;
    let to_replace_key = this.get_key().to_owned();
    get_writable(replace_with.get_key());
    let parent = this.get_parent_or_throw()?;
    get_writable(parent.get_key());
    let size = parent.get_children_size();
    let replace_with_old_parent = replace_with.get_parent();
    let range = selection.as_ref().filter(|s| s.is_range());
    detach_node_with_selection(replace_with, range)?;
    let prev_sibling = this.get_previous_sibling();
    let next_sibling = this.get_next_sibling();
    remove_node(this, false, true)?;
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
            this.is_element() && replace_with.is_element(),
            "includeChildren should only be true for ElementNodes"
        );
        prev_size_before_children_transfer = replace_with.get_children_size();
        replace_with.splice(prev_size_before_children_transfer, 0, &this.get_children())?;
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

/// `LexicalNode.insertAfter(nodeToInsert, restoreSelection)`, which nodes
/// that override it call as `super.insertAfter`.
pub(crate) fn insert_after(
    this: &NodeRef,
    node_to_insert: &NodeRef,
    restore_selection: bool,
) -> Result<NodeRef, LexicalError> {
    invariant!(
        !this.is_root(),
        "insertAfter: cannot be called on root nodes"
    );
    insert_sibling(this, true, node_to_insert, restore_selection)
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
