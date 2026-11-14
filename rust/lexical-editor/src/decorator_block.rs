//! `@lexical/react`'s `DecoratorBlockNode` (the `decorator-block` feature),
//! from `packages/lexical-react/src/LexicalDecoratorBlockNode.ts`: the base
//! of block embeds (videos, tweets), a block decorator that keeps an element
//! alignment (`format`) and is not indentable.
//!
//! JS's class is abstract, and an embed extends it. Here an embed is a node
//! whose [`LexicalNode::decorator_block`] returns its [`DecoratorBlockData`],
//! as a node that extends `ElementNode` returns its `ElementData`: the
//! trait's defaults then make it a decorator ([`LexicalNode::is_decorator`])
//! that is not inline and can't be indented, and [`export_decorator_block`]
//! and [`update_decorator_block_from_json`] are its schema's JSON
//! (`decoratorBlockNodeSchema`).
//!
//! Not ported: `createDOM` and `updateDOM`, which come with `html`, and
//! `BlockWithAlignableContents`, a React component (rust/PARITY.md).

use crate::json::{Object, Value};
use crate::lexical_utils::with_writable;
use crate::node::{LexicalNode, NodeRef};

/// The formats `decoratorBlockNodeSchema`'s `enumValue` takes, `''` (the
/// default) first.
const FORMATS: [&str; 7] = ["", "left", "start", "center", "right", "end", "justify"];

/// The fields `DecoratorBlockNode` adds: `__format`, an
/// `ElementFormatType`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecoratorBlockData {
    format: &'static str,
}

impl DecoratorBlockData {
    /// `new DecoratorBlockNode(format)`'s fields: `format`, or `''` when it
    /// names no format.
    pub fn new(format: &str) -> Self {
        DecoratorBlockData {
            format: parse_format(format),
        }
    }

    /// `__format`.
    pub fn format(&self) -> &'static str {
        self.format
    }
}

/// `enumValue([...])`'s parse: the format, or `''` for anything else.
fn parse_format(format: &str) -> &'static str {
    FORMATS.into_iter().find(|f| *f == format).unwrap_or("")
}

/// `exportJSON(compact)` of a node that extends `DecoratorBlockNode` and
/// adds nothing to its JSON: `LexicalNode`'s (`type`, `version`) with
/// `format`, which the compact form leaves out when it is `''`
/// (`GENERATED_DECORATORBLOCK`). An embed that adds properties inserts them
/// into the result.
pub fn export_decorator_block(node: &dyn LexicalNode, compact: bool) -> Object {
    let format = node.decorator_block().expect("a DecoratorBlockNode").format;
    let mut json = Object::new();
    if compact {
        json.insert("type", node.get_type().into());
        if !format.is_empty() {
            json.insert("format", format.into());
        }
    } else {
        json.insert("format", format.into());
        json.insert("type", node.get_type().into());
        json.insert("version", 1.0.into());
    }
    json
}

/// `updateFromJSON(json)`'s part for `DecoratorBlockNode`: `format`, `''`
/// when it is missing or names no format.
pub fn update_decorator_block_from_json(data: &mut DecoratorBlockData, json: &Object) {
    data.format = json
        .get("format")
        .and_then(Value::as_str)
        .map_or("", parse_format);
}

/// `$isDecoratorBlockNode(node)`: whether the node extends
/// `DecoratorBlockNode`.
pub fn is_decorator_block_node(node: &NodeRef) -> bool {
    node.get_latest().decorator_block().is_some()
}

impl NodeRef {
    /// `DecoratorBlockNode.getFormat()`; `None` for any other node.
    pub fn get_decorator_block_format(&self) -> Option<&'static str> {
        self.get_latest().decorator_block().map(|d| d.format)
    }

    /// `DecoratorBlockNode.setFormat(format)`: `''` for a string that names
    /// no format. Does nothing to any other node.
    pub fn set_decorator_block_format(&self, format: &str) {
        if is_decorator_block_node(self) {
            let format = parse_format(format);
            with_writable(self.get_key(), |n| {
                n.decorator_block_mut()
                    .expect("a DecoratorBlockNode")
                    .format = format;
            });
        }
    }
}
