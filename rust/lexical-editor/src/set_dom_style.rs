//! `utils/setDOMStyle.ts`: inline CSS text as a style object
//! ([`get_style_object_from_css`]), and, with the `html` feature, a style
//! object or CSS text applied to a DOM style declaration
//! ([`set_dom_style_object`], [`set_dom_style_from_css`]).

use crate::json::array_index;
use crate::selection_deletion::is_js_whitespace;

/// `String.prototype.trim`: without JS's whitespace (and line terminators)
/// at either end.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

/// A `Record<string, string>` of CSS properties, as
/// [`get_style_object_from_css`] returns it: keys in JS's property order
/// (array indices first, ascending, then the rest in the order they were
/// first set), so iterating it writes CSS in the order JS does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StyleObject {
    entries: Vec<(String, String)>,
}

impl StyleObject {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// `styles[key] = value`: an existing key keeps its position.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
            return;
        }
        let at = match array_index(&key) {
            Some(index) => self
                .entries
                .iter()
                .position(|(k, _)| array_index(k).is_none_or(|i| i > index))
                .unwrap_or(self.entries.len()),
            None => self.entries.len(),
        };
        self.entries.insert(at, (key, value));
    }

    /// `delete styles[key]`.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(index).1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }
}

impl IntoIterator for StyleObject {
    type Item = (String, String);
    type IntoIter = std::vec::IntoIter<(String, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for StyleObject {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut styles = StyleObject::new();
        for (k, v) in iter {
            styles.insert(k, v);
        }
        styles
    }
}

/// `getStyleObjectFromCSS(css)`: parses inline CSS text into an object that
/// is compatible with `CSSStyleDeclaration.setProperty()`. Comments are
/// dropped; `:` and `;` inside quotes or parentheses are not delimiters.
pub fn get_style_object_from_css(css: &str) -> StyleObject {
    let mut styles = StyleObject::new();
    if css.is_empty() {
        return styles;
    }
    // Every delimiter is ASCII, so byte offsets work as JS's code unit
    // offsets do: a slice never starts or ends inside a character.
    let bytes = css.as_bytes();
    let length = bytes.len();
    let mut current_property = String::new();
    let mut current_value = String::new();
    let mut current_quote: Option<u8> = None;
    let mut in_comment = false;
    let mut is_escaped = false;
    let mut is_parsing_value = false;
    let mut parenthesis_depth = 0usize;
    // The start of the pending run of kept characters, if any.
    let mut chunk_start: Option<usize> = None;

    let commit = |property: &str, value: &str, styles: &mut StyleObject| {
        let property = js_trim(property);
        let value = js_trim(value);
        if !property.is_empty() && !value.is_empty() {
            styles.insert(property, value);
        }
    };

    let mut i = 0;
    while i < length {
        let char = bytes[i];
        let next = bytes.get(i + 1).copied();
        if in_comment {
            if char == b'*' && next == Some(b'/') {
                in_comment = false;
                i += 1;
            }
            i += 1;
            continue;
        }
        if is_escaped {
            chunk_start.get_or_insert(i);
            is_escaped = false;
            i += 1;
            continue;
        }
        if let Some(quote) = current_quote {
            chunk_start.get_or_insert(i);
            if char == b'\\' {
                is_escaped = true;
            } else if char == quote {
                current_quote = None;
            }
            i += 1;
            continue;
        }
        if char == b'/' && next == Some(b'*') {
            // The comment is dropped, so flush everything accumulated before it.
            if let Some(start) = chunk_start.take() {
                if is_parsing_value {
                    current_value.push_str(&css[start..i]);
                } else {
                    current_property.push_str(&css[start..i]);
                }
            }
            in_comment = true;
            i += 2;
            continue;
        }
        match char {
            b'"' | b'\'' => {
                chunk_start.get_or_insert(i);
                current_quote = Some(char);
            }
            b'(' => {
                chunk_start.get_or_insert(i);
                parenthesis_depth += 1;
            }
            b')' => {
                chunk_start.get_or_insert(i);
                parenthesis_depth = parenthesis_depth.saturating_sub(1);
            }
            b':' if !is_parsing_value && parenthesis_depth == 0 => {
                // The separator is dropped; flush the accumulated property name.
                if let Some(start) = chunk_start.take() {
                    current_property.push_str(&css[start..i]);
                }
                is_parsing_value = true;
            }
            b';' if parenthesis_depth == 0 => {
                if let Some(start) = chunk_start.take() {
                    if is_parsing_value {
                        current_value.push_str(&css[start..i]);
                    } else {
                        current_property.push_str(&css[start..i]);
                    }
                }
                commit(&current_property, &current_value, &mut styles);
                current_property.clear();
                current_value.clear();
                is_parsing_value = false;
            }
            _ => {
                chunk_start.get_or_insert(i);
            }
        }
        i += 1;
    }
    if let Some(start) = chunk_start {
        if is_parsing_value {
            current_value.push_str(&css[start..]);
        } else {
            current_property.push_str(&css[start..]);
        }
    }
    commit(&current_property, &current_value, &mut styles);
    styles
}

#[cfg(feature = "html")]
mod dom_style {
    use super::{StyleObject, get_style_object_from_css, js_trim};
    use crate::dom::CssStyle;

    const IMPORTANT_FLAG: &str = "!important";

    /// `setDOMStyleProperty(domStyle, property, value)`: a trailing
    /// `!important` (in any case) is set as the priority.
    fn set_dom_style_property(dom_style: &CssStyle, property: &str, value: &str) {
        let trimmed = value.trim_end_matches(crate::selection_deletion::is_js_whitespace);
        let n = trimmed.len();
        let flag_start = n.checked_sub(IMPORTANT_FLAG.len());
        let has_important = flag_start.is_some_and(|start| {
            trimmed.is_char_boundary(start) && trimmed[start..].to_lowercase() == IMPORTANT_FLAG
        });
        match flag_start {
            Some(start) if has_important => {
                dom_style.set_property(property, js_trim(&trimmed[..start]), "important");
            }
            _ => dom_style.set_property(property, value, ""),
        }
    }

    /// `setDOMStyleObject(domStyle, styleObject)`: sets each property with a
    /// value and removes each one without (`null`).
    pub fn set_dom_style_object<'a>(
        dom_style: &CssStyle,
        style_object: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
    ) {
        for (property, value) in style_object {
            match value {
                None => {
                    dom_style.remove_property(property);
                }
                Some(value) => set_dom_style_property(dom_style, property, value),
            }
        }
    }

    /// `setDOMStyleFromCSS(domStyle, cssText, prevCSSText)`: sets the
    /// properties of `css_text` and removes those only `prev_css_text` has.
    pub fn set_dom_style_from_css(dom_style: &CssStyle, css_text: &str, prev_css_text: &str) {
        if css_text == prev_css_text {
            return;
        }
        let mut prev_css: StyleObject = get_style_object_from_css(prev_css_text);
        let next_css = get_style_object_from_css(css_text);
        for (property, value) in next_css.iter() {
            prev_css.remove(property);
            set_dom_style_property(dom_style, property, value);
        }
        for property in prev_css.keys() {
            dom_style.remove_property(property);
        }
    }
}

#[cfg(feature = "html")]
pub use dom_style::{set_dom_style_from_css, set_dom_style_object};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_objects_follow_js_property_order() {
        // Object.keys(getStyleObjectFromCSS('b: 1; 2: x; a: 2; 1: y; b: 3'))
        // is ['1', '2', 'b', 'a'], with b: '3'.
        let styles = get_style_object_from_css("b: 1; 2: x; a: 2; 1: y; b: 3");
        let entries: Vec<(&str, &str)> = styles.iter().collect();
        assert_eq!(entries, [("1", "y"), ("2", "x"), ("b", "3"), ("a", "2")]);
    }

    #[test]
    fn delimiters_in_quotes_parentheses_and_comments() {
        // getStyleObjectFromCSS(
        //   'content: "a;b:c"; /* x: y; */ background: url(a;b) ;font : \'q\\\'\' ')
        let styles = get_style_object_from_css(
            r#"content: "a;b:c"; /* x: y; */ background: url(a;b) ;font : 'q\'' "#,
        );
        let entries: Vec<(&str, &str)> = styles.iter().collect();
        assert_eq!(
            entries,
            [
                ("content", r#""a;b:c""#),
                ("background", "url(a;b)"),
                ("font", r"'q\''"),
            ]
        );
    }
}
