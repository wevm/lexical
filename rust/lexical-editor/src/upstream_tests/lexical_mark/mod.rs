//! Ports of `packages/lexical-mark`'s unit tests.

mod lexical_mark_node;
#[cfg(all(
    feature = "extension",
    feature = "rich-text",
    feature = "history",
    feature = "clipboard",
    feature = "html"
))]
mod mark_flavors;
mod mark_generated_json;
#[cfg(all(
    feature = "extension",
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "code-highlight",
    feature = "clipboard"
))]
mod mark_state_block_conversion;
#[cfg(all(
    feature = "extension",
    feature = "rich-text",
    feature = "history",
    feature = "clipboard",
    feature = "html",
    feature = "link",
    feature = "hashtag"
))]
mod mark_state_extension;
