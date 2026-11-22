//! `MarkStateExtension.ts`: mark ids kept as a NodeState of the leaves they
//! cover (a sorted array without duplicates, `arraySetValue(markIDValue())`)
//! instead of `MarkNode`s around them, one state key per mark flavor.
//!
//! The functions work without an extension, on `MarkStateExtension`'s ids
//! with the default `markableNodes`, as JS's do when the editor is not
//! built with it. With the `extension` feature, `MarkStateExtension`,
//! `MarkBaseExtension`, `defineMarkExtension` and the flavor-first forms of
//! the functions (`*_for`, which take the flavor first as JS's overloads
//! do), the `markableNodes` config, `disabled`, the `markIDNodeKeys` output
//! (which `$removeMarkID` reads), `registerMarkIDsListener` and
//! `registerMarkState`'s inserted-text rule (`mark_state_inserted_text`);
//! with `html` too, `exportIDsToHTML` and the ids attribute's import rule.
//! The renderer's half (`$decorateMarkIDs`, `isMarkIDShown`) needs a
//! reconciled DOM, which the port has not (rust/PARITY.md). The slots of a
//! node are looked through (`$forEachSlot`).

use crate::error::invariant;
use crate::json::Value;
use crate::lexical_selection::{PointType, SelectionRef, Which};
use crate::lexical_utils::get_root;
use crate::node::NodeRef;
use crate::node_state::{
    StateConfig, StateValueConfig, create_state_with, get_state, set_state_with,
};
use crate::nodes::line_break::LineBreakNode;
use crate::schema::{SerializationSchema, SerializationSchemaMeta, array_set_value};
use crate::{LexicalError, NodeKey};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::rc::Rc;

use super::is_mark_node;

#[cfg(feature = "extension")]
pub use extension::*;

/// `MARK_STATE_KEY`: the NodeState key that the mark ids of a leaf are
/// stored under, in the `$` record of its JSON.
pub const MARK_STATE_KEY: &str = "markIds";

/// `MARK_STATE_IDS_ATTRIBUTE`: the attribute `MarkStateExtension` writes
/// the ids of a leaf to in the HTML export (with `exportIDsToHTML`), and
/// reads them from in the import.
pub const MARK_STATE_IDS_ATTRIBUTE: &str = "data-lexical-mark-ids";

/// `MARK_STATE_EXTENSION_NAME`.
pub(super) const MARK_STATE_EXTENSION_NAME: &str = "@lexical/mark/MarkState";

/// JS's `\s` without the `u` flag: ECMAScript's WhiteSpace and
/// LineTerminator.
pub(super) fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// `isValidMarkID(id)`: a non-empty string without whitespace.
pub(super) fn is_valid_mark_id(id: &str) -> bool {
    !id.is_empty() && !id.chars().any(is_js_whitespace)
}

/// JS's default sort order of strings: by UTF-16 code units.
pub(super) fn js_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `markIDValue()`: the item schema of the ids, a string that
/// `isValidMarkID` accepts (`''` otherwise), hand-rolled with its own
/// `accepts`.
fn mark_id_value() -> SerializationSchema {
    SerializationSchema::new(
        SerializationSchemaMeta::String,
        "".into(),
        None,
        Some(Rc::new(|value: &Value| {
            value.as_str().is_some_and(is_valid_mark_id)
        })),
        |value| match value.as_str() {
            Some(id) if is_valid_mark_id(id) => value.clone(),
            _ => "".into(),
        },
    )
}

/// `createMarkIDsState(stateKey)`: the state of a flavor's ids,
/// `arraySetValue(markIDValue())`: the valid ids, sorted as JS sorts
/// strings, without duplicates, which `$setState` also makes of a value it
/// is given.
fn create_mark_ids_state(state_key: &str) -> StateConfig<Vec<String>> {
    create_state_with(
        state_key,
        StateValueConfig::from_schema(array_set_value(mark_id_value()).expect("a set of strings")),
    )
}

/// `MarkFlavor`: what the functions need of a mark flavor, made once per
/// flavor (`MarkStateExtension`'s, or `defineMarkExtension`'s). JS keeps
/// the compiled `markableNodes` and the pending edits of each editor in
/// `WeakMap`s on it; the port keeps them in the init result of the flavor's
/// extension in that editor.
pub struct MarkFlavor {
    name: String,
    state_key: String,
    attribute: String,
    pub(super) state: StateConfig<Vec<String>>,
}

impl MarkFlavor {
    fn new(name: &str, state_key: &str, attribute: &str) -> Self {
        MarkFlavor {
            name: name.to_owned(),
            state_key: state_key.to_owned(),
            attribute: attribute.to_owned(),
            state: create_mark_ids_state(state_key),
        }
    }

    /// The name of the flavor's extension.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `stateKey`: the NodeState key the ids are stored under.
    pub fn state_key(&self) -> &str {
        &self.state_key
    }

    /// `attribute`: the attribute the ids are written to in HTML.
    pub fn attribute(&self) -> &str {
        &self.attribute
    }
}

thread_local! {
    /// `MarkStateExtension.__markFlavor`.
    static MARK_STATE_FLAVOR: Rc<MarkFlavor> = Rc::new(MarkFlavor::new(
        MARK_STATE_EXTENSION_NAME,
        MARK_STATE_KEY,
        MARK_STATE_IDS_ATTRIBUTE,
    ));
}

/// `MarkStateExtension.__markFlavor`: the default flavor, which the forms
/// without a flavor use.
fn default_flavor() -> Rc<MarkFlavor> {
    MARK_STATE_FLAVOR.with(Rc::clone)
}

/// `$isInlineDecoratorNode(node)`: an inline `DecoratorNode`, one of the
/// default `markableNodes`.
pub fn is_inline_decorator_node(node: &NodeRef) -> bool {
    node.is_decorator() && node.is_inline()
}

/// `DEFAULT_MARKABLE_NODES`, compiled: `TextNode` and its subclasses,
/// `LineBreakNode` and inline decorators.
fn default_is_markable(node: &NodeRef) -> bool {
    node.is_text() || node.get_latest().is::<LineBreakNode>() || is_inline_decorator_node(node)
}

/// `$isMarkableNode(flavor, node)`: whether `node` carries ids of `flavor`
/// in the active editor, by the `markableNodes` config of the flavor's
/// extension there, or the default.
pub(super) fn is_markable_node(flavor: &MarkFlavor, node: &NodeRef) -> bool {
    #[cfg(feature = "extension")]
    if let Some(is_markable) = extension::editor_is_markable(flavor) {
        return is_markable(node);
    }
    let _ = flavor;
    default_is_markable(node)
}

/// `$readNodeMarkIDs(flavor, node)`.
pub(super) fn read_node_mark_ids(
    flavor: &MarkFlavor,
    node: &NodeRef,
) -> Result<Vec<String>, LexicalError> {
    get_state(node, &flavor.state)
}

/// `$getMarkableIDs(flavor, node)`: the ids of a markable node, otherwise
/// none.
pub(super) fn get_markable_ids(
    flavor: &MarkFlavor,
    node: Option<&NodeRef>,
) -> Result<Vec<String>, LexicalError> {
    match node {
        Some(node) if is_markable_node(flavor, node) => read_node_mark_ids(flavor, node),
        _ => Ok(Vec::new()),
    }
}

/// `$getNodeMarkIDs(node)`: the mark ids stored on a node, sorted; none for
/// a node that carries none.
pub fn get_node_mark_ids(node: &NodeRef) -> Result<Vec<String>, LexicalError> {
    read_node_mark_ids(&default_flavor(), node)
}

/// `$setState(node, state, updater)` for the set-valued state, which makes
/// the value the updater returns canonical (`canonicalValue`) before it is
/// compared and stored.
fn update_node_mark_ids(
    flavor: &MarkFlavor,
    node: &NodeRef,
    updater: impl FnOnce(&Vec<String>) -> Vec<String>,
) -> Result<(), LexicalError> {
    set_state_with(node, &flavor.state, updater).map(|_| ())
}

/// `$addNodeMarkIDs(flavor, node, ids)`.
pub(super) fn add_node_mark_ids(
    flavor: &MarkFlavor,
    node: &NodeRef,
    ids: &[String],
) -> Result<(), LexicalError> {
    update_node_mark_ids(flavor, node, |prev| {
        if ids.iter().all(|id| prev.contains(id)) {
            prev.clone()
        } else {
            prev.iter().chain(ids).cloned().collect()
        }
    })
}

/// `$removeNodeMarkIDs(flavor, node, ids)`.
fn remove_node_mark_ids(
    flavor: &MarkFlavor,
    node: &NodeRef,
    ids: &[String],
) -> Result<(), LexicalError> {
    update_node_mark_ids(flavor, node, |prev| {
        prev.iter()
            .filter(|id| !ids.contains(id))
            .cloned()
            .collect()
    })
}

/// `assertValidMarkID(id)`.
fn assert_valid_mark_id(id: &str) -> Result<(), LexicalError> {
    invariant!(
        is_valid_mark_id(id),
        "MarkStateExtension: invalid mark id {}, ids must be non-empty strings without whitespace",
        Value::from(id)
    );
    Ok(())
}

/// `$markableNodesInSelection(flavor, selection)`: the markable nodes
/// `selection.extract()` returns, which splits the text nodes at the
/// boundaries of a range selection. None for a collapsed range selection.
fn markable_nodes_in_selection(
    flavor: &MarkFlavor,
    selection: &SelectionRef,
) -> Result<Vec<NodeRef>, LexicalError> {
    if selection.range().is_some_and(|r| r.is_collapsed()) {
        return Ok(Vec::new());
    }
    let mut nodes = Vec::new();
    let mut seen = HashSet::new();
    // `extract()` does not include the content of slots, which is collected
    // from each selected slot host.
    for node in selection.extract()? {
        if is_markable_node(flavor, &node) && seen.insert(node.get_key().to_owned()) {
            nodes.push(node.clone());
        }
        for_each_slot(&node, |slot| {
            collect_markable_nodes(flavor, slot, &mut nodes, &mut seen)
        });
    }
    Ok(nodes)
}

/// `$forEachSlot(node, fn)`: `f` of the value of each slot of `node`.
fn for_each_slot(node: &NodeRef, mut f: impl FnMut(&NodeRef)) {
    for name in crate::slot::get_slot_names(node) {
        if let Some(slot) = crate::slot::get_slot(node, &name) {
            f(&slot);
        }
    }
}

/// `$addMarkID(flavor, id, selection)`.
fn add_mark_id_impl(
    flavor: &MarkFlavor,
    id: &str,
    selection: &SelectionRef,
) -> Result<Vec<NodeRef>, LexicalError> {
    assert_valid_mark_id(id)?;
    let nodes = markable_nodes_in_selection(flavor, selection)?;
    let ids = [id.to_owned()];
    for node in &nodes {
        add_node_mark_ids(flavor, node, &ids)?;
    }
    Ok(nodes)
}

/// `$addMarkID(id, selection)`: adds the mark `id` to every markable node
/// in `selection`, splitting the text nodes at its boundaries, and returns
/// the nodes that now carry it. Ids already on those nodes are kept, so
/// marks overlap freely; a collapsed selection marks nothing. Fails on an
/// id that is empty or holds whitespace.
pub fn add_mark_id(id: &str, selection: &SelectionRef) -> Result<Vec<NodeRef>, LexicalError> {
    add_mark_id_impl(&default_flavor(), id, selection)
}

/// `$collectMarkableNodes(flavor, node, out, seen)`: the markable nodes in
/// the subtree of `node`, in document order.
pub(super) fn collect_markable_nodes(
    flavor: &MarkFlavor,
    node: &NodeRef,
    out: &mut Vec<NodeRef>,
    seen: &mut HashSet<NodeKey>,
) {
    if is_markable_node(flavor, node) && seen.insert(node.get_key().to_owned()) {
        out.push(node.clone());
    }
    if node.is_element() {
        let mut child = node.get_first_child();
        while let Some(c) = child {
            collect_markable_nodes(flavor, &c, out, seen);
            child = c.get_next_sibling();
        }
    }
    for_each_slot(node, |slot| collect_markable_nodes(flavor, slot, out, seen));
}

/// `$markableNodesIn(flavor, node)`.
pub(super) fn markable_nodes_in(flavor: &MarkFlavor, node: &NodeRef) -> Vec<NodeRef> {
    let mut out = Vec::new();
    collect_markable_nodes(flavor, node, &mut out, &mut HashSet::new());
    out
}

/// `$removeMarkID(flavor, id, selection)`.
fn remove_mark_id_impl(
    flavor: &MarkFlavor,
    id: &str,
    selection: Option<&SelectionRef>,
) -> Result<(), LexicalError> {
    let nodes = match selection {
        Some(selection) => markable_nodes_in_selection(flavor, selection)?,
        None => {
            #[cfg(feature = "extension")]
            let candidates = extension::get_mark_id_candidates(flavor, id);
            #[cfg(not(feature = "extension"))]
            let candidates: Option<Vec<NodeRef>> = None;
            candidates.unwrap_or_else(|| markable_nodes_in(flavor, &get_root()))
        }
    };
    let ids = [id.to_owned()];
    for node in &nodes {
        remove_node_mark_ids(flavor, node, &ids)?;
    }
    // The inserted-text rule of this update must not give the id back
    #[cfg(feature = "extension")]
    extension::record_removed(flavor, id);
    Ok(())
}

/// `$removeMarkID(id, selection)`: removes the mark `id` from the nodes in
/// `selection` (splitting the text nodes at its boundaries), or with `None`
/// from the whole document: from the leaves the `markIDNodeKeys` output
/// has for it and those the update changed when `MarkStateExtension` is
/// enabled, otherwise walking the document. Text left like its neighbors
/// merges back in normalization.
pub fn remove_mark_id(id: &str, selection: Option<&SelectionRef>) -> Result<(), LexicalError> {
    remove_mark_id_impl(&default_flavor(), id, selection)
}

/// `$getAdjacentLeaf(node, direction)`: the leaf next to `node` within its
/// block (after it, or with `previous` before it), looking through the
/// boundaries of inline elements such as links. An empty inline element is
/// returned as it is.
pub(super) fn get_adjacent_leaf(node: &NodeRef, previous: bool) -> Option<NodeRef> {
    let mut current = node.clone();
    loop {
        let sibling = if previous {
            current.get_previous_sibling()
        } else {
            current.get_next_sibling()
        };
        let Some(sibling) = sibling else {
            let parent = current.get_parent()?;
            if parent.is_element() && parent.is_inline() {
                current = parent;
                continue;
            }
            return None;
        };
        let mut leaf = sibling;
        while leaf.is_element() && leaf.is_inline() {
            let child = if previous {
                leaf.get_last_child()
            } else {
                leaf.get_first_child()
            };
            match child {
                Some(child) => leaf = child,
                None => break,
            }
        }
        return Some(leaf);
    }
}

/// `$getPointMarkIDs(flavor, point)`: the ids of the leaf a caret is in,
/// or of the next leaf when it is at the end of a text node without ids;
/// at an element point, those of the leaf before it, or else after it.
fn get_point_mark_ids(
    flavor: &MarkFlavor,
    selection: &SelectionRef,
    which: Which,
) -> Result<Vec<String>, LexicalError> {
    let point = selection.point(which).expect("a range selection");
    let node = point.get_node()?;
    if point.point_type == PointType::Text {
        let ids = read_node_mark_ids(flavor, &node)?;
        if ids.is_empty() && node.is_text() && point.offset == node.get_text_content_size() {
            // Like $getMarkIDs: a caret right before a marked run is in it
            return get_markable_ids(flavor, get_adjacent_leaf(&node, false).as_ref());
        }
        return Ok(ids);
    }
    if node.is_element() {
        let before = get_markable_ids(
            flavor,
            point
                .offset
                .checked_sub(1)
                .and_then(|i| node.get_child_at_index(i))
                .as_ref(),
        )?;
        return if before.is_empty() {
            get_markable_ids(flavor, node.get_child_at_index(point.offset).as_ref())
        } else {
            Ok(before)
        };
    }
    Ok(Vec::new())
}

/// The `mode` of [`get_mark_ids_at_selection`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkIDsMode {
    /// `'any'`: the ids on any selected leaf.
    #[default]
    Any,
    /// `'all'`: the ids every selected leaf shares.
    All,
}

/// `$getMarkIDsAtSelection(flavor, selection, mode)`.
fn get_mark_ids_at_selection_impl(
    flavor: &MarkFlavor,
    selection: Option<&SelectionRef>,
    mode: MarkIDsMode,
) -> Result<Vec<String>, LexicalError> {
    let Some(selection) = selection else {
        return Ok(Vec::new());
    };
    let range = selection.range();
    if range.as_ref().is_some_and(|r| r.is_collapsed()) {
        return get_point_mark_ids(flavor, selection, Which::Anchor);
    }
    let mut leaves: Vec<NodeRef> = selection
        .get_nodes()?
        .into_iter()
        .filter(|node| is_markable_node(flavor, node))
        .collect();
    if let Some(range) = range {
        let (start, end) = if selection.is_backward()? {
            (&range.focus, &range.anchor)
        } else {
            (&range.anchor, &range.focus)
        };
        leaves.retain(|node| {
            if !node.is_text() {
                return true;
            }
            let key = node.get_key();
            let from = if start.point_type == PointType::Text && start.key == key {
                start.offset
            } else {
                0
            };
            let to = if end.point_type == PointType::Text && end.key == key {
                end.offset
            } else {
                node.get_text_content_size()
            };
            to > from
        });
    }
    if leaves.is_empty() {
        return Ok(Vec::new());
    }
    let id_sets = leaves
        .iter()
        .map(|node| read_node_mark_ids(flavor, node))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(match mode {
        MarkIDsMode::Any => {
            let mut ids: Vec<String> = id_sets.into_iter().flatten().collect();
            ids.sort_by(|a, b| js_cmp(a, b));
            ids.dedup();
            ids
        }
        MarkIDsMode::All => id_sets[0]
            .iter()
            .filter(|id| id_sets.iter().all(|ids| ids.contains(id)))
            .cloned()
            .collect(),
    })
}

/// `$getMarkIDsAtSelection(selection, mode)`: the mark ids at a selection,
/// sorted. For a collapsed range selection, the ids of the leaf holding the
/// caret (or of the next leaf, at the end of a text node without ids);
/// otherwise the ids on any selected leaf, or with [`MarkIDsMode::All`]
/// those shared by every one. Text nodes a range only touches at an edge
/// are left out. `None` is JS's `null`; JS's default, the current
/// selection, is `get_selection().as_ref()`.
pub fn get_mark_ids_at_selection(
    selection: Option<&SelectionRef>,
    mode: MarkIDsMode,
) -> Result<Vec<String>, LexicalError> {
    get_mark_ids_at_selection_impl(&default_flavor(), selection, mode)
}

/// `$migrateMarkNodesToMarkState(flavor, node)`.
fn migrate_mark_nodes_to_mark_state_impl(
    flavor: &MarkFlavor,
    node: Option<&NodeRef>,
) -> Result<usize, LexicalError> {
    fn visit(
        flavor: &MarkFlavor,
        current: &NodeRef,
        inherited: &[String],
        count: &mut usize,
    ) -> Result<(), LexicalError> {
        if is_mark_node(current) {
            let own_ids = current.get_ids().unwrap_or_default();
            for id in &own_ids {
                assert_valid_mark_id(id)?;
            }
            let mut ids: Vec<String> = Vec::new();
            for id in inherited.iter().chain(&own_ids) {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
            for child in current.get_children() {
                visit(flavor, &child, &ids, count)?;
            }
            // Re-read: unwrapping a nested MarkNode replaced it with its
            // children
            for child in current.get_children() {
                current.insert_before(&child, true)?;
            }
            current.remove(false)?;
            *count += 1;
        } else {
            if is_markable_node(flavor, current) && !inherited.is_empty() {
                add_node_mark_ids(flavor, current, inherited)?;
            }
            if current.is_element() {
                for child in current.get_children() {
                    visit(flavor, &child, inherited, count)?;
                }
            }
            for name in crate::slot::get_slot_names(current) {
                if let Some(slot) = crate::slot::get_slot(current, &name) {
                    visit(flavor, &slot, inherited, count)?;
                }
            }
        }
        Ok(())
    }
    let node = node.cloned().unwrap_or_else(get_root);
    let mut count = 0;
    visit(flavor, &node, &[], &mut count)?;
    Ok(count)
}

/// `$migrateMarkNodesToMarkState(node)`: converts a document that uses
/// `MarkNode` to mark ids on its leaves. The ids of every mark (with those
/// of the marks it is in) are added to the markable nodes inside it, and
/// the mark is unwrapped. `None` is the root. Returns the number of marks
/// unwrapped; fails on an id that is empty or holds whitespace.
pub fn migrate_mark_nodes_to_mark_state(node: Option<&NodeRef>) -> Result<usize, LexicalError> {
    migrate_mark_nodes_to_mark_state_impl(&default_flavor(), node)
}

/// `value.split(/\s+/)`, without the empty strings.
#[cfg(all(feature = "extension", feature = "html"))]
fn split_on_whitespace(value: &str) -> impl Iterator<Item = &str> {
    value.split(is_js_whitespace).filter(|s| !s.is_empty())
}

#[cfg(feature = "extension")]
mod extension {
    use super::super::mark_state_inserted_text::{
        PendingEdits, SplitOrigins, apply_inserted_text_mark_ids, get_block,
        get_composition_replaced_range, get_replaced_range,
    };
    use super::*;
    use crate::commands::{
        COMPOSITION_START_COMMAND, CONTROLLED_TEXT_INSERTION_COMMAND, PASTE_COMMAND,
    };
    use crate::editor::{
        COLLABORATION_TAG, COMMAND_PRIORITY_CRITICAL, COMPOSITION_START_TAG, DirtyType,
        HISTORIC_TAG, UpdateListenerPayload, has_update_tag,
    };
    use crate::extension::get_peer_dependency_from_editor;
    use crate::extension::signals::{ReadonlySignal, Signal, effect, signal};
    use crate::extension_core::{
        ExtensionBuildState, ExtensionConfigBase, ExtensionInitState, ExtensionRegisterState,
        InitialEditorConfig, LexicalExtension, LexicalExtensionDependency, define_extension,
    };
    use crate::lexical_selection::{Point, get_selection};
    use crate::lexical_utils::get_node_by_key;
    use crate::merge_register::merge_register;
    use crate::ordered::{OrderedMap, OrderedSet};
    use crate::updates::{get_active_editor, read_editor_state};
    use crate::{Editor, EditorState, Unregister};
    use std::cell::{Cell, OnceCell, RefCell};
    use std::collections::HashMap;
    use std::rc::Weak;

    /// `MarkableNodeMatch`: the nodes a flavor marks. JS takes a class
    /// (`instanceof`) or a guard; a class is its `$is…` guard here, as the
    /// port's nodes have no class hierarchy to test.
    #[derive(Clone)]
    pub struct MarkableNodeMatch(pub Rc<dyn Fn(&NodeRef) -> bool>);

    impl MarkableNodeMatch {
        pub fn new(guard: impl Fn(&NodeRef) -> bool + 'static) -> Self {
            MarkableNodeMatch(Rc::new(guard))
        }
    }

    /// `DEFAULT_MARKABLE_NODES`: `TextNode` (and its subclasses),
    /// `LineBreakNode` and inline decorators ([`is_inline_decorator_node`]).
    pub fn default_markable_nodes() -> Vec<MarkableNodeMatch> {
        vec![
            MarkableNodeMatch::new(NodeRef::is_text),
            MarkableNodeMatch::new(|node| node.get_latest().is::<LineBreakNode>()),
            MarkableNodeMatch::new(is_inline_decorator_node),
        ]
    }

    /// `compileMarkableNodes(matches)`.
    fn compile_markable_nodes(matches: &[MarkableNodeMatch]) -> IsMarkable {
        let guards: Vec<_> = matches.iter().map(|m| m.0.clone()).collect();
        Rc::new(move |node| guards.iter().any(|guard| guard(node)))
    }

    type IsMarkable = Rc<dyn Fn(&NodeRef) -> bool>;

    /// `MarkStateConfig`: the config of a mark flavor's extension. The
    /// renderer's `isMarkIDShown` is not ported (rust/PARITY.md).
    #[derive(Clone)]
    pub struct MarkStateConfig {
        /// `disabled`: when `true`, the `markIDNodeKeys` output is emptied
        /// and no longer maintained, nothing is reported to the ids
        /// listeners, and inserted text no longer has its ids adjusted. The
        /// ids stay in the document and the functions keep working.
        pub disabled: bool,
        /// `exportIDsToHTML`: write the ids attribute on the elements of
        /// marked leaves in the HTML export.
        pub export_ids_to_html: bool,
        /// `markableNodes`: the nodes that carry ids. Default
        /// [`default_markable_nodes`].
        pub markable_nodes: Vec<MarkableNodeMatch>,
        /// The flavor (`__markFlavor`, on the extension in JS).
        pub(crate) flavor: Rc<MarkFlavor>,
    }

    /// `Partial<MarkStateConfig>`.
    #[derive(Clone, Default)]
    pub struct MarkStateConfigOverrides {
        pub disabled: Option<bool>,
        pub export_ids_to_html: Option<bool>,
        pub markable_nodes: Option<Vec<MarkableNodeMatch>>,
    }

    impl ExtensionConfigBase for MarkStateConfig {
        type Partial = MarkStateConfigOverrides;

        fn shallow_merge(&self, overrides: &MarkStateConfigOverrides) -> Self {
            MarkStateConfig {
                disabled: overrides.disabled.unwrap_or(self.disabled),
                export_ids_to_html: overrides
                    .export_ids_to_html
                    .unwrap_or(self.export_ids_to_html),
                markable_nodes: overrides
                    .markable_nodes
                    .clone()
                    .unwrap_or_else(|| self.markable_nodes.clone()),
                flavor: self.flavor.clone(),
            }
        }
    }

    /// The `markIDNodeKeys` output: every mark id in the document, with
    /// the keys of the leaves that carry it, in JS's `Map` and `Set` order.
    /// A new value is published after each update that changes it; values
    /// compare by identity, as JS compares the maps.
    #[derive(Clone, Default)]
    pub struct MarkIDNodeKeys(Rc<Vec<(String, Vec<NodeKey>)>>);

    impl PartialEq for MarkIDNodeKeys {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }

    impl MarkIDNodeKeys {
        /// `get(id)`.
        pub fn get(&self, id: &str) -> Option<&[NodeKey]> {
            self.0
                .iter()
                .find(|(i, _)| i == id)
                .map(|(_, keys)| keys.as_slice())
        }

        /// `keys()`.
        pub fn ids(&self) -> impl Iterator<Item = &str> {
            self.0.iter().map(|(id, _)| id.as_str())
        }

        /// `entries()`.
        pub fn iter(&self) -> impl Iterator<Item = (&str, &[NodeKey])> {
            self.0
                .iter()
                .map(|(id, keys)| (id.as_str(), keys.as_slice()))
        }

        pub fn len(&self) -> usize {
            self.0.len()
        }

        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
    }

    /// `MarkStateOutput`: `namedSignals` of the config, but for
    /// `markableNodes`, and the `markIDNodeKeys` output (empty while
    /// disabled).
    #[derive(Clone)]
    pub struct MarkStateOutput {
        pub disabled: Signal<bool>,
        pub export_ids_to_html: Signal<bool>,
        pub mark_id_node_keys: ReadonlySignal<MarkIDNodeKeys>,
    }

    /// `MarkIDsChange`: what [`register_mark_ids_listener`] reports. The
    /// sets are in JS's iteration order (of the `markIDNodeKeys` output).
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct MarkIDsChange {
        /// The ids that no leaf carried before and at least one carries now.
        pub added: Vec<String>,
        /// Every id of the flavor in the document now.
        pub ids: Vec<String>,
        /// The ids that some leaf carried before and none carries now.
        pub removed: Vec<String>,
        /// The tags of the update that made the change; none for the first
        /// call.
        pub tags: Vec<String>,
    }

    /// `MarkIDsListener`.
    pub type MarkIDsListener = Rc<dyn Fn(&MarkIDsChange)>;

    /// A listener with the ids last reported to it.
    struct ListenerEntry {
        listener: MarkIDsListener,
        reported: RefCell<Vec<String>>,
    }

    /// `MarkStateInit`: what outlives a `disabled` toggle: the
    /// `markIDNodeKeys` signal and the listeners; and, for this editor, what
    /// JS keeps in the flavor's `WeakMap`s (`isMarkableByEditor`,
    /// `editsByEditor`).
    pub struct MarkStateInit {
        mark_id_node_keys: Signal<MarkIDNodeKeys>,
        listeners: RefCell<Vec<Rc<ListenerEntry>>>,
        is_markable: RefCell<Option<IsMarkable>>,
        edit: RefCell<Option<Rc<PendingEdits>>>,
    }

    impl MarkStateInit {
        fn new() -> Self {
            MarkStateInit {
                mark_id_node_keys: signal(MarkIDNodeKeys::default()),
                listeners: RefCell::default(),
                is_markable: RefCell::default(),
                edit: RefCell::default(),
            }
        }
    }

    /// `MarkFlavorExtension` (`AnyMarkFlavorExtension`): a mark flavor's
    /// extension. Its flavor is [`mark_flavor`].
    pub type MarkFlavorExtension =
        Rc<LexicalExtension<MarkStateConfig, MarkStateOutput, MarkStateInit>>;

    /// `extension.__markFlavor`.
    pub fn mark_flavor(extension: &MarkFlavorExtension) -> &Rc<MarkFlavor> {
        &extension.config.flavor
    }

    type FlavorDependency =
        LexicalExtensionDependency<MarkStateConfig, MarkStateOutput, MarkStateInit>;

    /// `getPeerDependencyFromEditor(editor, flavor.name)`.
    fn flavor_dependency(flavor: &MarkFlavor, editor: &Editor) -> Option<FlavorDependency> {
        get_peer_dependency_from_editor(editor, &flavor.name)
            .ok()
            .flatten()
    }

    /// `flavor.isMarkableByEditor.get($getEditor())`.
    pub(super) fn editor_is_markable(flavor: &MarkFlavor) -> Option<IsMarkable> {
        let editor = get_active_editor()?;
        let dep = flavor_dependency(flavor, &editor)?;
        dep.init?.is_markable.borrow().clone()
    }

    /// `flavor.editsByEditor.get(editor)`.
    fn editor_edit(flavor: &MarkFlavor, editor: &Editor) -> Option<Rc<PendingEdits>> {
        flavor_dependency(flavor, editor)?
            .init?
            .edit
            .borrow()
            .clone()
    }

    /// `$removeMarkID`'s `edit.removed.add(id); forgetSoon(edit)`.
    pub(super) fn record_removed(flavor: &MarkFlavor, id: &str) {
        if let Some(editor) = get_active_editor()
            && let Some(edit) = editor_edit(flavor, &editor)
        {
            edit.record(&editor, |edit| {
                edit.add_removed(id);
            });
        }
    }

    /// `$getMarkIDCandidates(flavor, id)`: the leaves that may carry `id`
    /// without a walk of the document: those the `markIDNodeKeys` output had
    /// for it at the last commit, and those the pending update changed.
    /// `None` without the flavor's extension, or while it is disabled.
    pub(super) fn get_mark_id_candidates(flavor: &MarkFlavor, id: &str) -> Option<Vec<NodeRef>> {
        let editor = get_active_editor()?;
        let output = flavor_dependency(flavor, &editor)?.output?;
        if output.disabled.peek() {
            return None;
        }
        let mut keys = OrderedSet::new();
        if let Some(known) = output.mark_id_node_keys.peek().get(id) {
            for key in known {
                keys.add(key);
            }
        }
        for key in editor.0.dirty.borrow().leaves.iter() {
            keys.add(key);
        }
        Some(
            keys.iter()
                .filter_map(get_node_by_key)
                .filter(|node| is_markable_node(flavor, node) && node.is_attached())
                .collect(),
        )
    }

    /// `$getNodeMarkIDs(flavor, node)`: [`get_node_mark_ids`] for the ids of
    /// `flavor`.
    pub fn get_node_mark_ids_for(
        flavor: &MarkFlavorExtension,
        node: &NodeRef,
    ) -> Result<Vec<String>, LexicalError> {
        read_node_mark_ids(mark_flavor(flavor), node)
    }

    /// `$addMarkID(flavor, id, selection)`: [`add_mark_id`] for the ids of
    /// `flavor`.
    pub fn add_mark_id_for(
        flavor: &MarkFlavorExtension,
        id: &str,
        selection: &SelectionRef,
    ) -> Result<Vec<NodeRef>, LexicalError> {
        add_mark_id_impl(mark_flavor(flavor), id, selection)
    }

    /// `$removeMarkID(flavor, id, selection)`: [`remove_mark_id`] for the
    /// ids of `flavor`.
    pub fn remove_mark_id_for(
        flavor: &MarkFlavorExtension,
        id: &str,
        selection: Option<&SelectionRef>,
    ) -> Result<(), LexicalError> {
        remove_mark_id_impl(mark_flavor(flavor), id, selection)
    }

    /// `$getMarkIDsAtSelection(flavor, selection, mode)`:
    /// [`get_mark_ids_at_selection`] for the ids of `flavor`.
    pub fn get_mark_ids_at_selection_for(
        flavor: &MarkFlavorExtension,
        selection: Option<&SelectionRef>,
        mode: MarkIDsMode,
    ) -> Result<Vec<String>, LexicalError> {
        get_mark_ids_at_selection_impl(mark_flavor(flavor), selection, mode)
    }

    /// `$migrateMarkNodesToMarkState(flavor, node)`:
    /// [`migrate_mark_nodes_to_mark_state`] to the ids of `flavor`.
    pub fn migrate_mark_nodes_to_mark_state_for(
        flavor: &MarkFlavorExtension,
        node: Option<&NodeRef>,
    ) -> Result<usize, LexicalError> {
        migrate_mark_nodes_to_mark_state_impl(mark_flavor(flavor), node)
    }

    /// `$getMarkIDsByFlavor(node)`: the ids of every mark flavor of the
    /// active editor on `node`, by `stateKey` in the order the flavors were
    /// built; a flavor whose `markableNodes` leave the node out has none.
    pub fn get_mark_ids_by_flavor(
        node: &NodeRef,
    ) -> Result<Vec<(String, Vec<String>)>, LexicalError> {
        let mut out = Vec::new();
        let Some(editor) = get_active_editor() else {
            return Ok(out);
        };
        if let Some(base) = get_peer_dependency_from_editor::<(), MarkBaseOutput, MarkBaseInit>(
            &editor,
            MARK_BASE_EXTENSION_NAME,
        )?
        .and_then(|dep| dep.output)
        {
            for (state_key, extension) in base.flavors() {
                out.push((
                    state_key,
                    get_markable_ids(mark_flavor(&extension), Some(node))?,
                ));
            }
        }
        Ok(out)
    }

    /// `reportMarkIDs(init, tags)`: reports to each listener the ids that
    /// appeared or disappeared since the last report to it.
    fn report_mark_ids(init: &MarkStateInit, tags: &[String]) {
        let entries = init.listeners.borrow().clone();
        if entries.is_empty() {
            return;
        }
        let ids: Vec<String> = init
            .mark_id_node_keys
            .peek()
            .ids()
            .map(str::to_owned)
            .collect();
        for entry in entries {
            let prev = entry.reported.borrow().clone();
            let added: Vec<String> = ids
                .iter()
                .filter(|id| !prev.contains(id))
                .cloned()
                .collect();
            let removed: Vec<String> = prev
                .iter()
                .filter(|id| !ids.contains(id))
                .cloned()
                .collect();
            // Unregistered by an earlier listener of this report
            let registered = init
                .listeners
                .borrow()
                .iter()
                .any(|e| Rc::ptr_eq(e, &entry));
            if (!added.is_empty() || !removed.is_empty()) && registered {
                *entry.reported.borrow_mut() = ids.clone();
                (entry.listener)(&MarkIDsChange {
                    added,
                    ids: ids.clone(),
                    removed,
                    tags: tags.to_vec(),
                });
            }
        }
    }

    /// `registerMarkIDsListener(editor, flavor, listener)`: calls `listener`
    /// after each commit that changes which ids of `flavor` are in the
    /// document, and right away (with all of them in `added`) when there
    /// are some already. Nothing is reported while the flavor is disabled;
    /// once it is enabled again the listener gets the net change. Fails when
    /// the editor was not built with the flavor.
    pub fn register_mark_ids_listener(
        editor: &Editor,
        flavor: &MarkFlavorExtension,
        listener: impl Fn(&MarkIDsChange) + 'static,
    ) -> Result<Unregister, LexicalError> {
        let dep = flavor_dependency(mark_flavor(flavor), editor);
        let (Some(init), Some(output)) = (
            dep.as_ref().and_then(|d| d.init.clone()),
            dep.and_then(|d| d.output),
        ) else {
            return Err(LexicalError::new(format!(
                "registerMarkIDsListener: the editor was not built with {}. Add it to the dependencies of the extension the editor is built from.",
                flavor.name
            )));
        };
        // While disabled the ids are not tracked: the first report comes when
        // the flavor is enabled again
        let ids: Vec<String> = if output.disabled.peek() {
            Vec::new()
        } else {
            output
                .mark_id_node_keys
                .peek()
                .ids()
                .map(str::to_owned)
                .collect()
        };
        if !ids.is_empty() {
            listener(&MarkIDsChange {
                added: ids.clone(),
                ids: ids.clone(),
                removed: Vec::new(),
                tags: Vec::new(),
            });
        }
        let entry = Rc::new(ListenerEntry {
            listener: Rc::new(listener),
            reported: RefCell::new(ids),
        });
        init.listeners.borrow_mut().push(entry.clone());
        let init = Rc::downgrade(&init);
        Ok(Box::new(move || {
            if let Some(init) = init.upgrade() {
                init.listeners
                    .borrow_mut()
                    .retain(|e| !Rc::ptr_eq(e, &entry));
            }
        }))
    }

    /// `registerMarkIDsState(flavor, editor)`: makes the flavor's state
    /// known to every node type of the editor, as using it on a node of that
    /// type would, so that ids read from JSON are parsed right away.
    fn register_mark_ids_state(flavor: &MarkFlavor, editor: &Editor) {
        for node_type in editor.registered_nodes().keys() {
            if let Some(shared) = editor.shared_node_state(node_type) {
                shared
                    .shared_config_map
                    .borrow_mut()
                    .entry(flavor.state_key.clone())
                    .or_insert_with(|| flavor.state.erased());
            }
        }
    }

    /// What `registerMarkState` keeps between its listener, transform and
    /// command handlers.
    struct MarkStateRegistration {
        flavor: Rc<MarkFlavor>,
        init: Rc<MarkStateInit>,
        splits: Rc<SplitOrigins>,
        edit: Rc<PendingEdits>,
        ids_by_key: RefCell<HashMap<NodeKey, Vec<String>>>,
        keys_by_id: RefCell<OrderedMap<OrderedSet>>,
        changed: Cell<bool>,
        replaced_state: Cell<bool>,
        composition_base: RefCell<Option<EditorState>>,
        // Set when an update tagged COLLABORATION_TAG (another editor's
        // update) changed the block of the composition while it was in
        // progress
        composition_tainted: Cell<bool>,
        // The selection the composition started with, once its first update
        // is seen to have replaced it (see getCompositionReplacedRange)
        composition_replaced: RefCell<Option<(Point, Point)>>,
        // Set when this registration is disposed: an update listener or
        // transform the editor had already collected must then do nothing
        disposed: Cell<bool>,
    }

    impl MarkStateRegistration {
        /// `setKey(key, ids)`.
        fn set_key(&self, key: &str, ids: Vec<String>) {
            let mut ids_by_key = self.ids_by_key.borrow_mut();
            let prev = ids_by_key.get(key).cloned().unwrap_or_default();
            if self.flavor.state.is_equal(&prev, &ids) {
                return;
            }
            self.changed.set(true);
            let mut keys_by_id = self.keys_by_id.borrow_mut();
            for id in &prev {
                if !ids.contains(id)
                    && let Some(keys) = keys_by_id.get_mut(id)
                {
                    keys.delete(key);
                    if keys.is_empty() {
                        keys_by_id.delete(id);
                    }
                }
            }
            for id in &ids {
                if keys_by_id.get(id).is_none() {
                    keys_by_id.set(id, OrderedSet::new());
                }
                keys_by_id.get_mut(id).expect("just set").add(key);
            }
            if ids.is_empty() {
                ids_by_key.remove(key);
            } else {
                ids_by_key.insert(key.to_owned(), ids);
            }
        }

        /// `$visit(key)`.
        fn visit(&self, key: &str) -> Result<(), LexicalError> {
            let node = get_node_by_key(key);
            self.set_key(key, get_markable_ids(&self.flavor, node.as_ref())?);
            Ok(())
        }

        /// `$rescan()`.
        fn rescan(&self) -> Result<(), LexicalError> {
            let mut seen = HashSet::new();
            for leaf in markable_nodes_in(&self.flavor, &get_root()) {
                seen.insert(leaf.get_key().to_owned());
                self.set_key(leaf.get_key(), read_node_mark_ids(&self.flavor, &leaf)?);
            }
            let keys: Vec<NodeKey> = self.ids_by_key.borrow().keys().cloned().collect();
            for key in keys {
                if !seen.contains(&key) {
                    self.set_key(&key, Vec::new());
                }
            }
            Ok(())
        }

        /// `rescanChanged(prevState, nextState)`: brings the output from
        /// `prev_state` to `next_state` when the dirty sets do not describe
        /// the change. A node that did not change is the same value in both
        /// states, so only the others are read.
        fn rescan_changed(
            &self,
            editor: &Editor,
            prev_state: &EditorState,
            next_state: &EditorState,
        ) -> Result<(), LexicalError> {
            let changed: Vec<NodeKey> = {
                let prev_nodes = prev_state.0.node_map.borrow();
                next_state
                    .0
                    .node_map
                    .borrow()
                    .iter()
                    .filter(|(key, node)| {
                        prev_nodes
                            .get(key)
                            .is_none_or(|prev| !Rc::ptr_eq(prev, node))
                    })
                    .map(|(key, _)| key.to_owned())
                    .collect()
            };
            read_editor_state(Some(editor), next_state, || {
                for key in &changed {
                    let node = NodeRef::new(key.clone());
                    let ids = if is_markable_node(&self.flavor, &node) && node.is_attached() {
                        read_node_mark_ids(&self.flavor, &node)?
                    } else {
                        Vec::new()
                    };
                    self.set_key(key, ids);
                }
                Ok::<_, LexicalError>(())
            })?;
            let gone: Vec<NodeKey> = {
                let next_nodes = next_state.0.node_map.borrow();
                self.ids_by_key
                    .borrow()
                    .keys()
                    .filter(|key| !next_nodes.contains_key(key.as_str()))
                    .cloned()
                    .collect()
            };
            for key in gone {
                self.set_key(&key, Vec::new());
            }
            Ok(())
        }

        /// `publish(tags)`.
        fn publish(&self, tags: &[String]) {
            if self.changed.replace(false) {
                let next: Vec<(String, Vec<NodeKey>)> = self
                    .keys_by_id
                    .borrow()
                    .iter()
                    .map(|(id, keys)| (id.to_owned(), keys.iter().map(str::to_owned).collect()))
                    .collect();
                self.init
                    .mark_id_node_keys
                    .set_value(MarkIDNodeKeys(Rc::new(next)));
                report_mark_ids(&self.init, tags);
            }
        }

        /// The update listener.
        fn on_update(
            &self,
            editor: &Editor,
            payload: &UpdateListenerPayload,
        ) -> Result<(), LexicalError> {
            if self.disposed.get() {
                return Ok(());
            }
            let UpdateListenerPayload {
                dirty_elements,
                dirty_leaves,
                tags,
                editor_state,
                prev_editor_state,
                ..
            } = payload;
            self.edit.forget();
            self.splits.clear();
            let has_tag = |tag: &str| tags.iter().any(|t| t == tag);
            let base = self.composition_base.borrow().clone();
            if let Some(base) = base {
                if !editor.is_composing() {
                    *self.composition_base.borrow_mut() = None;
                    self.composition_tainted.set(false);
                    *self.composition_replaced.borrow_mut() = None;
                } else if has_tag(COMPOSITION_START_TAG) {
                    let replaced = get_composition_replaced_range(editor, &base, editor_state)?;
                    *self.composition_replaced.borrow_mut() = replaced;
                } else if !self.composition_tainted.get() && has_tag(COLLABORATION_TAG) {
                    let tainted = read_editor_state(Some(editor), editor_state, || {
                        // @internal access: the node being composed
                        let composing = editor
                            .composition_key()
                            .and_then(|key| get_node_by_key(&key))
                            .and_then(|node| get_block(&node));
                        composing.is_none_or(|block| {
                            dirty_elements.iter().any(|(k, _)| k == block.get_key())
                        })
                    });
                    self.composition_tainted.set(tainted);
                }
            }
            if self.replaced_state.get()
                || has_tag(HISTORIC_TAG)
                || (dirty_leaves.is_empty()
                    && dirty_elements.len() == 1
                    && dirty_elements[0].0 == "root")
            {
                self.replaced_state.set(false);
                self.rescan_changed(editor, prev_editor_state, editor_state)?;
                self.publish(tags);
                return Ok(());
            }
            if dirty_leaves.is_empty() && dirty_elements.is_empty() {
                return Ok(());
            }
            // The dirty nodes that this update removed: the markable nodes
            // under them in the previous state are gone too, unless they were
            // moved (which made them dirty leaves of their own)
            let mut removed: Vec<NodeKey> = Vec::new();
            read_editor_state(Some(editor), editor_state, || {
                for key in dirty_leaves {
                    self.visit(key)?;
                    if get_node_by_key(key).is_none() {
                        removed.push(key.clone());
                    }
                }
                for (key, _) in dirty_elements {
                    match get_node_by_key(key) {
                        None => removed.push(key.clone()),
                        Some(node) if is_markable_node(&self.flavor, &node) => {
                            self.set_key(key, read_node_mark_ids(&self.flavor, &node)?)
                        }
                        Some(_) => {}
                    }
                }
                Ok::<_, LexicalError>(())
            })?;
            if !removed.is_empty() {
                let gone: Vec<NodeKey> = read_editor_state(Some(editor), prev_editor_state, || {
                    removed
                        .iter()
                        .filter_map(|key| get_node_by_key(key))
                        .flat_map(|node| markable_nodes_in(&self.flavor, &node))
                        .map(|leaf| leaf.get_key().to_owned())
                        .collect()
                });
                read_editor_state(Some(editor), editor_state, || {
                    for key in &gone {
                        if get_node_by_key(key).is_none() {
                            self.set_key(key, Vec::new());
                        }
                    }
                });
            }
            self.publish(tags);
            Ok(())
        }

        /// The root transform: the inserted-text rule.
        fn on_root_transform(&self, editor: &Editor) -> Result<(), LexicalError> {
            if self.disposed.get() {
                return Ok(());
            }
            // @internal access: nothing else tells a transform that this
            // update is a setEditorState, whose dirty sets the update
            // listener cannot use and which is not an edit
            if editor.dirty_type() == DirtyType::FullReconcile {
                self.replaced_state.set(true);
                return Ok(());
            }
            if editor.is_composing()
                || has_update_tag(COLLABORATION_TAG)
                || has_update_tag(HISTORIC_TAG)
            {
                return Ok(());
            }
            self.edit.refresh(editor);
            let removed: Vec<String> = self.edit.removed.borrow().iter().cloned().collect();
            let base = self.composition_base.borrow().clone();
            if let Some(base) = base {
                // The end of a composition: the composed text is compared
                // with the document from before it, and replaces the
                // selection it had
                if !self.composition_tainted.get() {
                    let composition_replaced = self.composition_replaced.borrow().clone();
                    let replaced_for_sure = composition_replaced.is_some();
                    let replaced = match composition_replaced {
                        Some(replaced) => Some(replaced),
                        None => read_editor_state(Some(editor), &base, || {
                            get_replaced_range(get_selection().as_ref())
                        })?,
                    };
                    apply_inserted_text_mark_ids(
                        &self.flavor,
                        editor,
                        &base,
                        replaced.as_ref(),
                        &removed,
                        &self.splits,
                        replaced_for_sure,
                    )?;
                }
                return Ok(());
            }
            let replaced = self.edit.replaced.borrow().clone();
            apply_inserted_text_mark_ids(
                &self.flavor,
                editor,
                &editor.get_editor_state(),
                replaced.as_ref(),
                &removed,
                &self.splits,
                false,
            )
        }
    }

    /// `registerMarkState(flavor, editor, init, splits)`: maintains the
    /// `markIDNodeKeys` output from the dirty leaves of each commit (a
    /// commit whose dirty sets do not describe the change, a
    /// `setEditorState` or a historic one, is compared with the previous
    /// state node by node), reports the ids that appeared or disappeared to
    /// the ids listeners, and keeps text inserted next to a marked run
    /// consistent with the documented rule
    /// (`$applyInsertedTextMarkIDs`).
    fn register_mark_state(
        flavor: &Rc<MarkFlavor>,
        editor: &Editor,
        init: &Rc<MarkStateInit>,
        splits: &Rc<SplitOrigins>,
    ) -> Unregister {
        let edit = Rc::new(PendingEdits::default());
        *init.edit.borrow_mut() = Some(edit.clone());
        let reg = Rc::new(MarkStateRegistration {
            flavor: flavor.clone(),
            init: init.clone(),
            splits: splits.clone(),
            edit: edit.clone(),
            ids_by_key: RefCell::default(),
            keys_by_id: RefCell::default(),
            changed: Cell::new(false),
            replaced_state: Cell::new(false),
            composition_base: RefCell::new(None),
            composition_tainted: Cell::new(false),
            composition_replaced: RefCell::new(None),
            disposed: Cell::new(false),
        });
        // An error can't leave a listener or a transform here, so the walk
        // stops where it failed, as an exception would stop JS's.
        let _ = read_editor_state(Some(editor), &editor.get_editor_state(), || reg.rescan());
        reg.publish(&[]);
        // Also when nothing changed: the ids may have gone while it was
        // disabled
        report_mark_ids(init, &[]);
        let weak = editor.weak();
        let upgrade = move |weak: &Weak<_>| weak.upgrade().map(Editor);
        let record_replaced = {
            let (edit, weak) = (edit.clone(), weak.clone());
            move || -> Result<bool, LexicalError> {
                if let Some(editor) = upgrade(&weak) {
                    let replaced = get_replaced_range(get_selection().as_ref())?;
                    edit.record(&editor, |edit| *edit.replaced.borrow_mut() = replaced);
                }
                Ok(false)
            }
        };
        let record_paste = record_replaced.clone();
        merge_register(vec![
            {
                let (reg, weak) = (reg.clone(), weak.clone());
                editor.register_update_listener(move |payload| {
                    if let Some(editor) = upgrade(&weak) {
                        let _ = reg.on_update(&editor, payload);
                    }
                })
            },
            {
                let (reg, weak) = (reg.clone(), weak.clone());
                editor.register_node_transform("root", move |_| match upgrade(&weak) {
                    Some(editor) => reg.on_root_transform(&editor),
                    None => Ok(()),
                })
            },
            {
                let (reg, weak) = (reg.clone(), weak.clone());
                editor.register_command(
                    &COMPOSITION_START_COMMAND,
                    move |_, _| {
                        // Transforms skip the node being composed, so the
                        // composed text is compared with the document from
                        // before the composition once it ends
                        if let Some(editor) = upgrade(&weak) {
                            *reg.composition_base.borrow_mut() = Some(editor.get_editor_state());
                        }
                        reg.composition_tainted.set(false);
                        *reg.composition_replaced.borrow_mut() = None;
                        Ok(false)
                    },
                    COMMAND_PRIORITY_CRITICAL,
                )
            },
            // The selection that an insertion replaces, before the handler of
            // the command replaces it
            editor.register_command(
                &CONTROLLED_TEXT_INSERTION_COMMAND,
                move |_, _| record_replaced(),
                COMMAND_PRIORITY_CRITICAL,
            ),
            editor.register_command(
                &PASTE_COMMAND,
                move |_, _| record_paste(),
                COMMAND_PRIORITY_CRITICAL,
            ),
            {
                let init = init.clone();
                Box::new(move || {
                    reg.disposed.set(true);
                    *reg.composition_base.borrow_mut() = None;
                    *reg.composition_replaced.borrow_mut() = None;
                    edit.forget();
                    let mut current = init.edit.borrow_mut();
                    if current.as_ref().is_some_and(|e| Rc::ptr_eq(e, &edit)) {
                        *current = None;
                    }
                    drop(current);
                    // Not reported to the listeners: the ids are still in
                    // the document. When the flavor is enabled again, the
                    // listeners get the net change.
                    init.mark_id_node_keys.set_value(MarkIDNodeKeys::default());
                })
            },
        ])
    }

    /// `MARK_BASE_EXTENSION_NAME`.
    const MARK_BASE_EXTENSION_NAME: &str = "@lexical/mark/MarkBase";

    /// `MarkBaseInit`: the editor's flavors (by `stateKey`) and the split
    /// origins their inserted-text rules share.
    #[derive(Default)]
    pub struct MarkBaseInit {
        flavors: Rc<RefCell<Vec<(String, MarkFlavorExtension)>>>,
        splits: Rc<SplitOrigins>,
    }

    /// `MarkBaseOutput`.
    #[derive(Clone)]
    pub struct MarkBaseOutput {
        flavors: Rc<RefCell<Vec<(String, MarkFlavorExtension)>>>,
    }

    impl MarkBaseOutput {
        /// `flavors`: the mark flavors of the editor, by their `stateKey`,
        /// in the order they were built.
        pub fn flavors(&self) -> Vec<(String, MarkFlavorExtension)> {
            self.flavors.borrow().clone()
        }
    }

    /// `MarkBaseExtension`.
    pub type MarkBaseExtensionType = LexicalExtension<(), MarkBaseOutput, MarkBaseInit>;

    fn make_mark_base() -> Rc<MarkBaseExtensionType> {
        define_extension(LexicalExtension {
            build: Some(Rc::new(
                |_: &Editor, _: &(), state: &ExtensionBuildState<MarkBaseInit>| {
                    Ok(MarkBaseOutput {
                        flavors: state.get_init_result()?.flavors.clone(),
                    })
                },
            )),
            init: Some(Rc::new(
                |_: &mut InitialEditorConfig, _: &(), _: &ExtensionInitState| {
                    Ok(MarkBaseInit::default())
                },
            )),
            register: Some(Rc::new(
                |editor: &Editor,
                 _: &(),
                 state: &ExtensionRegisterState<MarkBaseInit, MarkBaseOutput>| {
                    let splits = state.get_init_result()?.splits.clone();
                    let on_dispose = splits.clone();
                    Ok(merge_register(vec![
                        editor.register_update_listener(move |_| splits.clear()),
                        Box::new(move || on_dispose.clear()),
                    ]))
                },
            )),
            ..LexicalExtension::new(MARK_BASE_EXTENSION_NAME, ())
        })
    }

    /// `createMarkFlavorExtension(name, stateKey, attribute, ...,
    /// configDefaults)`.
    fn create_mark_flavor_extension(
        flavor: Rc<MarkFlavor>,
        config_defaults: &MarkStateConfigOverrides,
    ) -> MarkFlavorExtension {
        let name = flavor.name.clone();
        // The extension, for the registry its own build adds it to.
        let this: Rc<OnceCell<Weak<_>>> = Rc::default();
        let config = MarkStateConfig {
            disabled: false,
            export_ids_to_html: false,
            markable_nodes: default_markable_nodes(),
            flavor: flavor.clone(),
        }
        .shallow_merge(config_defaults);
        let this_in_build = this.clone();
        #[cfg_attr(not(feature = "html"), allow(unused_mut))]
        let mut dependencies = vec![mark_base_extension().into()];
        #[cfg(feature = "html")]
        dependencies.splice(0..0, html::dependencies(&flavor));
        let extension = define_extension(LexicalExtension {
            dependencies,
            init: Some(Rc::new(
                |_: &mut InitialEditorConfig, _: &MarkStateConfig, _: &ExtensionInitState| {
                    Ok(MarkStateInit::new())
                },
            )),
            build: Some(Rc::new(
                move |editor: &Editor,
                      config: &MarkStateConfig,
                      state: &ExtensionBuildState<MarkStateInit>| {
                    let base = state
                        .get_dependency(&mark_base_extension())?
                        .init
                        .expect("MarkBaseExtension has an init");
                    let flavor = &config.flavor;
                    for (_, other) in base.flavors.borrow().iter() {
                        let other = mark_flavor(other);
                        invariant!(
                            other.state_key != flavor.state_key,
                            "MarkBaseExtension: {} and {} both use stateKey {}. Give each flavor its own stateKey.",
                            other.name,
                            flavor.name,
                            Value::from(flavor.state_key.as_str())
                        );
                        invariant!(
                            other.attribute != flavor.attribute,
                            "MarkBaseExtension: {} and {} both render the attribute {}. Give each flavor its own attribute.",
                            other.name,
                            flavor.name,
                            flavor.attribute
                        );
                    }
                    if let Some(extension) = this_in_build.get().and_then(Weak::upgrade) {
                        base.flavors
                            .borrow_mut()
                            .push((flavor.state_key.clone(), extension));
                    }
                    let init = state.get_init_result()?;
                    *init.is_markable.borrow_mut() =
                        Some(compile_markable_nodes(&config.markable_nodes));
                    register_mark_ids_state(flavor, editor);
                    Ok(MarkStateOutput {
                        disabled: signal(config.disabled),
                        export_ids_to_html: signal(config.export_ids_to_html),
                        mark_id_node_keys: init.mark_id_node_keys.as_readonly(),
                    })
                },
            )),
            register: Some(Rc::new(
                |editor: &Editor,
                 config: &MarkStateConfig,
                 state: &ExtensionRegisterState<MarkStateInit, MarkStateOutput>| {
                    let disabled = state
                        .get_output()
                        .expect("a mark flavor has a build")
                        .disabled
                        .clone();
                    let init = state.get_init_result()?;
                    let splits = state
                        .get_dependency(&mark_base_extension())?
                        .init
                        .expect("MarkBaseExtension has an init")
                        .splits
                        .clone();
                    let flavor = config.flavor.clone();
                    let weak = editor.weak();
                    Ok(effect(move || {
                        if disabled.value() {
                            return None;
                        }
                        let editor = weak.upgrade().map(Editor)?;
                        Some(register_mark_state(&flavor, &editor, &init, &splits))
                    }))
                },
            )),
            ..LexicalExtension::new(name, config)
        });
        let _ = this.set(Rc::downgrade(&extension));
        extension
    }

    /// `MarkFlavorDefinition`: what [`define_mark_extension`] makes a flavor
    /// of.
    #[derive(Clone, Default)]
    pub struct MarkFlavorDefinition {
        /// `name`: the extension's name.
        pub name: String,
        /// `stateKey`: the NodeState key of the ids; non-empty, without
        /// whitespace, and neither `"markIds"` (`MarkStateExtension`'s) nor
        /// `"__proto__"`.
        pub state_key: String,
        /// `attribute`: the HTML attribute of the ids; by default
        /// `data-lexical-` and the key in kebab case. Lowercased.
        pub attribute: Option<String>,
        /// `className` and `overlapClassName`: the renderer's classes of a
        /// marked leaf and of one with several ids. Kept for the API; the
        /// renderer is not ported (rust/PARITY.md).
        pub class_name: Option<String>,
        pub overlap_class_name: Option<String>,
        /// `config`: the flavor's config defaults.
        pub config: MarkStateConfigOverrides,
    }

    /// `defaultMarkAttribute(stateKey)`: `commentIds` is
    /// `data-lexical-comment-ids`.
    fn default_mark_attribute(state_key: &str) -> String {
        let mut attribute = String::from("data-lexical-");
        for c in state_key.chars() {
            if c.is_ascii_uppercase() {
                attribute.push('-');
                attribute.push(c.to_ascii_lowercase());
            } else {
                attribute.push(c);
            }
        }
        attribute
    }

    /// `defineMarkExtension(definition)`: a mark flavor, an extension for
    /// one kind of annotation (comments, suggestions...) that works as
    /// `MarkStateExtension` does on a NodeState key of its own: two flavors
    /// on the same text never see each other's ids. The `*_for` functions
    /// take it first. Fails on an invalid `stateKey`.
    pub fn define_mark_extension(
        definition: &MarkFlavorDefinition,
    ) -> Result<MarkFlavorExtension, LexicalError> {
        let MarkFlavorDefinition {
            name, state_key, ..
        } = definition;
        invariant!(
            is_valid_mark_id(state_key) && state_key != MARK_STATE_KEY && state_key != "__proto__",
            "defineMarkExtension: invalid stateKey {} for {}. Use a non-empty name without whitespace, other than \"markIds\" (the key of MarkStateExtension) and \"__proto__\", such as \"commentIds\".",
            Value::from(state_key.as_str()),
            name
        );
        // Lowercase, as HTML stores attribute names, so that a clash is
        // caught
        let attribute = definition
            .attribute
            .clone()
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| default_mark_attribute(state_key))
            .to_lowercase();
        Ok(create_mark_flavor_extension(
            Rc::new(MarkFlavor::new(name, state_key, &attribute)),
            &definition.config,
        ))
    }

    thread_local! {
        static MARK_BASE_EXTENSION: Rc<MarkBaseExtensionType> = make_mark_base();
        static MARK_STATE_EXTENSION: MarkFlavorExtension = create_mark_flavor_extension(
            default_flavor(),
            &MarkStateConfigOverrides::default(),
        );
    }

    /// `MarkBaseExtension`: the base every mark flavor depends on. It keeps
    /// the editor's flavors (its `flavors` output), with which a flavor's
    /// build rejects another that has its `stateKey` or attribute, and lets
    /// the inserted-text rules of the flavors tell each other's splits from
    /// inserted text.
    pub fn mark_base_extension() -> Rc<MarkBaseExtensionType> {
        MARK_BASE_EXTENSION.with(Rc::clone)
    }

    /// `MarkStateExtension`: the default mark flavor, whose ids are the
    /// `markIds` NodeState of the leaves they cover. Marks overlap freely,
    /// adding or removing an id changes NodeState only, and ids travel with
    /// the text. Text inserted at the edge of a marked run does not join
    /// it, as with `MarkNode`; text inserted inside a run does; text that
    /// replaces a selection also gets the ids all of the replaced text had,
    /// and pasted content keeps its own ids.
    pub fn mark_state_extension() -> MarkFlavorExtension {
        MARK_STATE_EXTENSION.with(Rc::clone)
    }

    #[cfg(feature = "html")]
    mod html {
        use super::*;
        use crate::extension_core::{AnyLexicalExtensionArgument, config_extension};
        use crate::html::import::{
            DomImportConfigOverrides, define_import_rule, dom_import_extension, sel, sel::AttrMatch,
        };
        use crate::html::{
            DomOverrideOptions, DomRenderConfigOverrides, DomRenderMatchConfig, NodeMatch,
            dom_override, dom_render_extension,
        };

        /// `createMarkIDsImportRule(flavor)`: the ids in the flavor's
        /// attribute are added to the markable leaves the element imports
        /// to.
        fn create_mark_ids_import_rule(
            flavor: &Rc<MarkFlavor>,
        ) -> crate::html::import::DomImportRule {
            let f = flavor.clone();
            define_import_rule(
                Some(&format!("{}/ids", flavor.name)),
                sel::any().attr(&flavor.attribute, AttrMatch::Present),
                move |_, el, next| {
                    let nodes = next()?;
                    let value = el.get_attribute(&f.attribute).unwrap_or_default();
                    let ids: Vec<String> = split_on_whitespace(&value)
                        .filter(|id| is_valid_mark_id(id))
                        .map(str::to_owned)
                        .collect();
                    if !ids.is_empty() {
                        let mut leaves = Vec::new();
                        let mut seen = HashSet::new();
                        for node in &nodes {
                            collect_markable_nodes(&f, node, &mut leaves, &mut seen);
                        }
                        for leaf in &leaves {
                            add_node_mark_ids(&f, leaf, &ids)?;
                        }
                    }
                    Ok(nodes)
                },
            )
        }

        /// The flavor's `DOMRenderExtension` override (`$exportDOM`, which
        /// writes the ids with `exportIDsToHTML`) and `DOMImportExtension`
        /// rule. The `$decorateDOM` override needs the reconciler's DOM.
        pub(super) fn dependencies(flavor: &Rc<MarkFlavor>) -> Vec<AnyLexicalExtensionArgument> {
            let (f, g) = (flavor.clone(), flavor.clone());
            let export = dom_override(
                Some(vec![NodeMatch::predicate(move |_, node| {
                    is_markable_node(&f, node)
                })]),
                DomRenderMatchConfig {
                    export_dom: Some(Rc::new(move |_, node, next, editor| {
                        let output = next()?;
                        let export = flavor_dependency(&g, editor)
                            .and_then(|dep| dep.output)
                            .is_some_and(|output| output.export_ids_to_html.peek());
                        if !export {
                            return Ok(output);
                        }
                        let ids = read_node_mark_ids(&g, node)?;
                        if !ids.is_empty()
                            && let Some(element) =
                                output.element.as_ref().filter(|e| e.is_element())
                        {
                            element.set_attribute(&g.attribute, &ids.join(" "));
                        }
                        Ok(output)
                    })),
                    ..DomRenderMatchConfig::default()
                },
                DomOverrideOptions::default(),
            );
            vec![
                config_extension(
                    &dom_render_extension(),
                    [DomRenderConfigOverrides {
                        overrides: Some(vec![export]),
                        ..DomRenderConfigOverrides::default()
                    }],
                ),
                config_extension(
                    &dom_import_extension(),
                    [DomImportConfigOverrides {
                        rules: Some(vec![create_mark_ids_import_rule(flavor).into()]),
                        ..DomImportConfigOverrides::default()
                    }],
                ),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_sort_by_utf16_code_units() {
        // U+FF01 comes before U+1F600 by code point, after it by code unit.
        let ids = Value::Array(vec![
            "\u{ff01}".into(),
            "\u{1f600}".into(),
            "b".into(),
            "b".into(),
            "not valid".into(),
        ]);
        assert_eq!(
            default_flavor().state.parse(&ids),
            ["b", "\u{1f600}", "\u{ff01}"]
        );
        assert!(!is_valid_mark_id("a\u{feff}b"));
        assert!(is_valid_mark_id("a\u{85}b"));
    }
}
