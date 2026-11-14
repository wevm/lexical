//! `Node`, `Element`, `Text`, `Comment`, `DocumentFragment` and `Document`:
//! the part of the WHATWG DOM that Lexical's `createDOM`, `exportDOM` and
//! HTML import and export use. One handle type, [`DomNode`], stands for every
//! kind of node, as JS code reads them all through `Node`; the methods of a
//! more specific interface answer as an empty value on other nodes.

use super::css_style_declaration::{CssStyle, CssStyleDeclaration};
use super::html_serializer::{serialize_html, serialize_html_children};
use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

/// `Node.nodeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomNodeType {
    Element = 1,
    Text = 3,
    Comment = 8,
    Document = 9,
    DocumentFragment = 11,
}

/// A DOM node: a shared handle, compared by identity, as JS compares node
/// objects. A node holds its children; its parent and document are weak
/// links.
#[derive(Clone)]
pub struct DomNode(Rc<Inner>);

struct Inner {
    node_type: DomNodeType,
    owner_document: Weak<Inner>,
    parent: RefCell<Weak<Inner>>,
    children: RefCell<Vec<DomNode>>,
    /// `CharacterData.data` of a text or comment node.
    data: RefCell<String>,
    element: Option<ElementInner>,
}

/// What an element adds: its tag, its attributes in the order they were
/// set, and its inline style declaration once something reads it.
struct ElementInner {
    local_name: String,
    attributes: RefCell<Vec<(String, String)>>,
    pub(super) style: RefCell<Option<CssStyleDeclaration>>,
}

impl PartialEq for DomNode {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for DomNode {}

impl fmt::Debug for DomNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.node_type() {
            DomNodeType::Element => write!(f, "{}", self.outer_html()),
            DomNodeType::Text => write!(f, "#text {:?}", self.data()),
            DomNodeType::Comment => write!(f, "<!--{}-->", self.data()),
            DomNodeType::Document | DomNodeType::DocumentFragment => {
                write!(f, "{} {}", self.node_name(), self.inner_html())
            }
        }
    }
}

impl DomNode {
    fn new(
        node_type: DomNodeType,
        owner_document: Option<&DomNode>,
        data: String,
        element: Option<ElementInner>,
    ) -> Self {
        Self(Rc::new(Inner {
            node_type,
            owner_document: owner_document.map_or_else(Weak::new, |d| Rc::downgrade(&d.0)),
            parent: RefCell::new(Weak::new()),
            children: RefCell::new(Vec::new()),
            data: RefCell::new(data),
            element,
        }))
    }

    /// `new Document()`: an empty document, which creates nodes. A parsed
    /// HTML document has `documentElement`, `head` and `body`.
    pub fn new_document() -> Self {
        Self::new(DomNodeType::Document, None, String::new(), None)
    }

    /// An element with `local_name` (already lower case), as the parser and
    /// `createElement` make one.
    pub(crate) fn new_element(local_name: String, owner_document: Option<&DomNode>) -> Self {
        Self::new(
            DomNodeType::Element,
            owner_document,
            String::new(),
            Some(ElementInner {
                local_name,
                attributes: RefCell::new(Vec::new()),
                style: RefCell::new(None),
            }),
        )
    }

    /// `new Text(data)`.
    pub fn new_text(data: impl Into<String>, owner_document: Option<&DomNode>) -> Self {
        Self::new(DomNodeType::Text, owner_document, data.into(), None)
    }

    /// `new Comment(data)`.
    pub fn new_comment(data: impl Into<String>, owner_document: Option<&DomNode>) -> Self {
        Self::new(DomNodeType::Comment, owner_document, data.into(), None)
    }

    /// `new DocumentFragment()`.
    pub fn new_document_fragment(owner_document: Option<&DomNode>) -> Self {
        Self::new(
            DomNodeType::DocumentFragment,
            owner_document,
            String::new(),
            None,
        )
    }

    /// An identity for sets of nodes.
    pub(crate) fn identity(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }

    pub fn node_type(&self) -> DomNodeType {
        self.0.node_type
    }

    /// `nodeName`: the upper-case tag name of an element, `#text`,
    /// `#comment`, `#document` or `#document-fragment`.
    pub fn node_name(&self) -> String {
        match self.node_type() {
            DomNodeType::Element => self.tag_name(),
            DomNodeType::Text => "#text".to_owned(),
            DomNodeType::Comment => "#comment".to_owned(),
            DomNodeType::Document => "#document".to_owned(),
            DomNodeType::DocumentFragment => "#document-fragment".to_owned(),
        }
    }

    pub fn is_element(&self) -> bool {
        self.node_type() == DomNodeType::Element
    }

    pub fn is_text(&self) -> bool {
        self.node_type() == DomNodeType::Text
    }

    pub fn is_document(&self) -> bool {
        self.node_type() == DomNodeType::Document
    }

    pub fn is_document_fragment(&self) -> bool {
        self.node_type() == DomNodeType::DocumentFragment
    }

    /// `ownerDocument`: the document that created the node.
    pub fn owner_document(&self) -> Option<DomNode> {
        self.0.owner_document.upgrade().map(DomNode)
    }

    /// The document new nodes made for this one belong to: its owner, or
    /// itself when it is a document.
    fn document_for_new_nodes(&self) -> Option<DomNode> {
        if self.is_document() {
            Some(self.clone())
        } else {
            self.owner_document()
        }
    }

    pub fn parent_node(&self) -> Option<DomNode> {
        self.0.parent.borrow().upgrade().map(DomNode)
    }

    pub fn parent_element(&self) -> Option<DomNode> {
        self.parent_node().filter(DomNode::is_element)
    }

    /// `childNodes`, as a snapshot.
    pub fn child_nodes(&self) -> Vec<DomNode> {
        self.0.children.borrow().clone()
    }

    pub fn first_child(&self) -> Option<DomNode> {
        self.0.children.borrow().first().cloned()
    }

    pub fn last_child(&self) -> Option<DomNode> {
        self.0.children.borrow().last().cloned()
    }

    pub fn has_child_nodes(&self) -> bool {
        !self.0.children.borrow().is_empty()
    }

    fn index_of(&self, child: &DomNode) -> Option<usize> {
        self.0.children.borrow().iter().position(|c| c == child)
    }

    fn sibling(&self, offset: isize) -> Option<DomNode> {
        let parent = self.parent_node()?;
        let index = parent.index_of(self)?;
        let at = index.checked_add_signed(offset)?;
        parent.0.children.borrow().get(at).cloned()
    }

    pub fn next_sibling(&self) -> Option<DomNode> {
        self.sibling(1)
    }

    pub fn previous_sibling(&self) -> Option<DomNode> {
        self.sibling(-1)
    }

    /// `previousElementSibling`: the nearest element before this node.
    pub fn previous_element_sibling(&self) -> Option<DomNode> {
        let parent = self.parent_node()?;
        let index = parent.index_of(self)?;
        parent.0.children.borrow()[..index]
            .iter()
            .rev()
            .find(|n| n.is_element())
            .cloned()
    }

    /// `nextElementSibling`: the nearest element after this node.
    pub fn next_element_sibling(&self) -> Option<DomNode> {
        let parent = self.parent_node()?;
        let index = parent.index_of(self)?;
        parent.0.children.borrow()[index + 1..]
            .iter()
            .find(|n| n.is_element())
            .cloned()
    }

    /// `children`: the element children.
    pub fn children(&self) -> Vec<DomNode> {
        self.0
            .children
            .borrow()
            .iter()
            .filter(|n| n.is_element())
            .cloned()
            .collect()
    }

    /// `textContent`: a text or comment node's data, or the text of the
    /// descendants.
    pub fn text_content(&self) -> String {
        match self.node_type() {
            DomNodeType::Text | DomNodeType::Comment => self.data(),
            _ => {
                let mut out = String::new();
                self.collect_text(&mut out);
                out
            }
        }
    }

    fn collect_text(&self, out: &mut String) {
        for child in self.0.children.borrow().iter() {
            match child.node_type() {
                DomNodeType::Text => out.push_str(&child.0.data.borrow()),
                DomNodeType::Comment => {}
                _ => child.collect_text(out),
            }
        }
    }

    /// Setting `textContent`: a text or comment node's data, or the
    /// children replaced by one text node (none for empty text).
    pub fn set_text_content(&self, text: &str) {
        match self.node_type() {
            DomNodeType::Text | DomNodeType::Comment => self.set_data(text),
            _ => {
                let nodes = if text.is_empty() {
                    Vec::new()
                } else {
                    vec![DomNode::new_text(
                        text,
                        self.document_for_new_nodes().as_ref(),
                    )]
                };
                self.replace_children(&nodes);
            }
        }
    }

    /// `CharacterData.data` (and `nodeValue`) of a text or comment node.
    pub fn data(&self) -> String {
        self.0.data.borrow().clone()
    }

    pub fn set_data(&self, data: &str) {
        *self.0.data.borrow_mut() = data.to_owned();
    }

    /// Appends to a text node's data, as the parser does.
    pub(crate) fn push_data(&self, data: &str) {
        self.0.data.borrow_mut().push_str(data);
    }

    /// The nodes `node` stands for when inserted: a fragment's children.
    fn expand(node: &DomNode) -> Vec<DomNode> {
        if node.is_document_fragment() {
            node.child_nodes()
        } else {
            vec![node.clone()]
        }
    }

    fn detach(&self, child: &DomNode) {
        if let Some(index) = self.index_of(child) {
            self.0.children.borrow_mut().remove(index);
            *child.0.parent.borrow_mut() = Weak::new();
        }
    }

    /// `insertBefore(node, child)`: moves `node` (a fragment's children) in
    /// front of `child`, or to the end when `child` is `None`.
    pub fn insert_before(&self, node: &DomNode, child: Option<&DomNode>) -> DomNode {
        let nodes = Self::expand(node);
        for n in &nodes {
            if let Some(parent) = n.parent_node() {
                parent.detach(n);
            }
        }
        let index = child
            .and_then(|c| self.index_of(c))
            .unwrap_or_else(|| self.0.children.borrow().len());
        for n in &nodes {
            *n.0.parent.borrow_mut() = Rc::downgrade(&self.0);
        }
        self.0.children.borrow_mut().splice(index..index, nodes);
        node.clone()
    }

    /// `appendChild(node)`.
    pub fn append_child(&self, node: &DomNode) -> DomNode {
        self.insert_before(node, None)
    }

    /// `append(...nodes)`.
    pub fn append(&self, nodes: &[DomNode]) {
        for node in nodes {
            self.append_child(node);
        }
    }

    /// `append(text)`.
    pub fn append_text(&self, text: &str) {
        self.append_child(&DomNode::new_text(
            text,
            self.document_for_new_nodes().as_ref(),
        ));
    }

    /// `prepend(...nodes)`.
    pub fn prepend(&self, nodes: &[DomNode]) {
        let first = self.first_child();
        for node in nodes {
            self.insert_before(node, first.as_ref());
        }
    }

    /// `removeChild(child)`.
    pub fn remove_child(&self, child: &DomNode) -> DomNode {
        self.detach(child);
        child.clone()
    }

    /// `replaceChild(node, child)`.
    pub fn replace_child(&self, node: &DomNode, child: &DomNode) -> DomNode {
        let next = child.next_sibling();
        self.detach(child);
        self.insert_before(node, next.as_ref());
        child.clone()
    }

    /// `remove()`.
    pub fn remove(&self) {
        if let Some(parent) = self.parent_node() {
            parent.detach(self);
        }
    }

    /// `replaceWith(...nodes)`.
    pub fn replace_with(&self, nodes: &[DomNode]) {
        let Some(parent) = self.parent_node() else {
            return;
        };
        let next = self.next_sibling();
        parent.detach(self);
        for node in nodes {
            parent.insert_before(node, next.as_ref());
        }
    }

    /// `replaceChildren(...nodes)`.
    pub fn replace_children(&self, nodes: &[DomNode]) {
        let old = std::mem::take(&mut *self.0.children.borrow_mut());
        for child in old {
            *child.0.parent.borrow_mut() = Weak::new();
        }
        self.append(nodes);
    }

    /// `contains(other)`: whether `other` is this node or a descendant.
    pub fn contains(&self, other: Option<&DomNode>) -> bool {
        let mut node = other.cloned();
        while let Some(current) = node {
            if &current == self {
                return true;
            }
            node = current.parent_node();
        }
        false
    }

    /// `cloneNode(deep)`.
    pub fn clone_node(&self, deep: bool) -> DomNode {
        let document = self.owner_document();
        let copy = match self.node_type() {
            DomNodeType::Element => {
                let copy = DomNode::new_element(self.local_name().to_owned(), document.as_ref());
                *copy.element_inner().attributes.borrow_mut() = self.attributes();
                copy
            }
            DomNodeType::Text => DomNode::new_text(self.data(), document.as_ref()),
            DomNodeType::Comment => DomNode::new_comment(self.data(), document.as_ref()),
            DomNodeType::Document => DomNode::new_document(),
            DomNodeType::DocumentFragment => DomNode::new_document_fragment(document.as_ref()),
        };
        if deep {
            for child in self.child_nodes() {
                copy.append_child(&child.clone_node(true));
            }
        }
        copy
    }

    // Document

    /// `document.createElement(tagName)`.
    pub fn create_element(&self, tag_name: &str) -> DomNode {
        DomNode::new_element(tag_name.to_ascii_lowercase(), Some(self))
    }

    /// `document.createTextNode(data)`.
    pub fn create_text_node(&self, data: &str) -> DomNode {
        DomNode::new_text(data, Some(self))
    }

    /// `document.createComment(data)`.
    pub fn create_comment(&self, data: &str) -> DomNode {
        DomNode::new_comment(data, Some(self))
    }

    /// `document.createDocumentFragment()`.
    pub fn create_document_fragment(&self) -> DomNode {
        DomNode::new_document_fragment(Some(self))
    }

    /// `document.documentElement`.
    pub fn document_element(&self) -> Option<DomNode> {
        self.children().into_iter().next()
    }

    /// `document.head`.
    pub fn head(&self) -> Option<DomNode> {
        self.document_element()?
            .children()
            .into_iter()
            .find(|e| e.local_name() == "head")
    }

    /// `document.body`.
    pub fn body(&self) -> Option<DomNode> {
        self.document_element()?
            .children()
            .into_iter()
            .find(|e| e.local_name() == "body")
    }

    // Element

    fn element_inner(&self) -> &ElementInner {
        self.0
            .element
            .as_ref()
            .expect("an element method on a node that is not an element")
    }

    /// The element's style slot, for the declaration.
    pub(super) fn style_slot(&self) -> Option<&RefCell<Option<CssStyleDeclaration>>> {
        self.0.element.as_ref().map(|e| &e.style)
    }

    /// `localName`: the lower-case tag name, or empty for a node that is not
    /// an element.
    pub fn local_name(&self) -> &str {
        self.0
            .element
            .as_ref()
            .map_or("", |e| e.local_name.as_str())
    }

    /// `tagName`: the upper-case tag name.
    pub fn tag_name(&self) -> String {
        self.local_name().to_ascii_uppercase()
    }

    /// `attributes`, as `(name, value)` pairs in order.
    pub fn attributes(&self) -> Vec<(String, String)> {
        self.0
            .element
            .as_ref()
            .map(|e| e.attributes.borrow().clone())
            .unwrap_or_default()
    }

    /// `getAttribute(name)`.
    pub fn get_attribute(&self, name: &str) -> Option<String> {
        let element = self.0.element.as_ref()?;
        let name = name.to_ascii_lowercase();
        element
            .attributes
            .borrow()
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.clone())
    }

    pub fn has_attribute(&self, name: &str) -> bool {
        self.get_attribute(name).is_some()
    }

    pub fn get_attribute_names(&self) -> Vec<String> {
        self.attributes().into_iter().map(|(n, _)| n).collect()
    }

    /// `setAttribute(name, value)`. Setting `style` replaces the inline
    /// declaration's properties with those of the value.
    pub fn set_attribute(&self, name: &str, value: &str) {
        let name = name.to_ascii_lowercase();
        self.set_attribute_value(&name, value);
        if name == "style"
            && let Some(declaration) = self.element_inner().style.borrow_mut().as_mut()
        {
            declaration.parse(value);
        }
    }

    /// `removeAttribute(name)`.
    pub fn remove_attribute(&self, name: &str) {
        let name = name.to_ascii_lowercase();
        let element = self.element_inner();
        element.attributes.borrow_mut().retain(|(n, _)| *n != name);
        if name == "style"
            && let Some(declaration) = element.style.borrow_mut().as_mut()
        {
            declaration.parse("");
        }
    }

    /// `toggleAttribute(name, force)`.
    pub fn toggle_attribute(&self, name: &str, force: Option<bool>) -> bool {
        let present = self.has_attribute(name);
        let next = force.unwrap_or(!present);
        if next && !present {
            self.set_attribute(name, "");
        }
        if !next && present {
            self.remove_attribute(name);
        }
        next
    }

    /// Writes an attribute without touching the style declaration (which is
    /// how the declaration itself updates the `style` attribute).
    pub(super) fn set_attribute_value(&self, name: &str, value: &str) {
        let mut attributes = self.element_inner().attributes.borrow_mut();
        match attributes.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => value.clone_into(&mut entry.1),
            None => attributes.push((name.to_owned(), value.to_owned())),
        }
    }

    /// `id`.
    pub fn id(&self) -> String {
        self.get_attribute("id").unwrap_or_default()
    }

    pub fn set_id(&self, id: &str) {
        self.set_attribute("id", id);
    }

    /// `dir`.
    pub fn dir(&self) -> String {
        self.get_attribute("dir").unwrap_or_default()
    }

    pub fn set_dir(&self, dir: &str) {
        self.set_attribute("dir", dir);
    }

    /// `className`.
    pub fn class_name(&self) -> String {
        self.get_attribute("class").unwrap_or_default()
    }

    pub fn set_class_name(&self, class_name: &str) {
        self.set_attribute("class", class_name);
    }

    /// `classList`.
    pub fn class_list(&self) -> DomTokenList {
        DomTokenList {
            element: self.clone(),
            attribute: "class",
        }
    }

    /// `style`: the inline style declaration, kept in step with the `style`
    /// attribute.
    pub fn style(&self) -> CssStyle {
        CssStyle::new(self.clone())
    }

    /// `innerHTML`: the serialized children.
    pub fn inner_html(&self) -> String {
        serialize_html_children(self)
    }

    /// `outerHTML`: the element serialized.
    pub fn outer_html(&self) -> String {
        let mut out = String::new();
        serialize_html(self, &mut out);
        out
    }

    /// The descendant elements in document order.
    pub fn descendant_elements(&self) -> Vec<DomNode> {
        fn walk(node: &DomNode, out: &mut Vec<DomNode>) {
            for child in node.0.children.borrow().iter() {
                if child.is_element() {
                    out.push(child.clone());
                    walk(child, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, &mut out);
        out
    }

    /// `getElementsByTagName(name)`.
    pub fn get_elements_by_tag_name(&self, name: &str) -> Vec<DomNode> {
        let name = name.to_ascii_lowercase();
        self.descendant_elements()
            .into_iter()
            .filter(|e| name == "*" || e.local_name() == name)
            .collect()
    }
}

/// `DOMTokenList` over an attribute: unique tokens, separated by single
/// spaces once changed.
#[derive(Clone, Debug)]
pub struct DomTokenList {
    element: DomNode,
    attribute: &'static str,
}

impl DomTokenList {
    /// The tokens, without duplicates.
    pub fn tokens(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let value = self
            .element
            .get_attribute(self.attribute)
            .unwrap_or_default();
        for token in value
            .split([' ', '\t', '\n', '\r', '\u{0C}'])
            .filter(|t| !t.is_empty())
        {
            if !out.iter().any(|t| t == token) {
                out.push(token.to_owned());
            }
        }
        out
    }

    pub fn length(&self) -> usize {
        self.tokens().len()
    }

    pub fn value(&self) -> String {
        self.element
            .get_attribute(self.attribute)
            .unwrap_or_default()
    }

    pub fn contains(&self, token: &str) -> bool {
        self.tokens().iter().any(|t| t == token)
    }

    fn update(&self, tokens: &[String]) {
        // The spec skips the update when there is no attribute and no tokens.
        if !self.element.has_attribute(self.attribute) && tokens.is_empty() {
            return;
        }
        self.element
            .set_attribute(self.attribute, &tokens.join(" "));
    }

    /// `add(...tokens)`.
    pub fn add<S: AsRef<str>>(&self, tokens: &[S]) {
        let mut list = self.tokens();
        for token in tokens {
            let token = token.as_ref();
            if !list.iter().any(|t| t == token) {
                list.push(token.to_owned());
            }
        }
        self.update(&list);
    }

    /// `remove(...tokens)`.
    pub fn remove<S: AsRef<str>>(&self, tokens: &[S]) {
        let list: Vec<String> = self
            .tokens()
            .into_iter()
            .filter(|t| !tokens.iter().any(|r| r.as_ref() == t))
            .collect();
        self.update(&list);
    }

    /// `toggle(token, force)`.
    pub fn toggle(&self, token: &str, force: Option<bool>) -> bool {
        let present = self.contains(token);
        let next = force.unwrap_or(!present);
        if next && !present {
            self.add(&[token]);
        }
        if !next && present {
            self.remove(&[token]);
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use crate::dom::get_document;

    /// The style attribute and the declaration stay in step as jsdom keeps
    /// them: an attribute reads as written until the declaration changes,
    /// then as its `cssText`; removing every property leaves `style=""`.
    #[test]
    fn style_attribute_follows_the_declaration() {
        let document = get_document();
        let span = document.create_element("span");
        span.set_attribute("style", "color:RED");
        assert_eq!(span.get_attribute("style").as_deref(), Some("color:RED"));
        assert_eq!(span.style().get_property_value("color"), "red");
        span.style().set_property("font-size", "0", "");
        assert_eq!(
            span.get_attribute("style").as_deref(),
            Some("color: red; font-size: 0px;")
        );
        span.style().remove_property("color");
        span.style().remove_property("font-size");
        assert_eq!(span.outer_html(), r#"<span style=""></span>"#);
        span.set_attribute("style", "margin: 0");
        assert_eq!(span.style().css_text(), "margin: 0px;");
    }

    /// Appending a fragment moves its children and leaves it empty.
    #[test]
    fn appending_a_fragment_moves_its_children() {
        let document = get_document();
        let fragment = document.create_document_fragment();
        fragment.append(&[document.create_element("b")]);
        fragment.append_text("x");
        let p = document.create_element("p");
        p.append(std::slice::from_ref(&fragment));
        assert_eq!(p.outer_html(), "<p><b></b>x</p>");
        assert!(!fragment.has_child_nodes());
    }
}
