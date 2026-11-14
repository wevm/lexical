//! `nodes/LexicalRootNode.ts`.

use crate::LexicalError;
use crate::generated_json::{
    export_compact_element_node, export_element_node, update_element_node,
};
use crate::json::Object;
use crate::lexical_selection::SelectionRef;
use crate::lexical_utils::get_root;
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::ElementData;

/// The root of every editor state, under the key `"root"`.
///
/// JS's `__cachedText`, which the DOM reconciler fills, comes with the
/// reconciler; until then `getTextContent` always walks the children, as JS
/// does for a headless editor.
#[derive(Clone, Debug)]
pub struct RootNode {
    base: NodeBase,
    element: ElementData,
}

impl RootNode {
    pub(crate) fn new() -> Self {
        Self {
            base: NodeBase {
                key: "root".to_owned(),
                ..NodeBase::default()
            },
            element: ElementData::default(),
        }
    }

    /// `RootNode.importJSON`: applies the JSON to the existing root rather
    /// than creating one.
    pub(crate) fn import_json(json: &Object) -> Result<NodeRef, LexicalError> {
        let root = get_root();
        root.update_from_json(json);
        Ok(root)
    }
}

impl LexicalNode for RootNode {
    fn get_type(&self) -> &'static str {
        "root"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_element_node(self)
        } else {
            export_element_node(self)
        }
    }
    fn update_from_json(&mut self, json: &Object) {
        update_element_node(&mut self.element, json);
    }
    fn collapse_at_start(&self, _: &NodeRef, _: &SelectionRef) -> Result<bool, LexicalError> {
        Ok(true)
    }
}
