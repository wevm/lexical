//! `LexicalUtils.ts`: keys, node lookup and `getWritable`.

use crate::EditorState;
use crate::node::{LexicalNode, NodeRef};
use crate::updates::{error_on_read_only, get_active_editor_state};
use std::cell::Cell;
use std::rc::Rc;

thread_local! {
    static KEY_COUNTER: Cell<u64> = const { Cell::new(1) };
}

/// `resetRandomKey()`: restarts the key counter, for deterministic keys in
/// tests.
pub fn reset_random_key() {
    KEY_COUNTER.with(|c| c.set(1));
}

/// `generateRandomKey()`.
pub(crate) fn generate_random_key() -> String {
    KEY_COUNTER.with(|c| {
        let key = c.get();
        c.set(key + 1);
        key.to_string()
    })
}

pub(crate) fn get_node_by_key_impl(
    editor_state: &EditorState,
    key: &str,
) -> Option<Rc<dyn LexicalNode>> {
    editor_state.0.node_map.borrow().get(key).cloned()
}

/// `$getNodeByKey(key)`.
pub fn get_node_by_key(key: &str) -> Option<NodeRef> {
    get_node_by_key_impl(&get_active_editor_state(), key).map(|_| NodeRef::new(key))
}

/// `$getRoot()`.
pub fn get_root() -> NodeRef {
    NodeRef::new("root")
}

/// `$setNodeKey(node, null)`: gives a new node a key and puts it in the
/// active editor state.
pub(crate) fn set_node_key(mut node: Box<dyn LexicalNode>) -> NodeRef {
    error_on_read_only();
    let key = generate_random_key();
    node.base_mut().key = key.clone();
    get_active_editor_state()
        .0
        .node_map
        .borrow_mut()
        .insert(key.clone(), Rc::from(node));
    NodeRef::new(key)
}

/// `node.getWritable()`, then `f` on it. The node is cloned first when
/// anything else shares it, such as the previous editor state.
///
/// `f` runs while the node map is borrowed, so it must only change the
/// node's own fields, not read other nodes.
pub(crate) fn with_writable<R>(key: &str, f: impl FnOnce(&mut dyn LexicalNode) -> R) -> R {
    error_on_read_only();
    let editor_state = get_active_editor_state();
    let mut node_map = editor_state.0.node_map.borrow_mut();
    let node = node_map
        .get_mut(key)
        .unwrap_or_else(|| panic!("getWritable: node {key} does not exist"));
    if Rc::get_mut(node).is_none() {
        *node = Rc::from(node.clone_node());
    }
    f(Rc::get_mut(node).expect("unshared"))
}
