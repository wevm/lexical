//! `@lexical/text` (the `text` feature), from `packages/lexical-text/src`:
//! [`register_lexical_text_entity`], the transforms that make text entities
//! (such as hashtags) of the text a matcher finds, and turn them back into
//! text when an edit leaves them no match.
//!
//! Not ported: the rest of the package (`$canShowPlaceholder`,
//! `$isRootTextContentEmpty`, `$rootTextContent`,
//! `$findTextIntersectionFromCharacters`), which no vector exercises yet
//! (rust/PARITY.md).

mod register_lexical_text_entity;

pub use register_lexical_text_entity::{EntityMatch, register_lexical_text_entity};
