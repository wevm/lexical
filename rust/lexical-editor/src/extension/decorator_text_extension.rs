//! `DecoratorTextExtension.ts`: `DecoratorTextNode`, an inline decorator
//! with a text format, and `$isDecoratorTextNode`.

use crate::LexicalError;
use crate::constants::{TEXT_TYPE_TO_FORMAT, lookup};
use crate::generated_json::{export_compact_line_break_node, export_line_break_node, num};
use crate::json::Object;
use crate::lexical_utils::set_node_key;
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::node_state::{
    RequiredNodeStateConfig, StateConfig, create_state, get_state, get_state_direct, set_state,
};
use crate::nodes::text::{to_int32, toggle_text_format_type};

thread_local! {
    static FORMAT_STATE: StateConfig<f64> =
        // `numberValue()`: a finite number (or a string holding a JSON
        // number), else 0.
        create_state("format", |v| num(Some(v), 0.0));
}

/// `formatState`: the text format of a `DecoratorTextNode`, a flat
/// NodeState (serialized as `format` beside its other properties), 0 by
/// default.
pub fn decorator_text_format_state() -> StateConfig<f64> {
    FORMAT_STATE.with(Clone::clone)
}

/// `DecoratorTextNode`: an inline decorator that formatting the selection
/// formats, as it does text (`InlineFormattableNode`).
#[derive(Clone, Debug, Default)]
pub struct DecoratorTextNode {
    base: NodeBase,
}

/// A new `DecoratorTextNode` (`$create(DecoratorTextNode)`; JS exports no
/// `$create` function for it).
pub fn create_decorator_text_node() -> NodeRef {
    set_node_key(Box::new(DecoratorTextNode::default()))
}

/// `$isDecoratorTextNode(node)`.
pub fn is_decorator_text_node(node: &NodeRef) -> bool {
    node.get_latest().is::<DecoratorTextNode>()
}

impl DecoratorTextNode {
    /// `getFormat()` of the latest version of `node`.
    pub fn get_format(node: &NodeRef) -> Result<f64, LexicalError> {
        get_state(node, &decorator_text_format_state())
    }

    /// `hasFormat(type)`.
    pub fn has_format(node: &NodeRef, format_type: &str) -> Result<bool, LexicalError> {
        let flag = lookup(TEXT_TYPE_TO_FORMAT, format_type).unwrap_or(0);
        Ok((to_int32(Self::get_format(node)?) & flag) != 0)
    }

    /// `setFormat(format)`.
    pub fn set_format(node: &NodeRef, format: f64) -> Result<(), LexicalError> {
        set_state(node, &decorator_text_format_state(), format).map(|_| ())
    }

    /// `toggleFormat(type)`.
    pub fn toggle_format(node: &NodeRef, format_type: &str) -> Result<(), LexicalError> {
        let format = Self::get_format(node)?;
        Self::set_format(
            node,
            f64::from(toggle_text_format_type(to_int32(format), format_type, None)),
        )
    }
}

impl LexicalNode for DecoratorTextNode {
    fn get_type(&self) -> &'static str {
        "decorator-text"
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
    /// `DecoratorTextNode.isInline()`.
    fn is_inline(&self) -> bool {
        true
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![RequiredNodeStateConfig::new(
            &decorator_text_format_state(),
            true,
        )]
    }
    /// `__isInlineFormattable`.
    fn is_inline_formattable(&self) -> bool {
        true
    }
    fn get_inline_format(&self) -> f64 {
        get_state_direct(self, &decorator_text_format_state()).unwrap_or(0.0)
    }
    fn set_inline_format(&self, this: &NodeRef, format: f64) -> Result<(), LexicalError> {
        Self::set_format(this, format)
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

thread_local! {
    static DECORATOR_TEXT_EXTENSION: std::rc::Rc<crate::extension_core::LexicalExtension> =
        crate::extension_core::define_extension(crate::extension_core::LexicalExtension {
            nodes: vec![crate::NodeClass::of::<DecoratorTextNode>()],
            ..crate::extension_core::LexicalExtension::new("@lexical/extension/DecoratorText", ())
        });
}

/// `DecoratorTextExtension`: registers `DecoratorTextNode`.
pub fn decorator_text_extension() -> std::rc::Rc<crate::extension_core::LexicalExtension> {
    DECORATOR_TEXT_EXTENSION.with(std::rc::Rc::clone)
}
