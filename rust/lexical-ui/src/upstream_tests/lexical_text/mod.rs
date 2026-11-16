//! Ports of `packages/lexical-text`'s unit tests.

mod can_show_placeholder;
mod issue5466_repro;
mod register_lexical_text_entity;
mod register_lexical_text_entity_equivalence;
#[cfg(all(
    feature = "hashtag",
    feature = "list",
    feature = "mark",
    feature = "rich-text"
))]
mod register_lexical_text_entity_join;
#[cfg(feature = "hashtag")]
mod register_lexical_text_entity_lookbehind;
#[cfg(all(feature = "hashtag", feature = "mark", feature = "history"))]
mod register_lexical_text_entity_node_state;
#[cfg(all(feature = "hashtag", feature = "mark"))]
mod register_lexical_text_entity_node_state_property;
mod register_lexical_text_entity_settle;
mod support;
