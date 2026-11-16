//! Ports of `packages/lexical-selection`'s unit tests.

mod lexical_computed_style_slots;
mod lexical_is_at_edge_of_element;
// Named after its upstream file, LexicalSelection.test.tsx.
#[allow(clippy::module_inception)]
mod lexical_selection;
mod lexical_selection_helpers;
mod lexical_set_blocks_type_slots;
mod patch_style_detached_selection;
mod slice_selected_text_node_content;
mod support;
mod wrap_nodes_block_leaf;
mod wrap_nodes_inline_content;
