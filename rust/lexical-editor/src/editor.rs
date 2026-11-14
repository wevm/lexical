//! `LexicalEditor.ts`: creating an editor and its registered nodes.

use crate::editor_state::create_empty_editor_state;
use crate::json::{self, Object, Value};
use crate::node::{LexicalNode, NodeRef};
use crate::nodes::{
    line_break::LineBreakNode, paragraph::ParagraphNode, root::RootNode, tab::TabNode,
    text::TextNode,
};
use crate::updates::{parse_editor_state, read_editor_state};
use crate::utils::set_node_key;
use crate::{EditorState, LexicalError};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A node class an editor registers: its type and how it imports JSON (the
/// static `importJSON`).
#[derive(Clone, Copy)]
pub struct NodeClass {
    pub(crate) node_type: &'static str,
    pub(crate) import_json: fn(&Object) -> Result<NodeRef, LexicalError>,
}

/// The `importJSON` a class gets when it declares none
/// (`synthesizeImportJSON`): build the node, then apply the JSON.
fn import_default<T: LexicalNode + Default>(json: &Object) -> Result<NodeRef, LexicalError> {
    let mut node = T::default();
    node.update_from_json(json);
    Ok(set_node_key(Box::new(node)))
}

impl NodeClass {
    /// The class of `T`, which imports a node as `T::default()` with the
    /// JSON applied.
    pub fn of<T: LexicalNode + Default>() -> Self {
        Self {
            node_type: T::default().get_type(),
            import_json: import_default::<T>,
        }
    }

    /// A class with its own `importJSON`.
    pub fn with_import_json(
        node_type: &'static str,
        import_json: fn(&Object) -> Result<NodeRef, LexicalError>,
    ) -> Self {
        Self {
            node_type,
            import_json,
        }
    }

    pub fn node_type(&self) -> &'static str {
        self.node_type
    }
}

/// `CreateEditorArgs`: the node classes to register besides the core ones.
#[derive(Clone, Default)]
pub struct CreateEditorArgs {
    pub nodes: Vec<NodeClass>,
}

/// A Lexical editor. A handle; cloning it shares the same editor. Without a
/// DOM it is what JS calls a headless editor.
#[derive(Clone)]
pub struct Editor(Rc<EditorInner>);

struct EditorInner {
    nodes: HashMap<&'static str, NodeClass>,
    editor_state: RefCell<EditorState>,
}

/// `createEditor(config)`: registers the core nodes (root, text, line
/// break, tab, paragraph) and `args.nodes`.
pub fn create_editor(args: CreateEditorArgs) -> Editor {
    let core = [
        NodeClass::with_import_json("root", RootNode::import_json),
        NodeClass::of::<TextNode>(),
        NodeClass::of::<LineBreakNode>(),
        NodeClass::of::<TabNode>(),
        NodeClass::of::<ParagraphNode>(),
    ];
    let nodes = core
        .into_iter()
        .chain(args.nodes)
        .map(|class| (class.node_type, class))
        .collect();
    Editor(Rc::new(EditorInner {
        nodes,
        editor_state: RefCell::new(create_empty_editor_state()),
    }))
}

impl Editor {
    pub(crate) fn registered_nodes(&self) -> &HashMap<&'static str, NodeClass> {
        &self.0.nodes
    }

    /// `getEditorState()`.
    pub fn get_editor_state(&self) -> EditorState {
        self.0.editor_state.borrow().clone()
    }

    /// `parseEditorState(json)` from the JSON text.
    pub fn parse_editor_state(&self, json: &str) -> Result<EditorState, LexicalError> {
        let value = json::parse(json).map_err(|e| LexicalError::new(e.to_string()))?;
        self.parse_editor_state_json(&value)
    }

    /// `parseEditorState(serializedEditorState)` from parsed JSON.
    pub fn parse_editor_state_json(&self, json: &Value) -> Result<EditorState, LexicalError> {
        parse_editor_state(json, self)
    }

    /// `read(callbackFn)`: runs `f` with the editor's state active,
    /// read-only.
    pub fn read<R>(&self, f: impl FnOnce() -> R) -> R {
        read_editor_state(&self.get_editor_state(), f)
    }
}
