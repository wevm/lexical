//! `LexicalEditorState.ts`.

use crate::json::{Object, Value};
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::nodes::root::RootNode;
use crate::serialized_export::{export_node_json, with_compact_export};
use crate::updates::read_editor_state;
use crate::utils::get_root;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) type NodeMap = HashMap<NodeKey, Rc<dyn LexicalNode>>;

/// An editor state: the node map (and, once ported, the selection). A
/// handle; cloning it shares the same state. Once read-only, nothing changes
/// it.
#[derive(Clone, Debug)]
pub struct EditorState(pub(crate) Rc<EditorStateInner>);

#[derive(Debug)]
pub(crate) struct EditorStateInner {
    pub(crate) node_map: RefCell<NodeMap>,
    pub(crate) read_only: Cell<bool>,
}

/// `createEmptyEditorState()`.
pub(crate) fn create_empty_editor_state() -> EditorState {
    let mut node_map = NodeMap::new();
    node_map.insert("root".to_owned(), Rc::new(RootNode::new()));
    EditorState(Rc::new(EditorStateInner {
        node_map: RefCell::new(node_map),
        read_only: Cell::new(false),
    }))
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
    /// `isEmpty()`: whether the state has nothing but its root (and no
    /// selection).
    pub fn is_empty(&self) -> bool {
        self.0.node_map.borrow().len() <= 1
    }

    /// `read(callbackFn)`: runs `f` with this state active, read-only.
    pub fn read<R>(&self, f: impl FnOnce() -> R) -> R {
        read_editor_state(self, f)
    }

    /// `toJSON(compact)`: the document's JSON, in the compact form or the
    /// legacy form that writes every property. `to_json(false).to_string()`
    /// is `JSON.stringify(editorState.toJSON())`.
    pub fn to_json(&self, compact: bool) -> Value {
        with_compact_export(compact, || {
            read_editor_state(self, || {
                let mut json = Object::new();
                json.insert("root", export_node_to_json(&get_root()));
                Value::Object(json)
            })
        })
    }
}
