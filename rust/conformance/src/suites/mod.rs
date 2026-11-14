//! One suite per vector file the port replays. A file with no suite is
//! reported as not implemented.

mod css;
mod devtools;
mod entities;
mod file;
mod html;
mod markdown;
mod meta;
mod property;
mod serialization;

use crate::Suite;

pub fn all() -> Vec<Suite> {
    vec![
        Suite {
            file: "meta",
            variant: None,
            check: meta::check,
        },
        Suite {
            file: "css/properties",
            variant: None,
            check: css::check_property,
        },
        Suite {
            file: "css/values",
            variant: None,
            check: css::check_value,
        },
        Suite {
            file: "html/entities",
            variant: None,
            check: html::check_entity,
        },
        Suite {
            file: "html/export",
            variant: None,
            check: html::check_export,
        },
        Suite {
            file: "html/export-render",
            variant: None,
            check: html::check_export_render,
        },
        Suite {
            file: "html/import",
            variant: None,
            check: html::check_import,
        },
        Suite {
            file: "html/import-nodes",
            variant: None,
            check: html::check_import_nodes,
        },
        Suite {
            file: "html/import-extension",
            variant: None,
            check: html::check_import_extension,
        },
        Suite {
            file: "html/parse",
            variant: None,
            check: html::check_parse,
        },
        Suite {
            file: "devtools/tree",
            variant: None,
            check: devtools::check_tree,
        },
        Suite {
            file: "file/export",
            variant: None,
            check: file::check_export,
        },
        Suite {
            file: "file/import",
            variant: None,
            check: file::check_import,
        },
        Suite {
            file: "entities/autolink-match",
            variant: None,
            check: entities::check_autolink_match,
        },
        Suite {
            file: "entities/hashtag-match",
            variant: None,
            check: entities::check_hashtag_match,
        },
        Suite {
            file: "entities/hashtag-documents",
            variant: None,
            check: entities::check_hashtag_documents,
        },
        Suite {
            file: "entities/hashtag-typing",
            variant: None,
            check: entities::check_hashtag_typing,
        },
        Suite {
            file: "serialization/code",
            variant: None,
            check: serialization::check,
        },
        Suite {
            file: "serialization/decorator-block",
            variant: None,
            check: serialization::check,
        },
        Suite {
            file: "serialization/core",
            variant: None,
            check: serialization::check,
        },
        Suite {
            file: "serialization/link",
            variant: None,
            check: serialization::check,
        },
        Suite {
            file: "serialization/list",
            variant: None,
            check: serialization::check,
        },
        Suite {
            file: "serialization/mark",
            variant: None,
            check: serialization::check,
        },
        Suite {
            file: "serialization/overflow",
            variant: None,
            check: serialization::check,
        },
        Suite {
            file: "scripts/character-limit",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/table-of-contents",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/updates",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/transforms",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/listeners",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/code",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/commands",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/selection",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/caret",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/history",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/rich-text",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/rich-text-commands",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/overflow",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/mark",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/node-state",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/decorator",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/extensions",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/clipboard",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/package-extensions",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/plain-text",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/list",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/link",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/autolink",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/markdown-shortcuts",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/selection-utils",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/set-blocks-type",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/text",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/slots",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/node-structure",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/block-id",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/a11y",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/decorator-block",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/menu",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/auto-embed",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "scripts/code-highlight",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "fuzz/core",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "markdown/import-export",
            variant: None,
            check: markdown::check,
        },
        Suite {
            file: "property/documents",
            variant: Some("JSON, text, Markdown and HTML"),
            check: property::check_document,
        },
        Suite {
            file: "property/sequences",
            variant: None,
            check: property::check_sequence,
        },
    ]
}

/// The upstream package that registers each node type in the vectors, by
/// its directory under `packages/`, for cases that need one not ported yet.
pub fn package_of(node_type: &str) -> Option<&'static str> {
    Some(match node_type {
        "root"
        | "paragraph"
        | "text"
        | "linebreak"
        | "tab"
        | "heading"
        | "quote"
        | "list"
        | "listitem"
        | "link"
        | "autolink"
        | "code"
        | "code-highlight"
        | "overflow"
        | "hashtag"
        | "mark"
        | "test-decorator-block" => {
            return None;
        }
        _ => "unknown",
    })
}
