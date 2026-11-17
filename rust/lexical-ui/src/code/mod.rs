//! `@lexical/code-core` (the `code` feature), from
//! `packages/lexical-code-core/src`: `CodeNode` and `CodeHighlightNode`
//! with their generated JSON (`LexicalCodeCoreGeneratedJSON.ts`),
//! `$exitCodeNodeOnEnter` (`CodeNode.ts`), the line functions of
//! `FlatStructureUtils.ts`, `CodeExtension`'s transform ([`register_code`])
//! and `registerCodeIndentation`'s command handlers, Tab, the arrow keys
//! (`escapeWithArrows`), `MOVE_TO_START` and `MOVE_TO_END` among them
//! ([`register_code_indentation`]); with the `extension` feature,
//! `CodeExtension` and `CodeIndentExtension` (`code_extension`,
//! `code_indent_extension`).
//!
//! Not ported: the DOM parts (`createDOM`, `updateDOM`, `exportDOM`,
//! `importDOM`, `CodeImportRules`, the VS Code paste overlay), which come
//! with `html`. `registerCodeGutter` ([`register_code_gutter`]) gives the
//! app the line numbers JS writes to the block's `data-gutter`.
//!
//! With the `code-highlight` feature, the highlighting loop of
//! `@lexical/code-prism` (`CodeHighlighterPrism.ts`, with the lazy languages
//! of `@lexical/code-shiki`) over a [`Tokenizer`] the app supplies:
//! [`register_code_highlighting`], [`register_highlighting_only`] and, with
//! `extension`, `code_highlighter_extension` (`CodePrismExtension`).
//! Prism's grammars and Swift's tree-sitter tokenizer aren't ported.

mod code_extension;
mod code_gutter;
mod code_highlight_node;
#[cfg(feature = "code-highlight")]
mod code_highlighter;
#[cfg(all(feature = "html", feature = "extension"))]
mod code_import_extension;
mod code_indentation;
mod code_node;
#[cfg(feature = "code-highlight")]
mod code_tokenizer;
mod flat_structure_utils;
mod generated_json;

pub use code_extension::register_code;
#[cfg(feature = "extension")]
pub use code_extension::{
    CodeExtensionType, CodeIndentConfig, CodeIndentConfigOverrides, CodeIndentExtensionType,
    CodeIndentOutput, code_extension, code_indent_extension,
};
pub use code_gutter::register_code_gutter;
pub use code_highlight_node::{
    CodeHighlightNode, create_code_highlight_node, is_code_highlight_node,
};
#[cfg(all(feature = "code-highlight", feature = "extension"))]
pub use code_highlighter::{
    CodeHighlighterConfig, CodeHighlighterConfigOverrides, CodeHighlighterExtensionType,
    CodeHighlighterOutput, TokenizerRef, code_highlighter_extension,
};
#[cfg(feature = "code-highlight")]
pub use code_highlighter::{register_code_highlighting, register_highlighting_only};
#[cfg(all(feature = "html", feature = "extension"))]
pub use code_import_extension::{code_import_rules, install_vscode_code_paste_overlay};
pub use code_indentation::register_code_indentation;
pub use code_node::{
    CodeNode, DEFAULT_CODE_LANGUAGE, create_code_node, exit_code_node_on_enter,
    get_default_code_language, is_code_node,
};
#[cfg(feature = "code-highlight")]
pub use code_tokenizer::{
    CodeLanguageState, CodeToken, PlainTokenizer, Token, TokenContent, Tokenizer,
    map_tokens_to_lexical_structure,
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
