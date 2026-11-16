//! The HTML fragment serialization algorithm (`innerHTML` and
//! `outerHTML`), escaping as browsers do: attribute values escape `&`, `"`,
//! `<`, `>` and U+00A0; text escapes `&`, `<`, `>` and U+00A0, except inside
//! raw-text elements. (The HTML standard added `<` and `>` in attributes in
//! 2025, which the `html/parse` vectors record.)

use super::node::{DomNode, DomNodeType};

const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];

const RAW_TEXT_ELEMENTS: &[&str] = &[
    "style",
    "script",
    "xmp",
    "iframe",
    "noembed",
    "noframes",
    "plaintext",
];

/// Whether an element is void: it has no end tag and no children in HTML.
pub fn is_void_html_element(local_name: &str) -> bool {
    VOID_ELEMENTS.contains(&local_name)
}

pub(super) fn serialize_html_children(node: &DomNode) -> String {
    let mut out = String::new();
    for child in node.child_nodes() {
        serialize_html(&child, &mut out);
    }
    out
}

pub(super) fn serialize_html(node: &DomNode, out: &mut String) {
    match node.node_type() {
        DomNodeType::Element => {
            let name = node.local_name();
            out.push('<');
            out.push_str(name);
            for (attribute, value) in node.attributes() {
                out.push(' ');
                out.push_str(&attribute);
                out.push_str("=\"");
                escape_html(&value, true, out);
                out.push('"');
            }
            out.push('>');
            if is_void_html_element(name) {
                return;
            }
            for child in node.child_nodes() {
                serialize_html(&child, out);
            }
            out.push_str("</");
            out.push_str(name);
            out.push('>');
        }
        DomNodeType::Text => {
            let raw = node
                .parent_element()
                .is_some_and(|p| RAW_TEXT_ELEMENTS.contains(&p.local_name()));
            if raw {
                out.push_str(&node.data());
            } else {
                escape_html(&node.data(), false, out);
            }
        }
        DomNodeType::Comment => {
            out.push_str("<!--");
            out.push_str(&node.data());
            out.push_str("-->");
        }
        DomNodeType::Document | DomNodeType::DocumentFragment => {
            for child in node.child_nodes() {
                serialize_html(&child, out);
            }
        }
    }
}

fn escape_html(string: &str, attribute: bool, out: &mut String) {
    for c in string.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '\u{A0}' => out.push_str("&nbsp;"),
            '"' if attribute => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}
