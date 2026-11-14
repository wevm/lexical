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
    /// `DecoratorTextNode.createDOM()`: a `<span>`.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("span"))
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

/// `applyFormatFromStyle(lexicalNode, style, shouldApply)` (the `html`
/// feature): gives the decorator text node the formats a Google Docs style
/// implies (bold, strikethrough, italic, underline, subscript,
/// superscript), and `should_apply`, as `TextNode`'s import does.
#[cfg(feature = "html")]
pub fn apply_format_from_style(
    lexical_node: &NodeRef,
    style: &crate::dom::CssStyle,
    should_apply: Option<&str>,
) -> Result<(), LexicalError> {
    let font_weight = style.get_property_value("font-weight");
    let text_decoration = style.get_property_value("text-decoration");
    let text_decoration: Vec<&str> = text_decoration.split(' ').collect();
    let vertical_align = style.get_property_value("vertical-align");
    let mut formats = Vec::new();
    if font_weight == "700" || font_weight == "bold" {
        formats.push("bold");
    }
    if text_decoration.contains(&"line-through") {
        formats.push("strikethrough");
    }
    if style.get_property_value("font-style") == "italic" {
        formats.push("italic");
    }
    if text_decoration.contains(&"underline") {
        formats.push("underline");
    }
    if vertical_align == "sub" {
        formats.push("subscript");
    }
    if vertical_align == "super" {
        formats.push("superscript");
    }
    formats.extend(should_apply);
    for format in formats {
        if !DecoratorTextNode::has_format(lexical_node, format)? {
            DecoratorTextNode::toggle_format(lexical_node, format)?;
        }
    }
    Ok(())
}

/// `DEFAULT_TAG_NAME_TO_FORMAT`.
#[cfg(feature = "html")]
pub const DEFAULT_TAG_NAME_TO_FORMAT: &[(&str, &str)] = &[
    ("b", "bold"),
    ("code", "code"),
    ("em", "italic"),
    ("i", "italic"),
    ("mark", "highlight"),
    ("s", "strikethrough"),
    ("strong", "bold"),
    ("sub", "subscript"),
    ("sup", "superscript"),
    ("u", "underline"),
];

/// `applyFormatToDom(lexicalNode, domNode, tagNameToFormat)` (the `html`
/// feature): `dom_node` inside an element of each tag whose format the
/// node has, in the table's order.
#[cfg(feature = "html")]
pub fn apply_format_to_dom(
    lexical_node: &NodeRef,
    dom_node: &crate::dom::DomNode,
    tag_name_to_format: &[(&str, &str)],
) -> Result<crate::dom::DomNode, LexicalError> {
    let mut rval = dom_node.clone();
    for (tag, format) in tag_name_to_format {
        if DecoratorTextNode::has_format(lexical_node, format)? {
            let el = rval
                .owner_document()
                .unwrap_or_else(crate::lexical_dom::get_document)
                .create_element(tag);
            el.append_child(&rval);
            rval = el;
        }
    }
    Ok(rval)
}
