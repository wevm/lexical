//! `nodes/LexicalParagraphNode.ts`.

use crate::generated_json::{
    export_compact_element_node, export_element_node, update_element_node,
};
use crate::json::{Object, Value};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::ElementData;

/// `$createParagraphNode()`.
pub fn create_paragraph_node() -> NodeRef {
    crate::utils::set_node_key(Box::new(ParagraphNode::default()))
}

#[derive(Clone, Debug, Default)]
pub struct ParagraphNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for ParagraphNode {
    fn get_type(&self) -> &'static str {
        "paragraph"
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
        let mut json = if compact {
            export_compact_element_node(self)
        } else {
            export_element_node(self)
        };
        // Provide backwards compatible values, see #7971.
        let absent = |json: &Object, key: &str| json.get(key).is_none_or(Value::is_undefined);
        if absent(&json, "textFormat") || absent(&json, "textStyle") {
            let first_text_node = NodeRef::new(self.base.key.clone())
                .get_children()
                .into_iter()
                .map(|child| child.get_latest())
                .find(|child| child.text().is_some());
            let (text_format, text_style) = match &first_text_node {
                Some(text) => {
                    let text = text.text().expect("a text node");
                    (text.format, text.style.clone())
                }
                None => (self.element.text_format, self.element.text_style.clone()),
            };
            if !compact || text_format != 0.0 {
                json.insert("textFormat", text_format.into());
            }
            if !compact || !text_style.is_empty() {
                json.insert("textStyle", text_style.into());
            }
        }
        json
    }

    fn update_from_json(&mut self, json: &Object) {
        update_element_node(&mut self.element, json);
    }
}
