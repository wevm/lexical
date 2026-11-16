//! `CSSStyleDeclaration` for an element's inline style: properties in the
//! order they were first set, with their priority. Values are normalized
//! the way the declaration of the upstream tests (jsdom) normalizes the
//! values Lexical writes ([`normalize_css_value`]).

use super::css_values::{normalize_css_value, parse_css_declarations, preprocess_css};
use super::node::DomNode;
use crate::set_dom_style::js_trim;

/// The declarations of an inline style.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CssStyleDeclaration {
    entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    name: String,
    value: String,
    priority: String,
}

impl CssStyleDeclaration {
    /// A declaration that belongs to no element.
    pub fn new() -> Self {
        Self::default()
    }

    /// A declaration with the properties of `css`, as an element's style
    /// attribute gives one.
    pub fn from_css_text(css: &str) -> Self {
        let mut declaration = Self::new();
        declaration.parse(css);
        declaration
    }

    pub fn length(&self) -> usize {
        self.entries.len()
    }

    /// `item(index)`: the name of the property at `index`.
    pub fn item(&self, index: usize) -> String {
        self.entries
            .get(index)
            .map(|e| e.name.clone())
            .unwrap_or_default()
    }

    /// The property names in order.
    pub fn property_names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.name.clone()).collect()
    }

    fn find(&self, name: &str) -> Option<&Entry> {
        let name = canonical_name(name, false);
        self.entries.iter().find(|e| e.name == name)
    }

    /// `getPropertyValue(name)`.
    pub fn get_property_value(&self, name: &str) -> String {
        self.find(name).map(|e| e.value.clone()).unwrap_or_default()
    }

    /// `getPropertyPriority(name)`.
    pub fn get_property_priority(&self, name: &str) -> String {
        self.find(name)
            .map(|e| e.priority.clone())
            .unwrap_or_default()
    }

    /// `setProperty(name, value, priority)`: an empty value removes the
    /// property; an unknown property or an invalid priority is ignored.
    /// Whether the declaration changed.
    pub fn set_property(&mut self, name: &str, value: &str, priority: &str) -> bool {
        let name = canonical_name(name, false);
        let value = preprocess_css(value);
        if value.is_empty() {
            return self.remove_property(&name).is_some();
        }
        let priority = priority.to_lowercase();
        if !(priority.is_empty() || priority == "important") {
            return false;
        }
        self.write(name, &value, priority)
    }

    /// `removeProperty(name)`: the removed value, or `None` when the
    /// property wasn't set (JS returns `""` either way).
    pub fn remove_property(&mut self, name: &str) -> Option<String> {
        let name = canonical_name(name, false);
        let index = self.entries.iter().position(|e| e.name == name)?;
        Some(self.entries.remove(index).value)
    }

    /// `cssText`.
    pub fn css_text(&self) -> String {
        self.entries
            .iter()
            .map(|e| {
                let important = if e.priority.is_empty() {
                    ""
                } else {
                    " !important"
                };
                format!("{}: {}{important};", e.name, e.value)
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Replaces the declarations with those in `css`.
    pub fn parse(&mut self, css: &str) {
        self.entries.clear();
        for (name, raw_value) in parse_css_declarations(&preprocess_css(css)) {
            let (value, important) = split_important(&raw_value);
            self.write(
                canonical_name(&name, true),
                &value,
                if important { "important" } else { "" }.to_owned(),
            );
        }
    }

    fn write(&mut self, name: String, value: &str, priority: String) -> bool {
        let Some(value) = normalize_css_value(&name, value) else {
            return false;
        };
        match self.entries.iter_mut().find(|e| e.name == name) {
            Some(entry) => {
                entry.value = value;
                entry.priority = priority;
            }
            None => self.entries.push(Entry {
                name,
                value,
                priority,
            }),
        }
        true
    }
}

/// Property names are matched lower case, except custom properties
/// (`--name`). Declarations parsed from text keep their case, so an
/// upper-case name there is unknown, as in jsdom.
fn canonical_name(name: &str, from_text: bool) -> String {
    if name.starts_with("--") || from_text {
        name.to_owned()
    } else {
        name.to_lowercase()
    }
}

/// Splits a trailing `!important` (in any case) off a declaration value.
fn split_important(value: &str) -> (String, bool) {
    const FLAG: &str = "!important";
    let trimmed = js_trim(value);
    let n = trimmed.len();
    if n >= FLAG.len()
        && trimmed.is_char_boundary(n - FLAG.len())
        && trimmed[n - FLAG.len()..].eq_ignore_ascii_case(FLAG)
    {
        return (js_trim(&trimmed[..n - FLAG.len()]).to_owned(), true);
    }
    (trimmed.to_owned(), false)
}

/// `element.style`: the element's inline declaration, read from its `style`
/// attribute the first time, which writes the attribute back as `cssText`
/// whenever it changes.
#[derive(Clone, Debug)]
pub struct CssStyle {
    element: DomNode,
}

impl CssStyle {
    pub(super) fn new(element: DomNode) -> Self {
        Self { element }
    }

    fn with<T>(&self, f: impl FnOnce(&mut CssStyleDeclaration) -> T) -> T {
        let slot = self
            .element
            .style_slot()
            .expect("style of a node that is not an element");
        let mut slot = slot.borrow_mut();
        let declaration = slot.get_or_insert_with(|| {
            CssStyleDeclaration::from_css_text(
                &self.element.get_attribute("style").unwrap_or_default(),
            )
        });
        f(declaration)
    }

    /// Writes the `style` attribute from the declaration.
    fn notify(&self) {
        let css = self.with(|d| d.css_text());
        self.element.set_attribute_value("style", &css);
    }

    pub fn length(&self) -> usize {
        self.with(|d| d.length())
    }

    pub fn item(&self, index: usize) -> String {
        self.with(|d| d.item(index))
    }

    pub fn property_names(&self) -> Vec<String> {
        self.with(|d| d.property_names())
    }

    pub fn get_property_value(&self, name: &str) -> String {
        self.with(|d| d.get_property_value(name))
    }

    pub fn get_property_priority(&self, name: &str) -> String {
        self.with(|d| d.get_property_priority(name))
    }

    /// `setProperty(name, value, priority)`.
    pub fn set_property(&self, name: &str, value: &str, priority: &str) {
        if self.with(|d| d.set_property(name, value, priority)) {
            self.notify();
        }
    }

    /// `removeProperty(name)`: the removed value, or empty.
    pub fn remove_property(&self, name: &str) -> String {
        match self.with(|d| d.remove_property(name)) {
            Some(value) => {
                self.notify();
                value
            }
            None => String::new(),
        }
    }

    /// `cssText`.
    pub fn css_text(&self) -> String {
        self.with(|d| d.css_text())
    }

    /// Setting `cssText`.
    pub fn set_css_text(&self, css: &str) {
        self.with(|d| d.parse(css));
        self.notify();
    }
}
