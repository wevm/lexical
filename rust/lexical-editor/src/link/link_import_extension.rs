//! `LinkImportExtension.ts`: the `DOMImportExtension` rule for links
//! (`@experimental` upstream), which `LinkExtension` contributes.

use super::link_node::{LinkAttributes, create_link_node};
use crate::html::import::{
    DomImportRule, ImportChildrenOpts, define_import_rule, distribute_inline_wrapper, sel,
};

/// `AnchorRule`: a link around the `<a>`'s inline content. No schema, so
/// block descendants are lifted to siblings at the link's level, each
/// with its own link around its inline content.
fn anchor_rule() -> DomImportRule {
    define_import_rule(Some("@lexical/link/a"), sel::tag(&["a"]), |ctx, el, _| {
        if el.text_content().is_empty() && el.children().is_empty() {
            return Ok(Vec::new());
        }
        let href = el.get_attribute("href").unwrap_or_default();
        let attrs = LinkAttributes {
            rel: Some(el.get_attribute("rel")),
            target: Some(el.get_attribute("target")),
            title: Some(el.get_attribute("title")),
        };
        let children = ctx.import_children(el, &ImportChildrenOpts::default())?;
        distribute_inline_wrapper(&children, &|| create_link_node(&href, &attrs))
    })
}

/// `LinkImportRules` (`@experimental`): the import rule for `LinkNode`,
/// which `LinkExtension` contributes with `CoreImportExtension`.
pub fn link_import_rules() -> Vec<DomImportRule> {
    vec![anchor_rule()]
}
