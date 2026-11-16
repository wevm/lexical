//! `CodeHighlightNode.ts`: `CodeHighlightNode`, `$createCodeHighlightNode`
//! and `$isCodeHighlightNode`.

use super::code_node::create_code_node;
use super::generated_json::{
    export_code_highlight_node, export_compact_code_highlight_node, update_code_highlight_node,
};
use crate::json::Object;
use crate::json::Value;
use crate::lexical_utils::{set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::text::TextData;
use crate::schema::{
    NodeSchema, node_schema, nullable, optional, schema_getter, schema_setter, string_value,
    with_accessors,
};
use std::rc::Rc;

/// `CodeHighlightNode`: a text node in a [`CodeNode`](super::CodeNode), with
/// the token type a highlighter gave it. It takes no format.
#[derive(Clone, Debug, Default)]
pub struct CodeHighlightNode {
    base: NodeBase,
    pub(super) text: TextData,
    /// `__highlightType`: `None` where JS's is `undefined`. JS's constructor
    /// also keeps a `null` it is given; `setHighlightType` and the JSON
    /// never make one, and here there is none.
    pub(super) highlight_type: Option<String>,
}

impl CodeHighlightNode {
    /// `__highlightType`.
    pub fn highlight_type(&self) -> Option<&str> {
        self.highlight_type.as_deref()
    }
}

/// `$createCodeHighlightNode(text, highlightType)`: the highlight type as
/// given, an empty one included (only `setHighlightType` drops it).
pub fn create_code_highlight_node(text: &str, highlight_type: Option<&str>) -> NodeRef {
    set_node_key(Box::new(CodeHighlightNode {
        base: NodeBase::default(),
        text: TextData {
            text: text.to_owned(),
            ..TextData::default()
        },
        highlight_type: highlight_type.map(str::to_owned),
    }))
}

/// `$isCodeHighlightNode(node)`.
pub fn is_code_highlight_node(node: &NodeRef) -> bool {
    node.get_latest().is::<CodeHighlightNode>()
}

impl NodeRef {
    /// `CodeHighlightNode.getHighlightType()`; `None` when it is
    /// `undefined`, or for any other node.
    pub fn get_highlight_type(&self) -> Option<String> {
        self.get_latest()
            .downcast_ref::<CodeHighlightNode>()
            .and_then(|n| n.highlight_type.clone())
    }

    /// `CodeHighlightNode.setHighlightType(highlightType)`: an empty one is
    /// `undefined`. Does nothing to any other node.
    pub fn set_highlight_type(&self, highlight_type: Option<&str>) {
        if is_code_highlight_node(self) {
            with_writable(self.get_key(), |n| {
                n.downcast_mut::<CodeHighlightNode>()
                    .expect("a code highlight node")
                    .highlight_type = highlight_type.filter(|t| !t.is_empty()).map(str::to_owned);
            });
        }
    }
}

thread_local! {
    static CODE_HIGHLIGHT_NODE_SCHEMA: NodeSchema = node_schema(vec![(
        "highlightType",
        with_accessors(
            optional(
                nullable(string_value(""), false).expect("a nullable string"),
                false,
            )
            .expect("an optional string"),
            Some(schema_getter(|n: &CodeHighlightNode| {
                n.highlight_type
                    .as_deref()
                    .map_or(Value::Undefined, Value::from)
            })),
            Some(schema_setter(|n: &mut CodeHighlightNode, value| {
                n.highlight_type = super::code_node::truthy_value(&value);
            })),
        )
        .expect("a field"),
    )])
    .expect("CodeHighlightNode's schema");
}

/// `codeHighlightNodeSchema`: `highlightType`, read straight off the field
/// and applied through `setHighlightType`, which keeps only a non-empty
/// string.
pub fn code_highlight_node_schema() -> NodeSchema {
    CODE_HIGHLIGHT_NODE_SCHEMA.with(Clone::clone)
}

impl LexicalNode for CodeHighlightNode {
    /// Its own schema, then TextNode's as this class resolves it: it
    /// overrides `setFormat` (a highlight's format is its theme's), so the
    /// `format` the JSON carries is applied through that, which keeps the
    /// node's.
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            code_highlight_node_schema(),
            crate::nodes::text::text_node_schema().with_setter("format", Rc::new(|_, _| {})),
        ]
    }
    fn get_type(&self) -> &'static str {
        "code-highlight"
    }
    /// `CodeHighlightNode.createDOM(config)`: `TextNode`'s, with the
    /// theme's `codeHighlight[highlightType]` class names.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, crate::LexicalError> {
        let element = crate::nodes::text::create_dom(self, config)?;
        if let (Some(highlight_type), Some(crate::json::Value::Object(code_highlight))) = (
            self.highlight_type.as_deref().filter(|t| !t.is_empty()),
            config.theme.get("codeHighlight"),
        ) {
            crate::lexical_dom::add_theme_class_names(&element, code_highlight, highlight_type);
        }
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

    /// `CodeHighlightNode.setFormat(format)`: does nothing (no bold,
    /// underline...), and doesn't make the node writable.
    fn set_format(&self, _: &NodeRef, _: f64) {}

    /// `CodeHighlightNode.canHaveFormat()`.
    fn can_have_format(&self) -> bool {
        false
    }

    /// `CodeHighlightNode.isParentRequired()`.
    fn is_parent_required(&self) -> bool {
        true
    }

    /// `CodeHighlightNode.createParentElementNode()`: a code block with no
    /// language.
    fn create_parent_element_node(&self) -> NodeRef {
        create_code_node(None, None)
    }

    /// `exportCodeHighlightNode` and `exportCompactCodeHighlightNode`.
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            export_compact_code_highlight_node(self)
        } else {
            export_code_highlight_node(self)
        })
    }

    /// `updateCodeHighlightNode`.
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_code_highlight_node(self, json);
        true
    }
}
