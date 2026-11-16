//! Ports of `packages/lexical-code-prism`'s unit tests. Prism's grammars
//! aren't ported (rust/PARITY.md), so the highlighter runs over a tokenizer
//! of the tests' own (`support`).

mod code_highlighter_prism_retain_selection;
mod code_prism_leave_code_node_state;
mod code_prism_null_default_language;
mod code_prism_retain_selection;
mod code_prism_unknown_language;
mod lexical_code_node_tabs;
mod support;
