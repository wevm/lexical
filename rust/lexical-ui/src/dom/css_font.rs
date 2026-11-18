//! The `font` shorthand as the declaration of the upstream tests (jsdom)
//! expands it: `properties/font.js`' `parse`, which the declaration's
//! `cssText` setter and `setProperty("font", ...)` both run, with the
//! longhand parsers it calls (`fontStyle`, `fontVariant`, `fontWeight`,
//! `fontSize`, `lineHeight` and `fontFamily`'s `parse`). Pinned by the
//! `css/declarations` vectors, quirks included: a numeric weight before
//! the size is also read as the line height, and a `font-variant`
//! keyword other than `normal` (`small-caps`) makes the value invalid.
//!
//! jsdom first checks the whole value against the `font` grammar with
//! css-tree's lexer (`isValidPropertyValue`); [`is_valid_font`] is that
//! grammar written out for the values `parse` reads.

use super::css_values::{CSS_WIDE_KEYWORDS, evaluate_calc, normalize_css_value, parse_css_number};

/// The longhands of `font`, in the order its setter writes them
/// (`shorthandFor`).
pub(crate) const FONT_LONGHANDS: [&str; 6] = [
    "font-style",
    "font-variant",
    "font-weight",
    "font-size",
    "line-height",
    "font-family",
];

const STYLE_KEYWORDS: &[&str] = &["normal", "italic", "oblique"];
const WEIGHT_KEYWORDS: &[&str] = &["normal", "bold", "bolder", "lighter"];
const STRETCH_KEYWORDS: &[&str] = &[
    "normal",
    "ultra-condensed",
    "extra-condensed",
    "condensed",
    "semi-condensed",
    "semi-expanded",
    "expanded",
    "extra-expanded",
    "ultra-expanded",
];
const SIZE_KEYWORDS: &[&str] = &[
    "xx-small",
    "x-small",
    "small",
    "medium",
    "large",
    "x-large",
    "xx-large",
    "xxx-large",
    "larger",
    "smaller",
    "math",
];
/// The keywords `font-variant` takes, which `font` refuses but for
/// `normal`.
const VARIANT_KEYWORDS: &[&str] = &[
    "normal",
    "none",
    "small-caps",
    "all-small-caps",
    "petite-caps",
    "all-petite-caps",
    "unicase",
    "titling-caps",
    "common-ligatures",
    "no-common-ligatures",
    "discretionary-ligatures",
    "no-discretionary-ligatures",
    "historical-ligatures",
    "no-historical-ligatures",
    "contextual",
    "no-contextual",
    "historical-forms",
    "lining-nums",
    "oldstyle-nums",
    "proportional-nums",
    "tabular-nums",
    "diagonal-fractions",
    "stacked-fractions",
    "ordinal",
    "slashed-zero",
    "jis78",
    "jis83",
    "jis90",
    "jis04",
    "simplified",
    "traditional",
    "full-width",
    "proportional-width",
    "ruby",
    "sub",
    "super",
    "text",
    "emoji",
    "unicode",
];
const SYSTEM_FONTS: &[&str] = &[
    "caption",
    "icon",
    "menu",
    "message-box",
    "small-caption",
    "status-bar",
];
const GENERIC_FAMILIES: &[&str] = &[
    "serif",
    "sans-serif",
    "cursive",
    "fantasy",
    "monospace",
    "system-ui",
    "emoji",
    "math",
    "fangsong",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
];
const LENGTH_UNITS: &[&str] = &[
    "px", "em", "rem", "ex", "rex", "ch", "rch", "cap", "rcap", "ic", "ric", "lh", "rlh", "vw",
    "vh", "vi", "vb", "vmin", "vmax", "svw", "svh", "svi", "svb", "svmin", "svmax", "lvw", "lvh",
    "lvi", "lvb", "lvmin", "lvmax", "dvw", "dvh", "dvi", "dvb", "dvmin", "dvmax", "cqw", "cqh",
    "cqi", "cqb", "cqmin", "cqmax", "cm", "mm", "q", "in", "pt", "pc",
];
const ANGLE_UNITS: &[&str] = &["deg", "grad", "rad", "turn"];

fn is_keyword(word: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|k| word.eq_ignore_ascii_case(k))
}

/// `hasVarFunc`.
pub(crate) fn has_var_func(value: &str) -> bool {
    value.to_ascii_lowercase().contains("var(")
}

fn is_math_function(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    ["calc(", "min(", "max(", "clamp("]
        .iter()
        .any(|f| lower.starts_with(f))
        && word.ends_with(')')
}

/// A length or percentage that isn't negative, or a unitless `0`.
fn is_length_percentage(word: &str) -> bool {
    match parse_css_number(word) {
        Some((n, unit)) if n >= 0.0 => {
            (unit.is_empty() && n == 0.0) || unit == "%" || is_keyword(unit, LENGTH_UNITS)
        }
        _ => false,
    }
}

fn is_unitless_number(word: &str) -> Option<f64> {
    parse_css_number(word).and_then(|(n, unit)| unit.is_empty().then_some(n))
}

/// `fontStyle.parse(part)`.
fn parse_style(part: &str) -> Option<String> {
    is_keyword(part, STYLE_KEYWORDS).then(|| part.to_ascii_lowercase())
}

/// `fontVariant.parse(part)`, as far as `font` reads it.
fn parse_variant(part: &str) -> Option<String> {
    is_keyword(part, VARIANT_KEYWORDS).then(|| part.to_ascii_lowercase())
}

/// `fontWeight.parse(part)`: a keyword or a number from 1 to 1000.
fn parse_weight(part: &str) -> Option<String> {
    if is_keyword(part, WEIGHT_KEYWORDS) {
        return Some(part.to_ascii_lowercase());
    }
    let n = is_unitless_number(part)?;
    (1.0..=1000.0)
        .contains(&n)
        .then(|| normalize_css_value("font-weight", part))
        .flatten()
}

/// `fontSize.parse(part)`.
fn parse_size(part: &str) -> Option<String> {
    if is_keyword(part, SIZE_KEYWORDS) || is_length_percentage(part) || is_math_function(part) {
        normalize_css_value("font-size", part)
    } else {
        None
    }
}

/// `lineHeight.parse(part)`.
fn parse_line_height(part: &str) -> Option<String> {
    let valid = part.eq_ignore_ascii_case("normal")
        || is_unitless_number(part).is_some_and(|n| n >= 0.0)
        || is_length_percentage(part)
        || is_math_function(part);
    if valid {
        normalize_css_value("line-height", part)
    } else {
        None
    }
}

/// A CSS string token, whole: its contents.
fn string_body(word: &str) -> Option<&str> {
    let quote = word.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let body = word.get(1..word.len().checked_sub(1)?)?;
    (word.len() >= 2 && word.ends_with(quote) && !body.contains(quote)).then_some(body)
}

fn is_ident(word: &str) -> bool {
    let mut chars = word.chars();
    let first = match chars.next() {
        Some('-') => match chars.next() {
            Some('-') => return true,
            Some(c) => c,
            None => return false,
        },
        Some(c) => c,
        None => return false,
    };
    (first.is_ascii_alphabetic() || first == '_' || !first.is_ascii())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii())
}

/// Whether `words` are one entry of a `font-family` list: a string, a
/// generic family, or a name of identifiers that aren't CSS-wide
/// keywords, `default` or (in a name of several) generic families.
fn is_family_entry(words: &[&str]) -> bool {
    match words {
        [] => false,
        [word] if string_body(word).is_some() => true,
        [word] if is_keyword(word, GENERIC_FAMILIES) => true,
        _ => words.iter().all(|w| {
            is_ident(w)
                && !is_keyword(w, CSS_WIDE_KEYWORDS)
                && !w.eq_ignore_ascii_case("default")
                && !(words.len() > 1 && is_keyword(w, GENERIC_FAMILIES))
        }),
    }
}

/// `fontFamily.parse(v)`: each entry of the list, a string or a name of
/// several identifiers quoted, joined with `, `; an empty entry makes it
/// empty.
fn parse_family(value: &str) -> Option<String> {
    let mut out = Vec::new();
    for entry in split_top_level(value, ',') {
        if entry.is_empty() {
            return Some(String::new());
        }
        let words = split_words(&entry);
        let quote = |text: &str| format!("\"{}\"", text.replace('\\', "").replace('"', "\\\""));
        match words.as_slice() {
            [word] => match string_body(word) {
                Some(body) => out.push(quote(body)),
                None if is_ident(word) => out.push((*word).to_owned()),
                None => return None,
            },
            _ if words.iter().all(|w| is_ident(w)) => out.push(quote(&words.join(" "))),
            _ => return None,
        }
    }
    (!out.is_empty()).then(|| out.join(", "))
}

/// Splits at `delimiter` outside strings and parentheses, trimming each
/// part (`splitValue` with a delimiter).
fn split_top_level(value: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for c in value.chars() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
                current.push(c);
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    current.push(c);
                }
                '(' => {
                    depth += 1;
                    current.push(c);
                }
                ')' => {
                    depth = depth.saturating_sub(1);
                    current.push(c);
                }
                c if c == delimiter && depth == 0 => {
                    parts.push(current.trim().to_owned());
                    current.clear();
                }
                c => current.push(c),
            },
        }
    }
    parts.push(current.trim().to_owned());
    parts
}

/// Splits at whitespace outside strings and parentheses (`splitValue`).
fn split_words(value: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (i, c) in value.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                start.get_or_insert(i);
            }
            '(' => {
                depth += 1;
                start.get_or_insert(i);
            }
            ')' => depth = depth.saturating_sub(1),
            ' ' | '\t' | '\n' | '\u{0C}' | '\r' if depth == 0 => {
                if let Some(s) = start.take() {
                    words.push(&value[s..i]);
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(s) = start {
        words.push(&value[s..]);
    }
    words
}

/// Whether `value` matches the `font` grammar (css-tree's
/// `isValidPropertyValue("font", value)`):
/// `[ <font-style> || <font-variant-css2> || <font-weight> ||
/// <font-stretch-css3> ]? <font-size> [ / <line-height> ]? <font-family>`,
/// a system font, or a CSS-wide keyword.
fn is_valid_font(value: &str) -> bool {
    let chunks = split_top_level(value, ',');
    let (first, families) = chunks.split_first().expect("a chunk");
    if families.is_empty() {
        let words = split_words(first);
        if let [word] = words.as_slice()
            && (is_keyword(word, CSS_WIDE_KEYWORDS) || is_keyword(word, SYSTEM_FONTS))
        {
            return true;
        }
    }
    if !families.iter().all(|f| is_family_entry(&split_words(f))) {
        return false;
    }
    let halves = split_top_level(first, '/');
    let (before_size, size, family) = match halves.as_slice() {
        [a] => {
            let words = split_words(a);
            let Some(at) = words.iter().position(|w| parse_size(w).is_some()) else {
                return false;
            };
            (words[..at].to_vec(), words[at], words[at + 1..].to_vec())
        }
        [a, b] => {
            let words = split_words(a);
            let Some((size, before)) = words.split_last() else {
                return false;
            };
            let rest = split_words(b);
            let Some((line_height, family)) = rest.split_first() else {
                return false;
            };
            if parse_line_height(line_height).is_none() {
                return false;
            }
            (before.to_vec(), *size, family.to_vec())
        }
        _ => return false,
    };
    if parse_size(size).is_none() || !is_family_entry(&family) {
        return false;
    }
    // Each of style, variant, weight and stretch at most once, `normal`
    // standing for any of them.
    let (mut style, mut variant, mut weight, mut stretch, mut normals) =
        (false, false, false, false, 0);
    let mut words = before_size.iter().peekable();
    while let Some(word) = words.next() {
        let take = |seen: &mut bool| !std::mem::replace(seen, true);
        let ok = if word.eq_ignore_ascii_case("normal") {
            normals += 1;
            true
        } else if is_keyword(word, STYLE_KEYWORDS) {
            if word.eq_ignore_ascii_case("oblique")
                && words.peek().is_some_and(|next| {
                    parse_css_number(next).is_some_and(|(_, unit)| is_keyword(unit, ANGLE_UNITS))
                })
            {
                words.next();
            }
            take(&mut style)
        } else if word.eq_ignore_ascii_case("small-caps") {
            take(&mut variant)
        } else if parse_weight(word).is_some() {
            take(&mut weight)
        } else if is_keyword(word, STRETCH_KEYWORDS) {
            take(&mut stretch)
        } else {
            false
        };
        if !ok {
            return false;
        }
    }
    normals + usize::from(style) + usize::from(variant) + usize::from(weight) + usize::from(stretch)
        <= 4
}

/// `font.parse(value)`: the longhands `value` sets, in the order JS's
/// result object holds them, or `None` when it is invalid. `value` is
/// trimmed and not empty.
pub(crate) fn parse_font(value: &str) -> Option<Vec<(&'static str, String)>> {
    let [style, variant, weight, size, line_height, family] = FONT_LONGHANDS;
    let value = &resolve_calc(value);
    if !is_valid_font(value) {
        return None;
    }
    let chunks = split_top_level(value, ',');
    let (font_block, families) = chunks.split_first().expect("a chunk");
    let halves = split_top_level(font_block, '/');
    let mut font: Vec<(&'static str, String)> = vec![
        (style, "normal".to_owned()),
        (variant, "normal".to_owned()),
        (weight, "normal".to_owned()),
    ];
    let get = |font: &[(&str, String)], key: &str| {
        font.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
    };
    let set = |font: &mut Vec<(&'static str, String)>, key: &'static str, value: String| match font
        .iter_mut()
        .find(|(k, _)| *k == key)
    {
        Some(entry) => entry.1 = value,
        None => font.push((key, value)),
    };
    // The variant `font` refuses: any other than `normal` (JS accepts only
    // `small-cap`, which no keyword is).
    let refuses_variant = |font: &[(&str, String)], part: &str| {
        get(font, variant).as_deref() == Some("normal")
            && parse_variant(part).is_some_and(|v| v != "normal")
    };
    let mut font_families: Vec<String> = Vec::new();
    if let [font_block_a, font_block_b] = halves.as_slice() {
        // JS splits the part after the slash at single spaces.
        let mut rest = font_block_b.trim().split(' ');
        let line_b = rest.next().unwrap_or_default();
        let families_b: Vec<&str> = rest.collect();
        if line_b.is_empty() || families_b.is_empty() {
            return None;
        }
        let line_height_b = parse_line_height(line_b)?;
        font_families.push(parse_family(&families_b.join(" "))?);
        for part in split_words(font_block_a.trim()) {
            if part == "normal" {
                continue;
            }
            if let Some(parsed) = parse_size(part) {
                set(&mut font, size, parsed);
            }
            if get(&font, style).as_deref() == Some("normal")
                && let Some(parsed) = parse_style(part)
            {
                set(&mut font, style, parsed);
            }
            if refuses_variant(&font, part) {
                return None;
            }
            if get(&font, weight).as_deref() == Some("normal")
                && let Some(parsed) = parse_weight(part)
            {
                set(&mut font, weight, parsed);
            }
        }
        get(&font, size)?;
        set(&mut font, line_height, line_height_b);
    } else {
        let rev_parts: Vec<&str> = split_words(font_block.trim()).into_iter().rev().collect();
        if let [part] = rev_parts.as_slice() {
            if !is_keyword(part, CSS_WIDE_KEYWORDS) {
                return None;
            }
            let keyword = part.to_ascii_lowercase();
            return Some(FONT_LONGHANDS.map(|key| (key, keyword.clone())).to_vec());
        }
        set(&mut font, line_height, "normal".to_owned());
        let mut rev_font_family: Vec<String> = Vec::new();
        let mut font_size_a: Option<String> = None;
        for part in rev_parts {
            if font_size_a.is_some() {
                if part.eq_ignore_ascii_case("normal") {
                    continue;
                }
                for (key, parse) in [
                    (style, parse_style as fn(&str) -> Option<String>),
                    (weight, parse_weight),
                    (line_height, parse_line_height),
                ] {
                    if get(&font, key).as_deref() == Some("normal")
                        && let Some(parsed) = parse(part)
                    {
                        set(&mut font, key, parsed);
                    }
                }
                if refuses_variant(&font, part) {
                    return None;
                }
            } else if let Some(parsed) = parse_size(part) {
                font_size_a = Some(parsed);
            } else {
                rev_font_family.push(parse_family(part)?);
            }
        }
        rev_font_family.reverse();
        let family_value = parse_family(&rev_font_family.join(" ")).filter(|f| !f.is_empty());
        let (Some(font_size_a), Some(family_value)) = (font_size_a, family_value) else {
            return None;
        };
        set(&mut font, size, font_size_a);
        font_families.push(parse_family(&family_value)?);
    }
    for entry in families {
        match parse_family(entry) {
            Some(parsed) if !parsed.is_empty() => {
                if !font_families.contains(&parsed) {
                    font_families.push(parsed);
                }
            }
            _ => return None,
        }
    }
    set(&mut font, family, font_families.join(", "));
    Some(font)
}

/// `resolveCalc(value)`: each `calc()` that evaluates replaced with its
/// result, before the value is split.
fn resolve_calc(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(at) = rest.to_ascii_lowercase().find("calc(") {
        let open = at + "calc(".len();
        let mut depth = 1usize;
        let close = rest[open..].char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(open + i)
        });
        let Some(close) = close else { break };
        out.push_str(&rest[..at]);
        match evaluate_calc(&rest[open..close]) {
            Some(result) => out.push_str(&format!("calc({result})")),
            None => out.push_str(&rest[at..=close]),
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The value the `font` setter stores for the longhands it wrote: each
/// one that isn't `normal`, once, the line height after a `/`.
pub(crate) fn serialize_font(longhands: &[(&str, String)]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for key in FONT_LONGHANDS {
        let Some((_, value)) = longhands.iter().find(|(k, _)| *k == key) else {
            continue;
        };
        if value != "normal" && !seen.contains(&value.as_str()) {
            seen.push(value);
            parts.push(if key == "line-height" {
                format!("/ {value}")
            } else {
                value.clone()
            });
        }
    }
    parts.join(" ")
}
