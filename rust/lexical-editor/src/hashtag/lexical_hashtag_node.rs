//! `LexicalHashtagNode.ts`: `HashtagNode`, `$createHashtagNode` and
//! `$isHashtagNode`.

use crate::NodeClass;
use crate::generated_json::{export_compact_text_node, export_text_node, update_text_node};
use crate::json::Object;
use crate::lexical_utils::set_node_key;
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::text::TextData;

/// `HashtagNode`: a text node holding a `#hashtag`, a text entity. Text
/// typed at its start doesn't go into it.
#[derive(Clone, Debug, Default)]
pub struct HashtagNode {
    base: NodeBase,
    text: TextData,
}

/// `$createHashtagNode(text)`.
pub fn create_hashtag_node(text: &str) -> NodeRef {
    set_node_key(Box::new(HashtagNode {
        base: NodeBase::default(),
        text: TextData {
            text: text.to_owned(),
            ..TextData::default()
        },
    }))
}

/// `$isHashtagNode(node)`.
pub fn is_hashtag_node(node: &NodeRef) -> bool {
    node.get_latest().is::<HashtagNode>()
}

/// The node classes of this package (`HashtagExtension`'s `nodes`):
/// `HashtagNode`.
pub fn hashtag_nodes() -> Vec<NodeClass> {
    vec![NodeClass::of::<HashtagNode>()]
}

impl LexicalNode for HashtagNode {
    fn get_type(&self) -> &'static str {
        "hashtag"
    }
    /// `HashtagNode.createDOM(config)`: `TextNode`'s, with the theme's
    /// `hashtag` class names.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, crate::LexicalError> {
        let element = crate::nodes::text::create_dom(self, config)?;
        crate::lexical_dom::add_theme_class_names(&element, &config.theme, "hashtag");
        Ok(element)
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

    /// `HashtagNode.canInsertTextBefore()`.
    fn can_insert_text_before(&self) -> bool {
        false
    }

    /// `HashtagNode.isTextEntity()`.
    fn is_text_entity(&self) -> bool {
        true
    }

    /// `exportTextNode` and `exportCompactTextNode`, with its type.
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_text_node(self.get_type(), &self.text)
        } else {
            export_text_node(self.get_type(), &self.text)
        }
    }

    /// `updateTextNode`.
    fn update_from_json(&mut self, json: &Object) {
        update_text_node(&mut self.text, json);
    }
}
