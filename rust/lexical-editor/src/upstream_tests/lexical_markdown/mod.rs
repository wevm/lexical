//! Ports of `packages/lexical-markdown`'s unit tests.

mod code_block_fence_indent;
mod code_block_info_string;
mod code_block_leading_blank_line;
mod escaped_backslash_hard_line_break;
mod issue5366_repro;
#[allow(clippy::module_inception)]
mod lexical_markdown;
mod markdown_escaping;
mod markdown_transformers;
mod retained_text_match_import;
mod support;
