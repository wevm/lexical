//! `nodes/LexicalTabNode.ts`.

use crate::constants::IS_UNMERGEABLE;
use crate::generated_json::{export_compact_tab_node, export_tab_node, update_tab_node};
use crate::json::Object;
use crate::node::{LexicalNode, NodeBase};
use crate::nodes::text::TextData;

/// A text node that always holds one `\t` and never merges.
#[derive(Clone, Debug)]
pub struct TabNode {
    base: NodeBase,
    text: TextData,
}

impl Default for TabNode {
    fn default() -> Self {
        Self {
            base: NodeBase::default(),
            text: TextData {
                text: "\t".to_owned(),
                detail: IS_UNMERGEABLE as f64,
                ..TextData::default()
            },
        }
    }
}

impl LexicalNode for TabNode {
    fn get_type(&self) -> &'static str {
        "tab"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_tab_node(self.get_type(), &self.text)
        } else {
            export_tab_node(self.get_type(), &self.text)
        }
    }
    fn update_from_json(&mut self, json: &Object) {
        update_tab_node(&mut self.text, json);
    }
}
