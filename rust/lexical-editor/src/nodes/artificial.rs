//! `nodes/ArtificialNode.ts`: `ArtificialNode__DO_NOT_USE`, the element
//! `@lexical/html`'s import wraps inline content in when it is already
//! inside a block, then unwraps. Every editor registers it, as in JS.

use crate::generated_json::{
    export_compact_element_node, export_element_node, update_element_node,
};
use crate::json::Object;
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::ElementData;

/// `ArtificialNode__DO_NOT_USE`. Its JSON is `ElementNode`'s.
#[derive(Clone, Debug, Default)]
pub struct ArtificialNode {
    base: NodeBase,
    element: ElementData,
}

/// `new ArtificialNode__DO_NOT_USE()`.
pub fn create_artificial_node() -> NodeRef {
    crate::lexical_utils::set_node_key(Box::new(ArtificialNode::default()))
}

/// `node instanceof ArtificialNode__DO_NOT_USE`.
pub fn is_artificial_node(node: &NodeRef) -> bool {
    node.get_latest().is::<ArtificialNode>()
}

impl LexicalNode for ArtificialNode {
    fn get_type(&self) -> &'static str {
        "artificial"
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

    /// `ArtificialNode__DO_NOT_USE.createDOM()`: a `<div>`, which nothing
    /// uses.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, crate::LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("div"))
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
}
