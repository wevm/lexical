//! `nodes/LexicalTextNode.ts`.

use crate::constants::{TEXT_MODE_TO_TYPE, name_of};
use crate::generated_json::{export_compact_text_node, export_text_node, update_text_node};
use crate::json::Object;
use crate::node::{LexicalNode, NodeBase};

/// The fields `TextNode` adds. `format` and `detail` are kept as the JS
/// number they parsed to, so a document writes back exactly what JS writes;
/// JS's bitwise operators read them as `ToInt32`.
///
/// `text` is a Rust string: offsets into it are UTF-16 code units, as in JS,
/// converted where they are used (rust/DESIGN.md).
#[derive(Clone, Debug)]
pub struct TextData {
    pub(crate) text: String,
    pub(crate) format: f64,
    pub(crate) style: String,
    pub(crate) mode: u8,
    pub(crate) detail: f64,
}

impl Default for TextData {
    fn default() -> Self {
        Self {
            text: String::new(),
            format: 0.0,
            style: String::new(),
            mode: 0,
            detail: 0.0,
        }
    }
}

impl TextData {
    pub fn get_text(&self) -> &str {
        &self.text
    }

    pub fn get_format(&self) -> f64 {
        self.format
    }

    pub fn get_style(&self) -> &str {
        &self.style
    }

    pub fn get_detail(&self) -> f64 {
        self.detail
    }

    /// `getMode()`: `'normal'`, `'token'` or `'segmented'`.
    pub fn get_mode(&self) -> &'static str {
        name_of(TEXT_MODE_TO_TYPE, self.mode).unwrap_or("normal")
    }
}

#[derive(Clone, Debug, Default)]
pub struct TextNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for TextNode {
    fn get_type(&self) -> &'static str {
        "text"
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
            export_compact_text_node(self.get_type(), &self.text)
        } else {
            export_text_node(self.get_type(), &self.text)
        }
    }
    fn update_from_json(&mut self, json: &Object) {
        update_text_node(&mut self.text, json);
    }
}
