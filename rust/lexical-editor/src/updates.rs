//! `LexicalUpdates.ts`: the active editor and editor state that `$`
//! functions read, and `parseEditorState`. The update lifecycle
//! (`editor.update`, transforms, commit, listeners) comes next
//! (rust/DESIGN.md, milestones).

use crate::editor::NodeClass;
use crate::editor_state::create_empty_editor_state;
use crate::error::invariant;
use crate::json::Value;
use crate::node::NodeRef;
use crate::{Editor, EditorState, LexicalError, constants::NODE_STATE_KEY};
use std::cell::RefCell;
use std::collections::HashMap;

/// JS's module-scope `activeEditorState` and `isReadOnlyMode`, per thread.
/// `activeEditor` comes with the update API, its first reader.
#[derive(Clone, Default)]
struct Active {
    editor_state: Option<EditorState>,
    read_only: bool,
}

thread_local! {
    static ACTIVE: RefCell<Active> = RefCell::new(Active::default());
}

/// Runs `f` with `active` in place, restoring the previous one afterwards,
/// even when `f` panics.
fn with_active<R>(active: Active, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Active>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take().expect("previous");
            ACTIVE.with(|a| *a.borrow_mut() = previous);
        }
    }
    let previous = ACTIVE.with(|a| std::mem::replace(&mut *a.borrow_mut(), active));
    let _restore = Restore(Some(previous));
    f()
}

/// `getActiveEditorState()`.
pub(crate) fn get_active_editor_state() -> EditorState {
    ACTIVE
        .with(|a| a.borrow().editor_state.clone())
        .unwrap_or_else(|| {
            panic!(
                "Unable to find an active editor state. State helpers or node methods can only be used synchronously during the callback of editor.update(), editor.read(), or editorState.read()."
            )
        })
}

/// `isCurrentlyReadOnlyMode()`.
pub(crate) fn is_currently_read_only_mode() -> bool {
    ACTIVE.with(|a| a.borrow().read_only)
}

/// `errorOnReadOnly()`.
pub(crate) fn error_on_read_only() {
    if is_currently_read_only_mode() {
        panic!("Cannot use method in read-only mode.");
    }
}

/// `readEditorState(editor, editorState, callbackFn)`.
pub(crate) fn read_editor_state<R>(editor_state: &EditorState, f: impl FnOnce() -> R) -> R {
    with_active(
        Active {
            editor_state: Some(editor_state.clone()),
            read_only: true,
        },
        f,
    )
}

/// `$parseSerializedNodeImpl`.
fn parse_serialized_node_impl(
    serialized_node: &Value,
    registered_nodes: &HashMap<&'static str, NodeClass>,
) -> Result<NodeRef, LexicalError> {
    let Some(object) = serialized_node.as_object() else {
        // `serializedNode.type` of a primitive is `undefined`; of `null` it
        // throws a TypeError.
        invariant!(
            !serialized_node.is_null(),
            "Cannot read properties of null (reading 'type')"
        );
        return Err(LexicalError::new(
            "parseEditorState: type \"undefined\" + not found",
        ));
    };
    let node_type = object.get("type");
    let registered = node_type
        .and_then(Value::as_str)
        .and_then(|t| registered_nodes.get(t));
    let Some(registered) = registered else {
        let shown = match node_type {
            None | Some(Value::Undefined) => "undefined".to_owned(),
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
        };
        return Err(LexicalError::new(format!(
            "parseEditorState: type \"{shown}\" + not found"
        )));
    };
    // Not ported yet; refused rather than dropped (rust/PARITY.md).
    invariant!(
        !object.contains_key(NODE_STATE_KEY),
        "parseEditorState: NodeState (\"$\") is not ported to Rust yet"
    );
    invariant!(
        !object.contains_key("$slots"),
        "parseEditorState: slots (\"$slots\") are not ported to Rust yet"
    );

    let node = (registered.import_json)(object)?;
    if let Some(Value::Array(children)) = object.get("children")
        && node.is_element()
    {
        for child in children {
            let child = parse_serialized_node_impl(child, registered_nodes)?;
            node.append(&[child])?;
        }
    }
    Ok(node)
}

/// `parseEditorState(serializedEditorState, editor)`.
pub(crate) fn parse_editor_state(
    serialized_editor_state: &Value,
    editor: &Editor,
) -> Result<EditorState, LexicalError> {
    let editor_state = create_empty_editor_state();
    let active = Active {
        editor_state: Some(editor_state.clone()),
        read_only: false,
    };
    with_active(active, || {
        let serialized_node = serialized_editor_state.get("root");
        invariant!(
            serialized_node.is_some_and(|v| !v.is_undefined()),
            "Cannot read properties of undefined (reading 'type')"
        );
        parse_serialized_node_impl(serialized_node.unwrap(), editor.registered_nodes())?;
        Ok(())
    })?;
    // Make the editor state immutable.
    editor_state.0.read_only.set(true);
    Ok(editor_state)
}
