//! Ports of `packages/lexical-list`'s unit tests.

#[cfg(feature = "rich-text")]
mod format_list;
mod insert_list_keeps_list_state;
mod issue7032_repro;
mod lexical_format_list_slots;
#[cfg(feature = "html")]
mod lexical_list_item_node;
mod lexical_list_node;
#[cfg(feature = "rich-text")]
mod list_extension;
#[cfg(feature = "html")]
mod list_import_extension;
#[cfg(feature = "rich-text")]
mod list_nested_item_structure;
mod list_splice_keeps_formatting;
mod register_list_backspace_decorator;
#[cfg(feature = "html")]
mod register_list_strict_indent_transform;
mod remove_list_keeps_item_state;
mod remove_list_keeps_selection;
mod support;
mod utils;
