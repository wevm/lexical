//! `nodes/LexicalLineBreakNode.ts`.

use crate::generated_json::{export_compact_line_break_node, export_line_break_node};
use crate::json::Object;
use crate::node::{LexicalNode, NodeBase, NodeRef};

/// `$createLineBreakNode()`.
pub fn create_line_break_node() -> NodeRef {
    crate::lexical_utils::set_node_key(Box::new(LineBreakNode::default()))
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

    /// `LineBreakNode.createDOM()`: a `<br>`.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, crate::LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("br"))
    }

    /// `LineBreakNode`'s `importDOM`: a `<br>`, but for the one that only
    /// ends (or is all of) a block, which a browser doesn't render as a
    /// line.
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        use crate::lexical_dom::{
            DomConversion, DomConversionOutput, is_last_child_in_block_node,
            is_only_child_in_block_node,
        };
        let conversion: crate::lexical_dom::DomConversionFn = std::rc::Rc::new(|_| {
            Ok(Some(DomConversionOutput::node(Some(
                create_line_break_node(),
            ))))
        });
        Some(vec![(
            "br".to_owned(),
            std::rc::Rc::new(move |node: &crate::dom::DomNode| {
                if is_only_child_in_block_node(node) || is_last_child_in_block_node(node) {
                    return None;
                }
                Some(DomConversion {
                    conversion: conversion.clone(),
                    priority: 0,
                })
            }),
        )])
    }
}
