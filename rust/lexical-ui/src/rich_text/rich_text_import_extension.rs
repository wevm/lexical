//! `RichTextImportExtension.ts`: the `DOMImportExtension` rules for
//! headings and quotes (`@experimental` upstream), which
//! `RichTextExtension` contributes.

use super::{HeadingTagType, create_heading_node, create_quote_node};
use crate::dom::DomNode;
use crate::html::import::{
    DomImportRule, ImportChildrenOpts, block_schema, define_import_rule, sel,
};
use crate::lexical_dom::{set_direction_from_dom, set_format_from_dom, set_node_indent_from_dom};

/// `isGoogleDocsTitleSpan(node)`: Google Docs writes the document title as
/// a `<span style="font-size: 26pt">`, which is taken for a heading.
fn is_google_docs_title_span(node: &DomNode) -> bool {
    node.is_element()
        && node.node_name() == "SPAN"
        && node.style().get_property_value("font-size") == "26pt"
}

/// `HeadingRule`.
fn heading_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/rich-text/heading"),
        sel::tag(&["h1", "h2", "h3", "h4", "h5", "h6"]),
        |ctx, el, _| {
            let tag = HeadingTagType::parse(&el.node_name().to_lowercase()).unwrap_or_default();
            let node = create_heading_node(tag);
            set_node_indent_from_dom(el, &node)?;
            set_format_from_dom(&node, el);
            set_direction_from_dom(&node, el);
            node.splice(
                0,
                0,
                &ctx.import_children(el, &ImportChildrenOpts::default())?,
            )?;
            Ok(vec![node])
        },
    )
}

/// `QuoteRule`.
fn quote_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/rich-text/blockquote"),
        sel::tag(&["blockquote"]),
        |ctx, el, _| {
            let node = create_quote_node(false);
            set_format_from_dom(&node, el);
            set_node_indent_from_dom(el, &node)?;
            set_direction_from_dom(&node, el);
            node.splice(
                0,
                0,
                &ctx.import_children(el, &ImportChildrenOpts::default())?,
            )?;
            Ok(vec![node])
        },
    )
}

/// `ShadowRootQuoteRule` (`@experimental`): an opt-in replacement for the
/// `<blockquote>` rule that imports a shadow root quote
/// ([`quote_shadow_root_state`](super::quote_shadow_root_state)), keeping
/// block children as blocks and wrapping inline runs in paragraphs
/// (`BlockSchema`). Contribute it where it outranks
/// [`rich_text_import_rules`], such as from an extension that depends on
/// `RichTextExtension`.
pub fn shadow_root_quote_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/rich-text/blockquote-shadow-root"),
        sel::tag(&["blockquote"]),
        |ctx, el, _| {
            let node = create_quote_node(true);
            set_format_from_dom(&node, el);
            set_node_indent_from_dom(el, &node)?;
            set_direction_from_dom(&node, el);
            let opts = ImportChildrenOpts {
                schema: Some(block_schema()),
                ..ImportChildrenOpts::default()
            };
            node.splice(0, 0, &ctx.import_children(el, &opts)?)?;
            Ok(vec![node])
        },
    )
}

/// `GoogleDocsTitleParagraphRule`: a `<p>` around a title span gives way
/// to its children, so the span rule makes the heading at this level.
fn google_docs_title_paragraph_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/rich-text/google-docs-title-p"),
        sel::tag(&["p"]),
        |ctx, el, next| {
            if el
                .first_child()
                .is_some_and(|f| is_google_docs_title_span(&f))
            {
                return ctx.import_children(el, &ImportChildrenOpts::default());
            }
            next()
        },
    )
}

/// `GoogleDocsTitleSpanRule`.
fn google_docs_title_span_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/rich-text/google-docs-title-span"),
        sel::tag(&["span"]),
        |ctx, el, next| {
            if el.style().get_property_value("font-size") != "26pt" {
                return next();
            }
            let node = create_heading_node(HeadingTagType::H1);
            node.splice(
                0,
                0,
                &ctx.import_children(el, &ImportChildrenOpts::default())?,
            )?;
            Ok(vec![node])
        },
    )
}

/// `RichTextImportRules` (`@experimental`): the import rules for
/// `HeadingNode` and `QuoteNode`, with the Google Docs title heuristic of
/// the legacy `HeadingNode.importDOM`. `RichTextExtension` contributes them
/// with `CoreImportExtension`, so they are tried before the core `<p>` and
/// `<span>` rules, to which the Google Docs rules defer.
pub fn rich_text_import_rules() -> Vec<DomImportRule> {
    vec![
        heading_rule(),
        quote_rule(),
        google_docs_title_paragraph_rule(),
        google_docs_title_span_rule(),
    ]
}
