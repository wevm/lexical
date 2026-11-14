//! `@lexical/extension`'s decorators (the `extension` feature), from
//! `packages/lexical-extension/src`: `HorizontalRuleNode`, a block
//! decorator (`HorizontalRuleExtension.ts`), and `DecoratorTextNode`, an
//! inline one that carries a text format (`DecoratorTextExtension.ts`),
//! with their `$create`/`$is` functions. Neither has a schema of its own,
//! so their JSON is `LexicalNode`'s, with `DecoratorTextNode`'s `format` as
//! a flat NodeState. Also `NormalizeInlineElementsExtension`'s register
//! ([`register_normalize_inline_elements`]), which `RichTextExtension`
//! depends on.
//!
//! Not ported: their `createDOM`, `updateDOM`, `exportDOM` and `importDOM`,
//! which come with `html`; `applyFormatFromStyle`, which reads a CSS style
//! declaration; and the extensions themselves, which need the extension
//! API: `HorizontalRuleExtension`'s `INSERT_HORIZONTAL_RULE_COMMAND`, click
//! and selected-class handling, and `DecoratorTextExtension` (their `nodes`
//! are [`extension_nodes`]). The rest of `@lexical/extension` is the
//! extension API itself (rust/PARITY.md).

mod decorator_text_extension;
mod horizontal_rule_extension;
mod normalize_inline_elements_extension;

pub use decorator_text_extension::{
    DecoratorTextNode, create_decorator_text_node, decorator_text_format_state,
    is_decorator_text_node,
};
pub use horizontal_rule_extension::{
    HorizontalRuleNode, create_horizontal_rule_node, is_horizontal_rule_node,
};
pub use normalize_inline_elements_extension::register_normalize_inline_elements;

use crate::NodeClass;

/// The decorator node classes of this package, the `nodes` of
/// `HorizontalRuleExtension` and `DecoratorTextExtension`:
/// `HorizontalRuleNode` and `DecoratorTextNode`.
pub fn extension_nodes() -> Vec<NodeClass> {
    vec![
        NodeClass::of::<HorizontalRuleNode>(),
        NodeClass::of::<DecoratorTextNode>(),
    ]
}
