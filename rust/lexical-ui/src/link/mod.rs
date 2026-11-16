//! `@lexical/link` (the `link` feature), from `packages/lexical-link/src`:
//! `LinkNode` and `AutoLinkNode` with their generated JSON
//! (`LexicalLinkGeneratedJSON.ts`), `$linkNodeTransform`, `$toggleLink`,
//! `TOGGLE_LINK_COMMAND` and `formatUrl` (`LexicalLinkNode.ts`), and
//! `registerLink`'s transform and command handlers, with the paste of a
//! URL `validateUrl` accepts (`LexicalLinkExtension.ts`); with the `autolink` feature,
//! `registerAutoLink` and its matchers (`LexicalAutoLinkExtension.ts`),
//! whose regular expressions need the `regex` crate.
//!
//! With the `extension` feature, [`link_extension`] is `LinkExtension`, and
//! with `autolink` as well, `auto_link_extension` is `AutoLinkExtension`.
//! With `a11y`, `auto_link_announce_extension` is
//! `AutoLinkAnnounceExtension` (`AutoLinkAnnounceExtension.ts`), and with
//! `extension`, [`clickable_link_extension`] is `ClickableLinkExtension`
//! (`ClickableLinkExtension.ts`), which opens a clicked link in an app's
//! [`LinkWindow`].
//!
//! Not ported: the DOM parts (`createDOM`, `updateDOM`, `exportDOM`,
//! `importDOM`, `LinkImportRules`), which come with `html`
//! (rust/PARITY.md); `sanitizeUrl` is [`sanitize_url`].

#[cfg(feature = "a11y")]
mod auto_link_announce_extension;
#[cfg(feature = "autolink")]
mod auto_link_extension;
#[cfg(feature = "extension")]
mod clickable_link_extension;
mod generated_json;
mod link_extension;
#[cfg(all(feature = "html", feature = "extension"))]
mod link_import_extension;
mod link_node;

#[cfg(feature = "autolink")]
pub use auto_link_extension::{
    AutoLinkConfig, ChangeHandler, ExcludeParent, JS_WHITESPACE, LinkMatcher, LinkMatcherResult,
    UrlTransformer, auto_link_email_matcher, auto_link_url_matcher,
    create_link_matcher_with_reg_exp, register_auto_link,
};

#[cfg(feature = "a11y")]
pub use auto_link_announce_extension::{
    AutoLinkAnnounceExtensionConfig, AutoLinkAnnounceExtensionConfigOverrides,
    AutoLinkAnnounceExtensionType, AutoLinkAnnounceOutput, auto_link_announce_extension,
};
#[cfg(all(feature = "autolink", feature = "extension"))]
pub use auto_link_extension::{
    AutoLinkConfigOverrides, AutoLinkExtensionType, auto_link_extension,
};
#[cfg(feature = "extension")]
pub use clickable_link_extension::{
    ClickableLinkConfig, ClickableLinkConfigOverrides, ClickableLinkExtensionType,
    ClickableLinkOutput, GetLinkWindow, GetLinkWindowFn, LinkWindow, clickable_link_extension,
    register_clickable_link,
};
pub use link_extension::{LinkConfig, ValidateUrl, register_link, register_link_paste};
#[cfg(feature = "extension")]
pub use link_extension::{
    LinkConfigOverrides, LinkExtensionType, LinkOutput, ValidateUrlFn, link_extension,
};
#[cfg(all(feature = "html", feature = "extension"))]
pub use link_import_extension::link_import_rules;
pub use link_node::sanitize::sanitize_url;
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
