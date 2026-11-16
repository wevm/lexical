//! Ports of `packages/lexical-link`'s unit tests.

#[cfg(all(feature = "a11y", feature = "autolink", feature = "rich-text"))]
mod auto_link_announce_extension;
#[cfg(feature = "autolink")]
mod auto_link_url_matcher;
#[cfg(all(feature = "extension", feature = "rich-text"))]
mod clickable_link_extension;
#[cfg(all(feature = "autolink", feature = "extension"))]
mod lexical_auto_link_extension;
mod lexical_auto_link_node;
#[cfg(all(feature = "autolink", feature = "extension"))]
mod lexical_auto_link_samples;
mod lexical_link_node;
#[cfg(all(feature = "extension", feature = "rich-text"))]
mod link_extension;
#[cfg(all(feature = "extension", feature = "rich-text"))]
mod link_extension_validate_url;
mod link_generated_json;
#[cfg(all(feature = "html", feature = "extension"))]
mod link_import_extension;
mod link_node_stale_state;
