//! One suite per vector file the port replays. A file with no suite is
//! reported as not implemented.

mod css;
mod entities;
mod html;
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
            file: "html/import",
            variant: None,
            check: html::check_import,
        },
        Suite {
            file: "html/parse",
            variant: None,
            check: html::check_parse,
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
            file: "serialization/overflow",
            variant: None,
            check: serialization::check,
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
            file: "scripts/set-blocks-type",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "fuzz/core",
            variant: None,
            check: crate::scripts::check,
        },
        Suite {
            file: "property/documents",
            variant: Some("JSON and text"),
            check: property::check_document,
        },
    ]
}

/// The upstream package that registers each node type in the vectors, by
/// its directory under `packages/`, for cases that need one not ported yet.
pub fn package_of(node_type: &str) -> Option<&'static str> {
    Some(match node_type {
        "root" | "paragraph" | "text" | "linebreak" | "tab" | "heading" | "quote" | "list"
        | "listitem" | "link" | "autolink" | "code" | "code-highlight" | "overflow" | "hashtag" => {
            return None;
        }
        _ => "unknown",
    })
}
