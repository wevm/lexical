//! `LexicalStructure.ts` (`@experimental node-structure`): the structural
//! facts a node class declares in its `$config()` (`inline`, `children`,
//! `mergeAdjacent`, and the values it declares `derived`), read along its
//! config chain, and the normalization that `NodeStructureExtension`
//! (`@lexical/extension`) turns on with [`internal_register_node_structure`].
//!
//! JS reads the declarations off each class's `$config()` and walks the
//! class's prototype chain (`iterStaticNodeConfigChain`). A Rust node
//! declares them with [`LexicalNode::static_node_config_chain`]: its own
//! class's [`StaticNodeConfig`] first, then those of the classes it extends
//! (a class names another in a `children` list by [`NodeClassRef`], and
//! "allows its subclasses" means a node whose chain names that class). A
//! class's own hooks (`INTERNAL_$afterStructureMerge`,
//! `INTERNAL_$afterStructureSplit`, `INTERNAL_$liftsFromStructure`), which
//! JS finds by name, are [`LexicalNode`] hooks too.
//!
//! Where JS tells whether a class writes its own `isInline()` or
//! `isShadowRoot()` (a trait method can't tell an override), the port does
//! what Swift does: every class but the root counts as one whose rule may
//! differ between its nodes (rust/PARITY.md).

use crate::caret_tree::insert_sibling;
use crate::editor::Editor;
use crate::editor_state::EditorStateInner;
use crate::lexical_selection::{PointType, Which, get_selection};
use crate::lexical_utils::{copy_node, get_node_by_key, get_root, set_node_key};
use crate::node::{LexicalNode, NodeKey, NodeRef, remove_node};
use crate::node_state::AnyStateConfig;
use crate::nodes::element::{self, is_root_or_shadow_root};
use crate::nodes::line_break::{LineBreakNode, create_line_break_node};
use crate::nodes::paragraph::ParagraphNode;
use crate::ordered::{OrderedMap, OrderedSet};
use crate::updates::get_active_editor;
use crate::{LexicalError, Unregister};
use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

// MARK: Declarations

/// A node class that a `children` list names: its type, and how to make a
/// node of it (`$create(klass)`, for the wrapper a list of classes wraps a
/// stray child in).
#[derive(Clone, Copy)]
pub struct NodeClassRef {
    node_type: &'static str,
    new_node: fn() -> Box<dyn LexicalNode>,
}

fn new_node<T: LexicalNode + Default>() -> Box<dyn LexicalNode> {
    Box::new(T::default())
}

impl NodeClassRef {
    /// The class of `T`.
    pub fn of<T: LexicalNode + Default>() -> Self {
        Self {
            node_type: T::default().get_type(),
            new_node: new_node::<T>,
        }
    }

    /// The class's type.
    pub fn node_type(&self) -> &'static str {
        self.node_type
    }

    fn chain(&self) -> Vec<NodeConfigLink> {
        iter_static_node_config_chain(&*(self.new_node)())
    }
}

impl PartialEq for NodeClassRef {
    fn eq(&self, other: &Self) -> bool {
        self.node_type == other.node_type
    }
}

impl std::fmt::Debug for NodeClassRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeClassRef({:?})", self.node_type)
    }
}

/// One entry of a `children` list: `'inline'`, or a class (which allows its
/// subclasses).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeChildEntry {
    Inline,
    Class(NodeClassRef),
}

/// `NodeChildrenConfig` (`@experimental node-structure`): what an element
/// may hold. `Inline` is a paragraph's content, `Block` a table cell's, and
/// `List` only the listed entries.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeChildrenConfig {
    Inline,
    Block,
    List(Vec<NodeChildEntry>),
}

impl NodeChildrenConfig {
    fn entries(&self) -> Option<&[NodeChildEntry]> {
        match self {
            NodeChildrenConfig::List(entries) => Some(entries),
            _ => None,
        }
    }

    /// Whether it is `'inline'` or a list that names `'inline'`.
    fn holds_inline(&self) -> bool {
        match self {
            NodeChildrenConfig::Inline => true,
            NodeChildrenConfig::Block => false,
            NodeChildrenConfig::List(entries) => entries.contains(&NodeChildEntry::Inline),
        }
    }
}

/// One value `mergeAdjacent.by` names: a schema field of the class (a
/// property of its JSON), or a NodeState.
#[derive(Clone, Debug)]
pub enum MergeAdjacentEntry {
    Field(&'static str),
    State(AnyStateConfig),
}

/// `MergeAdjacentConfig` (`@experimental node-structure`).
#[derive(Clone, Debug)]
pub struct MergeAdjacentConfig {
    /// Schema field names of the class, or NodeState configs.
    pub by: Vec<MergeAdjacentEntry>,
}

/// A class's own `mergeAdjacent` declaration: `Off` is JS's `false`, which
/// turns off a declaration an ancestor made.
#[derive(Clone, Debug)]
pub enum MergeAdjacentDeclaration {
    By(Vec<MergeAdjacentEntry>),
    Off,
}

/// The node-structure keys of one class's own `$config()`
/// (`StaticNodeConfigValue`'s `inline`, `children` and `mergeAdjacent`).
/// What it declares `derived` comes from its schema
/// ([`LexicalNode::json_schema_chain`]) and its NodeState
/// ([`LexicalNode::state_configs`]).
#[derive(Clone, Debug, Default)]
pub struct StaticNodeConfig {
    pub inline: Option<bool>,
    pub children: Option<NodeChildrenConfig>,
    pub merge_adjacent: Option<MergeAdjacentDeclaration>,
}

/// One class of a config chain (`iterStaticNodeConfigChain`'s
/// `{klass, ownNodeType, ownNodeConfig}`).
#[derive(Clone, Debug)]
pub struct NodeConfigLink {
    pub node_type: &'static str,
    pub config: StaticNodeConfig,
}

impl NodeConfigLink {
    pub fn new(node_type: &'static str, config: StaticNodeConfig) -> Self {
        Self { node_type, config }
    }
}

/// `iterStaticNodeConfigChain(klass)`: the node's class and the classes it
/// extends, most derived first ([`LexicalNode::static_node_config_chain`]),
/// or the node's class alone, declaring nothing, for a class that gives no
/// chain.
pub fn iter_static_node_config_chain(node: &dyn LexicalNode) -> Vec<NodeConfigLink> {
    let chain = node.static_node_config_chain();
    if chain.is_empty() {
        vec![NodeConfigLink::new(
            node.get_type(),
            StaticNodeConfig::default(),
        )]
    } else {
        chain
    }
}

/// The chain of a class that declares only `children: 'inline'` (a
/// paragraph, a heading, a quote, a code block).
pub(crate) fn inline_holder_chain(node_type: &'static str) -> Vec<NodeConfigLink> {
    vec![NodeConfigLink::new(
        node_type,
        StaticNodeConfig {
            children: Some(NodeChildrenConfig::Inline),
            ..StaticNodeConfig::default()
        },
    )]
}

/// The class each key of a [`DeclaredNodeStructure`] came from, by type.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeclaredBy {
    pub inline: Option<&'static str>,
    pub children: Option<&'static str>,
    pub merge_adjacent: Option<&'static str>,
}

/// `DeclaredNodeStructure` (`@experimental node-structure`): the structural
/// facts a class declares, each from the nearest class in its config chain
/// that declares it, and the values it declares derived. Keys no class
/// declares are `None`; a `mergeAdjacent: false` declaration reads as `None`
/// too.
#[derive(Clone, Debug, Default)]
pub struct DeclaredNodeStructure {
    pub inline: Option<bool>,
    pub children: Option<NodeChildrenConfig>,
    pub merge_adjacent: Option<MergeAdjacentConfig>,
    /// The schema fields and NodeState keys of the class declared
    /// `derived`: the node's own code recomputes them.
    pub derived: Vec<String>,
    /// The class each key came from.
    pub declared_by: DeclaredBy,
}

struct CachedNodeStructure {
    declared: DeclaredNodeStructure,
    /// The types of the class's config chain, its own first.
    chain_types: Vec<&'static str>,
    /// `isOverflowClass(klass)`.
    is_overflow: bool,
}

impl CachedNodeStructure {
    fn own_type(&self) -> &'static str {
        self.chain_types[0]
    }

    /// Whether the class declares `mergeAdjacent` in its own `$config()`.
    fn merges_itself(&self) -> bool {
        self.declared.merge_adjacent.is_some()
            && self.declared.declared_by.merge_adjacent == Some(self.own_type())
    }
}

thread_local! {
    /// The nearest `inline` declaration of each class, by its Rust type.
    static DECLARED_INLINE: RefCell<HashMap<TypeId, Option<bool>>> = RefCell::new(HashMap::new());
}

/// The nearest `inline` declaration in the class's chain: what the
/// `isInline()` JS synthesizes (`injectDeclaredIsInline`) returns. Read
/// without the structure cache, which asks `isInline()`.
pub(crate) fn declared_inline(node: &dyn LexicalNode) -> Option<bool> {
    let id = node.as_any().type_id();
    if let Some(cached) = DECLARED_INLINE.with(|c| c.borrow().get(&id).copied()) {
        return cached;
    }
    let inline = node
        .static_node_config_chain()
        .iter()
        .find_map(|link| link.config.inline);
    DECLARED_INLINE.with(|c| c.borrow_mut().insert(id, inline));
    inline
}

thread_local! {
    /// `structureCache`, by the node's Rust type (one class).
    static STRUCTURE_CACHE: RefCell<HashMap<TypeId, Rc<CachedNodeStructure>>> =
        RefCell::new(HashMap::new());
}

/// Why `by` is invalid for the class, or `None` when it is valid
/// (`invalidMergeBy`). A schema field is a property of the class's JSON.
fn invalid_merge_by(
    node: &dyn LexicalNode,
    config: &MergeAdjacentConfig,
    derived: &[String],
) -> Option<(String, &'static str)> {
    let fields = crate::schema::get_composed_schema_fields(node);
    for entry in &config.by {
        match entry {
            MergeAdjacentEntry::Field(name) => {
                if !fields.iter().any(|(key, _)| key == name) {
                    return Some(((*name).to_owned(), "unknown"));
                }
                if derived.iter().any(|d| d == name) {
                    return Some(((*name).to_owned(), "derived"));
                }
            }
            MergeAdjacentEntry::State(state) => {
                if state.derived() {
                    return Some((state.key().to_owned(), "derived-state"));
                }
            }
        }
    }
    None
}

/// `declaredChildren(klass)`: the nearest `children` declaration, read
/// without the cache.
fn declared_children(chain: &[NodeConfigLink]) -> Option<NodeChildrenConfig> {
    chain.iter().find_map(|link| link.config.children.clone())
}

/// `mergingChildNotInline(klass, children)`: for a merging class that lists
/// child classes (no `'inline'`), the first listed class that does not hold
/// inline content (possibly with a nested instance of the class), or `None`.
fn merging_child_not_inline(
    chain_types: &[&'static str],
    children: Option<&NodeChildrenConfig>,
) -> Option<&'static str> {
    let entries = children?.entries()?;
    if entries.contains(&NodeChildEntry::Inline) {
        return None;
    }
    for entry in entries {
        let NodeChildEntry::Class(class) = entry else {
            continue;
        };
        let item = declared_children(&class.chain());
        let ok = match &item {
            Some(NodeChildrenConfig::Inline) => true,
            Some(NodeChildrenConfig::List(items)) => {
                items.contains(&NodeChildEntry::Inline)
                    && items.iter().all(|e| match e {
                        NodeChildEntry::Inline => true,
                        NodeChildEntry::Class(k) => chain_types.contains(&k.node_type),
                    })
            }
            _ => false,
        };
        if !ok {
            return Some(class.node_type);
        }
    }
    None
}

/// `derivedNamesOf(klass)`: the names of the schema fields and NodeState
/// keys the class declares derived.
fn derived_names_of(node: &dyn LexicalNode) -> Vec<String> {
    let mut derived: Vec<String> = crate::schema::get_composed_schema_fields(node)
        .into_iter()
        .filter(|(_, field)| field.is_derived())
        .map(|(key, _)| key)
        .collect();
    for required in node.state_configs() {
        let key = required.state_config.key();
        if required.state_config.derived() && !derived.iter().any(|d| d == key) {
            derived.push(key.to_owned());
        }
    }
    derived
}

/// `cachedNodeStructure(klass)`, of the class of `node`.
fn cached_node_structure(node: &dyn LexicalNode) -> Rc<CachedNodeStructure> {
    let id = node.as_any().type_id();
    if let Some(cached) = STRUCTURE_CACHE.with(|c| c.borrow().get(&id).cloned()) {
        return cached;
    }
    let chain = iter_static_node_config_chain(node);
    let own = chain[0].node_type;
    let chain_types: Vec<&'static str> = chain.iter().map(|l| l.node_type).collect();
    let mut out = DeclaredNodeStructure::default();
    let mut merge_seen = false;
    for link in &chain {
        let config = &link.config;
        if out.declared_by.inline.is_none()
            && let Some(inline) = config.inline
        {
            out.inline = Some(inline);
            out.declared_by.inline = Some(link.node_type);
        }
        if out.declared_by.children.is_none()
            && let Some(children) = &config.children
        {
            out.children = Some(children.clone());
            out.declared_by.children = Some(link.node_type);
        }
        if !merge_seen && let Some(merge) = &config.merge_adjacent {
            merge_seen = true;
            if let MergeAdjacentDeclaration::By(by) = merge {
                out.merge_adjacent = Some(MergeAdjacentConfig { by: by.clone() });
                out.declared_by.merge_adjacent = Some(link.node_type);
            }
        }
    }
    out.derived = derived_names_of(node);
    // A declaration the class only inherits never makes it invalid: a key
    // the class contradicts, or may contradict, reads as undeclared for it.
    if out.declared_by.inline.is_some_and(|by| by != own) && Some(node.is_inline()) != out.inline {
        out.inline = None;
        out.declared_by.inline = None;
    }
    if out.declared_by.merge_adjacent.is_some_and(|by| by != own)
        && (invalid_merge_by(node, out.merge_adjacent.as_ref().unwrap(), &out.derived).is_some()
            || merging_child_not_inline(&chain_types, out.children.as_ref()).is_some())
    {
        out.merge_adjacent = None;
        out.declared_by.merge_adjacent = None;
    }
    let is_overflow = node.element().is_some() && chain_types.contains(&"overflow");
    let result = Rc::new(CachedNodeStructure {
        declared: out,
        chain_types,
        is_overflow,
    });
    STRUCTURE_CACHE.with(|c| c.borrow_mut().insert(id, result.clone()));
    result
}

fn cached_of(node: &NodeRef) -> Rc<CachedNodeStructure> {
    cached_node_structure(&*node.get_latest())
}

/// `getDeclaredNodeStructure(klass)` (`@experimental node-structure`): the
/// structural declarations of the class of `node`. Depends on the class
/// only.
pub fn get_declared_node_structure(node: &dyn LexicalNode) -> DeclaredNodeStructure {
    cached_node_structure(node).declared.clone()
}

/// Whether `node` is of the class of type `node_type` or a subclass of it
/// (`node instanceof klass`).
fn is_instance_of(node: &NodeRef, node_type: &str) -> bool {
    cached_of(node).chain_types.contains(&node_type)
}

/// `isOverflowNode(node)`: `OverflowNode` (`@lexical/overflow`) or a class
/// that extends it, an element whose chain names the type `overflow`. It is
/// inline content although its `isInline()` is false.
fn is_overflow_node(node: &NodeRef) -> bool {
    cached_of(node).is_overflow
}

fn is_line_break(node: &NodeRef) -> bool {
    node.get_latest().is::<LineBreakNode>()
}

fn is_paragraph(node: &NodeRef) -> bool {
    node.get_latest().is::<ParagraphNode>()
}

fn is_inline_child(node: &NodeRef) -> bool {
    node.is_text() || is_line_break(node) || is_overflow_node(node) || node.is_inline()
}

/// `INTERNAL_isInlineContent(node)`: whether the node structure
/// normalization treats `node` as inline content: text, line breaks, inline
/// elements and decorators, and the character limit's `OverflowNode`.
pub fn internal_is_inline_content(node: &NodeRef) -> bool {
    is_inline_child(node)
}

/// A leaf the normalization never removes and that sits in a line.
fn is_inline_leaf(node: &NodeRef) -> bool {
    node.is_text() || is_line_break(node) || (node.is_decorator() && is_inline_child(node))
}

fn allows(children: &NodeChildrenConfig, child: &NodeRef) -> bool {
    if is_overflow_node(child) {
        return true;
    }
    match children {
        NodeChildrenConfig::Inline => is_inline_child(child),
        NodeChildrenConfig::Block => !is_inline_child(child),
        NodeChildrenConfig::List(entries) => {
            if is_inline_child(child) {
                return entries.contains(&NodeChildEntry::Inline);
            }
            entries.iter().any(|entry| match entry {
                NodeChildEntry::Class(class) => is_instance_of(child, class.node_type),
                NodeChildEntry::Inline => false,
            })
        }
    }
}

/// `effectiveChildren(node)`: a root or shadow root holds blocks whatever
/// its class declares, any other node what its class declares.
fn effective_children(node: &NodeRef) -> Option<NodeChildrenConfig> {
    if is_root_or_shadow_root(&*node.get_latest()) {
        Some(NodeChildrenConfig::Block)
    } else {
        cached_of(node).declared.children.clone()
    }
}

/// `isJoinableParagraph(holderChildren, node)`: a plain paragraph whose
/// content `holder_children` allows.
fn is_joinable_paragraph(holder_children: &NodeChildrenConfig, node: &NodeRef) -> bool {
    is_paragraph(node)
        && !node.get_latest().is_shadow_root()
        && node
            .get_children()
            .iter()
            .all(|c| allows(holder_children, c))
}

/// A selection point, as a position next to a node that survives repairs.
#[derive(Clone, Debug)]
enum Anchor {
    Keys { keys: Vec<NodeKey>, after: bool },
    Inside(NodeKey),
    Text { key: NodeKey, offset: usize },
}

/// `descend(node, last)`: `node` and its last (or first) descendants down to
/// a leaf.
fn descend(node: &NodeRef, last: bool) -> Vec<NodeKey> {
    let mut keys = Vec::new();
    let mut n = Some(node.clone());
    while let Some(current) = n {
        keys.push(current.get_key().to_owned());
        n = if current.is_element() {
            if last {
                current.get_last_child()
            } else {
                current.get_first_child()
            }
        } else {
            None
        };
    }
    keys
}

fn keys_after(node: &NodeRef) -> Anchor {
    Anchor::Keys {
        keys: descend(node, true),
        after: true,
    }
}

fn keys_before(node: &NodeRef) -> Anchor {
    Anchor::Keys {
        keys: descend(node, false),
        after: false,
    }
}

/// `$saveAnchor(point)`: an element point as the node next to it
/// (preferably a leaf in the same line, which is never removed), so that it
/// keeps its place among the content when its element is split, unwrapped
/// or merged.
fn save_anchor(point: &crate::Point) -> Result<Option<Anchor>, LexicalError> {
    if point.point_type == PointType::Text {
        // Text nodes are moved, never removed, but detaching one moves a
        // point in it to its parent: it is put back afterwards.
        return Ok(Some(Anchor::Text {
            key: point.key.clone(),
            offset: point.offset,
        }));
    }
    let node = point.get_node()?;
    if !node.is_element() {
        return Ok(None);
    }
    // A point past the end is at the end.
    let offset = point.offset.min(node.get_children_size());
    let prev_node = if offset > 0 {
        node.get_child_at_index(offset - 1)
    } else {
        None
    };
    let next_node = node.get_child_at_index(offset);
    let leaf = |n: &Option<NodeRef>| n.as_ref().filter(|n| is_inline_leaf(n)).cloned();
    if let Some(prev) = leaf(&prev_node) {
        return Ok(Some(Anchor::Keys {
            keys: vec![prev.get_key().to_owned()],
            after: true,
        }));
    }
    if let Some(next) = leaf(&next_node) {
        return Ok(Some(Anchor::Keys {
            keys: vec![next.get_key().to_owned()],
            after: false,
        }));
    }
    let mut prev = prev_node.clone();
    let mut next = next_node.clone();
    if effective_children(&node).is_some_and(|rule| rule.holds_inline()) {
        // In a node that holds lines, a block with nothing in it (an empty
        // list item) is passed over: a point on either side of it is at the
        // same place in the content.
        while let Some(p) = prev.clone().filter(is_passed_over) {
            prev = p.get_previous_sibling();
        }
        while let Some(n) = next.clone().filter(is_passed_over) {
            next = n.get_next_sibling();
        }
        if prev.is_none() && next.is_none() {
            prev = prev_node;
            next = next_node;
        }
    }
    if let Some(prev) = leaf(&prev) {
        return Ok(Some(Anchor::Keys {
            keys: vec![prev.get_key().to_owned()],
            after: true,
        }));
    }
    if let Some(next) = leaf(&next) {
        return Ok(Some(Anchor::Keys {
            keys: vec![next.get_key().to_owned()],
            after: false,
        }));
    }
    if let Some(next) = &next {
        return Ok(Some(keys_before(next)));
    }
    if let Some(prev) = &prev {
        return Ok(Some(keys_after(prev)));
    }
    Ok(Some(Anchor::Inside(node.get_key().to_owned())))
}

/// `isPassedOver(node)`: an element that is not inline, not a paragraph
/// and holds no leaf at any depth.
fn is_passed_over(node: &NodeRef) -> bool {
    if !node.is_element() || is_paragraph(node) || is_inline_child(node) {
        return false;
    }
    fn holds_leaf(n: &NodeRef) -> bool {
        n.get_children()
            .iter()
            .any(|c| !c.is_element() || holds_leaf(c))
    }
    !holds_leaf(node)
}

/// `Normalization`: what one pass of the normalization visits and changed.
#[derive(Default)]
struct Normalization {
    /// The dirty nodes and all their ancestors, and what the repairs put in
    /// a parent: the only nodes visited (unless `full`).
    path: HashSet<NodeKey>,
    /// The keys in `path` by the parent they were added under.
    path_by_parent: OrderedMap<OrderedSet>,
    /// Nodes the repairs put in a parent (lifted blocks, copies, wrappers).
    touched: OrderedSet,
    /// Where a removed childless element's caret goes.
    substitutes: HashMap<NodeKey, Anchor>,
    /// Elements whose children the repairs changed.
    written: OrderedSet,
    changed: bool,
    /// Whether every descendant is visited, not only those in `path`.
    full: bool,
    /// Whether this is `INTERNAL_$normalizeNodeStructure`'s normalization.
    explicit: bool,
    /// The roots and shadow roots whose inline children are all wrapped.
    wraps_all: HashSet<NodeKey>,
    children_cache: HashMap<NodeKey, (usize, Vec<NodeRef>)>,
}

fn parent_key_of(node: &NodeRef) -> Option<NodeKey> {
    node.get_latest().base().parent.clone()
}

impl Normalization {
    /// `addPath(node)`: add `node`, under its current parent, to the nodes
    /// visited.
    fn add_path(&mut self, node: &NodeRef) {
        let key = node.get_key().to_owned();
        self.path.insert(key.clone());
        if let Some(parent) = parent_key_of(node) {
            if !self.path_by_parent.contains(&parent) {
                self.path_by_parent.set(&parent, OrderedSet::new());
            }
            self.path_by_parent
                .get_mut(&parent)
                .expect("just set")
                .add(&key);
        }
    }

    /// `$children(parent)`: the children of `parent` to visit.
    fn children(&mut self, parent: &NodeRef) -> Vec<NodeRef> {
        if self.full {
            return parent.get_children();
        }
        let parent_key = parent.get_key();
        let Some(keys) = self.path_by_parent.get(parent_key) else {
            return Vec::new();
        };
        // The keys of a parent only grow: the same count is the same keys.
        if let Some((size, nodes)) = self.children_cache.get(parent_key)
            && *size == keys.len()
        {
            return nodes.clone();
        }
        let nodes: Vec<NodeRef> = keys
            .iter()
            .filter_map(get_node_by_key)
            .filter(|n| parent_key_of(n).as_deref() == Some(parent_key))
            .collect();
        self.children_cache
            .insert(parent_key.to_owned(), (keys.len(), nodes.clone()));
        nodes
    }

    /// `addPathChain(node)`: add `node`, and its ancestors up to one
    /// already visited under its current parent, to the nodes visited.
    fn add_path_chain(&mut self, node: &NodeRef) {
        let mut n = Some(node.clone());
        while let Some(current) = n {
            if let Some(parent) = parent_key_of(&current)
                && self
                    .path_by_parent
                    .get(&parent)
                    .is_some_and(|keys| keys.contains(current.get_key()))
            {
                return;
            }
            self.add_path(&current);
            n = current.get_parent();
        }
    }

    /// `repath()`: after a class's own seam hook, the entries of every
    /// removed parent go to where its children are now.
    fn repath(&mut self) {
        let entries: Vec<(NodeKey, Vec<NodeKey>)> = self
            .path_by_parent
            .iter()
            .map(|(k, keys)| (k.to_owned(), keys.iter().map(str::to_owned).collect()))
            .collect();
        for (parent_key, keys) in entries {
            if get_node_by_key(&parent_key).is_some_and(|p| p.is_attached()) {
                continue;
            }
            self.path_by_parent.delete(&parent_key);
            for key in keys {
                if let Some(node) = get_node_by_key(&key).filter(NodeRef::is_attached) {
                    self.add_path_chain(&node);
                }
            }
        }
    }

    /// `movePath(from, to)`: `from`'s children were moved to `to` (a merge),
    /// and so are their entries.
    fn move_path(&mut self, from: &str, to: &NodeRef) {
        if let Some(keys) = self.path_by_parent.delete(from) {
            for key in keys.iter() {
                if let Some(node) = get_node_by_key(key) {
                    self.add_path(&node);
                }
            }
        }
        self.add_path(to);
    }

    /// `$resolve(anchor, depth)`.
    fn resolve(&self, anchor: &Anchor, depth: usize) -> Option<(NodeKey, usize, PointType)> {
        match anchor {
            Anchor::Text { key, offset } => get_node_by_key(key)
                .filter(NodeRef::is_attached)
                .map(|_| (key.clone(), *offset, PointType::Text)),
            Anchor::Inside(key) => {
                if get_node_by_key(key).is_some_and(|n| n.is_attached()) {
                    return Some((key.clone(), 0, PointType::Element));
                }
                let substitute = self.substitutes.get(key)?;
                (depth < 16).then(|| self.resolve(substitute, depth + 1))?
            }
            Anchor::Keys { keys, after } => {
                // From the innermost key out: the leaf the point was next to
                // is moved, never removed.
                for i in (0..keys.len()).rev() {
                    let node = get_node_by_key(&keys[i])
                        .filter(|n| n.is_attached() && n.get_parent().is_some());
                    let Some(node) = node else {
                        // A node the repairs removed says where its place
                        // went.
                        if let Some(substitute) = self.substitutes.get(&keys[i])
                            && depth < 16
                        {
                            return self.resolve(substitute, depth + 1);
                        }
                        continue;
                    };
                    // Climb back out while the node is still the last child
                    // (or first, for a point before it).
                    let mut at = node;
                    for _ in 0..i {
                        let Some(parent) = at.get_parent() else {
                            break;
                        };
                        let sibling = if *after {
                            at.get_next_sibling()
                        } else {
                            at.get_previous_sibling()
                        };
                        if is_root_or_shadow_root(&*parent.get_latest()) || sibling.is_some() {
                            break;
                        }
                        at = parent;
                    }
                    if at.is_text() {
                        // Next to a text node in its own parent: a point in
                        // it.
                        let offset = if *after {
                            at.get_text_content_size()
                        } else {
                            0
                        };
                        return Some((at.get_key().to_owned(), offset, PointType::Text));
                    }
                    let parent = at.get_parent()?;
                    let index = at.get_index_within_parent()? + usize::from(*after);
                    return Some((parent.get_key().to_owned(), index, PointType::Element));
                }
                None
            }
        }
    }
}

/// `$append(parent, nodes)`: `ElementNode`'s own splice, so an override
/// (`ListNode.splice` wraps every child in a list item) doesn't rewrite
/// what the repair moves.
fn append(parent: &NodeRef, nodes: &[NodeRef]) -> Result<(), LexicalError> {
    if nodes.is_empty() {
        return Ok(());
    }
    element::splice(parent, parent.get_children_size(), 0, nodes)
}

/// `$detach(node)`: never through an override, never removing a parent it
/// empties, never moving the selection (the normalization restores it).
fn detach(node: &NodeRef) -> Result<(), LexicalError> {
    remove_node(node, false, true)
}

/// Two JSON values are `===` or `JSON.stringify` the same.
fn json_equal(a: Option<&crate::json::Value>, b: Option<&crate::json::Value>) -> bool {
    fn defined(v: Option<&crate::json::Value>) -> Option<&crate::json::Value> {
        v.filter(|v| !v.is_undefined())
    }
    match (defined(a), defined(b)) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b || a.to_string() == b.to_string(),
        _ => false,
    }
}

/// `$canMergeAdjacent(a, b)`: whether `a` and its next sibling `b` are the
/// same thing under the `mergeAdjacent` of `a`'s class, which declares it
/// itself.
fn can_merge_adjacent(a: &NodeRef, b: &NodeRef) -> Result<bool, LexicalError> {
    let (la, lb) = (a.get_latest(), b.get_latest());
    if la.as_any().type_id() != lb.as_any().type_id() || !a.is_element() || !b.is_element() {
        return Ok(false);
    }
    let cached = cached_node_structure(&*la);
    if !cached.merges_itself() {
        return Ok(false);
    }
    let config = cached.declared.merge_adjacent.as_ref().expect("merges");
    let mut json: Option<(crate::json::Object, crate::json::Object)> = None;
    for entry in &config.by {
        match entry {
            MergeAdjacentEntry::Field(name) => {
                let (ja, jb) =
                    json.get_or_insert_with(|| (la.export_json(false), lb.export_json(false)));
                if !json_equal(ja.get(name), jb.get(name)) {
                    return Ok(false);
                }
            }
            MergeAdjacentEntry::State(state) => {
                let va = crate::node_state::get_state_erased(&*la, state)?;
                let vb = crate::node_state::get_state_erased(&*lb, state)?;
                if !state.is_equal_erased(&*va, &*vb) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// `$mergeInto(a, b, ctx)`: append `b`'s children to `a` (its previous
/// sibling) and remove `b`; `a`'s values win. Children that meet at the
/// seam merge in turn.
fn merge_into(a: &NodeRef, b: &NodeRef, ctx: &mut Normalization) -> Result<(), LexicalError> {
    let a_last = a.get_last_child();
    let b_first = b.get_first_child();
    ctx.substitutes.insert(
        b.get_key().to_owned(),
        a_last
            .as_ref()
            .map_or_else(|| Anchor::Inside(a.get_key().to_owned()), keys_after),
    );
    append(a, &b.get_children())?;
    detach(b)?;
    ctx.move_path(b.get_key(), a);
    ctx.changed = true;
    ctx.written.add(a.get_key());
    let (Some(a_last), Some(b_first)) = (a_last, b_first) else {
        return Ok(());
    };
    if a_last.is_element() && can_merge_adjacent(&a_last, &b_first)? {
        return merge_into(&a_last, &b_first, ctx);
    }
    // A class may join what meets at the seam itself (a list joins two
    // items that only wrap nested lists).
    if a.get_latest()
        .internal_after_structure_merge(a, &a_last, &b_first)?
        && !ctx.full
    {
        // The hook moves nodes the normalization may not have visited yet.
        ctx.repath();
    }
    Ok(())
}

/// `$mergeRun(node, ctx)`: merge the run of mutually equal siblings around
/// `node` into its first node, and return that node.
fn merge_run(node: &NodeRef, ctx: &mut Normalization) -> Result<NodeRef, LexicalError> {
    let mut first = node.clone();
    while let Some(prev) = first.get_previous_sibling() {
        if !can_merge_adjacent(&prev, &first)? {
            break;
        }
        first = prev;
    }
    while let Some(next) = first.get_next_sibling() {
        if !can_merge_adjacent(&first, &next)? {
            break;
        }
        merge_into(&first, &next, ctx)?;
    }
    Ok(first)
}

/// `$mergeChildren(parent, ctx, all)`: merge the equal adjacent children of
/// `parent`, every one when `all` (or the normalization is `full`),
/// otherwise those around the children to visit.
fn merge_children(
    parent: &NodeRef,
    ctx: &mut Normalization,
    all: bool,
) -> Result<(), LexicalError> {
    if all || ctx.full {
        let mut c = parent.get_first_child();
        while let Some(current) = c {
            c = merge_run(&current, ctx)?.get_next_sibling();
        }
        return Ok(());
    }
    for child in ctx.children(parent) {
        // Only nodes of one class that declares mergeAdjacent itself merge.
        if cached_of(&child).merges_itself()
            && parent_key_of(&child).as_deref() == Some(parent.get_key())
        {
            merge_run(&child, ctx)?;
        }
    }
    Ok(())
}

/// A run of a holder's content between two lifted blocks.
#[derive(Default)]
struct Segment {
    content: Vec<NodeRef>,
    had_children: bool,
    /// The holder that received the segment, set by `rebuild`.
    holder: Option<NodeRef>,
}

enum RebuildPiece {
    /// An index into the segments.
    Segment(usize),
    Lifted(NodeRef),
}

/// `$rebuild(holder, pieces, ctx)`: split `holder` into its segments: the
/// original keeps the first segment that had children, copies take the
/// others, and the lifted blocks go between them, all in the holder's place
/// in its parent. Returns the holders.
fn rebuild(
    holder: &NodeRef,
    pieces: &[RebuildPiece],
    segments: &mut [Segment],
    ctx: &mut Normalization,
) -> Result<Vec<NodeRef>, LexicalError> {
    ctx.changed = true;
    for child in holder.get_children() {
        detach(&child)?;
    }
    let mut placed: Vec<NodeRef> = Vec::new();
    let mut holders: Vec<NodeRef> = Vec::new();
    let mut preceding: Vec<NodeRef> = Vec::new();
    let mut holder_placed = false;
    for piece in pieces {
        let index = match piece {
            RebuildPiece::Lifted(node) => {
                placed.push(node.clone());
                continue;
            }
            RebuildPiece::Segment(index) => *index,
        };
        if !segments[index].had_children {
            continue;
        }
        let target = if !holder_placed {
            holder_placed = true;
            holder.clone()
        } else {
            let target = copy_node(holder);
            target
                .get_latest()
                .internal_after_structure_split(&target, holder, &preceding)?;
            target
        };
        append(&target, &segments[index].content)?;
        segments[index].holder = Some(target.clone());
        preceding.extend(segments[index].content.iter().cloned());
        placed.push(target.clone());
        holders.push(target);
    }
    let parent = holder.get_parent_or_throw()?;
    ctx.written.add(parent.get_key());
    let at = placed.iter().position(|n| n == holder);
    if let Some(at) = at {
        let mut anchor = holder.clone();
        for node in placed[..at].iter().rev() {
            insert_sibling(&anchor, false, node, false)?;
            anchor = node.clone();
        }
    }
    let mut anchor = holder.clone();
    for node in &placed[at.map_or(0, |at| at + 1)..] {
        insert_sibling(&anchor, true, node, false)?;
        anchor = node.clone();
    }
    for node in &placed {
        ctx.add_path(node);
        ctx.touched.add(node.get_key());
    }
    if at.is_none() {
        // Every child was lifted out: the emptied holder goes, and a point
        // in it goes before what was lifted out of it.
        if let Some(first) = placed.first() {
            ctx.substitutes
                .insert(holder.get_key().to_owned(), keys_before(first));
        }
        detach(holder)?;
    }
    Ok(holders)
}

/// A holder's own hook for a child its declared `children` allow that must
/// still be lifted out (`INTERNAL_$liftsFromStructure`): the holder, the
/// child, and whether this is `INTERNAL_$normalizeNodeStructure`'s
/// normalization.
pub type LiftsFromStructure =
    fn(holder: &NodeRef, child: &NodeRef, explicit: bool) -> Result<bool, LexicalError>;

/// `liftsFromStructure(holder)`.
fn lifts_from_structure(holder: &NodeRef) -> Option<LiftsFromStructure> {
    holder.get_latest().internal_lifts_from_structure()
}

#[derive(Clone, Copy, PartialEq)]
enum InlineKind {
    Keep,
    Join,
    Lift,
}

#[derive(Clone, Copy, PartialEq)]
enum ClassKind {
    Keep,
    Wrap,
    Flatten,
    Lift,
}

/// `inlineRuleKinds(holder, rule, children, explicit)`: what a holder of
/// inline content does with each child.
fn inline_rule_kinds(
    holder: &NodeRef,
    rule: &NodeChildrenConfig,
    children: &[NodeRef],
    explicit: bool,
) -> Result<Vec<InlineKind>, LexicalError> {
    let inline_holder = holder.is_inline();
    let lifts = lifts_from_structure(holder);
    children
        .iter()
        .map(|c| {
            Ok(if allows(rule, c) {
                match lifts {
                    Some(lifts) if lifts(holder, c, explicit)? => InlineKind::Lift,
                    _ => InlineKind::Keep,
                }
            } else if !inline_holder && is_joinable_paragraph(rule, c) {
                InlineKind::Join
            } else if is_inline_child(c) {
                InlineKind::Keep
            } else {
                InlineKind::Lift
            })
        })
        .collect()
}

/// `rule[0]` as a class: a list that names no `'inline'` lists classes.
fn first_listed_class(rule: &NodeChildrenConfig) -> Option<NodeClassRef> {
    match rule.entries()?.first()? {
        NodeChildEntry::Class(class) => Some(*class),
        NodeChildEntry::Inline => None,
    }
}

/// `classRuleKinds(rule, children)`: what a holder that lists the classes
/// it allows (no `'inline'`) does with each child.
fn class_rule_kinds(rule: &NodeChildrenConfig, children: &[NodeRef]) -> Vec<ClassKind> {
    let wrapper = first_listed_class(rule);
    let wrapper_children = wrapper.and_then(|w| declared_children(&w.chain()));
    children
        .iter()
        .map(|c| {
            if allows(rule, c) {
                ClassKind::Keep
            } else if wrapper.is_some() && wrapper_children.as_ref().is_none_or(|w| allows(w, c)) {
                ClassKind::Wrap
            } else if wrapper.is_some()
                && wrapper_children
                    .as_ref()
                    .is_some_and(|w| is_joinable_paragraph(w, c))
            {
                ClassKind::Flatten
            } else if is_inline_child(c) {
                ClassKind::Keep
            } else {
                ClassKind::Lift
            }
        })
        .collect()
}

/// `$wrapLiftedContent(block, holder, ctx)`: before `block` is lifted out
/// of the inline element `holder` (a link, a mark), wrap each run of inline
/// content in it, and in every block it holds at any depth, in a copy of
/// `holder`.
fn wrap_lifted_content(
    block: &NodeRef,
    holder: &NodeRef,
    ctx: &mut Normalization,
) -> Result<(), LexicalError> {
    let mut copies: Vec<NodeRef> = Vec::new();
    let mut run: Vec<NodeRef> = Vec::new();
    let flush = |run: &mut Vec<NodeRef>, copies: &mut Vec<NodeRef>| -> Result<(), LexicalError> {
        if !run.is_empty() {
            let copy = copy_node(holder);
            insert_sibling(&run[0], false, &copy, false)?;
            append(&copy, run)?;
            copies.push(copy);
            run.clear();
        }
        Ok(())
    };
    for child in block.get_children() {
        // An OverflowNode is not wrapped but wrapped inside.
        if is_inline_child(&child) && !is_overflow_node(&child) {
            run.push(child);
            continue;
        }
        flush(&mut run, &mut copies)?;
        if child.is_element() {
            wrap_lifted_content(&child, holder, ctx)?;
        }
    }
    flush(&mut run, &mut copies)?;
    if copies.is_empty() {
        return Ok(());
    }
    ctx.changed = true;
    ctx.written.add(block.get_key());
    if let Some(rule) = effective_children(block)
        && copies.iter().any(|copy| !allows(&rule, copy))
    {
        apply_rules(block, &rule, true, ctx)?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
enum Last {
    None,
    Inline,
    Line,
    Block,
}

/// `$applyInlineRule(holder, rule, ctx)`: a holder of inline content: plain
/// paragraphs become lines of it, joined to the inline content next to them
/// with line breaks (an empty one keeps its line), and other blocks are
/// lifted out, the holder split around them.
fn apply_inline_rule(
    holder: &NodeRef,
    rule: &NodeChildrenConfig,
    ctx: &mut Normalization,
) -> Result<Vec<NodeRef>, LexicalError> {
    let inline_holder = holder.is_inline();
    let children = holder.get_children();
    let kinds = inline_rule_kinds(holder, rule, &children, ctx.explicit)?;
    if kinds.iter().all(|k| *k == InlineKind::Keep) {
        return Ok(vec![holder.clone()]);
    }
    let mut pieces: Vec<RebuildPiece> = Vec::new();
    let mut segments: Vec<Segment> = vec![Segment::default()];
    // (paragraph, segment, at)
    let mut empty_lines: Vec<(NodeRef, usize, usize)> = Vec::new();
    let mut last = Last::None;
    for (child, kind) in children.iter().zip(kinds) {
        let segment = segments.len() - 1;
        if kind == InlineKind::Lift {
            if inline_holder && child.is_element() {
                // Out of an inline element, the block's inline content is
                // wrapped in copies of it first.
                wrap_lifted_content(child, holder, ctx)?;
            }
            pieces.push(RebuildPiece::Segment(segment));
            pieces.push(RebuildPiece::Lifted(child.clone()));
            segments.push(Segment::default());
            last = Last::None;
            continue;
        }
        let s = &mut segments[segment];
        s.had_children = true;
        if kind == InlineKind::Join {
            if last == Last::Inline || last == Last::Line {
                s.content.push(create_line_break_node());
            }
            let content = child.get_children();
            if content.is_empty() {
                empty_lines.push((child.clone(), segment, s.content.len()));
            }
            s.content.extend(content);
            last = Last::Line;
            continue;
        }
        let unit = if is_inline_child(child) {
            Last::Inline
        } else {
            Last::Block
        };
        if unit == Last::Inline && last == Last::Line {
            s.content.push(create_line_break_node());
        }
        s.content.push(child.clone());
        last = unit;
    }
    pieces.push(RebuildPiece::Segment(segments.len() - 1));
    let holders = rebuild(holder, &pieces, &mut segments, ctx)?;
    for (paragraph, segment, at) in empty_lines {
        // An empty paragraph's line: after what precedes it in its segment,
        // or before what follows, or the holder of its segment.
        let s = &segments[segment];
        let anchor = if at > 0 {
            keys_after(&s.content[at - 1])
        } else if let Some(after) = s.content.get(at) {
            keys_before(after)
        } else {
            Anchor::Inside(s.holder.as_ref().unwrap_or(holder).get_key().to_owned())
        };
        ctx.substitutes
            .insert(paragraph.get_key().to_owned(), anchor);
    }
    Ok(holders)
}

/// `$applyClassRule(holder, rule, ctx)`: a holder that lists the classes it
/// allows and not `'inline'` (a list): a child the first listed class can
/// hold is wrapped in one (a run of inline content in one), a plain
/// paragraph becomes one holding its content, and any other block is lifted
/// out and the holder split around it.
fn apply_class_rule(
    holder: &NodeRef,
    rule: &NodeChildrenConfig,
    ctx: &mut Normalization,
) -> Result<Vec<NodeRef>, LexicalError> {
    let wrapper = first_listed_class(rule);
    let children = holder.get_children();
    let kinds = class_rule_kinds(rule, &children);
    if kinds.iter().all(|k| *k == ClassKind::Keep) {
        return Ok(vec![holder.clone()]);
    }
    let create_wrapper = || set_node_key((wrapper.expect("a listed class").new_node)());
    let mut pieces: Vec<RebuildPiece> = Vec::new();
    let mut segments: Vec<Segment> = vec![Segment::default()];
    let mut i = 0;
    while i < children.len() {
        let child = &children[i];
        let kind = kinds[i];
        let segment = segments.len() - 1;
        if kind == ClassKind::Lift {
            pieces.push(RebuildPiece::Segment(segment));
            pieces.push(RebuildPiece::Lifted(child.clone()));
            segments.push(Segment::default());
            i += 1;
            continue;
        }
        segments[segment].had_children = true;
        match kind {
            ClassKind::Keep => segments[segment].content.push(child.clone()),
            ClassKind::Flatten => {
                let w = create_wrapper();
                append(&w, &child.get_children())?;
                ctx.substitutes.insert(
                    child.get_key().to_owned(),
                    Anchor::Inside(w.get_key().to_owned()),
                );
                segments[segment].content.push(w);
            }
            _ => {
                let mut run = vec![child.clone()];
                while is_inline_child(child)
                    && i + 1 < children.len()
                    && kinds[i + 1] == ClassKind::Wrap
                    && is_inline_child(&children[i + 1])
                {
                    i += 1;
                    run.push(children[i].clone());
                }
                let w = create_wrapper();
                append(&w, &run)?;
                segments[segment].content.push(w);
            }
        }
        i += 1;
    }
    pieces.push(RebuildPiece::Segment(segments.len() - 1));
    rebuild(holder, &pieces, &mut segments, ctx)
}

/// `$wrapInlineRun(child, ctx)`: a holder of blocks: each run of inline
/// children is wrapped in the element its first node creates as a parent (a
/// paragraph), as core's shadow root transform does.
fn wrap_inline_run(child: &NodeRef, ctx: &mut Normalization) -> Result<(), LexicalError> {
    let blocked = |n: &NodeRef| !allows(&NodeChildrenConfig::Block, n);
    if !blocked(child) {
        return Ok(());
    }
    let mut first = child.clone();
    while let Some(p) = first.get_previous_sibling().filter(|p| blocked(p)) {
        first = p;
    }
    let mut run = vec![first.clone()];
    let mut n = first.get_next_sibling();
    while let Some(current) = n.filter(|n| blocked(n)) {
        n = current.get_next_sibling();
        run.push(current);
    }
    let w = first.create_parent_element_node();
    insert_sibling(&first, false, &w, false)?;
    append(&w, &run)?;
    ctx.changed = true;
    ctx.written.add(w.get_key());
    ctx.add_path(&w);
    ctx.touched.add(w.get_key());
    Ok(())
}

/// `$checksAllChildren(holder, ctx)`: whether the children of `holder`
/// must all be checked against its rule. JS asks whether the class writes
/// its own `isShadowRoot()` or `isInline()` (`hasInstanceRuleInputs`); as in
/// Swift, every class but the root counts as one that does.
fn checks_all_children(holder: &NodeRef, ctx: &Normalization) -> bool {
    if lifts_from_structure(holder).is_some() {
        // Whether a child is allowed depends on its siblings too.
        return true;
    }
    if holder.is_root() {
        return ctx.full;
    }
    let latest = holder.get_latest();
    let inputs = u8::from(is_root_or_shadow_root(&*latest)) + 2 * u8::from(is_inline_child(holder));
    let mut before = latest.base().structure_rule_inputs.get();
    if before.is_none()
        && let Some(editor) = get_active_editor()
    {
        let committed = editor.get_editor_state();
        before = committed
            .0
            .node_map
            .borrow()
            .get(holder.get_key())
            .and_then(|n| n.base().structure_rule_inputs.get());
    }
    latest.base().structure_rule_inputs.set(Some(inputs));
    ctx.full || before != Some(inputs)
}

/// `$applyRules(holder, rule, checksAll, ctx)`: apply `holder`'s rule.
/// Returns the holders it was split into, or `None` when it kept its
/// children as they were.
fn apply_rules(
    holder: &NodeRef,
    rule: &NodeChildrenConfig,
    checks_all: bool,
    ctx: &mut Normalization,
) -> Result<Option<Vec<NodeRef>>, LexicalError> {
    if *rule == NodeChildrenConfig::Block && checks_all && !holds_inline(holder) {
        // The common case (the root in every update).
        return Ok(None);
    }
    let holder_key = holder.get_key().to_owned();
    let children: Vec<NodeRef> = if checks_all {
        holder.get_children()
    } else {
        ctx.children(holder)
            .into_iter()
            .filter(|c| parent_key_of(c).as_deref() == Some(&holder_key))
            .collect()
    };
    if *rule == NodeChildrenConfig::Block {
        for c in &children {
            if parent_key_of(c).as_deref() == Some(&holder_key) {
                wrap_inline_run(c, ctx)?;
            }
        }
        return Ok(None);
    }
    if rule.holds_inline() {
        return Ok(
            if inline_rule_kinds(holder, rule, &children, ctx.explicit)?
                .iter()
                .all(|k| *k == InlineKind::Keep)
            {
                None
            } else {
                Some(apply_inline_rule(holder, rule, ctx)?)
            },
        );
    }
    Ok(
        if class_rule_kinds(rule, &children)
            .iter()
            .all(|k| *k == ClassKind::Keep)
        {
            None
        } else {
            Some(apply_class_rule(holder, rule, ctx)?)
        },
    )
}

/// `$normalizeElement(node, ctx)`: normalize `node`'s subtree bottom-up.
/// Returns whether `node` was removed.
fn normalize_element(node: &NodeRef, ctx: &mut Normalization) -> Result<bool, LexicalError> {
    let rule = effective_children(node);
    // Decided once per visit: it records what the rule depends on.
    let checks_all = rule.as_ref().is_some_and(|rule| {
        checks_all_children(node, ctx)
            || (*rule == NodeChildrenConfig::Block && ctx.wraps_all.contains(node.get_key()))
    });
    let root_or_shadow = |n: &NodeRef| is_root_or_shadow_root(&*n.get_latest());
    if !ctx.full && !ctx.path_by_parent.contains(node.get_key()) {
        // Written, but none of its children was.
        if node.is_empty() && !root_or_shadow(node) && is_removed_when_empty(node) {
            remove_empty(node, ctx)?;
            return Ok(true);
        }
        if !checks_all {
            return Ok(false);
        }
    }
    merge_children(node, ctx, false)?;
    let mut removed_child = false;
    for c in ctx.children(node) {
        if c.is_element()
            && parent_key_of(&c).as_deref() == Some(node.get_key())
            && normalize_element(&c, ctx)?
        {
            removed_child = true;
        }
    }
    if node.is_empty()
        && !root_or_shadow(node)
        && (is_removed_when_empty(node) || (removed_child && !node.can_be_empty()))
    {
        remove_empty(node, ctx)?;
        return Ok(true);
    }
    let holders = match &rule {
        Some(rule) => apply_rules(node, rule, checks_all, ctx)?,
        None => None,
    };
    match holders {
        // A rebuilt holder's children were all moved: every pair is new.
        Some(holders) => {
            for holder in holders {
                merge_children(&holder, ctx, true)?;
            }
        }
        None => merge_children(node, ctx, false)?,
    }
    Ok(false)
}

/// `isRemovedWhenEmpty(node)`: an inline element that cannot be empty (a
/// link, a mark), or the character limit's `OverflowNode`.
fn is_removed_when_empty(node: &NodeRef) -> bool {
    is_overflow_node(node) || (is_inline_child(node) && !node.can_be_empty())
}

/// `$removeEmpty(node, ctx)`.
fn remove_empty(node: &NodeRef, ctx: &mut Normalization) -> Result<(), LexicalError> {
    let prev = node.get_previous_sibling();
    let next = node.get_next_sibling();
    let parent = node.get_parent_or_throw()?;
    let anchor = match (&prev, &next) {
        (Some(prev), _) => keys_after(prev),
        (None, Some(next)) => keys_before(next),
        (None, None) => Anchor::Inside(parent.get_key().to_owned()),
    };
    ctx.substitutes.insert(node.get_key().to_owned(), anchor);
    detach(node)?;
    // The siblings it separated are now adjacent: they may merge.
    for sibling in [prev, next].into_iter().flatten() {
        ctx.add_path(&sibling);
        ctx.touched.add(sibling.get_key());
    }
    ctx.changed = true;
    ctx.written.add(parent.get_key());
    Ok(())
}

/// `isStructurallyUnchanged(node, committed)`. Only the root's class counts
/// as structurally static here (see [`checks_all_children`]).
fn is_structurally_unchanged(node: &NodeRef, committed: Option<&Rc<dyn LexicalNode>>) -> bool {
    let Some(committed) = committed else {
        return false;
    };
    if !node.is_root() || !committed.is::<crate::nodes::root::RootNode>() {
        return false;
    }
    let n = node.get_latest();
    let (nb, cb) = (n.base(), committed.base());
    let (ne, ce) = (
        n.element().expect("an element"),
        committed.element().expect("an element"),
    );
    cb.parent == nb.parent
        && cb.prev == nb.prev
        && cb.next == nb.next
        && ce.first == ne.first
        && ce.last == ne.last
        && ce.size == ne.size
}

/// `$normalizeDirty(dirty, full, committedNormalized)`: repair the
/// structure around every node written since the last pass, from the root
/// down.
fn normalize_dirty(
    dirty: &mut Vec<NodeKey>,
    full: bool,
    committed_normalized: bool,
) -> Result<(), LexicalError> {
    let root = get_root();
    if dirty.is_empty() && !full && !holds_inline(&root) {
        return Ok(());
    }
    let mut ctx = Normalization {
        full,
        ..Normalization::default()
    };
    let root_key = root.get_key().to_owned();
    // Core's shadow root transform, which the extension turns off, wraps the
    // inline children of the root in every update and those of a shadow
    // root that was written: so does this.
    ctx.wraps_all.insert(root_key.clone());
    ctx.path.insert(root_key);
    let committed = committed_normalized
        .then(get_active_editor)
        .flatten()
        .map(|editor| editor.get_editor_state());
    let keys: Vec<NodeKey> = if full { Vec::new() } else { dirty.clone() };
    for key in keys {
        let node = get_node_by_key(&key);
        if let Some(node) = &node
            && node.is_element()
            && node.get_latest().is_shadow_root()
        {
            ctx.wraps_all.insert(key.clone());
        }
        if let (Some(committed), Some(node)) = (&committed, &node)
            && is_structurally_unchanged(node, committed.0.node_map.borrow().get(&key))
        {
            continue;
        }
        let mut chain: Vec<NodeRef> = Vec::new();
        let mut attached = false;
        let mut n = node;
        while let Some(current) = n {
            if ctx.path.contains(current.get_key()) {
                attached = true;
                break;
            }
            n = current.get_parent();
            chain.push(current);
        }
        if attached {
            for n in &chain {
                ctx.add_path(n);
            }
        }
    }
    dirty.clear();
    if ctx.path.len() > 1 || full || holds_inline(&root) {
        normalize(&mut ctx, |ctx| normalize_element(&root, ctx).map(|_| ()))?;
    }
    Ok(())
}

/// `isBlockElement(node)`: an element whose `isInline()` returns false.
fn is_block_element(node: &NodeRef) -> bool {
    node.is_element() && !node.is_inline()
}

/// `$holdsInline(node)`: whether `node` holds content that a holder of
/// blocks wraps.
fn holds_inline(node: &NodeRef) -> bool {
    let mut c = node.get_first_child();
    while let Some(current) = c {
        if !is_block_element(&current) && !allows(&NodeChildrenConfig::Block, &current) {
            return true;
        }
        c = current.get_next_sibling();
    }
    false
}

/// `$normalize(ctx, repair)`: run `repair`, then make every ancestor of a
/// change dirty, so that its own transforms run again on the result, and
/// put the selection back among the content it was in.
fn normalize(
    ctx: &mut Normalization,
    repair: impl FnOnce(&mut Normalization) -> Result<(), LexicalError>,
) -> Result<(), LexicalError> {
    let selection = get_selection().filter(|s| s.is_range());
    let mut saved: Vec<(Which, Option<Anchor>)> = Vec::new();
    if let Some(selection) = &selection {
        for which in [Which::Anchor, Which::Focus] {
            let point = selection.point(which).expect("a range selection");
            saved.push((which, save_anchor(&point)?));
        }
    }
    repair(ctx)?;
    for key in ctx.written.iter() {
        if let Some(node) = get_node_by_key(key).filter(NodeRef::is_attached) {
            let mut n = Some(node);
            while let Some(current) = n.filter(|n| !n.is_root()) {
                current.mark_dirty();
                n = current.get_parent();
            }
        }
    }
    if ctx.changed
        && let Some(selection) = &selection
    {
        for (which, anchor) in saved {
            if let Some((key, offset, point_type)) =
                anchor.and_then(|anchor| ctx.resolve(&anchor, 0))
            {
                selection.point_set(which, &key, offset, point_type)?;
            }
        }
    }
    Ok(())
}

/// `INTERNAL_$normalizeNodeStructure(node)` (internal): normalize the
/// subtree rooted at `node` now, as `NodeStructureExtension` does, bottom-up
/// (which may split `node` in its parent), then merge `node` (or what it was
/// split into) with its equal siblings. For code that applies an update with
/// `skip_transforms`, which the extension doesn't normalize.
pub fn internal_normalize_node_structure(node: &NodeRef) -> Result<(), LexicalError> {
    if !node.is_element() || !node.is_attached() {
        return Ok(());
    }
    let mut ctx = Normalization {
        full: true,
        explicit: true,
        ..Normalization::default()
    };
    let parent = node.get_parent();
    normalize(&mut ctx, |ctx| {
        let Some(parent) = parent.filter(|_| !is_root_or_shadow_root(&*node.get_latest())) else {
            // A root (or shadow root) is never split: normalize it in place.
            return normalize_element(node, ctx).map(|_| ());
        };
        let parent_key = parent.get_key().to_owned();
        ctx.touched.add(node.get_key());
        normalize_element(node, ctx)?;
        let touched: Vec<NodeKey> = ctx.touched.iter().map(str::to_owned).collect();
        for key in touched {
            if let Some(piece) = get_node_by_key(&key)
                && parent_key_of(&piece).as_deref() == Some(&parent_key)
            {
                merge_run(&piece, ctx)?;
            }
        }
        Ok(())
    })
}

// MARK: Registration

/// One editor's structure registration (`StructureRegistration`), counted.
pub(crate) struct NodeStructureRegistration {
    refs: Cell<usize>,
    dispose: RefCell<Option<Unregister>>,
    /// The nodes collected since the last pass, and since the last commit.
    dirty: RefCell<Vec<NodeKey>>,
    collected: RefCell<HashSet<NodeKey>>,
    /// Committed states that may not be normalized: the one before the
    /// registration, and any that an update applied without transforms
    /// changed (a JS `WeakSet`).
    unnormalized: RefCell<Vec<Weak<EditorStateInner>>>,
    /// The first pass after registering normalizes the whole document.
    full: Cell<bool>,
}

impl NodeStructureRegistration {
    fn is_unnormalized(&self, state: &crate::EditorState) -> bool {
        let mut unnormalized = self.unnormalized.borrow_mut();
        unnormalized.retain(|w| w.strong_count() > 0);
        unnormalized
            .iter()
            .any(|w| std::ptr::eq(w.as_ptr(), Rc::as_ptr(&state.0)))
    }
}

/// `INTERNAL_registerNodeStructure(editor)` (internal, for
/// `NodeStructureExtension` and `registerNodeStructure`): enforce the
/// structure node classes declare on `editor`, and mark every element and
/// decorator dirty so the current document is normalized too.
/// Registrations are counted: the normalization stops when the last one is
/// disposed.
///
/// Every class gets a transform that records the nodes written in the
/// update, and the root a transform, which runs last in each transform
/// pass, that normalizes from the root down along those nodes' ancestors.
pub fn internal_register_node_structure(editor: &Editor) -> Unregister {
    let existing = editor.0.node_structure.borrow().clone();
    let registration = match existing {
        Some(registration) => registration,
        None => {
            let registration = Rc::new(NodeStructureRegistration {
                refs: Cell::new(0),
                dispose: RefCell::new(None),
                dirty: RefCell::new(Vec::new()),
                collected: RefCell::new(HashSet::new()),
                unnormalized: RefCell::new(vec![Rc::downgrade(&editor.get_editor_state().0)]),
                full: Cell::new(true),
            });
            let mut cleanups: Vec<Unregister> = Vec::new();
            {
                let registration = Rc::downgrade(&registration);
                cleanups.push(editor.register_update_listener(move |payload| {
                    let Some(registration) = registration.upgrade() else {
                        return;
                    };
                    let node_map = payload.editor_state.0.node_map.borrow();
                    let skipped = |key: &str| {
                        !registration.collected.borrow().contains(key) && node_map.contains_key(key)
                    };
                    let complete =
                        !payload.dirty_elements.iter().any(|(key, intentional)| {
                            *intentional && key != "root" && skipped(key)
                        }) && !payload.dirty_leaves.iter().any(|key| skipped(key));
                    registration.collected.borrow_mut().clear();
                    if !complete {
                        registration
                            .unnormalized
                            .borrow_mut()
                            .push(Rc::downgrade(&payload.editor_state.0));
                    }
                }));
            }
            let mut types: Vec<&'static str> = Vec::new();
            for class in editor.registered_nodes_in_order() {
                let node_type = class.node_type();
                let registration = Rc::downgrade(&registration);
                cleanups.push(if node_type == "root" {
                    editor.add_node_transform(node_type, move |_| {
                        let Some(registration) = registration.upgrade() else {
                            return Ok(());
                        };
                        let all = registration.full.replace(false);
                        let committed_normalized = get_active_editor()
                            .is_some_and(|e| !registration.is_unnormalized(&e.get_editor_state()));
                        let mut dirty = std::mem::take(&mut *registration.dirty.borrow_mut());
                        let result = normalize_dirty(&mut dirty, all, committed_normalized);
                        // Keys collected while it ran stay for the next pass.
                        let mut pending = registration.dirty.borrow_mut();
                        dirty.append(&mut pending);
                        *pending = dirty;
                        result
                    })
                } else {
                    editor.add_node_transform(node_type, move |node| {
                        if let Some(registration) = registration.upgrade() {
                            let key = node.get_key().to_owned();
                            registration.collected.borrow_mut().insert(key.clone());
                            registration.dirty.borrow_mut().push(key);
                        }
                        Ok(())
                    })
                });
                if (class.is_element_or_decorator)() {
                    types.push(node_type);
                }
            }
            *registration.dispose.borrow_mut() = Some(crate::merge_register(cleanups));
            *editor.0.node_structure.borrow_mut() = Some(registration.clone());
            crate::editor::mark_nodes_with_types_as_dirty(editor, &types);
            registration
        }
    };
    registration.refs.set(registration.refs.get() + 1);
    let editor = editor.weak();
    // An `Unregister` is called at most once.
    Box::new(move || {
        registration.refs.set(registration.refs.get() - 1);
        if registration.refs.get() == 0 {
            if let Some(dispose) = registration.dispose.borrow_mut().take() {
                dispose();
            }
            if let Some(inner) = editor.upgrade() {
                let mut current = inner.node_structure.borrow_mut();
                if current
                    .as_ref()
                    .is_some_and(|r| Rc::ptr_eq(r, &registration))
                {
                    *current = None;
                }
            }
        }
    })
}

/// `INTERNAL_isNodeStructureEnforced(editor)` (internal): whether `editor`
/// enforces declared node structure (`NodeStructureExtension` or
/// `registerNodeStructure` is registered on it).
pub fn internal_is_node_structure_enforced(editor: &Editor) -> bool {
    editor.0.node_structure.borrow().is_some()
}

/// Whether there is an active editor and it enforces declared structure.
pub(crate) fn is_structure_enforced_on_active_editor() -> bool {
    get_active_editor().is_some_and(|e| internal_is_node_structure_enforced(&e))
}

/// Whether the active editor enforces declared structure and `node`'s class
/// declares `mergeAdjacent` itself, so that the normalization merges it with
/// a next sibling of the same class (`$structureMerges` in `@lexical/link`
/// and `@lexical/list`).
#[cfg_attr(not(any(feature = "link", feature = "list")), allow(dead_code))]
pub(crate) fn structure_merges(node: &NodeRef, next: &NodeRef) -> bool {
    next.get_latest().as_any().type_id() == node.get_latest().as_any().type_id()
        && is_structure_enforced_on_active_editor()
        && cached_of(node).merges_itself()
}

/// `$isBlockContainer(node)` (`@experimental node-structure`): whether
/// structural commands treat `node` as a container of blocks: the root, a
/// shadow root, or, in an editor that enforces declared node structure, a
/// block element whose class declares `children: 'block'`. With no active
/// editor, only the root and shadow roots are containers.
pub fn is_block_container(node: &NodeRef) -> bool {
    if !node.is_element() {
        return false;
    }
    is_root_or_shadow_root(&*node.get_latest())
        || (cached_of(node).declared.children == Some(NodeChildrenConfig::Block)
            && !node.is_inline()
            && is_structure_enforced_on_active_editor())
}

/// `validateNodeStructure(klass)`: the development check of a class's
/// structural declarations, run when an editor is created. A bad
/// declaration is a bug in the node's implementation, so it panics with
/// JS's message.
pub(crate) fn validate_node_structure(node: &dyn LexicalNode) {
    let cached = cached_node_structure(node);
    let declared = &cached.declared;
    let name = cached.own_type();
    if cached.merges_itself() {
        let merge = declared.merge_adjacent.as_ref().expect("merges");
        match invalid_merge_by(node, merge, &declared.derived) {
            Some((entry, "unknown")) => panic!(
                "{name}: mergeAdjacent.by names {entry}, which is not a schema field of the class"
            ),
            Some((entry, "derived")) => {
                panic!("{name}: mergeAdjacent.by names {entry}, which is derived")
            }
            Some((entry, _)) => {
                panic!("{name}: mergeAdjacent.by names the derived NodeState {entry}")
            }
            None => {}
        }
        if let Some(item) =
            merging_child_not_inline(&cached.chain_types, declared.children.as_ref())
        {
            panic!(
                "{name}: a merging class that lists child classes must list classes that hold inline content ({item} does not)"
            );
        }
    }
    if let Some(inline) = declared.inline
        && declared.declared_by.inline == Some(name)
        && node.is_inline() != inline
    {
        panic!(
            "{name}: $config inline is {inline} but isInline() returns {}",
            node.is_inline()
        );
    }
}
