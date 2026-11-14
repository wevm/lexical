//! The functions of `MarkStateExtension.ts` that need no extension: mark ids
//! kept as the `markIds` NodeState of the leaves they cover (a sorted array
//! without duplicates, `arraySetValue(markIDValue())`), with the default
//! `markableNodes` (`TextNode`, `LineBreakNode` and inline decorators).
//! These are `MarkStateExtension`'s ids; JS's `$` functions read and write
//! them the same way when the extension is not installed, as here.
//!
//! Not ported: the extension itself and what needs it or a DOM (rust/PARITY.md):
//! the `markableNodes` config, mark flavors (`defineMarkExtension`,
//! `MarkBaseExtension`, the flavor-first forms, `$getMarkIDsByFlavor`), the
//! `markIDNodeKeys` output and `registerMarkIDsListener`, the inserted-text
//! rule, rendering and HTML import and export. Slots aren't ported, so the
//! slots of a node are not looked through.

use crate::error::invariant;
use crate::json::Value;
use crate::lexical_selection::{PointType, SelectionRef, Which};
use crate::lexical_utils::get_root;
use crate::node::NodeRef;
use crate::node_state::{
    StateConfig, StateValueConfig, create_state_with, get_state, set_state_with,
};
use crate::nodes::line_break::LineBreakNode;
use crate::{LexicalError, NodeKey};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::rc::Rc;

use super::is_mark_node;

/// `MARK_STATE_KEY`: the NodeState key that the mark ids of a leaf are
/// stored under, in the `$` record of its JSON.
pub const MARK_STATE_KEY: &str = "markIds";

/// JS's `\s` without the `u` flag: ECMAScript's WhiteSpace and
/// LineTerminator.
fn is_js_whitespace(c: char) -> bool {
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
fn is_valid_mark_id(id: &str) -> bool {
    !id.is_empty() && !id.chars().any(is_js_whitespace)
}

/// JS's default sort order of strings: by UTF-16 code units.
fn js_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `arraySetValue(markIDValue())`'s parse of strings: the valid ids,
/// sorted as JS sorts them, without duplicates.
fn canonical_ids(ids: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut members: Vec<String> = ids.into_iter().filter(|id| is_valid_mark_id(id)).collect();
    members.sort_by(|a, b| js_cmp(a, b));
    members.dedup();
    members
}

/// `arraySetValue(markIDValue())`'s parse: anything but an array is `[]`,
/// and a member that is not a valid id is dropped.
fn parse_ids(value: &Value) -> Vec<String> {
    value.as_array().map_or_else(Vec::new, |items| {
        canonical_ids(items.iter().filter_map(|v| v.as_str().map(str::to_owned)))
    })
}

thread_local! {
    /// `createMarkIDsState(MARK_STATE_KEY)`: `MarkStateExtension`'s state.
    static MARK_IDS_STATE: StateConfig<Vec<String>> = create_state_with(
        MARK_STATE_KEY,
        StateValueConfig {
            parse: Rc::new(parse_ids),
            unparse: Rc::new(|ids: &Vec<String>| {
                Value::Array(ids.iter().map(|id| id.as_str().into()).collect())
            }),
            // `isArraySetEqual`.
            is_equal: Rc::new(|a: &Vec<String>, b: &Vec<String>| a == b),
            reset_on_copy_node: false,
        },
    );
}

fn mark_ids_state() -> StateConfig<Vec<String>> {
    MARK_IDS_STATE.with(Clone::clone)
}

/// `$isInlineDecoratorNode(node)`: an inline `DecoratorNode`, one of the
/// default `markableNodes`.
pub fn is_inline_decorator_node(node: &NodeRef) -> bool {
    node.is_decorator() && node.is_inline()
}

/// `$isMarkableNode(flavor, node)` with the default `markableNodes`
/// (`DEFAULT_MARKABLE_NODES`): `TextNode` and its subclasses,
/// `LineBreakNode` and inline decorators.
fn is_markable_node(node: &NodeRef) -> bool {
    node.is_text() || node.get_latest().is::<LineBreakNode>() || is_inline_decorator_node(node)
}

/// `$readNodeMarkIDs(flavor, node)`.
fn read_node_mark_ids(node: &NodeRef) -> Result<Vec<String>, LexicalError> {
    get_state(node, &mark_ids_state())
}

/// `$getMarkableIDs(flavor, node)`: the ids of a markable node, otherwise
/// none.
fn get_markable_ids(node: Option<&NodeRef>) -> Result<Vec<String>, LexicalError> {
    match node {
        Some(node) if is_markable_node(node) => read_node_mark_ids(node),
        _ => Ok(Vec::new()),
    }
}

/// `$getNodeMarkIDs(node)`: the mark ids stored on a node, sorted; none for
/// a node that carries none.
pub fn get_node_mark_ids(node: &NodeRef) -> Result<Vec<String>, LexicalError> {
    read_node_mark_ids(node)
}

/// `$setState(node, state, updater)` for the set-valued state: the value
/// the updater returns is made canonical (`canonicalValue`) before it is
/// compared and stored.
fn update_node_mark_ids(
    node: &NodeRef,
    updater: impl FnOnce(&Vec<String>) -> Vec<String>,
) -> Result<(), LexicalError> {
    set_state_with(node, &mark_ids_state(), |prev| canonical_ids(updater(prev))).map(|_| ())
}

/// `$addNodeMarkIDs(flavor, node, ids)`.
fn add_node_mark_ids(node: &NodeRef, ids: &[String]) -> Result<(), LexicalError> {
    update_node_mark_ids(node, |prev| {
        if ids.iter().all(|id| prev.contains(id)) {
            prev.clone()
        } else {
            prev.iter().chain(ids).cloned().collect()
        }
    })
}

/// `$removeNodeMarkIDs(flavor, node, ids)`.
fn remove_node_mark_ids(node: &NodeRef, ids: &[String]) -> Result<(), LexicalError> {
    update_node_mark_ids(node, |prev| {
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
fn markable_nodes_in_selection(selection: &SelectionRef) -> Result<Vec<NodeRef>, LexicalError> {
    if selection.range().is_some_and(|r| r.is_collapsed()) {
        return Ok(Vec::new());
    }
    let mut nodes = Vec::new();
    let mut seen = HashSet::new();
    for node in selection.extract()? {
        if is_markable_node(&node) && seen.insert(node.get_key().to_owned()) {
            nodes.push(node);
        }
    }
    Ok(nodes)
}

/// `$addMarkID(id, selection)`: adds the mark `id` to every markable node
/// in `selection`, splitting the text nodes at its boundaries, and returns
/// the nodes that now carry it. Ids already on those nodes are kept, so
/// marks overlap freely; a collapsed selection marks nothing. Fails on an
/// id that is empty or holds whitespace.
pub fn add_mark_id(id: &str, selection: &SelectionRef) -> Result<Vec<NodeRef>, LexicalError> {
    assert_valid_mark_id(id)?;
    let nodes = markable_nodes_in_selection(selection)?;
    let ids = [id.to_owned()];
    for node in &nodes {
        add_node_mark_ids(node, &ids)?;
    }
    Ok(nodes)
}

/// `$collectMarkableNodes(flavor, node, out, seen)`: the markable nodes in
/// the subtree of `node`, in document order.
fn collect_markable_nodes(node: &NodeRef, out: &mut Vec<NodeRef>, seen: &mut HashSet<NodeKey>) {
    if is_markable_node(node) && seen.insert(node.get_key().to_owned()) {
        out.push(node.clone());
    }
    if node.is_element() {
        let mut child = node.get_first_child();
        while let Some(c) = child {
            collect_markable_nodes(&c, out, seen);
            child = c.get_next_sibling();
        }
    }
}

/// `$markableNodesIn(flavor, node)`.
fn markable_nodes_in(node: &NodeRef) -> Vec<NodeRef> {
    let mut out = Vec::new();
    collect_markable_nodes(node, &mut out, &mut HashSet::new());
    out
}

/// `$removeMarkID(id, selection)`: removes the mark `id` from the nodes in
/// `selection` (splitting the text nodes at its boundaries), or with `None`
/// from the whole document, which is walked, as JS does when the extension
/// is not active. Text left like its neighbors merges back in
/// normalization.
pub fn remove_mark_id(id: &str, selection: Option<&SelectionRef>) -> Result<(), LexicalError> {
    let nodes = match selection {
        Some(selection) => markable_nodes_in_selection(selection)?,
        None => markable_nodes_in(&get_root()),
    };
    let ids = [id.to_owned()];
    for node in &nodes {
        remove_node_mark_ids(node, &ids)?;
    }
    Ok(())
}

/// `$getAdjacentLeaf(node, 'next')`: the leaf after `node` within its
/// block, looking through the boundaries of inline elements such as links.
/// An empty inline element is returned as it is.
fn get_next_adjacent_leaf(node: &NodeRef) -> Option<NodeRef> {
    let mut current = node.clone();
    loop {
        let Some(sibling) = current.get_next_sibling() else {
            let parent = current.get_parent()?;
            if parent.is_element() && parent.is_inline() {
                current = parent;
                continue;
            }
            return None;
        };
        let mut leaf = sibling;
        while leaf.is_element() && leaf.is_inline() {
            match leaf.get_first_child() {
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
fn get_point_mark_ids(selection: &SelectionRef, which: Which) -> Result<Vec<String>, LexicalError> {
    let point = selection.point(which).expect("a range selection");
    let node = point.get_node()?;
    if point.point_type == PointType::Text {
        let ids = read_node_mark_ids(&node)?;
        if ids.is_empty() && node.is_text() && point.offset == node.get_text_content_size() {
            // Like $getMarkIDs: a caret right before a marked run is in it
            return get_markable_ids(get_next_adjacent_leaf(&node).as_ref());
        }
        return Ok(ids);
    }
    if node.is_element() {
        let before = get_markable_ids(
            point
                .offset
                .checked_sub(1)
                .and_then(|i| node.get_child_at_index(i))
                .as_ref(),
        )?;
        return if before.is_empty() {
            get_markable_ids(node.get_child_at_index(point.offset).as_ref())
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
    let Some(selection) = selection else {
        return Ok(Vec::new());
    };
    let range = selection.range();
    if range.as_ref().is_some_and(|r| r.is_collapsed()) {
        return get_point_mark_ids(selection, Which::Anchor);
    }
    let mut leaves: Vec<NodeRef> = selection
        .get_nodes()?
        .into_iter()
        .filter(is_markable_node)
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
        .map(read_node_mark_ids)
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

/// `$migrateMarkNodesToMarkState(node)`: converts a document that uses
/// `MarkNode` to mark ids on its leaves. The ids of every mark (with those
/// of the marks it is in) are added to the markable nodes inside it, and
/// the mark is unwrapped. `None` is the root. Returns the number of marks
/// unwrapped; fails on an id that is empty or holds whitespace.
pub fn migrate_mark_nodes_to_mark_state(node: Option<&NodeRef>) -> Result<usize, LexicalError> {
    fn visit(
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
                visit(&child, &ids, count)?;
            }
            // Re-read: unwrapping a nested MarkNode replaced it with its
            // children
            for child in current.get_children() {
                current.insert_before(&child, true)?;
            }
            current.remove(false)?;
            *count += 1;
        } else {
            if is_markable_node(current) && !inherited.is_empty() {
                add_node_mark_ids(current, inherited)?;
            }
            if current.is_element() {
                for child in current.get_children() {
                    visit(&child, inherited, count)?;
                }
            }
        }
        Ok(())
    }
    let node = node.cloned().unwrap_or_else(get_root);
    let mut count = 0;
    visit(&node, &[], &mut count)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_sort_by_utf16_code_units() {
        // U+FF01 comes before U+1F600 by code point, after it by code unit.
        assert_eq!(
            canonical_ids([
                "\u{ff01}".into(),
                "\u{1f600}".into(),
                "b".into(),
                "b".into()
            ]),
            ["b", "\u{1f600}", "\u{ff01}"]
        );
        assert!(!is_valid_mark_id("a\u{feff}b"));
        assert!(is_valid_mark_id("a\u{85}b"));
    }
}
