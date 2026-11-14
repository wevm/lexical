//! `LexicalEditorState.ts`.

use crate::json::{Object, Value};
use crate::lexical_selection::SelectionRef;
use crate::lexical_utils::get_root;
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::nodes::root::RootNode;
use crate::serialized_export::{export_node_json, with_compact_export};
use crate::updates::read_editor_state;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) type NodeMap = HashMap<NodeKey, Rc<dyn LexicalNode>>;

/// An editor state: the node map and the selection. A handle; cloning it
/// shares the same state. Once read-only (committed or parsed), nothing
/// changes it: an update works on a clone whose node map shares the nodes,
/// and a node is copied before its first change (`getWritable`).
#[derive(Clone, Debug)]
pub struct EditorState(pub(crate) Rc<EditorStateInner>);

#[derive(Debug)]
pub(crate) struct EditorStateInner {
    pub(crate) node_map: RefCell<NodeMap>,
    pub(crate) selection: RefCell<Option<SelectionRef>>,
    pub(crate) read_only: Cell<bool>,
    /// `_flushSync`: commit at the end of the update (`discrete`).
    pub(crate) flush_sync: Cell<bool>,
    /// `_parsed`: made by `parseEditorState`, so no transform has run on it.
    pub(crate) parsed: Cell<bool>,
    /// Tells this state from every other on the thread: a node version
    /// records the one it was made writable in (`NodeBase::written_in`),
    /// which is how a NodeState knows its version, as JS knows its object.
    pub(crate) id: u64,
}

thread_local! {
    static NEXT_EDITOR_STATE_ID: Cell<u64> = const { Cell::new(1) };
}

fn new_editor_state(node_map: NodeMap) -> EditorState {
    EditorState(Rc::new(EditorStateInner {
        node_map: RefCell::new(node_map),
        selection: RefCell::new(None),
        read_only: Cell::new(false),
        flush_sync: Cell::new(false),
        parsed: Cell::new(false),
        id: NEXT_EDITOR_STATE_ID.with(|n| n.replace(n.get() + 1)),
    }))
}

/// `createEmptyEditorState()`.
pub(crate) fn create_empty_editor_state() -> EditorState {
    let mut node_map = NodeMap::new();
    node_map.insert("root".to_owned(), Rc::new(RootNode::new()));
    new_editor_state(node_map)
}

/// `cloneEditorState(current)`: the same nodes, no selection, writable.
pub(crate) fn clone_editor_state(current: &EditorState) -> EditorState {
    new_editor_state(current.0.node_map.borrow().clone())
}

/// `$exportNodeToJSON(node)`: the node and its children. Slots come with
/// their port.
fn export_node_to_json(node: &NodeRef) -> Value {
    let mut serialized_node: Object = export_node_json(&*node.get_latest());
    if node.is_element() {
        let children: Vec<Value> = node
            .get_children()
            .iter()
            .map(export_node_to_json)
            .collect();
        serialized_node.insert("children", Value::Array(children));
    }
    Value::Object(serialized_node)
}

impl EditorState {
    /// `isEmpty()`: nothing but the root, and no selection.
    pub fn is_empty(&self) -> bool {
        self.0.node_map.borrow().len() <= 1 && self.0.selection.borrow().is_none()
    }

    /// The state's selection (`editorState._selection`).
    pub fn selection(&self) -> Option<SelectionRef> {
        self.0.selection.borrow().clone()
    }

    /// The node keys in the order JS's node map holds them, the order nodes
    /// were created in: the root, then by key, as keys come from a counter.
    pub(crate) fn keys_in_creation_order(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.0.node_map.borrow().keys().cloned().collect();
        keys.sort_by_key(|k| (k != "root", k.parse::<u64>().unwrap_or(u64::MAX), k.clone()));
        keys
    }

    /// Whether `self` and `other` are the same state.
    pub fn ptr_eq(&self, other: &EditorState) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// `read(callbackFn)`: runs `f` with this state active, read-only.
    pub fn read<R>(&self, f: impl FnOnce() -> R) -> R {
        read_editor_state(None, self, f)
    }

    /// `toJSON(compact)`: the document's JSON, in the compact form or the
    /// legacy form that writes every property. `to_json(false).to_string()`
    /// is `JSON.stringify(editorState.toJSON())`.
    pub fn to_json(&self, compact: bool) -> Value {
        with_compact_export(compact, || {
            read_editor_state(None, self, || {
                let mut json = Object::new();
                json.insert("root", export_node_to_json(&get_root()));
                Value::Object(json)
            })
        })
    }
}
