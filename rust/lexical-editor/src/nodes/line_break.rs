//! `nodes/LexicalLineBreakNode.ts`.

use crate::generated_json::{export_compact_line_break_node, export_line_break_node};
use crate::json::Object;
use crate::node::{LexicalNode, NodeBase, NodeRef};

/// `$createLineBreakNode()`.
pub fn create_line_break_node() -> NodeRef {
    crate::utils::set_node_key(Box::new(LineBreakNode::default()))
}

/// A soft line break (Shift+Enter).
#[derive(Clone, Debug, Default)]
pub struct LineBreakNode {
    base: NodeBase,
}

impl LexicalNode for LineBreakNode {
    fn get_type(&self) -> &'static str {
        "linebreak"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_inline(&self) -> bool {
        true
    }
    fn get_text_content(&self) -> String {
        "\n".to_owned()
    }
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_line_break_node(self.get_type())
        } else {
            export_line_break_node(self.get_type())
        }
    }
    fn update_from_json(&mut self, _json: &Object) {}
}
