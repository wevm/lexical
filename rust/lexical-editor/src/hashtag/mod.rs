//! `@lexical/hashtag` (the `hashtag` feature), from
//! `packages/lexical-hashtag/src`: `HashtagNode`, the text entity a
//! `#hashtag` becomes, `$createHashtagNode`, `$isHashtagNode`, the default
//! matcher ([`get_hashtag_match`]) and [`register_lexical_hashtag`], which
//! makes hashtags of the text it matches with
//! [`register_lexical_text_entity`](crate::text::register_lexical_text_entity).
//! Its JSON is `TextNode`'s.
//!
//! The matcher does what upstream's regular expression does without the
//! `regex` crate: its character classes are tables generated from the JS
//! source (`tables.rs`), over UTF-16 code units as JS's regular expression
//! (`i`, no `u`) sees them.
//!
//! Not ported: `createDOM`, which comes with `html`, and `HashtagExtension`,
//! which needs the extension API (its `nodes` are [`hashtag_nodes`], its
//! `register` is [`register_lexical_hashtag`]).

mod lexical_hashtag_extension;
mod lexical_hashtag_node;
// Generated; regenerated rather than formatted.
#[rustfmt::skip]
mod tables;

pub use lexical_hashtag_extension::{
    GetHashtagMatch, HashtagConfig, get_hashtag_match, register_lexical_hashtag,
};
pub use lexical_hashtag_node::{HashtagNode, create_hashtag_node, hashtag_nodes, is_hashtag_node};
