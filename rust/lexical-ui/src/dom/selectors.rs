//! `querySelectorAll` and `matches` for the selectors a pasted style sheet
//! uses: compound selectors (a type or `*`, `#id`, `.class`, attribute
//! selectors with every operator and the `i` and `s` flags, and
//! pseudo-classes) joined by the descendant, child (`>`), next-sibling
//! (`+`) and subsequent-sibling (`~`) combinators, in comma lists.
//!
//! The pseudo-classes are those of a document nobody interacts with, as
//! jsdom's selector engine (`@asamuzakjp/dom-selector`) matches them: the
//! tree-structural ones (`:root`, `:empty`, `:first-child` and the other
//! child and type positions, `:nth-child(An+B of S)` and the rest of the
//! `:nth-` family), the logical ones (`:not()`, `:is()` and `:where()`,
//! forgiving as in JS, and `:has()`), `:link` and `:any-link`, `:lang()`,
//! `:checked`, `:disabled` and `:enabled`, and `:scope`; the user action
//! and location ones (`:hover`, `:focus`, `:visited`, `:target`...) and
//! pseudo-elements (`::before`, `:first-line`...) match nothing. Their names
//! are lower case, as jsdom's engine takes no other.
//!
//! Anything else is an error, as an invalid selector is in JS (a
//! `SyntaxError`); in JS some of it is valid (namespaces, escapes, the
//! form-state and `:dir()` pseudo-classes), which
//! [`SelectorError::Unsupported`] tells apart. Type selectors and attribute
//! names match in any case, as in an HTML document; `#id` and `.class`
//! match as written (a no-quirks document).

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
    Pseudo(Pseudo),
}

/// A pseudo-class (or a pseudo-element, which matches nothing).
#[derive(Clone, Debug)]
enum Pseudo {
    Root,
    Empty,
    /// `:nth-child(An+B of S)` and its family: from the end with `last`,
    /// among siblings of the element's type with `of_type`.
    Nth {
        a: i64,
        b: i64,
        last: bool,
        of_type: bool,
        of: Option<Vec<Complex>>,
    },
    /// `:only-child` and `:only-of-type`.
    Only {
        of_type: bool,
    },
    Not(Vec<Complex>),
    /// `:is()` and `:where()`.
    Is(Vec<Complex>),
    Has(Vec<(Combinator, Complex)>),
    AnyLink,
    Lang(String),
    Checked,
    Disabled,
    Enabled,
    /// A state no element of a parsed document is in, or a pseudo-element.
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    /// A pseudo-class or pseudo-element, after its `:`.
    fn pseudo(&mut self) -> Result<Pseudo, SelectorError> {
        self.pos += 1;
        let element = self.peek() == Some(':');
        if element {
            self.pos += 1;
        }
        let name = self.ident()?;
        // jsdom's engine knows the names in lower case only.
        if name != name.to_ascii_lowercase() {
            return Err(SelectorError::Invalid);
        }
        let functional = self.peek() == Some('(');
        if element
            || matches!(
                name.as_str(),
                "before" | "after" | "first-line" | "first-letter"
            )
        {
            if functional {
                return Err(SelectorError::Unsupported);
            }
            return match name.as_str() {
                "before"
                | "after"
                | "first-line"
                | "first-letter"
                | "marker"
                | "selection"
                | "placeholder"
                | "backdrop"
                | "file-selector-button"
                | "target-text"
                | "spelling-error"
                | "grammar-error"
                | "cue" => Ok(Pseudo::Never),
                _ => Err(SelectorError::Invalid),
            };
        }
        if !functional {
            return match name.as_str() {
                "root" | "scope" => Ok(Pseudo::Root),
                "empty" => Ok(Pseudo::Empty),
                "first-child" | "last-child" | "first-of-type" | "last-of-type" => {
                    Ok(Pseudo::Nth {
                        a: 0,
                        b: 1,
                        last: name.starts_with("last"),
                        of_type: name.ends_with("of-type"),
                        of: None,
                    })
                }
                "only-child" => Ok(Pseudo::Only { of_type: false }),
                "only-of-type" => Ok(Pseudo::Only { of_type: true }),
                "link" | "any-link" => Ok(Pseudo::AnyLink),
                "checked" => Ok(Pseudo::Checked),
                "disabled" => Ok(Pseudo::Disabled),
                "enabled" => Ok(Pseudo::Enabled),
                "visited" | "hover" | "active" | "focus" | "focus-within" | "focus-visible"
                | "target" | "autofill" => Ok(Pseudo::Never),
                "read-only" | "read-write" | "required" | "optional" | "default"
                | "indeterminate" | "placeholder-shown" | "defined" | "valid" | "invalid"
                | "in-range" | "out-of-range" | "user-valid" | "user-invalid" | "blank"
                | "playing" | "paused" | "modal" | "fullscreen" | "popover-open" | "open"
                | "closed" | "local-link" | "current" | "past" | "future" => {
                    Err(SelectorError::Unsupported)
                }
                _ => Err(SelectorError::Invalid),
            };
        }
        self.pos += 1;
        let pseudo = match name.as_str() {
            "not" => Pseudo::Not(self.selector_list(false)?),
            "is" | "where" => Pseudo::Is(self.selector_list(true)?),
            "has" => Pseudo::Has(self.relative_list()?),
            "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
                let (a, b) = self.an_plus_b()?;
                let of_type = name.ends_with("of-type");
                self.skip_whitespace();
                let mut of = None;
                if !of_type && self.peek() != Some(')') {
                    if self.ident()? != "of" || !self.skip_whitespace() {
                        return Err(SelectorError::Invalid);
                    }
                    of = Some(self.selector_list(false)?);
                }
                Pseudo::Nth {
                    a,
                    b,
                    last: name.contains("last"),
                    of_type,
                    of,
                }
            }
            "lang" => {
                self.skip_whitespace();
                let range = match self.peek() {
                    Some('"' | '\'') => self.string()?,
                    _ => self.ident()?,
                };
                self.skip_whitespace();
                Pseudo::Lang(range)
            }
            "dir" | "host" | "host-context" | "state" | "nth-col" | "nth-last-col" => {
                return Err(SelectorError::Unsupported);
            }
            _ => return Err(SelectorError::Invalid),
        };
        if self.peek() != Some(')') {
            return Err(SelectorError::Invalid);
        }
        self.pos += 1;
        Ok(pseudo)
    }

    /// The argument of `:nth-child()` and its family: `odd`, `even`, or
    /// `An+B` with its parts optional.
    fn an_plus_b(&mut self) -> Result<(i64, i64), SelectorError> {
        self.skip_whitespace();
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-'))
        {
            self.pos += 1;
        }
        let mut text: String = self.chars[start..self.pos].iter().collect();
        text.make_ascii_lowercase();
        match text.as_str() {
            "odd" => return Ok((2, 1)),
            "even" => return Ok((2, 0)),
            _ => {}
        }
        let number = |digits: &str| -> Result<i64, SelectorError> {
            digits.parse().map_err(|_| SelectorError::Invalid)
        };
        let Some(n) = text.find('n') else {
            return Ok((0, number(&text)?));
        };
        let a = match &text[..n] {
            "" | "+" => 1,
            "-" => -1,
            digits => number(digits)?,
        };
        let mut b_text = text[n + 1..].to_owned();
        if b_text.is_empty() {
            // `An + B` with spaces around the sign.
            let had_space = self.skip_whitespace();
            if let Some(sign @ ('+' | '-')) = self.peek().filter(|_| had_space) {
                self.pos += 1;
                self.skip_whitespace();
                let start = self.pos;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.pos += 1;
                }
                if self.pos == start {
                    return Err(SelectorError::Invalid);
                }
                b_text = format!(
                    "{sign}{}",
                    self.chars[start..self.pos].iter().collect::<String>()
                );
            } else {
                return Ok((a, 0));
            }
        } else if !b_text.starts_with(['+', '-']) || b_text[1..].starts_with(['+', '-']) {
            return Err(SelectorError::Invalid);
        } else if b_text.len() == 1 {
            // `An+ B`: the sign, then the digits after a space.
            self.skip_whitespace();
            let start = self.pos;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
            if self.pos == start {
                return Err(SelectorError::Invalid);
            }
            b_text.extend(&self.chars[start..self.pos]);
        }
        Ok((a, number(&b_text)?))
    }

    /// A comma list inside a functional pseudo-class, up to its `)`.
    /// Forgiving (`:is()`, `:where()`), an item that isn't a selector is
    /// dropped, and the list may be empty.
    fn selector_list(&mut self, forgiving: bool) -> Result<Vec<Complex>, SelectorError> {
        let mut out = Vec::new();
        loop {
            let start = self.pos;
            match self.complex() {
                Ok(complex) => out.push(complex),
                Err(SelectorError::Invalid) if forgiving => self.skip_item(start),
                Err(e) => return Err(e),
            }
            match self.peek() {
                Some(',') => self.pos += 1,
                Some(')') => break,
                _ => return Err(SelectorError::Invalid),
            }
        }
        if out.is_empty() && !forgiving {
            return Err(SelectorError::Invalid);
        }
        Ok(out)
    }

    /// Moves from `start` to the next `,` or `)` at this nesting level.
    fn skip_item(&mut self, start: usize) {
        self.pos = start;
        let mut depth = 0usize;
        while let Some(c) = self.peek() {
            match c {
                '(' => depth += 1,
                ')' | ',' if depth == 0 => return,
                ')' => depth -= 1,
                _ => {}
            }
            self.pos += 1;
        }
    }

    /// `:has()`'s relative selectors, each with its leading combinator.
    fn relative_list(&mut self) -> Result<Vec<(Combinator, Complex)>, SelectorError> {
        let mut out = Vec::new();
        loop {
            self.skip_whitespace();
            let combinator = match self.peek() {
                Some('>') => Combinator::Child,
                Some('+') => Combinator::NextSibling,
                Some('~') => Combinator::SubsequentSibling,
                _ => Combinator::Descendant,
            };
            if combinator != Combinator::Descendant {
                self.pos += 1;
            }
            out.push((combinator, self.complex()?));
            match self.peek() {
                Some(',') => self.pos += 1,
                Some(')') => return Ok(out),
                _ => return Err(SelectorError::Invalid),
            }
        }
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
                Some(':') => out.push(Simple::Pseudo(self.pseudo()?)),
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
                None | Some(',' | ')') => break,
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
        Simple::Pseudo(pseudo) => matches_pseudo(element, pseudo),
    }
}

/// The form controls `:disabled` and `:enabled` apply to.
const FORM_CONTROLS: &[&str] = &[
    "button", "input", "select", "textarea", "optgroup", "option", "fieldset",
];

fn matches_pseudo(element: &DomNode, pseudo: &Pseudo) -> bool {
    let name = element.local_name();
    match pseudo {
        Pseudo::Root => element.parent_node().is_some_and(|p| p.is_document()),
        Pseudo::Empty => element
            .child_nodes()
            .iter()
            .all(|c| !c.is_element() && !(c.is_text() && !c.data().is_empty())),
        Pseudo::Nth {
            a,
            b,
            last,
            of_type,
            of,
        } => {
            if of.as_ref().is_some_and(|list| !matches_list(element, list)) {
                return false;
            }
            let Some(position) = nth_position(element, *last, *of_type, of.as_deref()) else {
                return false;
            };
            let p = position as i64 - b;
            if *a == 0 {
                p == 0
            } else {
                p % a == 0 && p / a >= 0
            }
        }
        Pseudo::Only { of_type } => {
            nth_position(element, false, *of_type, None) == Some(1)
                && nth_position(element, true, *of_type, None) == Some(1)
        }
        Pseudo::Not(list) => !matches_list(element, list),
        Pseudo::Is(list) => matches_list(element, list),
        Pseudo::Has(relatives) => relatives.iter().any(|(combinator, complex)| {
            has_candidates(element, *combinator)
                .iter()
                .any(|c| matches_complex(c, complex, Some((*combinator, element))))
        }),
        Pseudo::AnyLink => matches!(name, "a" | "area") && element.has_attribute("href"),
        Pseudo::Lang(range) => {
            let mut node = Some(element.clone());
            while let Some(n) = node {
                if let Some(lang) = n.get_attribute("lang") {
                    let (lang, range) = (lang.to_ascii_lowercase(), range.to_ascii_lowercase());
                    return lang == range
                        || lang
                            .strip_prefix(range.as_str())
                            .is_some_and(|rest| rest.starts_with('-'));
                }
                node = n.parent_element();
            }
            false
        }
        Pseudo::Checked => match name {
            "input" => {
                element.has_attribute("checked")
                    && element.get_attribute("type").is_some_and(|t| {
                        t.eq_ignore_ascii_case("checkbox") || t.eq_ignore_ascii_case("radio")
                    })
            }
            "option" => element.has_attribute("selected"),
            _ => false,
        },
        Pseudo::Disabled => FORM_CONTROLS.contains(&name) && element.has_attribute("disabled"),
        Pseudo::Enabled => FORM_CONTROLS.contains(&name) && !element.has_attribute("disabled"),
        Pseudo::Never => false,
    }
}

/// The element's 1-based position among its siblings (from the end with
/// `last`), counting those of its type with `of_type`, or those that match
/// `of`.
fn nth_position(
    element: &DomNode,
    last: bool,
    of_type: bool,
    of: Option<&[Complex]>,
) -> Option<usize> {
    let siblings = element.parent_node()?.children();
    let counted = siblings.iter().filter(|s| {
        (!of_type || s.local_name() == element.local_name())
            && of.is_none_or(|list| matches_list(s, list))
    });
    let identity = element.identity();
    let position = |mut it: Box<dyn Iterator<Item = &DomNode> + '_>| {
        it.position(|s| s.identity() == identity).map(|i| i + 1)
    };
    if last {
        position(Box::new(counted.rev()))
    } else {
        position(Box::new(counted))
    }
}

/// The elements a relative selector of `:has()` on `anchor` may match:
/// its descendants, or its following siblings and theirs.
fn has_candidates(anchor: &DomNode, combinator: Combinator) -> Vec<DomNode> {
    match combinator {
        Combinator::Descendant | Combinator::Child => anchor.descendant_elements(),
        Combinator::NextSibling | Combinator::SubsequentSibling => {
            let mut out = Vec::new();
            let mut sibling = anchor.next_element_sibling();
            while let Some(s) = sibling {
                out.push(s.clone());
                out.extend(s.descendant_elements());
                sibling = s.next_element_sibling();
            }
            out
        }
    }
}

/// Whether `element` stands in `combinator`'s relation to `anchor`.
fn is_related(element: &DomNode, combinator: Combinator, anchor: &DomNode) -> bool {
    let anchor = anchor.identity();
    match combinator {
        Combinator::Child => element
            .parent_element()
            .is_some_and(|p| p.identity() == anchor),
        Combinator::Descendant => {
            let mut ancestor = element.parent_element();
            while let Some(a) = ancestor {
                if a.identity() == anchor {
                    return true;
                }
                ancestor = a.parent_element();
            }
            false
        }
        Combinator::NextSibling => element
            .previous_element_sibling()
            .is_some_and(|s| s.identity() == anchor),
        Combinator::SubsequentSibling => {
            let mut sibling = element.previous_element_sibling();
            while let Some(s) = sibling {
                if s.identity() == anchor {
                    return true;
                }
                sibling = s.previous_element_sibling();
            }
            false
        }
    }
}

fn matches_compound(element: &DomNode, compound: &[Simple]) -> bool {
    compound.iter().all(|s| matches_simple(element, s))
}

/// Whether `element` matches `complex`; with an anchor (a `:has()`
/// argument), its leftmost compound must stand in the anchor's relation.
fn matches_complex(
    element: &DomNode,
    complex: &Complex,
    anchor: Option<(Combinator, &DomNode)>,
) -> bool {
    matches_compound(element, &complex.subject) && matches_rest(element, &complex.rest, anchor)
}

/// Whether `element` and its relatives match `rest` (combinators and the
/// compounds to their left, nearest first), the last of them related to
/// `anchor` when there is one.
fn matches_rest(
    element: &DomNode,
    rest: &[(Combinator, Vec<Simple>)],
    anchor: Option<(Combinator, &DomNode)>,
) -> bool {
    let Some(((combinator, compound), more)) = rest.split_first() else {
        return anchor.is_none_or(|(c, a)| is_related(element, c, a));
    };
    let step = |e: &DomNode| matches_compound(e, compound) && matches_rest(e, more, anchor);
    match combinator {
        Combinator::Child => element.parent_element().is_some_and(|p| step(&p)),
        Combinator::Descendant => {
            let mut ancestor = element.parent_element();
            while let Some(a) = ancestor {
                if step(&a) {
                    return true;
                }
                ancestor = a.parent_element();
            }
            false
        }
        Combinator::NextSibling => element.previous_element_sibling().is_some_and(|s| step(&s)),
        Combinator::SubsequentSibling => {
            let mut sibling = element.previous_element_sibling();
            while let Some(s) = sibling {
                if step(&s) {
                    return true;
                }
                sibling = s.previous_element_sibling();
            }
            false
        }
    }
}

fn matches_list(element: &DomNode, list: &[Complex]) -> bool {
    list.iter().any(|c| matches_complex(element, c, None))
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
        assert_eq!(ids("", "a:hover"), Ok(vec![]));
        assert_eq!(ids("", ":dir(ltr)"), Err(SelectorError::Unsupported));
        assert_eq!(ids("", ":foo"), Err(SelectorError::Invalid));
        assert_eq!(ids("", "LI:NTH-CHILD(2)"), Err(SelectorError::Invalid));
        assert_eq!(ids("", ":not()"), Err(SelectorError::Invalid));
        assert_eq!(ids("", "svg|a"), Err(SelectorError::Unsupported));
        assert_eq!(ids("", "p >"), Err(SelectorError::Invalid));
        assert_eq!(ids("", ".1a"), Err(SelectorError::Invalid));
        assert_eq!(ids("", "p,,a"), Err(SelectorError::Invalid));
        assert_eq!(ids("", ""), Err(SelectorError::Invalid));
    }
}
