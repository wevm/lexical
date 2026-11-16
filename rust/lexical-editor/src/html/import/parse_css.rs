//! `import/parseCss.ts`: `parseSelector`, a selector from a reduced CSS
//! subset (`@experimental` upstream).

use super::sel::AttrMatch;
use super::sel::{
    CompiledSelector, ImportCaptures, Predicate, build_attr_predicate, build_class_all_predicate,
    build_selector,
};
use crate::LexicalError;
use crate::dom::DomNode;
use std::rc::Rc;

/// `IDENT_CHAR`: `/[A-Za-z0-9_-]/`.
fn is_ident_char(c: Option<u16>) -> bool {
    c.and_then(|c| char::from_u32(u32::from(c)))
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `/\s/` on one UTF-16 unit.
fn is_js_whitespace(c: u16) -> bool {
    matches!(
        c,
        0x09..=0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F
            | 0x3000 | 0xFEFF
    )
}

/// `Cursor`, over the source's UTF-16 units as JS indexes a string.
struct Cursor<'a> {
    source: &'a str,
    units: Vec<u16>,
    pos: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u16> {
        self.units.get(self.pos).copied()
    }
    fn peek_is(&self, c: char) -> bool {
        self.peek() == Some(c as u16)
    }
    fn consume(&mut self) -> Option<u16> {
        let c = self.peek();
        self.pos += 1;
        c
    }
    fn eof(&self) -> bool {
        self.pos >= self.units.len()
    }
    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(is_js_whitespace) {
            self.pos += 1;
        }
    }
    fn slice(&self, start: usize, end: usize) -> String {
        String::from_utf16_lossy(&self.units[start..end.min(self.units.len())])
    }
    fn read_ident(&mut self) -> String {
        let start = self.pos;
        while is_ident_char(self.peek()) {
            self.pos += 1;
        }
        self.slice(start, self.pos)
    }
    fn read_quoted(&mut self) -> Result<String, LexicalError> {
        let quote = self.consume();
        self.check(
            quote == Some('"' as u16) || quote == Some('\'' as u16),
            "expected quote",
        )?;
        let start = self.pos;
        while !self.eof() && self.peek() != quote {
            if self.peek_is('\\') {
                self.pos += 2;
            } else {
                self.pos += 1;
            }
        }
        self.check(!self.eof(), "unterminated string")?;
        let value = self.slice(start, self.pos);
        self.pos += 1;
        // `value.replace(/\\(.)/g, '$1')`.
        let mut out = String::new();
        let mut chars = value.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\'
                && let Some(&next) = chars.peek()
                && !matches!(next, '\n' | '\r' | '\u{2028}' | '\u{2029}')
            {
                out.push(next);
                chars.next();
            } else {
                out.push(c);
            }
        }
        Ok(out)
    }
    /// `assert(cond, msg)`.
    fn check(&self, cond: bool, msg: &str) -> Result<(), LexicalError> {
        if cond {
            return Ok(());
        }
        Err(LexicalError::new(format!(
            "invalid CSS selector at col {}: {msg} in {}",
            self.pos + 1,
            self.source
        )))
    }
}

/// `ParsedSimpleSelector`.
struct ParsedSimpleSelector {
    tags: Vec<String>,
    predicates: Vec<Predicate>,
}

fn parse_simple_selector(c: &mut Cursor) -> Result<ParsedSimpleSelector, LexicalError> {
    let mut tags = Vec::new();
    let mut predicates = Vec::new();
    let mut classes: Vec<String> = Vec::new();
    let mut is_universal = false;

    c.skip_whitespace();

    if c.peek_is('*') {
        c.consume();
        is_universal = true;
    } else if is_ident_char(c.peek()) {
        let tag = c.read_ident();
        if !tag.is_empty() {
            tags.push(tag.to_uppercase());
        }
    }

    while !c.eof() {
        if c.peek_is('.') {
            c.consume();
            let cls = c.read_ident();
            c.check(!cls.is_empty(), "expected class name after \".\"")?;
            classes.push(cls);
        } else if c.peek_is('#') {
            c.consume();
            let id = c.read_ident();
            c.check(!id.is_empty(), "expected id after \"#\"")?;
            predicates.push(build_attr_predicate("id", AttrMatch::Equals(id)));
        } else if c.peek_is('[') {
            c.consume();
            c.skip_whitespace();
            let name = c.read_ident();
            c.check(!name.is_empty(), "expected attribute name after \"[\"")?;
            c.skip_whitespace();
            let mut value = AttrMatch::Present;
            if c.peek_is('=') {
                c.consume();
                c.skip_whitespace();
                if c.peek_is('"') || c.peek_is('\'') {
                    value = AttrMatch::Equals(c.read_quoted()?);
                } else {
                    let ident = c.read_ident();
                    c.check(!ident.is_empty(), "expected attribute value")?;
                    value = AttrMatch::Equals(ident);
                }
                c.skip_whitespace();
            }
            c.check(c.peek_is(']'), "expected \"]\"")?;
            c.consume();
            predicates.push(build_attr_predicate(&name, value));
        } else {
            break;
        }
    }

    if !classes.is_empty() {
        let classes: Vec<&str> = classes.iter().map(String::as_str).collect();
        predicates.push(build_class_all_predicate(&classes));
    }

    // Neither tag nor refinement is only legitimate for a lone `*`.
    c.check(
        is_universal || !tags.is_empty() || !predicates.is_empty(),
        "expected a selector",
    )?;

    Ok(ParsedSimpleSelector { tags, predicates })
}

/// `parseSelector(source)` (`@experimental`): a selector from a reduced
/// CSS subset: tags, `*`, comma lists, `.class`, `#id`, `[attr]` and
/// `[attr="value"]`. Anything else (combinators, pseudo-classes, regex
/// attributes) is an error; chain the builder methods instead.
pub fn parse_selector(source: &str) -> Result<CompiledSelector, LexicalError> {
    let mut c = Cursor {
        source,
        units: source.encode_utf16().collect(),
        pos: 0,
    };
    let mut groups = Vec::new();
    loop {
        groups.push(parse_simple_selector(&mut c)?);
        c.skip_whitespace();
        if c.eof() {
            break;
        }
        c.check(
            c.peek_is(','),
            "expected \",\" (selector lists are the only supported combinator)",
        )?;
        c.consume();
        c.skip_whitespace();
    }

    if groups.len() == 1 {
        let g = groups.pop().expect("a group");
        return Ok(build_selector(g.tags, g.predicates));
    }

    // Merge tag sets only when every group is tag-restricted, and OR the
    // groups' refinements so a candidate satisfies some group entirely.
    let mut tags: Vec<String> = Vec::new();
    if groups.iter().all(|g| !g.tags.is_empty()) {
        for g in &groups {
            for t in &g.tags {
                if !tags.contains(t) {
                    tags.push(t.clone());
                }
            }
        }
    }
    let or_predicate: Predicate = Rc::new(move |node: &DomNode, captures: &mut ImportCaptures| {
        let upper = node.node_name();
        groups.iter().any(|g| {
            (g.tags.is_empty() || g.tags.contains(&upper))
                && g.predicates.iter().all(|p| p(node, captures))
        })
    });
    Ok(build_selector(tags, vec![or_predicate]))
}
