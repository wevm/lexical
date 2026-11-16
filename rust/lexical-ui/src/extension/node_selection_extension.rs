//! `NodeSelectionExtension.ts`.

use super::editor_state_extension::editor_state_extension;
use super::signals::{ReadonlySignal, Signal, computed, effect, signal};
use super::watched_signal::watched_signal;
use crate::extension_core::{ExtensionBuildState, LexicalExtension, define_extension};
use crate::{Editor, NodeKey, get_node_by_key, get_selection};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// The keys `watchNodeKey` was asked for, in the order first asked, each
/// with the signals it returned for it (JS's `Map<NodeKey,
/// Set<ReadonlySignal<boolean>>>`).
type WatchedNodeKeys = Rc<RefCell<Vec<(NodeKey, Vec<ReadonlySignal<bool>>)>>>;

/// `{watchedNodeKeys}`, the value of the watched node store. JS stores a
/// new object around the same map when a key is added, so the store's
/// signal notifies; this is the same only as itself.
#[derive(Clone)]
struct WatchedNodeStore {
    watched_node_keys: WatchedNodeKeys,
    identity: Rc<()>,
}

impl WatchedNodeStore {
    fn new(watched_node_keys: WatchedNodeKeys) -> Self {
        Self {
            watched_node_keys,
            identity: Rc::new(()),
        }
    }
}

impl PartialEq for WatchedNodeStore {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.identity, &other.identity)
    }
}

/// `watchNodeKey`.
type WatchNodeKey = Rc<dyn Fn(&str) -> ReadonlySignal<bool>>;

/// The output of `NodeSelectionExtension`.
#[derive(Clone)]
pub struct NodeSelectionOutput {
    watch_node_key: WatchNodeKey,
}

impl NodeSelectionOutput {
    /// `watchNodeKey(key)`: a signal of whether the node with `key` is
    /// selected (`node.isSelected()`), kept up to date from the editor's
    /// state while something watches it. Before then, and while there is
    /// no selection, it is `false`.
    pub fn watch_node_key(&self, key: &str) -> ReadonlySignal<bool> {
        (self.watch_node_key)(key)
    }
}

/// `NodeSelectionExtension`.
pub type NodeSelectionExtensionType = LexicalExtension<(), NodeSelectionOutput, ()>;

fn make() -> Rc<NodeSelectionExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, _: &(), state: &ExtensionBuildState<()>| {
                let editor_state_store = state
                    .get_dependency(&editor_state_extension())?
                    .output
                    .expect("EditorStateExtension has a build");
                let watched_node_store =
                    signal(WatchedNodeStore::new(Rc::new(RefCell::new(Vec::new()))));
                let selected_node_keys: Signal<Option<HashSet<NodeKey>>> = {
                    let watched_node_store = watched_node_store.clone();
                    watched_signal(
                        || None,
                        move |selected_node_keys| {
                            let selected_node_keys = selected_node_keys.clone();
                            let editor_state_store = editor_state_store.clone();
                            let watched_node_store = watched_node_store.clone();
                            effect(move || {
                                let prev_selected_node_keys: Option<HashSet<NodeKey>> =
                                    selected_node_keys.peek();
                                let watched_node_keys =
                                    watched_node_store.value().watched_node_keys;
                                let mut next_selected_node_keys: Option<HashSet<NodeKey>> = None;
                                let mut did_change = false;
                                editor_state_store.value().read(|| {
                                    if get_selection().is_none() {
                                        return;
                                    }
                                    // We intentionally mutate this without
                                    // firing a signal, to avoid re-triggering
                                    // this effect. There are no subscribers so
                                    // nothing can observe whether key was in
                                    // the set or not.
                                    watched_node_keys
                                        .borrow_mut()
                                        .retain(|(_, listeners)| !listeners.is_empty());
                                    let keys: Vec<NodeKey> = watched_node_keys
                                        .borrow()
                                        .iter()
                                        .map(|(key, _)| key.clone())
                                        .collect();
                                    for key in keys {
                                        let is_selected =
                                            get_node_by_key(&key).is_some_and(|node| {
                                                matches!(node.is_selected(None), Ok(true))
                                            });
                                        did_change = did_change
                                            || is_selected
                                                != prev_selected_node_keys
                                                    .as_ref()
                                                    .is_some_and(|prev| prev.contains(&key));
                                        if is_selected {
                                            next_selected_node_keys
                                                .get_or_insert_with(HashSet::new)
                                                .insert(key);
                                        }
                                    }
                                });
                                let same = !did_change
                                    && match (&next_selected_node_keys, &prev_selected_node_keys) {
                                        (Some(next), Some(prev)) => next.len() == prev.len(),
                                        _ => false,
                                    };
                                if !same {
                                    selected_node_keys.set_value(next_selected_node_keys);
                                }
                                None
                            })
                        },
                    )
                };
                let watch_node_key = move |key: &str| -> ReadonlySignal<bool> {
                    let watcher = {
                        let selected_node_keys = selected_node_keys.clone();
                        let key = key.to_owned();
                        computed(move || {
                            selected_node_keys
                                .value()
                                .is_some_and(|keys| keys.contains(&key))
                        })
                    };
                    let watched_node_keys = watched_node_store.peek().watched_node_keys;
                    let had_listener = {
                        let mut entries = watched_node_keys.borrow_mut();
                        match entries.iter_mut().find(|(k, _)| k == key) {
                            Some((_, listeners)) => {
                                listeners.push(watcher.clone());
                                true
                            }
                            None => {
                                entries.push((key.to_owned(), vec![watcher.clone()]));
                                false
                            }
                        }
                    };
                    if !had_listener {
                        watched_node_store.set_value(WatchedNodeStore::new(watched_node_keys));
                    }
                    watcher
                };
                Ok(NodeSelectionOutput {
                    watch_node_key: Rc::new(watch_node_key),
                })
            },
        )),
        dependencies: vec![editor_state_extension().into()],
        ..LexicalExtension::new("@lexical/extension/NodeSelection", ())
    })
}

thread_local! {
    static NODE_SELECTION_EXTENSION: Rc<NodeSelectionExtensionType> = make();
}

/// `NodeSelectionExtension`: an output whose `watchNodeKey` returns a
/// signal of whether a node is selected, typically a decorator's (a
/// framework independent alternative to `useLexicalNodeSelection`).
pub fn node_selection_extension() -> Rc<NodeSelectionExtensionType> {
    NODE_SELECTION_EXTENSION.with(Rc::clone)
}
