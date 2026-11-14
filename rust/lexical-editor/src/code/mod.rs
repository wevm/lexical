//! `@lexical/code-core` (the `code` feature), from
//! `packages/lexical-code-core/src`: `CodeNode` and `CodeHighlightNode`
//! with their generated JSON (`LexicalCodeCoreGeneratedJSON.ts`),
//! `$exitCodeNodeOnEnter` (`CodeNode.ts`), the line functions of
//! `FlatStructureUtils.ts`, `CodeExtension`'s transform ([`register_code`])
//! and `registerCodeIndentation`'s model command handlers
//! ([`register_code_indentation`]).
//!
//! Not ported: the DOM parts (`createDOM`, `updateDOM`, `exportDOM`,
//! `importDOM`, `CodeImportRules`, the VS Code paste overlay), which come
//! with `html`; `CodeExtension` and `CodeIndentExtension`, which need the
//! extension API; the handlers of key events (Enter, Tab, the arrow keys,
//! Home and End), which wait for an input layer; and `registerCodeGutter`,
//! which needs a DOM (rust/PARITY.md). Highlighting itself
//! (`@lexical/code-prism`, `@lexical/code-shiki`) is not part of this
//! package.

mod code_extension;
mod code_highlight_node;
mod code_indentation;
mod code_node;
mod flat_structure_utils;
mod generated_json;

pub use code_extension::register_code;
pub use code_highlight_node::{
    CodeHighlightNode, create_code_highlight_node, is_code_highlight_node,
};
pub use code_indentation::register_code_indentation;
pub use code_node::{
    CodeNode, DEFAULT_CODE_LANGUAGE, create_code_node, exit_code_node_on_enter,
    get_default_code_language, is_code_node,
};
pub use flat_structure_utils::{
    CodeLinePoint, get_code_line_direction, get_end_of_code_in_line, get_first_code_node_of_line,
    get_last_code_node_of_line, get_start_of_code_in_line, outdent_leading_spaces,
    plainify_code_content,
};

use crate::{NodeClass, Unregister};

/// The node classes of this package (`CodeExtension`'s `nodes`):
/// `CodeNode` and `CodeHighlightNode`.
pub fn code_nodes() -> Vec<NodeClass> {
    vec![
        NodeClass::of::<CodeNode>(),
        NodeClass::of::<CodeHighlightNode>(),
    ]
}

/// `mergeRegister(...)`.
fn merge(unregisters: Vec<Unregister>) -> Unregister {
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}
