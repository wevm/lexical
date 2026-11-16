//! `@lexical/selection` (the `selection` feature): what `index.ts` exports
//! from `lexical-node.ts`, `range-selection.ts` and the model part of
//! `utils.ts`, and what it re-exports from `lexical` (deprecated upstream).
//!
//! The computed styles (`$getComputedStyleForElement`,
//! `$getComputedStyleForParent`, `$isParentRTL`) come from the editor's
//! root element ([`crate::RootElement::computed_style`]).
//!
//! Of `createRectsFromDOMRange`, the filtering of a range's rects is
//! [`filter_selection_rects`]; measuring them is the platform's.
//!
//! Not ported: the DOM range (`createDOMRange`), which needs a DOM;
//! `trimTextContentFromAnchor`, JS's other name for
//! [`trim_text_content_from_anchor`].

mod lexical_node;
mod range_selection;
mod utils;

#[allow(deprecated)]
pub use lexical_node::add_node_style;
pub use lexical_node::{
    StylePatchFn, StylePatchTarget, StylePatchValue, ensure_forward_range_selection,
    for_each_selected_text_node, is_at_node_end, patch_style_text,
    slice_selected_text_node_content, trim_text_content_from_anchor,
};
pub use range_selection::{
    AfterCreateElement, copy_block_format_indent, get_selection_style_value_for_property,
    is_at_edge_of_element, is_parent_element_rtl, move_caret_selection, move_character,
    set_blocks_type, should_override_default_character_selection, wrap_nodes,
};
pub use utils::{
    ComputedStyle, SelectionRect, filter_selection_rects, get_computed_style_for_element,
    get_computed_style_for_parent, get_css_from_style_object, get_style_object_from_raw_css,
    is_parent_rtl,
};

/// `$cloneWithProperties`, which moved to `lexical`.
pub use crate::lexical_utils::clone_with_properties;
/// `$selectAll`, which moved to `lexical`.
pub use crate::selection_insert_nodes::{select_all, select_all_in};
/// `getStyleObjectFromCSS`, which moved to `lexical`.
pub use crate::set_dom_style::{StyleObject, get_style_object_from_css};
