//! A small DOM (the `html` feature): what a browser, or jsdom in upstream's
//! unit tests, gives Lexical's `createDOM`, `exportDOM` and HTML import and
//! export. Nothing renders it; it exists so that HTML export and import run
//! the same node methods as JS and write the same bytes, as the Swift
//! port's `DOMDocument` does (`swift/DESIGN.md`, "The DOM is a small model of
//! jsdom's").
//!
//! - [`DomNode`]: `Node` and the interfaces Lexical reads through it
//!   (`Element`, `Text`, `Comment`, `DocumentFragment`, `Document`).
//! - [`CssStyle`] and [`CssStyleDeclaration`]: an element's inline style,
//!   which normalizes values as jsdom's declaration does (colors to
//!   `rgb()`, `0` lengths to `0px`, lower-case keywords), pinned by the
//!   `css/properties` and `css/values` vectors.
//! - [`parse_from_string`]: `DOMParser`, a WHATWG tokenizer and tree
//!   builder pinned against jsdom's by the `html/parse` and `html/entities`
//!   vectors.
//! - [`get_document`]: the one `document` of the thread, as JS has one
//!   global `document`.

mod css_properties;
mod css_style_declaration;
mod css_values;
mod html_entities;
mod html_parser;
mod html_serializer;
mod node;

pub use css_style_declaration::{CssStyle, CssStyleDeclaration};
pub use css_values::{CssPropertyTraits, css_property_names, css_property_traits};
pub use html_parser::{is_legacy_entity_name, named_entity, named_entity_count, parse_from_string};
pub use html_serializer::is_void_html_element;
pub use node::{DomNode, DomNodeType, DomTokenList};

thread_local! {
    static DOCUMENT: DomNode = DomNode::new_document();
}

/// The thread's `document`, which `$getDocument()` returns when no editor
/// root names another.
pub fn get_document() -> DomNode {
    DOCUMENT.with(DomNode::clone)
}
