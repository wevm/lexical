//! One suite per vector file the port replays. A file with no suite is
//! reported as not implemented.

mod meta;
mod property;
mod scripts;
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
            file: "serialization/core",
            variant: None,
            check: serialization::check,
        },
        Suite {
            file: "scripts/updates",
            variant: None,
            check: scripts::check,
        },
        Suite {
            file: "scripts/transforms",
            variant: None,
            check: scripts::check,
        },
        Suite {
            file: "scripts/listeners",
            variant: None,
            check: scripts::check,
        },
        Suite {
            file: "scripts/commands",
            variant: None,
            check: scripts::check,
        },
        Suite {
            file: "scripts/selection",
            variant: None,
            check: scripts::check,
        },
        Suite {
            file: "scripts/caret",
            variant: None,
            check: scripts::check,
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
        "root" | "paragraph" | "text" | "linebreak" | "tab" => return None,
        "heading" | "quote" => "lexical-rich-text",
        "list" | "listitem" => "lexical-list",
        "link" | "autolink" => "lexical-link",
        "code" | "code-highlight" => "lexical-code-core",
        "overflow" => "lexical-overflow",
        "hashtag" => "lexical-hashtag",
        _ => "unknown",
    })
}
