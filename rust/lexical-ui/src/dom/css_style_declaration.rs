//! `CSSStyleDeclaration` for an element's inline style: properties in the
//! order they were first set, with their priority. Values are normalized
//! the way the declaration of the upstream tests (jsdom) normalizes the
//! values Lexical writes ([`normalize_css_value`]).

use super::css_font::{FONT_LONGHANDS, has_var_func, parse_font, serialize_font};
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
        if js_trim(&value).is_empty() {
            // The `font` setter clears its longhands too.
            let mut changed = false;
            if name == "font" {
                for longhand in FONT_LONGHANDS {
                    changed |= self.remove_property(longhand).is_some();
                }
            }
            return self.remove_property(&name).is_some() || changed;
        }
        let priority = priority.to_lowercase();
        if !(priority.is_empty() || priority == "important") {
            return false;
        }
        self.set_property_value(name, &value, priority)
    }

    /// jsdom's `_setPropertyValue`: the property's setter, which for `font`
    /// writes its longhands, and for a longhand of it set to a `var()`
    /// clears the shorthand.
    fn set_property_value(&mut self, name: String, value: &str, priority: String) -> bool {
        if name == "font" {
            return self.set_font(js_trim(value), &priority);
        }
        let mut changed = false;
        if FONT_LONGHANDS.contains(&name.as_str()) && has_var_func(value) {
            changed = self.remove_property("font").is_some();
        }
        self.write(name, value, priority) || changed
    }

    /// The `font` setter (`properties/font.js`' `descriptor.set`): the
    /// longhands `value` expands into, then the shorthand, as the longhands
    /// that aren't `normal`; a `var()` is kept whole, without longhands.
    fn set_font(&mut self, value: &str, priority: &str) -> bool {
        if has_var_func(value) {
            for longhand in FONT_LONGHANDS {
                self.remove_property(longhand);
            }
            self.write_raw("font", value, priority);
            return true;
        }
        let Some(longhands) = parse_font(value) else {
            return false;
        };
        for key in FONT_LONGHANDS {
            if let Some((_, value)) = longhands.iter().find(|(k, _)| *k == key) {
                self.write_raw(key, value, priority);
            }
        }
        self.write_raw("font", &serialize_font(&longhands), priority);
        true
    }

    /// jsdom's `_setProperty`: stores the value as it is, in place when the
    /// property is set already.
    fn write_raw(&mut self, name: &str, value: &str, priority: &str) {
        match self.entries.iter_mut().find(|e| e.name == name) {
            Some(entry) => {
                entry.value = value.to_owned();
                entry.priority = priority.to_owned();
            }
            None => self.entries.push(Entry {
                name: name.to_owned(),
                value: value.to_owned(),
                priority: priority.to_owned(),
            }),
        }
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
        // `font` stands for the longhands set before it, unless one of them
        // is important and it isn't.
        let mut serialized: Vec<&Entry> = Vec::new();
        for entry in &self.entries {
            if entry.name == "font" {
                serialized.retain(|e| {
                    !(FONT_LONGHANDS.contains(&e.name.as_str())
                        && (!entry.priority.is_empty() || e.priority.is_empty()))
                });
            }
            serialized.push(entry);
        }
        serialized
            .into_iter()
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

    /// A declaration with the properties of a style rule's block, as
    /// jsdom's CSSOM gives a rule's `style`: as [`from_css_text`], but
    /// property names match in any case.
    ///
    /// [`from_css_text`]: CssStyleDeclaration::from_css_text
    pub fn from_rule_text(css: &str) -> Self {
        let mut declaration = Self::new();
        declaration.parse_declarations(css, false);
        declaration
    }

    /// Replaces the declarations with those in `css`.
    pub fn parse(&mut self, css: &str) {
        self.parse_declarations(css, true);
    }

    /// jsdom's `cssText` setter: each property's last declaration, unless
    /// an earlier one is important, in the place of its first; then
    /// `prepareProperties`, which puts the longhands of a `font` before it;
    /// then each property's setter in that order.
    fn parse_declarations(&mut self, css: &str, keep_case: bool) {
        self.entries.clear();
        let mut declared: Vec<Entry> = Vec::new();
        for (name, raw_value) in parse_css_declarations(&preprocess_css(css)) {
            let (value, important) = split_important(&raw_value);
            let name = canonical_name(&name, keep_case);
            if name != "font" && normalize_css_value(&name, &value).is_none() {
                continue;
            }
            let priority = if important { "important" } else { "" }.to_owned();
            match declared.iter_mut().find(|e| e.name == name) {
                Some(entry) if entry.priority.is_empty() => {
                    entry.value = value;
                    entry.priority = priority;
                }
                Some(_) => {}
                None => declared.push(Entry {
                    name,
                    value,
                    priority,
                }),
            }
        }
        let mut prepared: Vec<Entry> = Vec::new();
        let mut prepare = |name: &str, value: String, priority: &str| match prepared
            .iter_mut()
            .find(|e| e.name == name)
        {
            Some(entry) => {
                entry.value = value;
                entry.priority = priority.to_owned();
            }
            None => prepared.push(Entry {
                name: name.to_owned(),
                value,
                priority: priority.to_owned(),
            }),
        };
        for entry in declared {
            if entry.name == "font" && !has_var_func(&entry.value) {
                for (longhand, value) in parse_font(&entry.value).unwrap_or_default() {
                    prepare(longhand, value, &entry.priority);
                }
            }
            prepare(&entry.name, entry.value, &entry.priority);
        }
        for entry in prepared {
            self.set_property_value(entry.name, &entry.value, entry.priority);
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
