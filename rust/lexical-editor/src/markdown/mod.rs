//! `@lexical/markdown` (the `markdown` feature), from
//! `packages/lexical-markdown/src`: markdown import
//! (`$convertFromMarkdownString`), export (`$convertToMarkdownString`,
//! `$convertSelectionToMarkdownString`), the transformers
//! (`MarkdownTransformers.ts`) and the markdown shortcuts
//! (`registerMarkdownShortcuts`).
//!
//! Upstream's transformers are JS regular expressions, and custom
//! transformers bring their own; [`RegExp`] is a small JS regular
//! expression engine of this module's, so the feature needs no
//! third-party crate (rust/PARITY.md).
//!
//! Not ported: `$generateNodesFromMarkdownString`, which imports into an
//! `ArtificialNode__DO_NOT_USE` the core doesn't have yet.

mod import_text_format_transformer;
mod import_text_match_transformer;
mod import_text_transformers;
mod markdown_export;
mod markdown_import;
mod markdown_shortcuts;
mod markdown_transformers;
mod reg_exp;
mod reg_exp_tables;
mod utils;

pub use markdown_import::import_markdown_nodes;
pub use markdown_shortcuts::register_markdown_shortcuts;
pub use markdown_transformers::{
    CodeSpanDelimiter, ElementExport, ElementReplace, ElementTransformer, ExportChildren,
    ExportFormat, GetEndIndex, HandleImportAfterStartMatch, MultilineElementTransformer,
    MultilineImportArgs, MultilineImportResult, MultilineReplace, RegExpEnd, TextFormatTransformer,
    TextMatchExport, TextMatchReplace, TextMatchTransformer, Transformer, bold_italic_star,
    bold_italic_underscore, bold_star, bold_underscore, check_list, code, code_fence_state,
    code_meta_state, create_markdown_line_break_node, element_transformers,
    get_code_span_delimiter, get_link_label_opener, hard_line_break_state, heading, highlight,
    inline_code, is_table_row_divider, italic_star, italic_underscore, link, list_marker_state,
    multiline_element_transformers, normalize_markdown, ordered_list,
    parse_markdown_hard_line_break, quote, strikethrough, text_format_transformers,
    text_match_transformers, transformers, unordered_list,
};
pub use reg_exp::{RegExp, RegExpMatch};

use crate::{LexicalError, NodeRef, SelectionRef, get_root, get_selection};

/// `$convertFromMarkdownString(markdown, transformers, node,
/// shouldPreserveNewLines, shouldMergeAdjacentLines)`: replaces the
/// children of `node` (the root by default) with the markdown's nodes, and
/// moves a selection, if there is one, to the start.
/// `should_merge_adjacent_lines` merges adjacent non-empty lines as
/// CommonMark does, unless `should_preserve_new_lines`.
pub fn convert_from_markdown_string(
    markdown: &str,
    transformers: &[Transformer],
    node: Option<&NodeRef>,
    should_preserve_new_lines: bool,
    should_merge_adjacent_lines: bool,
) -> Result<(), LexicalError> {
    let sanitized_markdown = if should_preserve_new_lines {
        markdown.to_owned()
    } else {
        normalize_markdown(markdown, should_merge_adjacent_lines)
    };
    let root = node.cloned().unwrap_or_else(get_root);
    root.clear()?;
    import_markdown_nodes(
        &sanitized_markdown,
        &root,
        transformers,
        should_preserve_new_lines,
    )?;
    if get_selection().is_some() {
        root.select_start()?;
    }
    Ok(())
}

/// `$convertToMarkdownString(transformers, node, shouldPreserveNewLines)`:
/// the markdown of `node`'s children (the root's by default).
pub fn convert_to_markdown_string(
    transformers: &[Transformer],
    node: Option<&NodeRef>,
    should_preserve_new_lines: bool,
) -> Result<String, LexicalError> {
    markdown_export::export_markdown(transformers, should_preserve_new_lines, node)
}

/// `$convertSelectionToMarkdownString(transformers, selection,
/// shouldPreserveNewLines)`: the markdown of the selected content; empty
/// for no selection or a collapsed range.
pub fn convert_selection_to_markdown_string(
    transformers: &[Transformer],
    selection: Option<&SelectionRef>,
    should_preserve_new_lines: bool,
) -> Result<String, LexicalError> {
    let Some(selection) = selection else {
        return Ok(String::new());
    };
    if selection.range().is_some_and(|r| r.is_collapsed()) {
        return Ok(String::new());
    }
    markdown_export::export_selection_markdown(transformers, should_preserve_new_lines, selection)
}
