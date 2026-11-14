//! `packages/lexical-text/src/rootTextContent.ts`.

use crate::get_root;

/// `$rootTextContent()`: the root's text content.
pub fn root_text_content() -> String {
    let root = get_root();
    root.get_text_content()
}
