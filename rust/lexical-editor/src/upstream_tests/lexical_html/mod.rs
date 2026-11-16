//! Ports of `packages/lexical-html`'s unit tests.

mod block_id_docs;
mod block_id_html_extension;
mod block_id_html_wrapper;
mod core_import_extension;
mod dom_import_extension;
mod dom_preprocess;
mod dom_render_conditional_overrides;
mod dom_render_extension;
mod export_after_fragment;
mod horizontal_rule_import_extension;
mod issue8391_whitespace_around_unknown_inline;
mod issue8915_repro;
#[allow(clippy::module_inception)]
mod lexical_html;
mod lexical_html_backward_compat;
mod line_break_round_trip;
mod slot_node_selection_export;
mod support;
