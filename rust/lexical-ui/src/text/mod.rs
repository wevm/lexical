//! `@lexical/text` (the `text` feature), from `packages/lexical-text/src`:
//! [`register_lexical_text_entity`], the transforms that make text entities
//! (such as hashtags) of the text a matcher finds, and turn them back into
//! text when an edit leaves them no match; and the root helpers
//! ([`root_text_content`], [`is_root_text_content_empty`],
//! [`can_show_placeholder`], [`find_text_intersection_from_characters`]).

mod can_show_placeholder;
mod find_text_intersection_from_characters;
mod is_root_text_content_empty;
mod register_lexical_text_entity;
mod root_text_content;

use crate::NodeRef;

pub use can_show_placeholder::{can_show_placeholder, can_show_placeholder_curry};
pub use find_text_intersection_from_characters::find_text_intersection_from_characters;
pub use is_root_text_content_empty::{
    is_root_text_content_empty, is_root_text_content_empty_curry,
};
pub use register_lexical_text_entity::{EntityMatch, register_lexical_text_entity};
pub use root_text_content::root_text_content;

/// `TextNodeWithOffset`: a text node and a UTF-16 offset in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextNodeWithOffset {
    pub node: NodeRef,
    pub offset: usize,
}
