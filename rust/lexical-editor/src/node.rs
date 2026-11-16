//! `LexicalNode.ts`: the node trait every node implements, and [`NodeRef`],
//! the handle through which code outside a node reads and changes one.

use crate::caret::{
    CommonAncestorResult, get_common_ancestor, get_common_ancestor_result_branch_order,
};
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
use crate::node_state::{NodeStateRef, RequiredNodeStateConfig};
use crate::nodes::element::{self, ElementData, is_root_or_shadow_root};
use crate::nodes::root::RootNode;
use crate::nodes::text::TextData;
use crate::updates::{error_on_read_only, get_active_editor_state};
use crate::{LexicalError, json::Object, utf16};
use std::any::Any;
use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

/// A node's key in its editor state's node map. Keys are strings, as in JS,
/// from a per-thread counter (`generateRandomKey`).
pub type NodeKey = String;

/// The fields every node has: its key, its links to its parent and
/// siblings (`__key`, `__parent`, `__prev`, `__next`) and its NodeState
/// (`__state`), which a clone shares until one of them writes it; and, used
/// by elements and decorators only, the slot fields JS's `ElementNode` and
/// `DecoratorNode` add (`__slotHost`, `__slots`). A clone keeps them, as
/// their `afterCloneFrom` does for the same key; `$copyNode` drops them.
#[derive(Clone, Debug, Default)]
pub struct NodeBase {
    pub(crate) key: NodeKey,
    pub(crate) parent: Option<NodeKey>,
    pub(crate) prev: Option<NodeKey>,
    pub(crate) next: Option<NodeKey>,
    pub(crate) state: Option<NodeStateRef>,
    /// `__slotHost` (an element or a decorator): the host whose named slot
    /// holds this node, which then has no parent.
    pub(crate) slot_host: Option<NodeKey>,
    /// `__slots` (an element or a decorator): its named slots, shared by its
    /// versions until one writes them.
    pub(crate) slots: Option<Rc<crate::slot::SlotMap>>,
    /// The id of the editor state this version was made writable (or
    /// created) in: with the key, which version of the node this is.
    pub(crate) written_in: u64,
    /// What the node structure normalization recorded of this version's
    /// rule inputs (JS's `ruleInputs` `WeakMap`, keyed by the node object):
    /// a new version starts without.
    pub(crate) structure_rule_inputs: StructureRuleInputs,
}

/// [`NodeBase::structure_rule_inputs`]: kept by this version only, so a
/// clone (the next version) starts without one.
#[derive(Debug, Default)]
pub(crate) struct StructureRuleInputs(std::cell::Cell<Option<u8>>);

impl Clone for StructureRuleInputs {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl StructureRuleInputs {
    pub(crate) fn get(&self) -> Option<u8> {
        self.0.get()
    }
    pub(crate) fn set(&self, value: Option<u8>) {
        self.0.set(value);
    }
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
///
/// A node that extends `DecoratorNode` (`nodes/LexicalDecoratorNode.ts`)
/// returns true from [`is_decorator`](Self::is_decorator) and may override
/// the decorator's hooks: [`decorate`](Self::decorate),
/// [`is_isolated`](Self::is_isolated) and
/// [`is_keyboard_selectable`](Self::is_keyboard_selectable). A decorator is
/// inline unless it overrides [`is_inline`](Self::is_inline).
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

    /// The `DecoratorBlockNode` fields (`@lexical/react`), for a node that
    /// extends it: such a node is a decorator that is not inline and can't
    /// be indented, unless it overrides those hooks.
    #[cfg(feature = "decorator-block")]
    fn decorator_block(&self) -> Option<&crate::decorator_block::DecoratorBlockData> {
        None
    }
    #[cfg(feature = "decorator-block")]
    fn decorator_block_mut(&mut self) -> Option<&mut crate::decorator_block::DecoratorBlockData> {
        None
    }

    /// `isInline()`: false for an element, true for text and a decorator
    /// (false for a `DecoratorBlockNode`). A class whose config chain
    /// declares `inline` ([`static_node_config_chain`](Self::static_node_config_chain))
    /// returns the nearest declaration, as the `isInline()` JS synthesizes
    /// for a class that declares it does.
    fn is_inline(&self) -> bool {
        if self.text().is_some() {
            return true;
        }
        if let Some(inline) = crate::structure::declared_inline(self.as_node()) {
            return inline;
        }
        self.is_decorator() && !is_decorator_block(self)
    }

    /// Whether the node extends `DecoratorNode` (`$isDecoratorNode`): a leaf
    /// whose content an app renders ([`decorate`](Self::decorate)), which
    /// the editor moves, selects and deletes as one unit. False by default,
    /// but for a `DecoratorBlockNode`.
    fn is_decorator(&self) -> bool {
        is_decorator_block(self)
    }

    /// `DecoratorNode.decorate(editor, config)`: the value an app's renderer
    /// shows for the decorator, or `None` (the default). JS's reconciler
    /// collects these in `editor._decorators`; with no DOM, nothing in the
    /// port calls it, and an app's own renderer may.
    fn decorate(&self, editor: &crate::Editor) -> Option<Rc<dyn Any>> {
        let _ = editor;
        None
    }

    /// `DecoratorNode.isIsolated()`: whether the decorator is isolated from
    /// caret interaction (not traversed, extended over, selected as a node
    /// or deleted by an adjacent caret operation). False by default.
    fn is_isolated(&self) -> bool {
        false
    }

    /// `DecoratorNode.isKeyboardSelectable()`: whether moving the caret onto
    /// the decorator selects it as a node. True by default.
    fn is_keyboard_selectable(&self) -> bool {
        true
    }

    /// `$isInlineFormattable(node)`: whether the node carries a text format
    /// that formatting the selection changes (`InlineFormattableNode`): text,
    /// and a node with the `__isInlineFormattable` brand.
    fn is_inline_formattable(&self) -> bool {
        self.text().is_some()
    }

    /// `InlineFormattableNode.getFormat()`.
    fn get_inline_format(&self) -> f64 {
        self.text().map_or(0.0, |t| t.format)
    }

    /// `InlineFormattableNode.setFormat(format)` on `this`: a text node's
    /// [`set_format`](Self::set_format).
    fn set_inline_format(&self, this: &NodeRef, format: f64) -> Result<(), LexicalError> {
        self.set_format(this, format);
        Ok(())
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
    /// this block's indent (`DecoratorBlockNode.canIndent()` is false).
    fn can_indent(&self) -> bool {
        !is_decorator_block(self)
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

    /// The `stateConfigs` of the node's `$config`, its class's and those it
    /// inherits: the NodeState its type knows before any node reads it.
    /// None by default.
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        Vec::new()
    }

    /// `resetOnCopyNodeFrom(originalNode)` on `this`, the copy `$copyNode`
    /// just made of `original`, beyond `LexicalNode`'s, which resets the
    /// NodeState that asks for it (`$copyNode` does that first). Nothing by
    /// default.
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

    /// The `slots` of the node's `$config()` (`getDeclaredSlots`): the
    /// canonical order of its named slots, before the names it doesn't
    /// declare. None by default.
    fn declared_slots(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether the `children` the class declares in its `$config()`
    /// (`getDeclaredNodeStructure(klass).children`) is `'inline'` or a list
    /// that includes `'inline'`: a block that holds text. By default, what
    /// [`static_node_config_chain`](Self::static_node_config_chain)
    /// declares.
    fn declares_inline_children(&self) -> bool {
        use crate::structure::{NodeChildEntry, NodeChildrenConfig};
        match crate::structure::get_declared_node_structure(self.as_node()).children {
            Some(NodeChildrenConfig::Inline) => true,
            Some(NodeChildrenConfig::List(entries)) => entries.contains(&NodeChildEntry::Inline),
            _ => false,
        }
    }

    /// The node-structure keys of the class's `$config()` and of the classes
    /// it extends (`iterStaticNodeConfigChain(klass)`), most derived first:
    /// `inline`, `children`, `mergeAdjacent` and the values it declares
    /// `derived` (`@experimental node-structure`). A class that extends
    /// another returns its own link followed by that class's chain. None by
    /// default: the class declares nothing.
    fn static_node_config_chain(&self) -> Vec<crate::structure::NodeConfigLink> {
        Vec::new()
    }

    /// `INTERNAL_$afterStructureMerge(last, first)` on the element `this`
    /// (internal, for the node structure normalization): `this` has just
    /// merged its next sibling, and `last` (its last child before) and
    /// `first` (the sibling's first child) meet at the seam. Returns whether
    /// the class has the hook (`ListNode` joins nested lists there). False by
    /// default.
    fn internal_after_structure_merge(
        &self,
        this: &NodeRef,
        last: &NodeRef,
        first: &NodeRef,
    ) -> Result<bool, LexicalError> {
        let _ = (this, last, first);
        Ok(false)
    }

    /// `INTERNAL_$afterStructureSplit(original, preceding)` on `this`, a
    /// copy of `original` that the node structure normalization split off
    /// it, after the content `preceding` (an ordered list continues its
    /// numbering). Nothing by default.
    fn internal_after_structure_split(
        &self,
        this: &NodeRef,
        original: &NodeRef,
        preceding: &[NodeRef],
    ) -> Result<(), LexicalError> {
        let _ = (this, original, preceding);
        Ok(())
    }

    /// `INTERNAL_$liftsFromStructure`: the holder's hook for a child its
    /// declared `children` allow that the node structure normalization must
    /// still lift out, given what else it holds (a list item lifts a nested
    /// list that is not all it holds). None by default.
    fn internal_lifts_from_structure(&self) -> Option<crate::structure::LiftsFromStructure> {
        None
    }

    /// `TextNode.isTextEntity()`: whether `registerLexicalTextEntity` (the
    /// `text` feature) treats the text node as an entity. False by default.
    fn is_text_entity(&self) -> bool {
        false
    }

    /// `TextNode.canHaveFormat()`: whether the text node can carry formats
    /// and styles that `@lexical/selection` applies to it. True by default.
    fn can_have_format(&self) -> bool {
        true
    }

    /// `ElementNode.excludeFromCopy(destination)`: whether copying (to
    /// `"clone"` or `"html"`) leaves this element out and keeps its
    /// children. `ElementNode` returns false. Nothing in the core calls it;
    /// `@lexical/clipboard`'s and `@lexical/html`'s copies do.
    fn exclude_from_copy(&self, destination: &str) -> bool {
        let _ = destination;
        false
    }

    /// `ElementNode.canReplaceWith(replacement)` (deprecated and internal
    /// upstream): true by default. Nothing in the core calls it.
    fn can_replace_with(&self, replacement: &NodeRef) -> bool {
        let _ = replacement;
        true
    }

    /// `ElementNode.canInsertAfter(node)` (deprecated and internal
    /// upstream): true by default. Nothing in the core calls it.
    fn can_insert_after(&self, node: &NodeRef) -> bool {
        let _ = node;
        true
    }

    /// `TextNode.selectionTransform(prevSelection, nextSelection)`: called
    /// on the text nodes a range selection's anchor and focus are in at the
    /// end of each update (`applySelectionTransforms`), before the
    /// transforms run, with the committed state's selection and the
    /// update's. Does nothing by default.
    fn selection_transform(
        &self,
        this: &NodeRef,
        prev_selection: Option<&SelectionRef>,
        next_selection: &SelectionRef,
    ) -> Result<(), LexicalError> {
        let _ = (this, prev_selection, next_selection);
        Ok(())
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
        // `LexicalNode.getTextContent()`: a decorator host's slots.
        crate::slot::get_slots_text_content(self.as_node())
    }

    /// `ElementNode.shouldSerializeTextStyles()`.
    fn should_serialize_text_styles(&self) -> bool {
        element::should_serialize_text_styles(self.as_node())
    }

    /// `createDOM(config, editor)` (the `html` feature): the element the
    /// node renders as. `LexicalNode` throws, and `TextNode` renders its
    /// text; every other node that renders overrides it.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        this: &NodeRef,
        config: &crate::EditorConfig,
        editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        let _ = (this, editor);
        if self.text().is_some() {
            return crate::nodes::text::create_dom(self.as_node(), config);
        }
        Err(LexicalError::new("createDOM: base method not extended"))
    }

    /// `exportDOM(editor)` (the `html` feature): the DOM the node exports
    /// to HTML. `LexicalNode` exports the element `$createDOM` makes, which
    /// `ElementNode` gives the indent and direction and `TextNode` its
    /// formats.
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        if self.element().is_some() {
            element::export_dom(self.as_node(), this, editor)
        } else if self.text().is_some() {
            crate::nodes::text::export_dom(self.as_node(), this, editor)
        } else {
            crate::lexical_dom::export_dom_base(self.as_node(), this, editor)
        }
    }

    /// The node types of the classes this node's class extends, nearest
    /// first, without the base classes that have no type of their own
    /// (`ElementNode`, `DecoratorNode`): JS's class chain, which matches
    /// `DOMRenderExtension`'s overrides of a class to the nodes of the
    /// classes that extend it (the `html` feature). `["text"]` for a node
    /// with `TextNode` fields that isn't a `TextNode` itself, and none for
    /// any other; a class that extends another typed class overrides it.
    /// It also stands for `instanceof` in `$applyNodeReplacement` and
    /// `createEditor`'s check of a replacement's `withKlass`.
    fn super_types(&self) -> Vec<&'static str> {
        if self.text().is_some() && self.get_type() != "text" {
            vec!["text"]
        } else {
            Vec::new()
        }
    }

    /// The class's static `importDOM()` (the `html` feature): the
    /// conversions it offers by DOM node name, or `None`.
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        None
    }

    /// The `json` of the node's `$config()` and of the classes it extends
    /// (`iterStaticNodeConfigChain`), most derived first: the serialization
    /// schemas `composeSchema` composes, a subclass's declaration of a key
    /// winning. By default ElementNode's for an element
    /// ([`element::element_node_schema`]), TextNode's for a text node and
    /// DecoratorBlockNode's for one; a class that declares its own returns
    /// it followed by those of the class it extends. The composed schema is
    /// compiled once per Rust type, so it must not differ between nodes.
    fn json_schema_chain(&self) -> Vec<crate::schema::NodeSchema> {
        if self.element().is_some() {
            return vec![element::element_node_schema()];
        }
        if self.text().is_some() {
            return vec![crate::nodes::text::text_node_schema()];
        }
        #[cfg(feature = "decorator-block")]
        if self.decorator_block().is_some() {
            return vec![crate::decorator_block::decorator_block_node_schema()];
        }
        Vec::new()
    }

    /// The class's generated `exportJSON` (`$config`'s `generated`,
    /// `LexicalGeneratedJSON.ts` and the packages' counterparts, ported by
    /// hand): the same JSON as the walk of its schema, written faster.
    /// `None` (the default) for a class without one, which the walk
    /// serves.
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        let _ = compact;
        None
    }

    /// The class's generated `updateFromJSON` on a writable node, which
    /// applies what the walk of its schema would; false (the default) for a
    /// class without one.
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        let _ = json;
        false
    }

    /// `exportJSON(compact)`: the node's own properties, with an empty
    /// `children` array for an element (the export walk fills it), without
    /// its NodeState. By default `$exportNodeJSONOnce`: the class's
    /// [generated code](Self::generated_export_json), or the walk of its
    /// schema ([`crate::schema::walk_export_json`]). A class that adds to
    /// its JSON overrides it, calling [`crate::schema::export_node_json_once`]
    /// for what JS's `super.exportJSON` returns.
    fn export_json(&self, compact: bool) -> Object {
        crate::schema::export_node_json_once(self.as_node(), compact)
    }

    /// `updateFromJSON(json)` on a writable node: applies every property the
    /// node's schema declares, a missing one as its default
    /// (`$applyJSONSetters`, the class's
    /// [generated code](Self::generated_update_from_json) or the walk); the
    /// caller applies the NodeState first.
    fn update_from_json(&mut self, json: &Object) {
        crate::schema::apply_json_setters(self.as_node_mut(), json);
    }

    /// The model half of `updateDOM(prevNode, dom, config)`: whether the
    /// reconciler makes the node's DOM again (what JS's returns), which
    /// reports the node's children as created. By default a text node
    /// compares its tags (`TextNode.updateDOM`) and any other node keeps
    /// its DOM.
    fn update_dom(&self, prev_node: &dyn LexicalNode) -> bool {
        match (self.text(), prev_node.text()) {
            (Some(next), Some(prev)) => crate::nodes::text::update_dom(prev, next),
            _ => false,
        }
    }
}

/// Whether `node` extends `DecoratorBlockNode`.
#[allow(unused_variables)]
fn is_decorator_block<N: LexicalNode + ?Sized>(node: &N) -> bool {
    #[cfg(feature = "decorator-block")]
    return node.decorator_block().is_some();
    #[cfg(not(feature = "decorator-block"))]
    false
}

/// The parts of [`LexicalNode`] every implementation writes the same way,
/// implemented for any `Clone` node.
pub trait NodeClone {
    fn clone_node(&self) -> Box<dyn LexicalNode>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_node(&self) -> &dyn LexicalNode;
    fn as_node_mut(&mut self) -> &mut dyn LexicalNode;
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
    fn as_node_mut(&mut self) -> &mut dyn LexicalNode {
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

    /// `$isDecoratorNode(node)`.
    pub fn is_decorator(&self) -> bool {
        self.get_latest().is_decorator()
    }

    /// `DecoratorNode.isIsolated()`.
    pub fn is_isolated(&self) -> bool {
        self.get_latest().is_isolated()
    }

    /// `DecoratorNode.isKeyboardSelectable()`.
    pub fn is_keyboard_selectable(&self) -> bool {
        self.get_latest().is_keyboard_selectable()
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

    /// `exportJSON(compact)`, with the node's NodeState.
    pub fn export_json(&self, compact: bool) -> Object {
        crate::node_state::export_json_with_state(&*self.get_latest(), compact)
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

    /// `isAttached()`: whether the node's parents (and slot hosts) lead to
    /// the root.
    pub fn is_attached(&self) -> bool {
        let editor_state = get_active_editor_state();
        let node_map = editor_state.0.node_map.borrow();
        let mut key = self.key.as_str();
        loop {
            if key == "root" {
                return true;
            }
            // A slotted node has no parent; its slot host leads on.
            match node_map.get(key).and_then(|n| {
                n.base()
                    .parent
                    .as_deref()
                    .or_else(|| crate::slot::slot_host_key_field(&**n).map(String::as_str))
            }) {
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

    /// `TextNode.canHaveFormat()`.
    pub fn can_have_format(&self) -> bool {
        self.get_latest().can_have_format()
    }

    /// `ElementNode.excludeFromCopy(destination)`.
    pub fn exclude_from_copy(&self, destination: &str) -> bool {
        self.get_latest().exclude_from_copy(destination)
    }

    /// `ElementNode.canReplaceWith(replacement)` (deprecated upstream).
    pub fn can_replace_with(&self, replacement: &NodeRef) -> bool {
        self.get_latest().can_replace_with(replacement)
    }

    /// `ElementNode.canInsertAfter(node)` (deprecated upstream).
    pub fn can_insert_after(&self, node: &NodeRef) -> bool {
        self.get_latest().can_insert_after(node)
    }

    /// `getTopLevelElement()`: the highest ancestor of this node (or the
    /// node) below the root or a shadow root, or a slot value (the top of
    /// its isolated scope), or `None` when there is none (the root, or a
    /// node that isn't attached). Fails where JS's invariant does: on a
    /// non-element ancestor of the node, or a text node at the top.
    pub fn get_top_level_element(&self) -> Result<Option<NodeRef>, LexicalError> {
        let mut node = Some(self.clone());
        while let Some(n) = node {
            let parent = n.get_parent();
            if parent
                .as_ref()
                .is_some_and(|p| crate::nodes::element::is_root_or_shadow_root(&*p.get_latest()))
                || crate::slot::get_slot_host_key(&n).is_some()
            {
                if !(n.is_element() || (n == *self && n.is_decorator())) {
                    return Err(LexicalError::new(
                        "Children of root nodes must be elements or decorators",
                    ));
                }
                return Ok(Some(n));
            }
            node = parent;
        }
        Ok(None)
    }

    /// `getTopLevelElementOrThrow()`: [`get_top_level_element`](Self::get_top_level_element),
    /// failing where it is `None` (with `RootNode`'s own message for the
    /// root).
    pub fn get_top_level_element_or_throw(&self) -> Result<NodeRef, LexicalError> {
        if self.is_root() {
            return Err(LexicalError::new(
                "getTopLevelElementOrThrow: root nodes are not top level elements",
            ));
        }
        self.get_top_level_element()?.ok_or_else(|| {
            LexicalError::new(format!(
                "Expected node {} to have a top parent element.",
                self.key
            ))
        })
    }

    /// `getParentKeys()`: the keys of the ancestors, nearest first.
    pub fn get_parent_keys(&self) -> Vec<NodeKey> {
        self.get_parents()
            .into_iter()
            .map(|p| p.get_key().to_owned())
            .collect()
    }

    /// `ElementNode.getChildrenKeys()`.
    pub fn get_children_keys(&self) -> Vec<NodeKey> {
        self.get_children()
            .into_iter()
            .map(|c| c.get_key().to_owned())
            .collect()
    }

    /// `ElementNode.isLastChild()`: whether this is its parent's last child.
    /// Fails without a parent.
    pub fn is_last_child(&self) -> Result<bool, LexicalError> {
        Ok(self.get_parent_or_throw()?.get_last_child().as_ref() == Some(self))
    }

    /// `ElementNode.getFirstChildOrThrow()`.
    pub fn get_first_child_or_throw(&self) -> Result<NodeRef, LexicalError> {
        self.get_first_child().ok_or_else(|| {
            LexicalError::new(format!("Expected node {} to have a first child.", self.key))
        })
    }

    /// `ElementNode.getLastChildOrThrow()`.
    pub fn get_last_child_or_throw(&self) -> Result<NodeRef, LexicalError> {
        self.get_last_child().ok_or_else(|| {
            LexicalError::new(format!("Expected node {} to have a last child.", self.key))
        })
    }

    /// `ElementNode.hasTextFormat(type)`: whether the element's text format
    /// (`__textFormat`) has the format.
    pub fn has_text_format(&self, format_type: &str) -> bool {
        use crate::constants::{TEXT_TYPE_TO_FORMAT, lookup};
        let flag = lookup(TEXT_TYPE_TO_FORMAT, format_type).unwrap_or(0);
        let format = self.get_latest().element().map_or(0.0, |e| e.text_format);
        crate::nodes::text::to_int32(format) & flag != 0
    }

    /// `getFormatFlags(type, alignWithFormat)` of a text node (its
    /// `__format`) or an element (its `__textFormat`): the format with
    /// `type` toggled (`toggleTextFormatType`).
    pub fn get_format_flags(&self, format_type: &str, align_with_format: Option<i32>) -> i32 {
        let latest = self.get_latest();
        let format = match (latest.text(), latest.element()) {
            (Some(text), _) => text.format,
            (None, Some(element)) => element.text_format,
            (None, None) => 0.0,
        };
        crate::nodes::text::toggle_text_format_type(
            crate::nodes::text::to_int32(format),
            format_type,
            align_with_format,
        )
    }

    /// `TextNode.isDirectionless()`: whether the text's detail has
    /// `IS_DIRECTIONLESS`.
    pub fn is_directionless(&self) -> bool {
        let detail = self.get_latest().text().map_or(0.0, |t| t.detail);
        crate::nodes::text::to_int32(detail) & crate::constants::IS_DIRECTIONLESS != 0
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

    /// `isSelected(selection)`: whether the node is among the nodes of
    /// `selection`, or of the current selection when `None`. An element or
    /// decorator isn't selected by a collapsed range between children, and
    /// an inline decorator that is its parent's last child isn't selected by
    /// an element range starting at the parent's end.
    pub fn is_selected(&self, selection: Option<&SelectionRef>) -> Result<bool, LexicalError> {
        let current;
        let target_selection = match selection {
            Some(selection) => selection,
            None => match get_selection() {
                Some(selection) => {
                    current = selection;
                    &current
                }
                None => return Ok(false),
            },
        };
        // `$isKeyInSelectionNodes(targetSelection, this.__key)`.
        let is_selected = target_selection
            .get_nodes()?
            .iter()
            .any(|node| node.get_key() == self.key);
        if !is_selected || self.is_text() {
            return Ok(is_selected);
        }
        // For inline images inside of element nodes.
        // Without this change the image will be selected if the cursor is
        // before or after it.
        if let Some(range) = target_selection.range()
            && range.anchor.point_type == PointType::Element
            && range.focus.point_type == PointType::Element
        {
            if range.is_collapsed() {
                return Ok(false);
            }
            let parent_node = self.get_parent();
            if let Some(parent_node) =
                parent_node.filter(|_| self.is_decorator() && self.is_inline())
            {
                let first_point = if target_selection.is_backward()? {
                    &range.focus
                } else {
                    &range.anchor
                };
                if first_point.key == parent_node.get_key()
                    && first_point.offset == parent_node.get_children_size()
                    && parent_node.get_last_child().as_ref() == Some(self)
                {
                    return Ok(false);
                }
            }
        }
        Ok(is_selected)
    }

    /// `isBefore(targetNode)`: whether this node comes before `target` in
    /// the document.
    pub fn is_before(&self, target: &NodeRef) -> bool {
        match get_common_ancestor(self, target) {
            None => false,
            Some(CommonAncestorResult::Descendant { .. }) => true,
            Some(CommonAncestorResult::Branch { a, b, .. }) => {
                get_common_ancestor_result_branch_order(&a, &b) == Ordering::Less
            }
            Some(CommonAncestorResult::Same { .. } | CommonAncestorResult::Ancestor { .. }) => {
                false
            }
        }
    }

    /// `getNodesBetween(targetNode)`: the nodes from this one to `target`,
    /// both included, in document order, with the ancestors the walk
    /// enters or leaves on the way.
    pub fn get_nodes_between(&self, target: &NodeRef) -> Vec<NodeRef> {
        let forward = self.is_before(target);
        let mut nodes = Vec::new();
        let mut node = Some(self.clone());
        let mut entering = true;
        let mut open_elements = 0usize;
        while let Some(current) = node {
            // Only ancestors above the starting point lack an enter event.
            // Count open elements to recognize their first visit without a
            // visited set.
            if entering || open_elements == 0 {
                nodes.push(current.clone());
            }
            if &current == target {
                break;
            }
            if !entering && open_elements > 0 {
                open_elements -= 1;
            }
            let child = if entering && current.is_element() {
                if forward {
                    current.get_first_child()
                } else {
                    current.get_last_child()
                }
            } else {
                None
            };
            if child.is_some() {
                open_elements += 1;
            }
            let adjacent = child.or_else(|| {
                if forward {
                    current.get_next_sibling()
                } else {
                    current.get_previous_sibling()
                }
            });
            entering = adjacent.is_some();
            node = adjacent.or_else(|| current.get_parent());
        }
        if !forward {
            nodes.reverse();
        }
        nodes
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
    if let Some(slot_host) = crate::slot::get_slot_host(this) {
        return Err(LexicalError::new(format!(
            "replace: node {to_replace_key} (type {}) is slotted into host {} (type {}); a slot value cannot be replaced through the tree API. Use $setSlot on its host to assign a replacement.",
            this.get_type(),
            slot_host.get_key(),
            slot_host.get_type()
        )));
    }
    get_writable(replace_with.get_key());
    let parent = this.get_parent_or_throw()?;
    get_writable(parent.get_key());
    crate::slot::error_on_slot_cycle_child(&parent, replace_with)?;
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
    if crate::lexical_utils::get_composition_key().as_deref() == Some(to_replace_key.as_str()) {
        crate::lexical_utils::set_composition_key(Some(replace_with.get_key()));
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
        let slot_host_key = crate::slot::get_slot_host_key(node_to_remove);
        invariant!(
            slot_host_key.is_none(),
            "$removeNode: node {key} is slotted into host {}; use removeSlot on the host instead of remove().",
            slot_host_key.unwrap_or_default()
        );
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
