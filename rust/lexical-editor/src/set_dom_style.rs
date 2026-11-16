//! `utils/setDOMStyle.ts`: the CSS text parser. Applying a style to a DOM
//! element (`setDOMStyleObject`, `setDOMStyleFromCSS`) comes with the DOM.

use crate::json::{Object, Value};
use crate::selection_deletion::is_js_whitespace;

/// The `Record<string, string>` of CSS properties that
/// [`get_style_object_from_css`] returns, in a JS object's property order:
/// keys that are array indices first, then the others in the order they
/// were first set. Setting a property that exists keeps its place.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StyleObject(Object);

impl StyleObject {
    pub fn new() -> Self {
        Self::default()
    }

    /// `styles[property]`.
    pub fn get(&self, property: &str) -> Option<&str> {
        self.0.get(property).and_then(Value::as_str)
    }

    /// `styles[property] = value`.
    pub fn insert(&mut self, property: impl Into<String>, value: impl Into<String>) {
        self.0.insert(property, Value::String(value.into()));
    }

    /// `delete styles[property]`.
    pub fn remove(&mut self, property: &str) -> Option<String> {
        match self.0.remove(property)? {
            Value::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn contains_key(&self, property: &str) -> bool {
        self.0.contains_key(property)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `Object.entries(styles)`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(k, v)| (k, v.as_str().expect("a style value is a string")))
    }
}

/// `getStyleObjectFromCSS(css)`: parses inline CSS text into its
/// properties. Comments are dropped; `:` and `;` inside quotes or
/// parentheses are not delimiters; a property or value that `trim()`s to
/// nothing is left out. Every delimiter is ASCII, so the bytes of the
/// UTF-8 text are scanned where JS scans UTF-16 code units.
pub fn get_style_object_from_css(css: &str) -> StyleObject {
    let mut styles = StyleObject::new();
    if css.is_empty() {
        return styles;
    }
    let bytes = css.as_bytes();
    let length = bytes.len();
    let mut current_property = String::new();
    let mut current_value = String::new();
    let mut current_quote: Option<u8> = None;
    let mut in_comment = false;
    let mut is_escaped = false;
    let mut is_parsing_value = false;
    let mut parenthesis_depth: usize = 0;
    // The start of the pending run of characters that belong to the current
    // property or value, flushed at a dropped character or a delimiter.
    let mut chunk_start: Option<usize> = None;
    let commit = |styles: &mut StyleObject, property: &str, value: &str| {
        let property = property.trim_matches(is_js_whitespace);
        let value = value.trim_matches(is_js_whitespace);
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
        let flush = |chunk_start: &mut Option<usize>,
                     property: &mut String,
                     value: &mut String,
                     is_parsing_value: bool| {
            if let Some(start) = chunk_start.take() {
                // An escape can end on a byte inside a character; the run
                // always ends at an ASCII delimiter, so the slice is whole.
                let chunk = &css[start..i];
                if is_parsing_value {
                    value.push_str(chunk);
                } else {
                    property.push_str(chunk);
                }
            }
        };
        if char == b'/' && next == Some(b'*') {
            // The comment is dropped, so flush everything accumulated before it.
            flush(
                &mut chunk_start,
                &mut current_property,
                &mut current_value,
                is_parsing_value,
            );
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
                flush(
                    &mut chunk_start,
                    &mut current_property,
                    &mut current_value,
                    false,
                );
                is_parsing_value = true;
            }
            b';' if parenthesis_depth == 0 => {
                flush(
                    &mut chunk_start,
                    &mut current_property,
                    &mut current_value,
                    is_parsing_value,
                );
                commit(&mut styles, &current_property, &current_value);
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
    commit(&mut styles, &current_property, &current_value);
    styles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(css: &str) -> Vec<(String, String)> {
        get_style_object_from_css(css)
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect()
    }

    #[test]
    fn parses_as_js_does() {
        // Object.entries(getStyleObjectFromCSS(...)) in Node.
        assert_eq!(
            entries("color: red; font-size : 12px ;"),
            [
                ("color".into(), "red".into()),
                ("font-size".into(), "12px".into())
            ]
        );
        assert_eq!(
            entries(
                "background: url(\"a;b:c\"); /* x: y; */ font-family: 'A;B'; :x; y:; color: blue"
            ),
            [
                ("background".into(), "url(\"a;b:c\")".into()),
                ("font-family".into(), "'A;B'".into()),
                ("color".into(), "blue".into()),
            ]
        );
        assert_eq!(
            entries("b: 1; 2: two; a: x; b: 3"),
            [
                ("2".into(), "two".into()),
                ("b".into(), "3".into()),
                ("a".into(), "x".into()),
            ]
        );
        assert_eq!(entries(""), []);
    }
}
