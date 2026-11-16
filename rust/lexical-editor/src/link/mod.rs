//! `@lexical/link` (the `link` feature), from `packages/lexical-link/src`:
//! `LinkNode` and `AutoLinkNode` with their generated JSON
//! (`LexicalLinkGeneratedJSON.ts`), `$linkNodeTransform`, `$toggleLink`,
//! `TOGGLE_LINK_COMMAND` and `formatUrl` (`LexicalLinkNode.ts`), and
//! `registerLink`'s transform and command handler
//! (`LexicalLinkExtension.ts`); with the `autolink` feature,
//! `registerAutoLink` and its matchers (`LexicalAutoLinkExtension.ts`),
//! whose regular expressions need the `regex` crate.
//!
//! Not ported: the DOM parts (`createDOM`, `updateDOM`, `exportDOM`,
//! `importDOM`, `sanitizeUrl`, `LinkImportRules`), which come with `html`;
//! `LinkExtension` and `AutoLinkExtension`, which need the extension API;
//! `registerLink`'s paste handler, which reads clipboard data; and
//! `ClickableLinkExtension` and `AutoLinkAnnounceExtension`, which need a
//! DOM (rust/PARITY.md).

#[cfg(feature = "autolink")]
mod auto_link_extension;
mod generated_json;
mod link_extension;
mod link_node;

#[cfg(feature = "autolink")]
pub use auto_link_extension::{
    AutoLinkConfig, ChangeHandler, ExcludeParent, JS_WHITESPACE, LinkMatcher, LinkMatcherResult,
    UrlTransformer, auto_link_email_matcher, auto_link_url_matcher,
    create_link_matcher_with_reg_exp, register_auto_link,
};

pub use link_extension::{LinkConfig, register_link};
pub use link_node::{
    AutoLinkAttributes, AutoLinkNode, LinkAttributes, LinkNode, TOGGLE_LINK_COMMAND,
    ToggleLinkPayload, create_auto_link_node, create_link_node, format_url, is_auto_link_node,
    is_link_node, link_node_transform, toggle_link,
};

use crate::NodeClass;

/// The node classes of this package: `LinkNode` (`LinkExtension`'s `nodes`)
/// and `AutoLinkNode` (`AutoLinkExtension`'s).
pub fn link_nodes() -> Vec<NodeClass> {
    vec![NodeClass::of::<LinkNode>(), NodeClass::of::<AutoLinkNode>()]
}
