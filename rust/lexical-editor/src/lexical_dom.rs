//! The DOM side of the core (the `html` feature): what `createDOM`,
//! `exportDOM` and `importDOM` exchange (`DOMExportOutput`,
//! `DOMConversionMap` and the rest, `LexicalNode.ts` and
//! `LexicalEditor.ts`), the editor's default render config
//! (`DEFAULT_EDITOR_DOM_CONFIG`), the import conversion cache
//! (`initializeConversionCache`), and the DOM helpers of `LexicalUtils.ts`,
//! `utils/classNames.ts` and `nodes/LexicalLineBreakNode.ts` that nodes and
//! `@lexical/html` use. The DOM is [`crate::dom`].
//!
//! `DOMRenderExtension`'s overrides of the render config need the extension
//! API, so an editor's render config is always the default one.

use crate::dom::DomNode;
use crate::editor::NodeClass;
use crate::json::{Object, Value};
use crate::lexical_selection::SelectionRef;
use crate::node::{LexicalNode, NodeRef};
use crate::selection_deletion::is_js_whitespace;
use crate::{Editor, LexicalError};
use std::collections::HashMap;
use std::rc::Rc;

pub use crate::dom::get_document;

/// Runs once a node's DOM has its children; a node it returns replaces the
/// DOM (`DOMExportOutput.after`).
pub type DomExportAfter = Box<dyn FnOnce(&DomNode) -> Result<Option<DomNode>, LexicalError>>;

/// Appends the exported children, as a fragment (`DOMExportOutput.append`).
pub type DomExportAppend = Box<dyn FnOnce(&DomNode) -> Result<(), LexicalError>>;

/// `DOMExportOutput`: the DOM a node exports (an element, a fragment, a
/// text node or nothing), with optional hooks.
#[derive(Default)]
pub struct DomExportOutput {
    pub element: Option<DomNode>,
    /// `after`.
    pub after: Option<DomExportAfter>,
    /// `append`: appends the exported children (a fragment) somewhere other
    /// than `element`.
    pub append: Option<DomExportAppend>,
    /// `$getChildNodes`: the children to export in place of the node's own.
    pub get_child_nodes: Option<Box<dyn FnOnce() -> Vec<NodeRef>>>,
}

impl DomExportOutput {
    /// `{element}`.
    pub fn element(element: DomNode) -> Self {
        Self {
            element: Some(element),
            ..Self::default()
        }
    }
}

/// `DOMChildConversion`: maps the Lexical node made from a descendant
/// (given the one made from its parent) to the node to keep, or `None` to
/// drop it.
pub type DomChildConversion =
    Rc<dyn Fn(&NodeRef, Option<&NodeRef>) -> Result<Option<NodeRef>, LexicalError>>;

/// What `DOMConversionOutput.node` holds: nothing, one node, or an array
/// (whose last node receives the children).
#[derive(Clone, Debug, Default)]
pub enum DomConversionNode {
    #[default]
    None,
    One(NodeRef),
    Many(Vec<NodeRef>),
}

/// Rewrites the Lexical nodes made from a DOM node's children
/// (`DOMConversionOutput.after`).
pub type DomConversionAfter = Box<dyn FnOnce(Vec<NodeRef>) -> Result<Vec<NodeRef>, LexicalError>>;

/// `DOMConversionOutput`: what a conversion makes of a DOM node.
#[derive(Default)]
pub struct DomConversionOutput {
    pub node: DomConversionNode,
    /// `forChild`: applied to each Lexical node made from a descendant.
    pub for_child: Option<DomChildConversion>,
    /// `after`.
    pub after: Option<DomConversionAfter>,
}

impl DomConversionOutput {
    /// `{node}`, or `{node: null}`.
    pub fn node(node: Option<NodeRef>) -> Self {
        Self {
            node: node.map_or(DomConversionNode::None, DomConversionNode::One),
            ..Self::default()
        }
    }
}

/// `DOMConversionFn`.
pub type DomConversionFn =
    Rc<dyn Fn(&DomNode) -> Result<Option<DomConversionOutput>, LexicalError>>;

/// `DOMConversion`: a conversion and its priority (0 to 4); the highest
/// priority wins, and the last registered among equals.
#[derive(Clone)]
pub struct DomConversion {
    pub conversion: DomConversionFn,
    pub priority: u8,
}

/// What a `DOMConversionMap` holds for a node name: a function that offers
/// a conversion for a DOM node, or `None` to decline it.
pub type DomImporter = Rc<dyn Fn(&DomNode) -> Option<DomConversion>>;

/// `DOMConversionMap`: lower-case node names (`p`, `#text`) to importers, in
/// the order the map's keys are written.
pub type DomConversionMap = Vec<(String, DomImporter)>;

/// An importer that always offers `conversion` with `priority`, as most
/// `importDOM` entries (`() => ({conversion, priority})`) do.
pub fn importer(
    priority: u8,
    conversion: impl Fn(&DomNode) -> Result<Option<DomConversionOutput>, LexicalError> + 'static,
) -> DomImporter {
    let conversion: DomConversionFn = Rc::new(conversion);
    Rc::new(move |_| {
        Some(DomConversion {
            conversion: conversion.clone(),
            priority,
        })
    })
}

/// An `exportDOM` override of `html.export`, for a node value and its
/// handle.
pub type ExportOverride =
    Rc<dyn Fn(&Editor, &dyn LexicalNode, &NodeRef) -> Result<DomExportOutput, LexicalError>>;

/// `HTMLConfig`: `exportDOM` overrides by node type (`export`) and import
/// conversions to add to those of the node classes (`import`).
#[derive(Clone, Default)]
pub struct HtmlConfig {
    pub export: Vec<(&'static str, ExportOverride)>,
    pub import: DomConversionMap,
}

/// `initializeConversionCache(nodes, additionalConversions)`: each class's
/// own `importDOM`, then `html.import`, by node name, in that order.
pub(crate) fn initialize_conversion_cache(
    nodes: &[NodeClass],
    additional: &DomConversionMap,
) -> HashMap<String, Vec<DomImporter>> {
    let mut cache: HashMap<String, Vec<DomImporter>> = HashMap::new();
    let mut add = |map: &DomConversionMap| {
        for (key, importer) in map {
            cache.entry(key.clone()).or_default().push(importer.clone());
        }
    };
    for class in nodes {
        if let Some(map) = (class.import_dom)() {
            add(&map);
        }
    }
    add(additional);
    cache
}

/// `getConversionFunction(domNode, editor)` (`@lexical/html`): the
/// conversion with the highest priority among those offered for the node,
/// the last registered among equals.
pub(crate) fn get_conversion_function(
    dom_node: &DomNode,
    editor: &Editor,
) -> Option<DomConversionFn> {
    let name = dom_node.node_name().to_lowercase();
    let mut current: Option<DomConversion> = None;
    for importer in editor.0.html_conversions.get(&name)? {
        if let Some(conversion) = importer(dom_node)
            && current
                .as_ref()
                .is_none_or(|c| c.priority <= conversion.priority)
        {
            current = Some(conversion);
        }
    }
    current.map(|c| c.conversion)
}

// `DEFAULT_EDITOR_DOM_CONFIG`.

/// `$createDOM(node, editor)`: `node.createDOM(editor._config, editor)`.
pub fn create_dom(
    node: &dyn LexicalNode,
    this: &NodeRef,
    editor: &Editor,
) -> Result<DomNode, LexicalError> {
    node.create_dom(this, editor.config(), editor)
}

/// `$exportDOM(node, editor)`: the `html.export` override for the node's
/// type, if there is one, or `node.exportDOM(editor)`.
pub fn export_dom(
    node: &dyn LexicalNode,
    this: &NodeRef,
    editor: &Editor,
) -> Result<DomExportOutput, LexicalError> {
    match editor.0.html_export.get(node.get_type()) {
        Some(export) => export(editor, node, this),
        None => node.export_dom(this, editor),
    }
}

/// `$extractWithChild(node, childNode, selection, destination)`.
pub fn extract_with_child(
    node: &NodeRef,
    child: &NodeRef,
    selection: Option<&SelectionRef>,
    destination: &str,
) -> Result<bool, LexicalError> {
    if !node.is_element() {
        return Ok(false);
    }
    node.extract_with_child(child, selection, destination)
}

/// `$shouldExclude(node)`: an element that leaves itself out of HTML.
pub fn should_exclude(node: &NodeRef) -> bool {
    node.is_element() && node.exclude_from_copy("html")
}

/// `$shouldInclude(node, selection)`: whether the node is in the
/// selection, or true without one.
pub fn should_include(
    node: &NodeRef,
    selection: Option<&SelectionRef>,
) -> Result<bool, LexicalError> {
    match selection {
        Some(selection) => node.is_selected(Some(selection)),
        None => Ok(true),
    }
}

/// `LexicalNode.exportDOM(editor)`: the element `$createDOM` makes. The
/// `ElementNode` and `TextNode` overrides call it.
pub fn export_dom_base(
    node: &dyn LexicalNode,
    this: &NodeRef,
    editor: &Editor,
) -> Result<DomExportOutput, LexicalError> {
    Ok(DomExportOutput::element(create_dom(node, this, editor)?))
}

// DOM helpers.

fn matches_tag(node: &DomNode, tags: &[&str]) -> bool {
    let name = node.node_name().to_ascii_lowercase();
    tags.contains(&name.as_str())
}

const INLINE_TAGS: &[&str] = &[
    "a", "abbr", "acronym", "b", "cite", "code", "del", "em", "i", "ins", "kbd", "label", "mark",
    "output", "q", "ruby", "s", "samp", "span", "strong", "sub", "sup", "time", "u", "tt", "var",
    "#text",
];

const BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "canvas",
    "dd",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "noscript",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "td",
    "tfoot",
    "ul",
    "video",
];

fn is_displayed_inline(node: &DomNode) -> bool {
    node.is_element()
        && node
            .style()
            .get_property_value("display")
            .starts_with("inline")
}

/// `isInlineDomNode(node)`: an element displayed inline, or a text node or
/// an element with an inline tag.
pub fn is_inline_dom_node(node: &DomNode) -> bool {
    is_displayed_inline(node) || matches_tag(node, INLINE_TAGS)
}

/// `isBlockDomNode(node)`: an element with a block tag, not displayed
/// inline.
pub fn is_block_dom_node(node: &DomNode) -> bool {
    !is_displayed_inline(node) && matches_tag(node, BLOCK_TAGS)
}

/// `normalizeClassNames(...classNames)`: the whitespace-separated tokens of
/// the strings, in order.
pub fn normalize_class_names(class_names: &[&str]) -> Vec<String> {
    class_names
        .iter()
        .flat_map(|c| c.split(is_js_whitespace).filter(|s| !s.is_empty()))
        .map(str::to_owned)
        .collect()
}

/// `addClassNamesToElement(element, ...classNames)`.
pub fn add_class_names_to_element(element: &DomNode, class_names: &[&str]) {
    let tokens = normalize_class_names(class_names);
    if !tokens.is_empty() {
        element.class_list().add(&tokens);
    }
}

/// `removeClassNamesFromElement(element, ...classNames)`.
pub fn remove_class_names_from_element(element: &DomNode, class_names: &[&str]) {
    let tokens = normalize_class_names(class_names);
    if !tokens.is_empty() {
        element.class_list().remove(&tokens);
    }
}

/// `getCachedClassNameArray(theme, key)`: the class names a theme entry
/// names, as a string of whitespace-separated names or an array of them.
pub fn get_cached_class_name_array(theme: &Object, key: &str) -> Option<Vec<String>> {
    match theme.get(key)? {
        Value::String(names) => Some(normalize_class_names(&[names])),
        Value::Array(names) => Some(
            names
                .iter()
                .filter_map(|n| n.as_str().map(str::to_owned))
                .collect(),
        ),
        _ => None,
    }
}

/// A theme's `theme[key]` as a string, for `addClassNamesToElement`.
pub fn theme_string<'a>(theme: &'a Object, key: &str) -> Option<&'a str> {
    theme.get(key).and_then(Value::as_str)
}

/// `removeEmptyDOMAttribute(dom, attributeName)`.
pub fn remove_empty_dom_attribute(dom: &DomNode, attribute_name: &str) {
    if dom.get_attribute(attribute_name).as_deref() == Some("") {
        dom.remove_attribute(attribute_name);
    }
}

/// `parseInt(string, 10)`: the leading decimal integer after JS
/// whitespace, or `None` for `NaN`.
pub fn js_parse_int(string: &str) -> Option<f64> {
    let s = string.trim_start_matches(is_js_whitespace);
    let (negative, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let end = digits
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(digits.len());
    if end == 0 {
        return None;
    }
    let value: f64 = digits[..end].parse().ok()?;
    Some(if negative { -value } else { value })
}

/// `Math.round(x)`.
pub fn js_round(x: f64) -> f64 {
    let r = (x + 0.5).floor();
    if r == 0.0 && x < 0.0 { -0.0 } else { r }
}

/// `setNodeIndentFromDOM(elementDom, elementNode)`: the indent from
/// `data-lexical-indent`, or from `padding-inline-start` in steps of 40px.
pub fn set_node_indent_from_dom(
    element_dom: &DomNode,
    element_node: &NodeRef,
) -> Result<(), LexicalError> {
    if let Some(indent_attr) = element_dom.get_attribute("data-lexical-indent")
        && let Some(parsed) = js_parse_int(&indent_attr).filter(|p| p.is_finite() && *p >= 0.0)
    {
        return element_node.set_indent(parsed);
    }
    let padding = element_dom
        .style()
        .get_property_value("padding-inline-start");
    let indent_size = js_parse_int(&padding).filter(|n| *n != 0.0).unwrap_or(0.0);
    element_node.set_indent(js_round(indent_size / 40.0) + 0.0)
}

/// `$setDirectionFromDOM(node, domNode)`: the `dir` attribute, when it is
/// `ltr` or `rtl`.
pub fn set_direction_from_dom(node: &NodeRef, dom_node: &DomNode) {
    use crate::nodes::element::Direction;
    match dom_node.get_attribute("dir").as_deref() {
        Some("ltr") => node.set_direction(Some(Direction::Ltr)),
        Some("rtl") => node.set_direction(Some(Direction::Rtl)),
        _ => {}
    }
}

/// Whether `ElementNode.setFormat` knows an alignment (`type in
/// ELEMENT_TYPE_TO_FORMAT`).
pub fn is_element_format_type(format_type: &str) -> bool {
    !format_type.is_empty()
        && crate::constants::lookup(crate::constants::ELEMENT_TYPE_TO_FORMAT, format_type).is_some()
}

/// `$setFormatFromDOM(node, domNode)`: the alignment of `text-align`, when
/// it is one.
pub fn set_format_from_dom(node: &NodeRef, dom_node: &DomNode) {
    let alignment = dom_node.style().get_property_value("text-align");
    if is_element_format_type(&alignment) {
        node.set_element_format(&alignment);
    }
}

/// `isWhitespaceDomTextNode(node)`: a text node of spaces, tabs and
/// newlines.
fn is_whitespace_dom_text_node(node: &DomNode) -> bool {
    if !node.is_text() {
        return false;
    }
    let text = node.text_content();
    !text.is_empty() && is_whitespace_run(&text)
}

/// `/^( |\t|\r?\n)+$/` without its emptiness check.
fn is_whitespace_run(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\n' => i += 1,
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => i += 2,
            _ => return false,
        }
    }
    true
}

/// `isOnlyChildInBlockNode(node)`: the one child of a block element, but
/// for whitespace text on either side.
pub fn is_only_child_in_block_node(node: &DomNode) -> bool {
    let Some(parent) = node.parent_element().filter(is_block_dom_node) else {
        return false;
    };
    let first = parent.first_child().expect("a parent has children");
    let at_start = &first == node
        || (first.next_sibling().as_ref() == Some(node) && is_whitespace_dom_text_node(&first));
    if !at_start {
        return false;
    }
    let last = parent.last_child().expect("a parent has children");
    &last == node
        || (last.previous_sibling().as_ref() == Some(node) && is_whitespace_dom_text_node(&last))
}

/// `isLastChildInBlockNode(node)`: the last child of a block element (but
/// for whitespace text after it), other than its only one.
pub fn is_last_child_in_block_node(node: &DomNode) -> bool {
    let Some(parent) = node.parent_element().filter(is_block_dom_node) else {
        return false;
    };
    let first = parent.first_child().expect("a parent has children");
    if &first == node
        || (first.next_sibling().as_ref() == Some(node) && is_whitespace_dom_text_node(&first))
    {
        return false;
    }
    let last = parent.last_child().expect("a parent has children");
    &last == node
        || (last.previous_sibling().as_ref() == Some(node) && is_whitespace_dom_text_node(&last))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_int_and_round_as_js() {
        // parseInt(' 12px'), parseInt('-3.9'), parseInt('px'), parseInt('+7')
        assert_eq!(js_parse_int(" 12px"), Some(12.0));
        assert_eq!(js_parse_int("-3.9"), Some(-3.0));
        assert_eq!(js_parse_int("px"), None);
        assert_eq!(js_parse_int("+7"), Some(7.0));
        // Math.round(2.5), Math.round(-2.5), Math.round(-0.4)
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert!(js_round(-0.4) == 0.0 && js_round(-0.4).is_sign_negative());
    }
}
