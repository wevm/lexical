//! A `<style>` element's `CSSStyleSheet`: its top-level rules, as
//! `sheet.cssRules` gives them. Only style rules (`CSSStyleRule`) carry
//! anything here; at-rules (`@media`, `@page`, `@font-face`...) are parsed
//! past, as `$inlineStylesFromStyleSheets` skips every rule that isn't a
//! style rule.
//!
//! The rules follow CSS Syntax's "consume a list of rules" as jsdom's CSSOM
//! parses them: comments and the `<!--`/`-->` tokens between rules are
//! dropped, strings and nested blocks are skipped over when finding a
//! block's end, and an unterminated block runs to the end of the sheet. A
//! style rule's declarations parse as an inline style's do, but with
//! property names matched in any case, as a rule's are.

use super::css_style_declaration::CssStyleDeclaration;
use super::css_values::preprocess_css;
use crate::set_dom_style::js_trim;

/// A rule of a style sheet (`CSSRule`).
#[derive(Clone, Debug)]
pub enum CssRule {
    /// `CSSStyleRule`: `selectorText` and `style`.
    Style {
        selector_text: String,
        style: CssStyleDeclaration,
    },
    /// Any other rule (an at-rule), whose contents nothing here reads.
    Other,
}

/// The top-level rules of the style sheet `css` (`sheet.cssRules`).
pub fn parse_style_sheet(css: &str) -> Vec<CssRule> {
    let chars: Vec<char> = preprocess_css(css).chars().collect();
    let mut rules = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if starts_with(&chars, i, "/*") {
            i = skip_comment(&chars, i);
        } else if starts_with(&chars, i, "<!--") {
            i += 4;
        } else if starts_with(&chars, i, "-->") {
            i += 3;
        } else if c == '@' {
            // An at-rule ends at a `;` or after its block.
            let mut j = i + 1;
            while j < chars.len() && chars[j] != ';' && chars[j] != '{' {
                j = skip_token(&chars, j);
            }
            i = if j < chars.len() && chars[j] == '{' {
                block_end(&chars, j).1
            } else {
                j + 1
            };
            rules.push(CssRule::Other);
        } else {
            // A qualified rule: its prelude, then its block. One without a
            // block is a parse error and dropped.
            let start = i;
            let mut j = i;
            while j < chars.len() && chars[j] != '{' {
                j = skip_token(&chars, j);
            }
            if j >= chars.len() {
                break;
            }
            let prelude = strip_comments(&chars[start..j]);
            let (body_end, next) = block_end(&chars, j);
            let body = strip_comments(&chars[j + 1..body_end]);
            rules.push(CssRule::Style {
                selector_text: js_trim(&prelude).to_owned(),
                style: CssStyleDeclaration::from_rule_text(&body),
            });
            i = next;
        }
    }
    rules
}

fn starts_with(chars: &[char], i: usize, s: &str) -> bool {
    let pattern: Vec<char> = s.chars().collect();
    chars
        .get(i..)
        .is_some_and(|rest| rest.starts_with(&pattern))
}

/// The index after the comment starting at `i`.
fn skip_comment(chars: &[char], i: usize) -> usize {
    let mut j = i + 2;
    while j < chars.len() && !starts_with(chars, j, "*/") {
        j += 1;
    }
    (j + 2).min(chars.len())
}

/// The index after the string starting at `i` (its quote). A newline ends
/// an unterminated string.
fn skip_string(chars: &[char], i: usize) -> usize {
    let quote = chars[i];
    let mut j = i + 1;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 2,
            '\n' => return j,
            c if c == quote => return j + 1,
            _ => j += 1,
        }
    }
    chars.len()
}

/// The index after the token at `i`: a comment, a string, an escape, or one
/// character.
fn skip_token(chars: &[char], i: usize) -> usize {
    match chars[i] {
        '"' | '\'' => skip_string(chars, i),
        '\\' => (i + 2).min(chars.len()),
        '/' if starts_with(chars, i, "/*") => skip_comment(chars, i),
        _ => i + 1,
    }
}

/// For the block opening at `open`: where its contents end and the index
/// after it (the end of the sheet when it is unterminated).
fn block_end(chars: &[char], open: usize) -> (usize, usize) {
    let mut depth = 0usize;
    let mut j = open;
    while j < chars.len() {
        match chars[j] {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && chars[j] == '}' {
                    return (j, j + 1);
                }
            }
            _ => {}
        }
        j = skip_token(chars, j);
    }
    (chars.len(), chars.len())
}

/// `chars` as a string without its comments (strings kept as written).
fn strip_comments(chars: &[char]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let next = skip_token(chars, i);
        if !(chars[i] == '/' && starts_with(chars, i, "/*")) {
            out.extend(&chars[i..next.min(chars.len())]);
        } else {
            out.push(' ');
        }
        i = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(css: &str) -> Vec<String> {
        parse_style_sheet(css)
            .into_iter()
            .map(|rule| match rule {
                CssRule::Style {
                    selector_text,
                    style,
                } => format!("{selector_text} {{{}}}", style.css_text()),
                CssRule::Other => "@".to_owned(),
            })
            .collect()
    }

    /// The rules jsdom 30's `sheet.cssRules` gives for the same sheets
    /// (`selectorText`, and the declarations; `css/declarations` pins the
    /// shorthands).
    #[test]
    fn rules_as_jsdom_parses_them() {
        assert_eq!(
            summary("p { COLOR: RED; Font-Weight: Bold !important } .a,.b{foo:y}"),
            ["p {color: red; font-weight: bold !important;}", ".a,.b {}"]
        );
        assert_eq!(
            summary("/* c */ <!-- p{color:red} --> @media print { p { color: red } } span{}"),
            ["p {color: red;}", "@", "span {}"]
        );
        assert_eq!(
            summary(r#"p > span, div  p[data-a="1"] { color : blue ; ; } p::before{content:"}"}"#),
            [
                r#"p > span, div  p[data-a="1"] {color: blue;}"#,
                r#"p::before {content: "}";}"#
            ]
        );
        assert_eq!(
            summary("@page{margin:1in} q{color:red"),
            ["@", "q {color: red;}"]
        );
        assert_eq!(
            summary("x{--Custom: Val ue; color:red !IMPORTANT}"),
            ["x {--Custom: Val ue; color: red !important;}"]
        );
    }
}
