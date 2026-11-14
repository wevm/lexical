//! `HorizontalRuleExtension.ts`: `HorizontalRuleNode`,
//! `$createHorizontalRuleNode` and `$isHorizontalRuleNode`.

use crate::generated_json::{export_compact_line_break_node, export_line_break_node};
use crate::json::Object;
use crate::lexical_utils::set_node_key;
use crate::node::{LexicalNode, NodeBase, NodeRef};

/// `HorizontalRuleNode`: a horizontal rule, a block decorator whose text is
/// a line break.
#[derive(Clone, Debug, Default)]
pub struct HorizontalRuleNode {
    base: NodeBase,
}

/// `$createHorizontalRuleNode()`.
pub fn create_horizontal_rule_node() -> NodeRef {
    set_node_key(Box::new(HorizontalRuleNode::default()))
}

/// `$isHorizontalRuleNode(node)`.
pub fn is_horizontal_rule_node(node: &NodeRef) -> bool {
    node.get_latest().is::<HorizontalRuleNode>()
}

impl LexicalNode for HorizontalRuleNode {
    fn get_type(&self) -> &'static str {
        "horizontalrule"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    /// `HorizontalRuleNode.isInline()`.
    fn is_inline(&self) -> bool {
        false
    }
    /// `HorizontalRuleNode.getTextContent()`.
    fn get_text_content(&self) -> String {
        "\n".to_owned()
    }
    /// `LexicalNode`'s generated JSON: the type and the version.
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_line_break_node(self.get_type())
        } else {
            export_line_break_node(self.get_type())
        }
    }
    fn update_from_json(&mut self, _json: &Object) {}
}
