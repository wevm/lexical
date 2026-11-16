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
        // Its NodeState is applied by `$parseSerializedNode`.
        crate::lexical_utils::with_writable(root.get_key(), |node| node.update_from_json(json));
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
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            export_compact_element_node(self)
        } else {
            export_element_node(self)
        })
    }
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_element_node(&mut self.element, json);
        true
    }
    fn collapse_at_start(&self, _: &NodeRef, _: &SelectionRef) -> Result<bool, LexicalError> {
        Ok(true)
    }
    /// `RootNode.splice`: the root takes only elements and decorators, then
    /// `ElementNode`'s splice.
    fn splice(
        &self,
        this: &NodeRef,
        start: usize,
        delete_count: usize,
        nodes_to_insert: &[NodeRef],
    ) -> Result<(), LexicalError> {
        for node in nodes_to_insert {
            crate::error::invariant!(
                node.is_element() || node.is_decorator(),
                "rootNode.splice: Only element or decorator nodes can be inserted to the root node"
            );
        }
        crate::nodes::element::splice(this, start, delete_count, nodes_to_insert)
    }
}
