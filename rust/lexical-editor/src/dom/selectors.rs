//! `querySelectorAll` and `matches` for the selectors a pasted style sheet
//! uses: compound selectors (a type or `*`, `#id`, `.class`, and
//! attribute selectors with every operator and the `i` and `s` flags)
//! joined by the descendant, child (`>`), next-sibling (`+`) and
//! subsequent-sibling (`~`) combinators, in comma lists.
//!
//! Anything else (pseudo-classes and pseudo-elements, namespaces, escapes)
//! is an error, as an invalid selector is in JS (a `SyntaxError`); in JS
//! some of it is valid, which [`SelectorError::Unsupported`] tells apart.
//! Type selectors and attribute names match in any case, as in an HTML
//! document; `#id` and `.class` match as written (a no-quirks document).

use super::node::DomNode;

/// Why a selector can't be matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectorError {
    /// Not a selector (JS throws a `SyntaxError`).
    Invalid,
    /// A valid selector outside the subset above.
    Unsupported,
}

#[derive(Clone, Debug)]
enum AttrOp {
    Exists,
    Equals,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

#[derive(Clone, Debug)]
enum Simple {
    Type(String),
    Id(String),
    Class(String),
    Attr {
        name: String,
        op: AttrOp,
        value: String,
        ignore_case: bool,
    },
}

#[derive(Clone, Copy, Debug)]
enum Combinator {
    Descendant,
    Child,
    NextSibling,
    SubsequentSibling,
}

/// A complex selector, right to left: the subject's compound, then each
/// combinator with the compound to its left.
#[derive(Clone, Debug)]
struct Complex {
    subject: Vec<Simple>,
    rest: Vec<(Combinator, Vec<Simple>)>,
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}

fn is_ident_char(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit() || c == '-'
}

fn is_css_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{0C}')
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) -> bool {
        let start = self.pos;
        while self.peek().is_some_and(is_css_whitespace) {
            self.pos += 1;
        }
        self.pos > start
    }

    fn ident(&mut self) -> Result<String, SelectorError> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
            if self.peek() == Some('-') {
                self.pos += 1;
            }
        }
        match self.peek() {
            Some('\\') => return Err(SelectorError::Unsupported),
            Some(c) if is_ident_start(c) || self.pos > start + 1 => {}
            _ => return Err(SelectorError::Invalid),
        }
        while let Some(c) = self.peek() {
            if c == '\\' {
                return Err(SelectorError::Unsupported);
            }
            if !is_ident_char(c) {
                break;
            }
            self.pos += 1;
        }
        Ok(self.chars[start..self.pos].iter().collect())
    }

    fn string(&mut self) -> Result<String, SelectorError> {
        let quote = self.peek().ok_or(SelectorError::Invalid)?;
        self.pos += 1;
        let mut value = String::new();
        loop {
            match self.peek() {
                None => return Ok(value),
                Some('\\') => return Err(SelectorError::Unsupported),
                Some('\n') => return Err(SelectorError::Invalid),
                Some(c) if c == quote => {
                    self.pos += 1;
                    return Ok(value);
                }
                Some(c) => {
                    value.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    fn attribute(&mut self) -> Result<Simple, SelectorError> {
        self.pos += 1;
        self.skip_whitespace();
        if self.peek() == Some('|') {
            return Err(SelectorError::Unsupported);
        }
        let name = self.ident()?.to_ascii_lowercase();
        self.skip_whitespace();
        let op = match self.peek() {
            Some(']') => {
                self.pos += 1;
                return Ok(Simple::Attr {
                    name,
                    op: AttrOp::Exists,
                    value: String::new(),
                    ignore_case: false,
                });
            }
            Some('=') => AttrOp::Equals,
            Some(c) => {
                if self.chars.get(self.pos + 1) != Some(&'=') {
                    return Err(if c == '|' {
                        SelectorError::Unsupported
                    } else {
                        SelectorError::Invalid
                    });
                }
                self.pos += 1;
                match c {
                    '~' => AttrOp::Includes,
                    '|' => AttrOp::DashMatch,
                    '^' => AttrOp::Prefix,
                    '$' => AttrOp::Suffix,
                    '*' => AttrOp::Substring,
                    _ => return Err(SelectorError::Invalid),
                }
            }
            None => return Err(SelectorError::Invalid),
        };
        self.pos += 1;
        self.skip_whitespace();
        let value = match self.peek() {
            Some('"' | '\'') => self.string()?,
            _ => self.ident()?,
        };
        self.skip_whitespace();
        let mut ignore_case = false;
        if let Some(c @ ('i' | 'I' | 's' | 'S')) = self.peek() {
            ignore_case = c.eq_ignore_ascii_case(&'i');
            self.pos += 1;
            self.skip_whitespace();
        }
        if self.peek() != Some(']') {
            return Err(SelectorError::Invalid);
        }
        self.pos += 1;
        Ok(Simple::Attr {
            name,
            op,
            value,
            ignore_case,
        })
    }

    /// A compound selector, which may be empty.
    fn compound(&mut self) -> Result<Vec<Simple>, SelectorError> {
        let mut out = Vec::new();
        match self.peek() {
            Some('*') => {
                self.pos += 1;
                if self.peek() == Some('|') {
                    return Err(SelectorError::Unsupported);
                }
                out.push(Simple::Type("*".to_owned()));
            }
            Some(c) if is_ident_start(c) || c == '-' || c == '\\' => {
                let name = self.ident()?;
                if self.peek() == Some('|') {
                    return Err(SelectorError::Unsupported);
                }
                out.push(Simple::Type(name.to_ascii_lowercase()));
            }
            _ => {}
        }
        loop {
            match self.peek() {
                Some('#') => {
                    self.pos += 1;
                    // An id is a name: it may start with a digit.
                    let start = self.pos;
                    while self.peek().is_some_and(is_ident_char) {
                        self.pos += 1;
                    }
                    if self.peek() == Some('\\') {
                        return Err(SelectorError::Unsupported);
                    }
                    if self.pos == start {
                        return Err(SelectorError::Invalid);
                    }
                    out.push(Simple::Id(self.chars[start..self.pos].iter().collect()));
                }
                Some('.') => {
                    self.pos += 1;
                    out.push(Simple::Class(self.ident()?));
                }
                Some('[') => out.push(self.attribute()?),
                Some(':') => return Err(SelectorError::Unsupported),
                _ => return Ok(out),
            }
        }
    }

    fn complex(&mut self) -> Result<Complex, SelectorError> {
        self.skip_whitespace();
        let mut compounds = vec![self.compound()?];
        let mut combinators = Vec::new();
        if compounds[0].is_empty() {
            return Err(SelectorError::Invalid);
        }
        loop {
            let had_space = self.skip_whitespace();
            let combinator = match self.peek() {
                None | Some(',') => break,
                Some('>') => Combinator::Child,
                Some('+') => Combinator::NextSibling,
                Some('~') => Combinator::SubsequentSibling,
                Some(_) if had_space => Combinator::Descendant,
                Some(_) => return Err(SelectorError::Invalid),
            };
            if !matches!(combinator, Combinator::Descendant) {
                self.pos += 1;
                self.skip_whitespace();
            }
            let compound = self.compound()?;
            if compound.is_empty() {
                return Err(SelectorError::Invalid);
            }
            combinators.push(combinator);
            compounds.push(compound);
        }
        let subject = compounds.pop().expect("a compound");
        let rest = combinators
            .into_iter()
            .rev()
            .zip(compounds.into_iter().rev());
        Ok(Complex {
            subject,
            rest: rest.collect(),
        })
    }
}

fn parse_selector_list(source: &str) -> Result<Vec<Complex>, SelectorError> {
    let mut parser = Parser {
        chars: source.chars().collect(),
        pos: 0,
    };
    let mut out = Vec::new();
    loop {
        out.push(parser.complex()?);
        match parser.peek() {
            None => return Ok(out),
            Some(',') => parser.pos += 1,
            Some(_) => return Err(SelectorError::Invalid),
        }
    }
}

fn matches_simple(element: &DomNode, simple: &Simple) -> bool {
    match simple {
        Simple::Type(name) => name == "*" || element.local_name().eq_ignore_ascii_case(name),
        Simple::Id(id) => element.get_attribute("id").as_deref() == Some(id.as_str()),
        Simple::Class(class) => element
            .get_attribute("class")
            .is_some_and(|c| c.split(is_css_whitespace).any(|t| t == class)),
        Simple::Attr {
            name,
            op,
            value,
            ignore_case,
        } => {
            let Some(actual) = element.get_attribute(name) else {
                return false;
            };
            let (actual, value) = if *ignore_case {
                (actual.to_ascii_lowercase(), value.to_ascii_lowercase())
            } else {
                (actual, value.clone())
            };
            match op {
                AttrOp::Exists => true,
                AttrOp::Equals => actual == value,
                AttrOp::Includes => {
                    !value.is_empty()
                        && !value.contains(is_css_whitespace)
                        && actual.split(is_css_whitespace).any(|t| t == value)
                }
                AttrOp::DashMatch => {
                    actual == value
                        || actual
                            .strip_prefix(value.as_str())
                            .is_some_and(|rest| rest.starts_with('-'))
                }
                AttrOp::Prefix => !value.is_empty() && actual.starts_with(&value),
                AttrOp::Suffix => !value.is_empty() && actual.ends_with(&value),
                AttrOp::Substring => !value.is_empty() && actual.contains(&value),
            }
        }
    }
}

fn matches_compound(element: &DomNode, compound: &[Simple]) -> bool {
    compound.iter().all(|s| matches_simple(element, s))
}

/// Whether `element` and its relatives match `rest` (combinators and the
/// compounds to their left, nearest first).
fn matches_rest(element: &DomNode, rest: &[(Combinator, Vec<Simple>)]) -> bool {
    let Some(((combinator, compound), more)) = rest.split_first() else {
        return true;
    };
    match combinator {
        Combinator::Child => element
            .parent_element()
            .is_some_and(|p| matches_compound(&p, compound) && matches_rest(&p, more)),
        Combinator::Descendant => {
            let mut ancestor = element.parent_element();
            while let Some(a) = ancestor {
                if matches_compound(&a, compound) && matches_rest(&a, more) {
                    return true;
                }
                ancestor = a.parent_element();
            }
            false
        }
        Combinator::NextSibling => element
            .previous_element_sibling()
            .is_some_and(|s| matches_compound(&s, compound) && matches_rest(&s, more)),
        Combinator::SubsequentSibling => {
            let mut sibling = element.previous_element_sibling();
            while let Some(s) = sibling {
                if matches_compound(&s, compound) && matches_rest(&s, more) {
                    return true;
                }
                sibling = s.previous_element_sibling();
            }
            false
        }
    }
}

fn matches_list(element: &DomNode, list: &[Complex]) -> bool {
    list.iter()
        .any(|c| matches_compound(element, &c.subject) && matches_rest(element, &c.rest))
}

impl DomNode {
    /// `querySelectorAll(selectors)`: the descendant elements that match, in
    /// document order.
    pub fn query_selector_all(&self, selectors: &str) -> Result<Vec<DomNode>, SelectorError> {
        let list = parse_selector_list(selectors)?;
        Ok(self
            .descendant_elements()
            .into_iter()
            .filter(|e| matches_list(e, &list))
            .collect())
    }

    /// `matches(selectors)`: whether this element matches.
    pub fn matches(&self, selectors: &str) -> Result<bool, SelectorError> {
        let list = parse_selector_list(selectors)?;
        Ok(self.is_element() && matches_list(self, &list))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::parse_from_string;

    fn ids(html: &str, selectors: &str) -> Result<Vec<String>, SelectorError> {
        let doc = parse_from_string(html);
        Ok(doc
            .query_selector_all(selectors)?
            .iter()
            .map(|e| e.id())
            .collect())
    }

    #[test]
    fn compounds_and_combinators() {
        let html = r#"<div id="d" class="a b"><p id="p1" data-x="one two" lang="en-US"><span id="s1"></span></p><p id="p2"></p><span id="s2"></span></div>"#;
        assert_eq!(ids(html, "P").unwrap(), ["p1", "p2"]);
        assert_eq!(ids(html, "div.a.b > p").unwrap(), ["p1", "p2"]);
        assert_eq!(ids(html, "div span").unwrap(), ["s1", "s2"]);
        assert_eq!(ids(html, "div > span").unwrap(), ["s2"]);
        assert_eq!(ids(html, "p + p, #p1 ~ span").unwrap(), ["p2", "s2"]);
        assert_eq!(ids(html, "[data-x~=two]").unwrap(), ["p1"]);
        assert_eq!(ids(html, "[lang|=en]").unwrap(), ["p1"]);
        assert_eq!(ids(html, r#"[data-x^="ONE" i]"#).unwrap(), ["p1"]);
        assert_eq!(ids(html, "[DATA-X$=two][data-x*='e t']").unwrap(), ["p1"]);
        assert_eq!(ids(html, "*").unwrap().len(), 8);
    }

    #[test]
    fn errors() {
        assert_eq!(ids("", "a:hover"), Err(SelectorError::Unsupported));
        assert_eq!(ids("", "svg|a"), Err(SelectorError::Unsupported));
        assert_eq!(ids("", "p >"), Err(SelectorError::Invalid));
        assert_eq!(ids("", ".1a"), Err(SelectorError::Invalid));
        assert_eq!(ids("", "p,,a"), Err(SelectorError::Invalid));
        assert_eq!(ids("", ""), Err(SelectorError::Invalid));
    }
}
