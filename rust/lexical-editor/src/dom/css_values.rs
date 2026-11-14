//! How the inline style declaration normalizes the values written to it.
//! Upstream's tests run on jsdom, whose declaration parses each value and
//! serializes it back, and exported HTML carries that form byte for byte.
//! This follows it for the values Lexical and its tests write, pinned by
//! the `css/values` vectors: hex and functional colors become
//! `rgb()`/`rgba()`, numbers lose their redundant digits, a bare `0` length
//! becomes `0px`, keywords are lowercased, `calc()` of a product is
//! evaluated, commas are spaced and single-quoted strings are requoted.
//! Values outside that are kept as written, with their whitespace
//! collapsed; jsdom also rejects values it cannot parse, which this does
//! not (as in the Swift port).

use super::css_properties::CSS_PROPERTY_TRAITS;
use crate::json::number_to_string;
use crate::set_dom_style::{get_style_object_from_css, js_trim};

/// How a property an inline style accepts normalizes its values (the
/// generated `css_properties.rs` table holds them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssPropertyTraits(pub u8);

impl CssPropertyTraits {
    /// Hex and functional colors serialize as `rgb()`/`rgba()`.
    pub const COLOR: u8 = 1;
    /// A bare `0` serializes as `0px`.
    pub const ZERO_LENGTH: u8 = 2;
    /// Identifiers keep their case (font and animation names).
    pub const PRESERVE_CASE: u8 = 4;
    /// The value is kept as written.
    pub const VERBATIM: u8 = 8;

    fn has(self, bit: u8) -> bool {
        self.0 & bit != 0
    }
}

/// The traits of a property an inline style accepts, or `None` for one it
/// doesn't (custom properties aside).
pub fn css_property_traits(name: &str) -> Option<CssPropertyTraits> {
    CSS_PROPERTY_TRAITS
        .binary_search_by(|(n, _)| (*n).cmp(name))
        .ok()
        .map(|i| CssPropertyTraits(CSS_PROPERTY_TRAITS[i].1))
}

/// The property names an inline style accepts, sorted.
pub fn css_property_names() -> impl Iterator<Item = &'static str> {
    CSS_PROPERTY_TRAITS.iter().map(|(n, _)| *n)
}

/// CSS input preprocessing, as jsdom's `preprocessValue` does it to every
/// value and to `cssText`: form feeds and CR LF or CR become LF, and NUL and
/// a trailing backslash (an escape cut off by the end) become U+FFFD.
pub(crate) fn preprocess_css(css: &str) -> String {
    if !css.contains(['\u{0C}', '\r', '\0']) && !css.ends_with('\\') {
        return css.to_owned();
    }
    let mut chars: Vec<char> = css.chars().collect();
    if chars.last() == Some(&'\\') {
        *chars.last_mut().expect("a last char") = '\u{FFFD}';
    }
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\u{0C}' => out.push('\n'),
            '\r' => {
                out.push('\n');
                if chars.get(i + 1) == Some(&'\n') {
                    i += 1;
                }
            }
            '\0' => out.push('\u{FFFD}'),
            c => out.push(c),
        }
        i += 1;
    }
    out
}

/// The declarations in inline CSS text, in order, a repeated property
/// keeping the position of its first declaration and its last value.
pub(crate) fn parse_css_declarations(css: &str) -> Vec<(String, String)> {
    get_style_object_from_css(css).into_iter().collect()
}

/// The value `name` serializes `value` as, or `None` when the declaration
/// doesn't accept the property.
pub(crate) fn normalize_css_value(name: &str, value: &str) -> Option<String> {
    if name.starts_with("--") {
        let trimmed = js_trim(value);
        return (!trimmed.is_empty()).then(|| trimmed.to_owned());
    }
    let traits = css_property_traits(name)?;
    let trimmed = js_trim(value);
    // A CSS-wide keyword is the whole value, in lower case, whatever the
    // property keeps of other identifiers.
    if CSS_WIDE_KEYWORDS
        .iter()
        .any(|k| trimmed.eq_ignore_ascii_case(k))
    {
        return Some(trimmed.to_ascii_lowercase());
    }
    if traits.has(CssPropertyTraits::VERBATIM) {
        // The verbatim properties are the shadows: jsdom keeps a shadow as
        // written, but reads `none` as a keyword and rejects a value of
        // nothing but other identifiers, which no shadow is.
        if trimmed.eq_ignore_ascii_case("none") {
            return Some("none".to_owned());
        }
        let only_identifiers = tokenize_css_value(trimmed).iter().all(|t| match t {
            Token::Word(word) => {
                word.chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_')
                    && !word.contains('(')
            }
            Token::Comma => true,
        });
        if only_identifiers {
            return None;
        }
    }
    let mut out = String::new();
    let mut needs_space = false;
    for token in tokenize_css_value(value) {
        match token {
            Token::Comma => {
                out.push(',');
                needs_space = true;
            }
            Token::Word(word) => {
                if needs_space || !out.is_empty() {
                    out.push(' ');
                }
                needs_space = false;
                if traits.has(CssPropertyTraits::VERBATIM) {
                    out.push_str(&word);
                } else {
                    out.push_str(&normalize_css_word(&word, traits));
                }
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// The keywords every property accepts.
const CSS_WIDE_KEYWORDS: &[&str] = &["initial", "inherit", "unset", "revert", "revert-layer"];

enum Token {
    Word(String),
    Comma,
}

/// Splits a value into words and commas at the top level; strings and
/// parenthesized groups stay inside their word.
fn tokenize_css_value(value: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let flush = |word: &mut String, tokens: &mut Vec<Token>| {
        if !word.is_empty() {
            tokens.push(Token::Word(std::mem::take(word)));
        }
    };
    for c in value.chars() {
        if let Some(current) = quote {
            word.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == current {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                word.push(c);
            }
            '(' => {
                depth += 1;
                word.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                word.push(c);
            }
            ',' if depth == 0 => {
                flush(&mut word, &mut tokens);
                tokens.push(Token::Comma);
            }
            ' ' | '\t' | '\n' | '\r' | '\u{0C}' => {
                if depth == 0 {
                    flush(&mut word, &mut tokens);
                } else {
                    word.push(c);
                }
            }
            _ => word.push(c),
        }
    }
    flush(&mut word, &mut tokens);
    tokens
}

fn normalize_css_word(word: &str, traits: CssPropertyTraits) -> String {
    let Some(first) = word.chars().next() else {
        return word.to_owned();
    };
    if first == '\'' {
        return requote_css_string(word);
    }
    if first == '"' {
        return word.to_owned();
    }
    if traits.has(CssPropertyTraits::COLOR)
        && first == '#'
        && let Some(rgba) = parse_hex_color(&word[1..])
    {
        return serialize_color(rgba, true);
    }
    if let Some(open) = word.find('(')
        && word.ends_with(')')
    {
        let function = word[..open].to_lowercase();
        let arguments = &word[open + 1..word.len() - 1];
        if traits.has(CssPropertyTraits::COLOR)
            && let Some(rgba) = parse_color_function(&function, arguments)
        {
            return serialize_color(rgba, false);
        }
        if function == "calc"
            && let Some(result) = evaluate_calc_product(arguments)
        {
            return format!("calc({result})");
        }
        return word.to_owned();
    }
    if let Some((number, unit)) = parse_css_number(word) {
        if unit.is_empty() && number == 0.0 && traits.has(CssPropertyTraits::ZERO_LENGTH) {
            return "0px".to_owned();
        }
        return format_css_number(number) + &unit.to_lowercase();
    }
    if traits.has(CssPropertyTraits::PRESERVE_CASE) {
        word.to_owned()
    } else {
        word.to_lowercase()
    }
}

/// `'text'` as `"text"`.
fn requote_css_string(word: &str) -> String {
    let mut body = &word[1..];
    if body.ends_with('\'') {
        body = &body[..body.len() - 1];
    }
    let mut out = String::from("\"");
    let mut escaped = false;
    for c in body.chars() {
        if escaped {
            if c != '\'' {
                out.push('\\');
            }
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            out.push_str("\\\"");
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}

/// A number with an optional unit: `+5px`, `1.50em`, `1e1px`, `.5`.
fn parse_css_number(word: &str) -> Option<(f64, &str)> {
    let bytes = word.as_bytes();
    let digit = |i: usize| bytes.get(i).is_some_and(u8::is_ascii_digit);
    let mut i = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let mut digits = 0;
    while digit(i) {
        i += 1;
        digits += 1;
    }
    if bytes.get(i) == Some(&b'.') && digit(i + 1) {
        i += 1;
        while digit(i) {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(bytes.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        if digit(j) {
            while digit(j) {
                j += 1;
            }
            i = j;
        }
    }
    let unit = &word[i..];
    if !unit.bytes().all(|b| b == b'%' || b.is_ascii_alphabetic()) {
        return None;
    }
    let number: f64 = word[..i].parse().ok()?;
    Some((number, unit))
}

fn format_css_number(number: f64) -> String {
    if number == 0.0 {
        "0".to_owned()
    } else {
        number_to_string(number)
    }
}

/// `calc(n * length)` and `calc(length * n)` as their product.
fn evaluate_calc_product(arguments: &str) -> Option<String> {
    let parts: Vec<&str> = arguments
        .split([' ', '\t', '\n'])
        .filter(|p| !p.is_empty())
        .collect();
    let [a, "*", b] = parts.as_slice() else {
        return None;
    };
    let (a, unit_a) = parse_css_number(a)?;
    let (b, unit_b) = parse_css_number(b)?;
    if !(unit_a.is_empty() || unit_b.is_empty()) {
        return None;
    }
    Some(format_css_number(a * b) + &format!("{unit_a}{unit_b}").to_lowercase())
}

#[derive(Clone, Copy)]
struct Rgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

fn parse_hex_color(hex: &str) -> Option<Rgba> {
    let digits: Vec<u32> = hex.chars().map(|c| c.to_digit(16)).collect::<Option<_>>()?;
    let channels: Vec<u32> = match digits.len() {
        3 | 4 => digits.iter().map(|d| d * 17).collect(),
        6 | 8 => digits.chunks(2).map(|p| p[0] * 16 + p[1]).collect(),
        _ => return None,
    };
    Some(Rgba {
        r: f64::from(channels[0]),
        g: f64::from(channels[1]),
        b: f64::from(channels[2]),
        a: if channels.len() == 4 {
            f64::from(channels[3]) / 255.0
        } else {
            1.0
        },
    })
}

/// Rounds half up, as Swift's `rounded()` and JS's `Math.round` do for the
/// non-negative values here.
fn round(x: f64) -> f64 {
    x.round()
}

/// `rgb()`, `rgba()`, `hsl()` and `hsla()`, with commas or spaces and a
/// `/` alpha.
fn parse_color_function(function: &str, arguments: &str) -> Option<Rgba> {
    if !matches!(function, "rgb" | "rgba" | "hsl" | "hsla") {
        return None;
    }
    let mut parts: Vec<&str> = arguments
        .split([',', ' ', '/', '\t'])
        .filter(|p| !p.is_empty())
        .collect();
    if !(parts.len() == 3 || parts.len() == 4) {
        return None;
    }
    let mut alpha = 1.0;
    if parts.len() == 4 {
        let last = parts.pop().expect("four parts");
        let (value, unit) = parse_css_number(last)?;
        if !(unit.is_empty() || unit == "%") {
            return None;
        }
        alpha = if unit == "%" { value / 100.0 } else { value };
    }
    let values: Vec<(f64, &str)> = parts.iter().filter_map(|p| parse_css_number(p)).collect();
    if values.len() != 3 {
        return None;
    }
    let clamp_alpha = alpha.clamp(0.0, 1.0);
    if function.starts_with("rgb") {
        if !values.iter().all(|(_, unit)| unit.is_empty()) {
            return None;
        }
        let channel = |v: f64| round(v).clamp(0.0, 255.0);
        return Some(Rgba {
            r: channel(values[0].0),
            g: channel(values[1].0),
            b: channel(values[2].0),
            a: clamp_alpha,
        });
    }
    let h = ((values[0].0 % 360.0) + 360.0) % 360.0 / 360.0;
    let s = (values[1].0 / 100.0).clamp(0.0, 1.0);
    let l = (values[2].0 / 100.0).clamp(0.0, 1.0);
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue = |t: f64| {
        let mut t = t;
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 1.0 / 2.0 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    Some(Rgba {
        r: round(hue(h + 1.0 / 3.0) * 255.0),
        g: round(hue(h) * 255.0),
        b: round(hue(h - 1.0 / 3.0) * 255.0),
        a: clamp_alpha,
    })
}

fn serialize_color(color: Rgba, hex_alpha: bool) -> String {
    let rgb = format!("{}, {}, {}", color.r as i64, color.g as i64, color.b as i64);
    if color.a >= 1.0 {
        return format!("rgb({rgb})");
    }
    let mut alpha = round(color.a * 1000.0) / 1000.0;
    if hex_alpha {
        // A byte alpha prints with two decimals when they round-trip to it.
        let byte = round(color.a * 255.0);
        let two_decimals = round(color.a * 100.0) / 100.0;
        if round(two_decimals * 255.0) == byte {
            alpha = two_decimals;
        }
    }
    format!("rgba({rgb}, {})", format_css_number(alpha))
}
