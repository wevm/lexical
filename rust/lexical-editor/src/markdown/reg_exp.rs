//! JS regular expressions (`RegExp`), for the markdown feature.
//!
//! Upstream's transformers are written as JS `RegExp`s, and their sources
//! and flags are public API: custom transformers pass their own, and import
//! rebuilds them with `new RegExp(regExp.source, flags)`. The default build
//! has no third-party dependencies, so this is a small backtracking engine
//! that follows ECMA-262's pattern semantics (22.2.2) on the UTF-16 code
//! units of the text, as JS does: offsets are code units, a pattern without
//! the `u` flag reads a character outside the BMP as two, and one with it as
//! one code point.
//!
//! It implements the syntax the markdown package and its tests use, and the
//! rest of the common syntax: alternation, capturing, non-capturing and
//! named groups, lookahead and lookbehind, backreferences (numbered and
//! named), greedy and lazy quantifiers (with the spec's capture reset and
//! empty-iteration check), `^`, `$`, `\b`, `\B`, `.`, classes with ranges,
//! `\d \D \w \W \s \S`, `\p{..}` and `\P{..}` with `u` for the general
//! categories in `reg_exp_tables.rs` (generated from Node), and the flags
//! `d g i m s u y`. Not implemented: the `v` flag, and Annex B's legacy
//! octal escapes and `\c` inside classes. Two documented approximations
//! (rust/PARITY.md): with `u` and `i`, characters are folded with the
//! `Canonicalize` of `i` without `u` (`toUpperCase`), not Unicode simple
//! case folding; and a lookbehind is matched forwards from each earlier
//! position, which agrees with JS on whether it matches but may capture a
//! different substring inside it.

use super::reg_exp_tables::{CANONICALIZE, LETTER, NUMBER, PUNCTUATION, SPACE_SEPARATOR, SYMBOL};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

/// A `RegExp`: `/source/flags`.
#[derive(Clone)]
pub struct RegExp {
    source: String,
    flags: String,
    program: Rc<Program>,
    /// `regExp.lastIndex`, in UTF-16 code units, for `g` and `y`.
    last_index: Cell<usize>,
}

struct Program {
    root: Node,
    /// The number of capturing groups, without the whole match.
    groups: usize,
    names: Vec<(String, usize)>,
    ignore_case: bool,
    multiline: bool,
    dot_all: bool,
    unicode: bool,
    global: bool,
    sticky: bool,
}

impl fmt::Debug for RegExp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}/{}", self.source, self.flags)
    }
}

impl PartialEq for RegExp {
    /// The same source and flags.
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.flags == other.flags
    }
}

/// A `RegExpExecArray`: `[0]` the match, then each group (`None` for one
/// that did not participate, JS `undefined`), with `index` in UTF-16 code
/// units. Code that builds a match array by hand, as upstream does, builds
/// one of these.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegExpMatch {
    pub groups: Vec<Option<String>>,
    /// `match.indices`: the UTF-16 range of each group, when known.
    pub ranges: Vec<Option<(usize, usize)>>,
    /// The named groups, by name, with their group number.
    pub names: Vec<(String, usize)>,
    /// `match.index`.
    pub index: usize,
    /// `match.input`.
    pub input: String,
}

impl RegExpMatch {
    /// A match array of `groups`, as `[a, b, c]` with `index` and `input`
    /// set by hand.
    pub fn new(groups: Vec<Option<String>>, index: usize, input: &str) -> Self {
        Self {
            groups,
            ranges: Vec::new(),
            names: Vec::new(),
            index,
            input: input.to_owned(),
        }
    }

    /// `match[i]`: `None` for `undefined`.
    pub fn get(&self, i: usize) -> Option<&str> {
        self.groups.get(i).and_then(|g| g.as_deref())
    }

    /// `match[i]`, with `undefined` as `''` (JS's `match[i] || ''`).
    pub fn str(&self, i: usize) -> &str {
        self.get(i).unwrap_or("")
    }

    /// `match[0]`.
    pub fn text(&self) -> &str {
        self.str(0)
    }

    /// `match.length`.
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// `match.groups[name]`.
    pub fn named(&self, name: &str) -> Option<&str> {
        let (_, i) = self.names.iter().find(|(n, _)| n == name)?;
        self.get(*i)
    }

    /// `match[0].length`, in UTF-16 code units.
    pub fn match_len(&self) -> usize {
        crate::utf16::len(self.text())
    }
}

impl std::ops::Index<usize> for RegExpMatch {
    type Output = str;
    fn index(&self, i: usize) -> &str {
        self.str(i)
    }
}

impl RegExp {
    /// `new RegExp(source, flags)`, or the `SyntaxError` JS throws.
    pub fn new(source: &str, flags: &str) -> Result<Self, String> {
        let mut sorted: Vec<char> = flags.chars().collect();
        sorted.sort_unstable();
        for (i, f) in sorted.iter().enumerate() {
            if !"dgimsuy".contains(*f) || (i > 0 && sorted[i - 1] == *f) {
                return Err(format!(
                    "Invalid flags supplied to RegExp constructor '{flags}'"
                ));
            }
        }
        let sorted_flags: String = sorted.iter().collect();
        let has = |c| sorted.contains(&c);
        let unicode = has('u');
        let units: Vec<u32> = if unicode {
            source.chars().map(|c| c as u32).collect()
        } else {
            source.encode_utf16().map(u32::from).collect()
        };
        let mut parser = Parser {
            src: &units,
            pos: 0,
            unicode,
            groups: 0,
            names: Vec::new(),
            total_groups: count_groups(&units),
            named_refs: Vec::new(),
        };
        let root = parser.alternation()?;
        if parser.pos < units.len() {
            return Err(format!(
                "Invalid regular expression: /{source}/: Unmatched ')'"
            ));
        }
        let names = parser.names;
        let mut root = root;
        if !parser.named_refs.is_empty() {
            resolve_named(&mut root, &names)?;
        }
        Ok(Self {
            source: source.to_owned(),
            flags: sorted_flags,
            program: Rc::new(Program {
                root,
                groups: parser.groups,
                names,
                ignore_case: has('i'),
                multiline: has('m'),
                dot_all: has('s'),
                unicode,
                global: has('g'),
                sticky: has('y'),
            }),
            last_index: Cell::new(0),
        })
    }

    /// `/source/flags`, for a pattern known to be valid; panics otherwise.
    pub fn of(source: &str, flags: &str) -> Self {
        Self::new(source, flags)
            .unwrap_or_else(|e| panic!("invalid regular expression /{source}/{flags}: {e}"))
    }

    /// `regExp.source`.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// `regExp.flags`, in JS's order (`dgimsuy`).
    pub fn flags(&self) -> &str {
        &self.flags
    }

    pub fn global(&self) -> bool {
        self.program.global
    }

    pub fn sticky(&self) -> bool {
        self.program.sticky
    }

    /// `regExp.lastIndex`.
    pub fn last_index(&self) -> usize {
        self.last_index.get()
    }

    pub fn set_last_index(&self, index: usize) {
        self.last_index.set(index);
    }

    /// `regExp.exec(text)`: from `lastIndex` for a `g` or `y` regex, which
    /// it then moves past the match (or back to 0).
    pub fn exec(&self, text: &str) -> Option<RegExpMatch> {
        let units: Vec<u16> = text.encode_utf16().collect();
        self.exec_units(&units, text)
    }

    fn exec_units(&self, units: &[u16], text: &str) -> Option<RegExpMatch> {
        let p = &self.program;
        let uses_last_index = p.global || p.sticky;
        let start = if uses_last_index {
            self.last_index.get()
        } else {
            0
        };
        let found = self.find_at(units, start);
        match found {
            None => {
                if uses_last_index {
                    self.last_index.set(0);
                }
                None
            }
            Some(caps) => {
                let (s, e) = caps[0].expect("the whole match");
                if uses_last_index {
                    self.last_index.set(e);
                }
                Some(self.to_match(units, text, caps, s))
            }
        }
    }

    fn to_match(
        &self,
        units: &[u16],
        text: &str,
        caps: Vec<Option<(usize, usize)>>,
        index: usize,
    ) -> RegExpMatch {
        RegExpMatch {
            groups: caps
                .iter()
                .map(|c| c.map(|(s, e)| String::from_utf16_lossy(&units[s..e])))
                .collect(),
            ranges: caps,
            names: self.program.names.clone(),
            index,
            input: text.to_owned(),
        }
    }

    /// The captures of the first match at or after `start` (only at it, for
    /// a sticky regex), without touching `lastIndex`.
    fn find_at(&self, units: &[u16], start: usize) -> Option<Vec<Option<(usize, usize)>>> {
        let p = &self.program;
        if start > units.len() {
            return None;
        }
        let mut ctx = Ctx {
            text: units,
            p,
            caps: vec![None; p.groups + 1],
        };
        let mut s = start;
        loop {
            let mut end = None;
            if m(&p.root, s, &mut ctx, &mut |e, _| {
                end = Some(e);
                true
            }) {
                ctx.caps[0] = Some((s, end.expect("an end")));
                return Some(ctx.caps);
            }
            if p.sticky || s >= units.len() {
                return None;
            }
            s = advance(units, s, p.unicode);
            for c in ctx.caps.iter_mut() {
                *c = None;
            }
        }
    }

    /// `regExp.test(text)`.
    pub fn test(&self, text: &str) -> bool {
        self.exec(text).is_some()
    }

    /// The UTF-16 offset of the first match, as `text.search(regExp)`
    /// (`None` for -1).
    pub fn search(&self, text: &str) -> Option<usize> {
        let units: Vec<u16> = text.encode_utf16().collect();
        self.find_at(&units, 0).map(|c| c[0].expect("a match").0)
    }

    /// `text.match(regExp)` for a regex without `g`: the first match.
    pub fn match_first(&self, text: &str) -> Option<RegExpMatch> {
        if self.program.global {
            let units: Vec<u16> = text.encode_utf16().collect();
            return self.find_at(&units, 0).map(|c| {
                let s = c[0].expect("a match").0;
                self.to_match(&units, text, c, s)
            });
        }
        self.exec(text)
    }

    /// `text.match(regExp)`: the first match (from `lastIndex` for a sticky
    /// regex) without `g`; with it, every match's `[0]` as the groups of
    /// one match array at index 0, as JS's array of strings reads.
    pub fn string_match(&self, text: &str) -> Option<RegExpMatch> {
        if !self.program.global {
            return self.exec(text);
        }
        let all = self.match_all(text);
        self.last_index.set(0);
        if all.is_empty() {
            return None;
        }
        Some(RegExpMatch::new(
            all.into_iter()
                .map(|m| m.groups.into_iter().next().flatten())
                .collect(),
            0,
            text,
        ))
    }

    /// Every match, as `text.matchAll(regExp)` or a `g` regex's
    /// `text.match(regExp)` (whose strings are each match's `[0]`).
    pub fn match_all(&self, text: &str) -> Vec<RegExpMatch> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut out = Vec::new();
        let mut at = 0;
        while at <= units.len() {
            let Some(caps) = self.find_at(&units, at) else {
                break;
            };
            let (s, e) = caps[0].expect("a match");
            out.push(self.to_match(&units, text, caps, s));
            at = if e == s {
                advance(&units, e, self.program.unicode)
            } else {
                e
            };
            if self.program.sticky && out.last().is_some_and(|m| m.index != s) {
                break;
            }
        }
        out
    }

    /// `text.replace(regExp, fn)`: every match for a `g` regex, else the
    /// first, replaced by what `replacer` returns for it.
    pub fn replace_with(
        &self,
        text: &str,
        mut replacer: impl FnMut(&RegExpMatch) -> String,
    ) -> String {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut out: Vec<u16> = Vec::with_capacity(units.len());
        let mut from = 0;
        let mut at = if self.program.global {
            0
        } else {
            self.last_index_for_replace()
        };
        while at <= units.len() {
            let Some(caps) = self.find_at(&units, at) else {
                break;
            };
            let (s, e) = caps[0].expect("a match");
            let found = self.to_match(&units, text, caps, s);
            out.extend_from_slice(&units[from..s]);
            out.extend(replacer(&found).encode_utf16());
            from = e;
            if !self.program.global {
                break;
            }
            at = if e == s {
                advance(&units, e, self.program.unicode)
            } else {
                e
            };
        }
        out.extend_from_slice(&units[from..]);
        String::from_utf16_lossy(&out)
    }

    fn last_index_for_replace(&self) -> usize {
        if self.program.sticky {
            self.last_index.get()
        } else {
            0
        }
    }

    /// `text.replace(regExp, template)`, with `$$`, `$&`, `` $` ``, `$'`,
    /// `$n`, `$nn` and `$<name>` (`GetSubstitution`).
    pub fn replace(&self, text: &str, template: &str) -> String {
        self.replace_with(text, |m| substitute(m, template))
    }

    /// `text.split(regExp)`, without a limit. A group that did not
    /// participate in a separator's match, which JS splices in as
    /// `undefined`, is left out.
    pub fn split(&self, text: &str) -> Vec<String> {
        let units: Vec<u16> = text.encode_utf16().collect();
        if units.is_empty() {
            return if self.find_at(&units, 0).is_some() {
                Vec::new()
            } else {
                vec![String::new()]
            };
        }
        let mut out = Vec::new();
        let mut p = 0;
        let mut q = p;
        while q < units.len() {
            let sticky = RegExp {
                program: self.program.clone(),
                source: self.source.clone(),
                flags: self.flags.clone(),
                last_index: Cell::new(0),
            };
            match sticky.match_sticky(&units, q) {
                Some(caps) if caps[0].expect("a match").1 != p => {
                    let e = caps[0].expect("a match").1;
                    out.push(String::from_utf16_lossy(&units[p..q]));
                    for (s, e) in caps.iter().skip(1).flatten() {
                        out.push(String::from_utf16_lossy(&units[*s..*e]));
                    }
                    p = e;
                    q = p;
                }
                _ => q = advance(&units, q, self.program.unicode),
            }
        }
        out.push(String::from_utf16_lossy(&units[p..]));
        out
    }

    fn match_sticky(&self, units: &[u16], at: usize) -> Option<Vec<Option<(usize, usize)>>> {
        let p = &self.program;
        let mut ctx = Ctx {
            text: units,
            p,
            caps: vec![None; p.groups + 1],
        };
        let mut end = None;
        if m(&p.root, at, &mut ctx, &mut |e, _| {
            end = Some(e);
            true
        }) {
            ctx.caps[0] = Some((at, end.expect("an end")));
            Some(ctx.caps)
        } else {
            None
        }
    }
}

/// `GetSubstitution`.
fn substitute(m: &RegExpMatch, template: &str) -> String {
    let t: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let groups = m.len().saturating_sub(1);
    while i < t.len() {
        if t[i] != '$' || i + 1 == t.len() {
            out.push(t[i]);
            i += 1;
            continue;
        }
        let c = t[i + 1];
        match c {
            '$' => {
                out.push('$');
                i += 2;
            }
            '&' => {
                out.push_str(m.text());
                i += 2;
            }
            '`' => {
                out.push_str(&crate::utf16::slice(&m.input, 0, m.index));
                i += 2;
            }
            '\'' => {
                out.push_str(&crate::utf16::slice_from(&m.input, m.index + m.match_len()));
                i += 2;
            }
            '0'..='9' => {
                let d1 = c as usize - '0' as usize;
                let two = t
                    .get(i + 2)
                    .filter(|d| d.is_ascii_digit())
                    .map(|d| d1 * 10 + (*d as usize - '0' as usize));
                if let Some(n) = two.filter(|n| *n >= 1 && *n <= groups) {
                    out.push_str(m.str(n));
                    i += 3;
                } else if d1 >= 1 && d1 <= groups {
                    out.push_str(m.str(d1));
                    i += 2;
                } else {
                    out.push('$');
                    i += 1;
                }
            }
            '<' if !m.names.is_empty() => match t[i + 2..].iter().position(|c| *c == '>') {
                Some(close) => {
                    let name: String = t[i + 2..i + 2 + close].iter().collect();
                    out.push_str(m.named(&name).unwrap_or(""));
                    i += close + 3;
                }
                None => {
                    out.push('$');
                    i += 1;
                }
            },
            _ => {
                out.push('$');
                i += 1;
            }
        }
    }
    out
}

/// `AdvanceStringIndex`.
fn advance(units: &[u16], i: usize, unicode: bool) -> usize {
    if unicode && i + 1 < units.len() && is_high(units[i]) && is_low(units[i + 1]) {
        i + 2
    } else {
        i + 1
    }
}

fn is_high(u: u16) -> bool {
    (0xD800..0xDC00).contains(&u)
}

fn is_low(u: u16) -> bool {
    (0xDC00..0xE000).contains(&u)
}

// --- Syntax -------------------------------------------------------------

#[derive(Debug)]
enum Node {
    Empty,
    Char(u32),
    Any,
    Class(Class),
    Start,
    End,
    WordBoundary(bool),
    Group(Box<Node>, Option<usize>),
    Look {
        node: Box<Node>,
        ahead: bool,
        negate: bool,
    },
    BackRef(usize),
    NamedBackRef(String),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: u32,
        max: Option<u32>,
        greedy: bool,
        /// The capturing groups inside, `first..last`.
        groups: (usize, usize),
    },
}

#[derive(Debug, Clone)]
enum Item {
    Range(u32, u32),
    Digit(bool),
    Word(bool),
    Space(bool),
    Prop(Prop, bool),
}

#[derive(Debug, Clone, Copy)]
enum Prop {
    Table(&'static [(u32, u32)]),
    Control,
    Surrogate,
    Noncharacter,
    Any,
    Ascii,
}

#[derive(Debug)]
struct Class {
    negate: bool,
    items: Vec<Item>,
}

struct Parser<'a> {
    src: &'a [u32],
    pos: usize,
    unicode: bool,
    groups: usize,
    names: Vec<(String, usize)>,
    total_groups: usize,
    named_refs: Vec<String>,
}

/// The number of capturing groups in a pattern, for reading `\N`.
fn count_groups(src: &[u32]) -> usize {
    let mut n = 0;
    let mut i = 0;
    let mut in_class = false;
    while i < src.len() {
        let c = src[i];
        if c == '\\' as u32 {
            i += 2;
            continue;
        }
        if in_class {
            in_class = c != ']' as u32;
        } else if c == '[' as u32 {
            in_class = true;
        } else if c == '(' as u32 {
            let next = src.get(i + 1).copied();
            let after = src.get(i + 2).copied();
            if next != Some('?' as u32)
                || (after == Some('<' as u32)
                    && !matches!(
                        src.get(i + 3).map(|c| char::from_u32(*c)),
                        Some(Some('=' | '!'))
                    ))
            {
                n += 1;
            }
        }
        i += 1;
    }
    n
}

fn resolve_named(node: &mut Node, names: &[(String, usize)]) -> Result<(), String> {
    match node {
        Node::NamedBackRef(name) => {
            let (_, i) = names
                .iter()
                .find(|(n, _)| n == name)
                .ok_or_else(|| "Invalid named capture referenced".to_owned())?;
            *node = Node::BackRef(*i);
        }
        Node::Group(n, _) | Node::Look { node: n, .. } | Node::Repeat { node: n, .. } => {
            resolve_named(n, names)?
        }
        Node::Concat(ns) | Node::Alt(ns) => {
            for n in ns {
                resolve_named(n, names)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn err<T>(message: &str) -> Result<T, String> {
    Err(format!("Invalid regular expression: {message}"))
}

impl Parser<'_> {
    fn peek(&self) -> Option<u32> {
        self.src.get(self.pos).copied()
    }

    fn peek_is(&self, c: char) -> bool {
        self.peek() == Some(c as u32)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek_is(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn alternation(&mut self) -> Result<Node, String> {
        let mut alts = vec![self.sequence()?];
        while self.eat('|') {
            alts.push(self.sequence()?);
        }
        Ok(if alts.len() == 1 {
            alts.pop().expect("one")
        } else {
            Node::Alt(alts)
        })
    }

    fn sequence(&mut self) -> Result<Node, String> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' as u32 || c == ')' as u32 {
                break;
            }
            let groups_before = self.groups;
            let atom = self.atom()?;
            let quantifiable = !matches!(
                atom,
                Node::Start | Node::End | Node::WordBoundary(_) | Node::Look { ahead: false, .. }
            ) && !(self.unicode && matches!(atom, Node::Look { .. }));
            let atom = self.quantifier(atom, groups_before, quantifiable)?;
            items.push(atom);
        }
        Ok(match items.len() {
            0 => Node::Empty,
            1 => items.pop().expect("one"),
            _ => Node::Concat(items),
        })
    }

    /// `{n}`, `{n,}` or `{n,m}` at the current position, if there is one.
    fn braces(&mut self) -> Option<(u32, Option<u32>)> {
        let start = self.pos;
        if !self.eat('{') {
            return None;
        }
        let number = |p: &mut Self| -> Option<u32> {
            let s = p.pos;
            let mut v: u64 = 0;
            while let Some(d) = p.peek().filter(|d| (b'0' as u32..=b'9' as u32).contains(d)) {
                v = (v * 10 + u64::from(d - b'0' as u32)).min(u64::from(u32::MAX));
                p.pos += 1;
            }
            (p.pos > s).then_some(v as u32)
        };
        let Some(min) = number(self) else {
            self.pos = start;
            return None;
        };
        let max = if self.eat(',') {
            if self.peek_is('}') {
                None
            } else {
                match number(self) {
                    Some(m) => Some(m),
                    None => {
                        self.pos = start;
                        return None;
                    }
                }
            }
        } else {
            Some(min)
        };
        if !self.eat('}') {
            self.pos = start;
            return None;
        }
        Some((min, max))
    }

    fn quantifier(
        &mut self,
        atom: Node,
        groups_before: usize,
        quantifiable: bool,
    ) -> Result<Node, String> {
        let (min, max) = match self.peek().and_then(char::from_u32) {
            Some('*') => {
                self.pos += 1;
                (0, None)
            }
            Some('+') => {
                self.pos += 1;
                (1, None)
            }
            Some('?') => {
                self.pos += 1;
                (0, Some(1))
            }
            Some('{') => match self.braces() {
                Some(q) => q,
                None if self.unicode => return err("Incomplete quantifier"),
                None => return Ok(atom),
            },
            _ => return Ok(atom),
        };
        if !quantifiable {
            return err("Nothing to repeat");
        }
        if max.is_some_and(|m| m < min) {
            return err("numbers out of order in {} quantifier");
        }
        let greedy = !self.eat('?');
        Ok(Node::Repeat {
            node: Box::new(atom),
            min,
            max,
            greedy,
            groups: (groups_before + 1, self.groups + 1),
        })
    }

    fn atom(&mut self) -> Result<Node, String> {
        let c = self.peek().expect("an atom");
        self.pos += 1;
        Ok(match char::from_u32(c) {
            Some('^') => Node::Start,
            Some('$') => Node::End,
            Some('.') => Node::Any,
            Some('*' | '+' | '?') => return err("Nothing to repeat"),
            Some('{') => {
                self.pos -= 1;
                if self.unicode || self.braces().is_some() {
                    return err("Nothing to repeat");
                }
                self.pos += 1;
                Node::Char(c)
            }
            Some('}' | ']') if self.unicode => return err("Lone quantifier brackets"),
            Some('(') => self.group()?,
            Some('[') => Node::Class(self.class()?),
            Some('\\') => self.atom_escape()?,
            _ => {
                if !self.unicode && is_high(c as u16) && false {
                    unreachable!()
                }
                Node::Char(c)
            }
        })
    }

    fn group(&mut self) -> Result<Node, String> {
        let node = if self.eat('?') {
            if self.eat(':') {
                Node::Group(Box::new(self.alternation()?), None)
            } else if self.eat('=') {
                look(self.alternation()?, true, false)
            } else if self.eat('!') {
                look(self.alternation()?, true, true)
            } else if self.eat('<') {
                if self.eat('=') {
                    look(self.alternation()?, false, false)
                } else if self.eat('!') {
                    look(self.alternation()?, false, true)
                } else {
                    let name = self.group_name()?;
                    if self.names.iter().any(|(n, _)| *n == name) {
                        return err("Duplicate capture group name");
                    }
                    self.groups += 1;
                    let index = self.groups;
                    self.names.push((name, index));
                    Node::Group(Box::new(self.alternation()?), Some(index))
                }
            } else {
                return err("Invalid group");
            }
        } else {
            self.groups += 1;
            let index = self.groups;
            Node::Group(Box::new(self.alternation()?), Some(index))
        };
        if !self.eat(')') {
            return err("Unterminated group");
        }
        Ok(node)
    }

    fn group_name(&mut self) -> Result<String, String> {
        let mut name = String::new();
        loop {
            match self.peek().and_then(char::from_u32) {
                Some('>') => {
                    self.pos += 1;
                    break;
                }
                Some(c) if c == '$' || c == '_' || c.is_alphanumeric() => {
                    name.push(c);
                    self.pos += 1;
                }
                _ => return err("Invalid capture group name"),
            }
        }
        if name.is_empty() {
            return err("Invalid capture group name");
        }
        Ok(name)
    }

    fn atom_escape(&mut self) -> Result<Node, String> {
        let Some(c) = self.peek() else {
            return err("\\ at end of pattern");
        };
        match char::from_u32(c) {
            Some('b') => {
                self.pos += 1;
                Ok(Node::WordBoundary(true))
            }
            Some('B') => {
                self.pos += 1;
                Ok(Node::WordBoundary(false))
            }
            Some('1'..='9') => {
                let start = self.pos;
                let mut n = 0usize;
                while let Some(d) = self
                    .peek()
                    .filter(|d| (b'0' as u32..=b'9' as u32).contains(d))
                {
                    n = n
                        .saturating_mul(10)
                        .saturating_add((d - b'0' as u32) as usize);
                    self.pos += 1;
                }
                if n <= self.total_groups {
                    Ok(Node::BackRef(n))
                } else if self.unicode {
                    err("Invalid escape")
                } else {
                    // Annex B: an identity escape of the first digit.
                    self.pos = start + 1;
                    Ok(Node::Char(c))
                }
            }
            Some('k') if self.unicode || !self.names_absent() => {
                self.pos += 1;
                if !self.eat('<') {
                    return err("Invalid named reference");
                }
                let name = self.group_name()?;
                self.named_refs.push(name.clone());
                Ok(Node::NamedBackRef(name))
            }
            _ => match self.class_escape()? {
                Item::Range(a, b) if a == b => Ok(Node::Char(a)),
                item => Ok(Node::Class(Class {
                    negate: false,
                    items: vec![item],
                })),
            },
        }
    }

    /// Whether the pattern has no named groups, so that `\k` is an identity
    /// escape (Annex B).
    fn names_absent(&self) -> bool {
        let mut i = 0;
        while i + 2 < self.src.len() {
            if self.src[i] == '(' as u32
                && self.src[i + 1] == '?' as u32
                && self.src[i + 2] == '<' as u32
                && !matches!(
                    self.src.get(i + 3).and_then(|c| char::from_u32(*c)),
                    Some('=' | '!')
                )
            {
                return false;
            }
            i += 1;
        }
        true
    }

    /// An escape after `\` that stands for characters, inside or outside a
    /// class.
    fn class_escape(&mut self) -> Result<Item, String> {
        let Some(c) = self.peek() else {
            return err("\\ at end of pattern");
        };
        self.pos += 1;
        let ch = |v: u32| Ok(Item::Range(v, v));
        match char::from_u32(c) {
            Some('d') => Ok(Item::Digit(false)),
            Some('D') => Ok(Item::Digit(true)),
            Some('w') => Ok(Item::Word(false)),
            Some('W') => Ok(Item::Word(true)),
            Some('s') => Ok(Item::Space(false)),
            Some('S') => Ok(Item::Space(true)),
            Some('t') => ch(9),
            Some('n') => ch(10),
            Some('v') => ch(11),
            Some('f') => ch(12),
            Some('r') => ch(13),
            Some('0')
                if !self
                    .peek()
                    .is_some_and(|d| (b'0' as u32..=b'9' as u32).contains(&d)) =>
            {
                ch(0)
            }
            Some('c') => match self.peek().and_then(char::from_u32) {
                Some(l) if l.is_ascii_alphabetic() => {
                    self.pos += 1;
                    ch(l as u32 % 32)
                }
                _ if self.unicode => err("Invalid unicode escape"),
                _ => {
                    self.pos -= 1;
                    ch('\\' as u32)
                }
            },
            Some('x') => match self.hex(2) {
                Some(v) => ch(v),
                None if self.unicode => err("Invalid escape"),
                None => ch('x' as u32),
            },
            Some('u') => {
                if self.unicode && self.eat('{') {
                    let start = self.pos;
                    let mut v: u32 = 0;
                    while let Some(d) = self
                        .peek()
                        .and_then(char::from_u32)
                        .and_then(|c| c.to_digit(16))
                    {
                        v = v.saturating_mul(16).saturating_add(d);
                        self.pos += 1;
                    }
                    if self.pos == start || !self.eat('}') || v > 0x10FFFF {
                        return err("Invalid Unicode escape");
                    }
                    return ch(v);
                }
                match self.hex(4) {
                    Some(v) => {
                        if self.unicode && is_high(v as u16) && v < 0x10000 {
                            let save = self.pos;
                            if self.eat('\\')
                                && self.eat('u')
                                && let Some(lo) = self.hex(4)
                                && is_low(lo as u16)
                            {
                                return ch(0x10000 + ((v - 0xD800) << 10) + (lo - 0xDC00));
                            }
                            self.pos = save;
                        }
                        ch(v)
                    }
                    None if self.unicode => err("Invalid Unicode escape"),
                    None => ch('u' as u32),
                }
            }
            Some(p @ ('p' | 'P')) if self.unicode => {
                if !self.eat('{') {
                    return err("Invalid property name");
                }
                let mut name = String::new();
                while let Some(c) = self.peek().and_then(char::from_u32) {
                    self.pos += 1;
                    if c == '}' {
                        let prop = property(&name).ok_or_else(|| {
                            format!(
                                "Invalid regular expression: Invalid property name \\{p}{{{name}}}"
                            )
                        })?;
                        return Ok(Item::Prop(prop, p == 'P'));
                    }
                    name.push(c);
                }
                err("Invalid property name")
            }
            Some(c) if self.unicode && !"^$\\.*+?()[]{}|/-".contains(c) => err("Invalid escape"),
            _ => ch(c),
        }
    }

    fn hex(&mut self, n: usize) -> Option<u32> {
        let mut v = 0;
        for i in 0..n {
            let d = self
                .src
                .get(self.pos + i)
                .and_then(|c| char::from_u32(*c))
                .and_then(|c| c.to_digit(16))?;
            v = v * 16 + d;
        }
        self.pos += n;
        Some(v)
    }

    fn class(&mut self) -> Result<Class, String> {
        let negate = self.eat('^');
        let mut items = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return err("Unterminated character class");
            };
            if c == ']' as u32 {
                self.pos += 1;
                return Ok(Class { negate, items });
            }
            let first = self.class_atom()?;
            if self.peek_is('-')
                && self.src.get(self.pos + 1) != Some(&(']' as u32))
                && self.src.get(self.pos + 1).is_some()
            {
                self.pos += 1;
                let second = self.class_atom()?;
                match (&first, &second) {
                    (Item::Range(a, _), Item::Range(b, _)) => {
                        if a > b {
                            return err("Range out of order in character class");
                        }
                        items.push(Item::Range(*a, *b));
                    }
                    _ if self.unicode => return err("Invalid character class"),
                    _ => {
                        items.push(first);
                        items.push(Item::Range('-' as u32, '-' as u32));
                        items.push(second);
                    }
                }
            } else {
                items.push(first);
            }
        }
    }

    fn class_atom(&mut self) -> Result<Item, String> {
        let c = self.peek().expect("a class atom");
        self.pos += 1;
        if c != '\\' as u32 {
            return Ok(Item::Range(c, c));
        }
        match self.peek().and_then(char::from_u32) {
            Some('b') => {
                self.pos += 1;
                Ok(Item::Range(8, 8))
            }
            Some('-') if self.unicode => {
                self.pos += 1;
                Ok(Item::Range('-' as u32, '-' as u32))
            }
            Some(d @ '1'..='9') if !self.unicode => {
                self.pos += 1;
                Ok(Item::Range(d as u32, d as u32))
            }
            _ => self.class_escape(),
        }
    }
}

fn look(node: Node, ahead: bool, negate: bool) -> Node {
    Node::Look {
        node: Box::new(node),
        ahead,
        negate,
    }
}

/// The general categories and binary properties `\p{..}` knows.
fn property(name: &str) -> Option<Prop> {
    let name = name
        .strip_prefix("General_Category=")
        .or_else(|| name.strip_prefix("gc="))
        .unwrap_or(name);
    Some(match name {
        "L" | "Letter" => Prop::Table(LETTER),
        "N" | "Number" => Prop::Table(NUMBER),
        "P" | "Punctuation" | "punct" => Prop::Table(PUNCTUATION),
        "S" | "Symbol" => Prop::Table(SYMBOL),
        "Zs" | "Space_Separator" => Prop::Table(SPACE_SEPARATOR),
        "Cc" | "Control" | "cntrl" => Prop::Control,
        "Cs" | "Surrogate" => Prop::Surrogate,
        "Noncharacter_Code_Point" | "NChar" => Prop::Noncharacter,
        "Any" => Prop::Any,
        "ASCII" => Prop::Ascii,
        _ => return None,
    })
}

// --- Matching -----------------------------------------------------------

struct Ctx<'a> {
    text: &'a [u16],
    p: &'a Program,
    caps: Vec<Option<(usize, usize)>>,
}

type Cont<'k> = dyn FnMut(usize, &mut Ctx) -> bool + 'k;

/// The character at `pos` and its length in code units: a code point for a
/// `u` regex, else a code unit.
fn char_at(ctx: &Ctx, pos: usize) -> Option<(u32, usize)> {
    let t = ctx.text;
    let u = *t.get(pos)?;
    if ctx.p.unicode && is_high(u) && pos + 1 < t.len() && is_low(t[pos + 1]) {
        let lo = t[pos + 1];
        return Some((
            0x10000 + ((u32::from(u) - 0xD800) << 10) + (u32::from(lo) - 0xDC00),
            2,
        ));
    }
    Some((u32::from(u), 1))
}

fn is_line_terminator(c: u32) -> bool {
    matches!(c, 0x0A | 0x0D | 0x2028 | 0x2029)
}

/// JS's `\s`: WhiteSpace and LineTerminator.
pub(crate) fn is_js_space(c: u32) -> bool {
    matches!(
        c,
        0x09..=0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

fn is_word(c: u32) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A | 0x5F)
}

pub(crate) fn in_table(table: &[(u32, u32)], c: u32) -> bool {
    table
        .binary_search_by(|&(a, b)| {
            if c < a {
                std::cmp::Ordering::Greater
            } else if c > b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn prop_has(p: Prop, c: u32) -> bool {
    match p {
        Prop::Table(t) => in_table(t, c),
        Prop::Control => c < 0x20 || (0x7F..=0x9F).contains(&c),
        Prop::Surrogate => (0xD800..=0xDFFF).contains(&c),
        Prop::Noncharacter => (0xFDD0..=0xFDEF).contains(&c) || (c & 0xFFFE) == 0xFFFE,
        Prop::Any => true,
        Prop::Ascii => c < 0x80,
    }
}

/// `Canonicalize` without `u`: `toUpperCase` when it is one code unit and
/// does not map a non-ASCII unit to ASCII.
fn canonicalize(c: u32) -> u32 {
    if c > 0xFFFF {
        return c;
    }
    match CANONICALIZE.binary_search_by_key(&(c as u16), |&(a, _)| a) {
        Ok(i) => u32::from(CANONICALIZE[i].1),
        Err(_) => c,
    }
}

/// The characters that canonicalize as `c` does, `c` among them.
fn case_variants(c: u32) -> Vec<u32> {
    let canon = canonicalize(c);
    let mut out = vec![canon];
    for &(a, b) in CANONICALIZE {
        if u32::from(b) == canon && u32::from(a) != canon {
            out.push(u32::from(a));
        }
    }
    out
}

fn item_has(item: &Item, c: u32) -> bool {
    match item {
        Item::Range(a, b) => (*a..=*b).contains(&c),
        Item::Digit(neg) => (0x30..=0x39).contains(&c) != *neg,
        Item::Word(neg) => is_word(c) != *neg,
        Item::Space(neg) => is_js_space(c) != *neg,
        Item::Prop(p, neg) => prop_has(*p, c) != *neg,
    }
}

fn class_has(class: &Class, c: u32, ignore_case: bool) -> bool {
    let found = if ignore_case {
        case_variants(c)
            .into_iter()
            .chain(std::iter::once(c))
            .any(|v| class.items.iter().any(|i| item_has(i, v)))
    } else {
        class.items.iter().any(|i| item_has(i, c))
    };
    found != class.negate
}

/// Whether the character `c` matches the single-character `node`.
fn single_matches(node: &Node, c: u32, p: &Program) -> bool {
    match node {
        Node::Char(x) => *x == c || (p.ignore_case && canonicalize(*x) == canonicalize(c)),
        Node::Any => p.dot_all || !is_line_terminator(c),
        Node::Class(class) => class_has(class, c, p.ignore_case),
        _ => unreachable!("not a single character"),
    }
}

fn is_single(node: &Node) -> bool {
    matches!(node, Node::Char(_) | Node::Any | Node::Class(_))
}

fn word_at(ctx: &Ctx, pos: isize) -> bool {
    if pos < 0 || pos as usize >= ctx.text.len() {
        return false;
    }
    let c = u32::from(ctx.text[pos as usize]);
    is_word(c) || (ctx.p.ignore_case && ctx.p.unicode && matches!(c, 0x017F | 0x212A))
}

/// Matches `node` at `pos`, then calls `k` with where it ends; backtracks
/// into `node` while `k` fails. Restores the captures it set when it fails.
fn m(node: &Node, pos: usize, ctx: &mut Ctx, k: &mut Cont) -> bool {
    match node {
        Node::Empty => k(pos, ctx),
        Node::Char(_) | Node::Any | Node::Class(_) => match char_at(ctx, pos) {
            Some((c, len)) if single_matches(node, c, ctx.p) => k(pos + len, ctx),
            _ => false,
        },
        Node::Start => {
            (pos == 0 || (ctx.p.multiline && is_line_terminator(u32::from(ctx.text[pos - 1]))))
                && k(pos, ctx)
        }
        Node::End => {
            (pos == ctx.text.len()
                || (ctx.p.multiline && is_line_terminator(u32::from(ctx.text[pos]))))
                && k(pos, ctx)
        }
        Node::WordBoundary(is) => {
            let a = word_at(ctx, pos as isize - 1);
            let b = word_at(ctx, pos as isize);
            ((a != b) == *is) && k(pos, ctx)
        }
        Node::Group(inner, index) => match index {
            None => m(inner, pos, ctx, k),
            Some(g) => {
                let g = *g;
                m(inner, pos, ctx, &mut |e, c: &mut Ctx| {
                    let old = c.caps[g];
                    c.caps[g] = Some((pos, e));
                    if k(e, c) {
                        true
                    } else {
                        c.caps[g] = old;
                        false
                    }
                })
            }
        },
        Node::Look {
            node: inner,
            ahead,
            negate,
        } => {
            let saved = ctx.caps.clone();
            let found = if *ahead {
                m(inner, pos, ctx, &mut |_, _| true)
            } else {
                (0..=pos)
                    .rev()
                    .any(|start| m(inner, start, ctx, &mut |e, _| e == pos))
            };
            if *negate {
                ctx.caps = saved;
                !found && k(pos, ctx)
            } else if !found {
                ctx.caps = saved;
                false
            } else if k(pos, ctx) {
                true
            } else {
                ctx.caps = saved;
                false
            }
        }
        Node::BackRef(g) => {
            let Some((s, e)) = ctx.caps.get(*g).copied().flatten() else {
                return k(pos, ctx);
            };
            let len = e - s;
            if pos + len > ctx.text.len() {
                return false;
            }
            let same = (0..len).all(|i| {
                let a = u32::from(ctx.text[s + i]);
                let b = u32::from(ctx.text[pos + i]);
                a == b || (ctx.p.ignore_case && canonicalize(a) == canonicalize(b))
            });
            same && k(pos + len, ctx)
        }
        Node::NamedBackRef(_) => unreachable!("resolved when compiled"),
        Node::Concat(nodes) => seq(nodes, pos, ctx, k),
        Node::Alt(alts) => alts.iter().any(|alt| m(alt, pos, ctx, &mut *k)),
        Node::Repeat {
            node: inner,
            min,
            max,
            greedy,
            groups,
        } => {
            if is_single(inner) {
                repeat_single(inner, *min, *max, *greedy, pos, ctx, k)
            } else {
                repeat(inner, *min, *max, *greedy, *groups, pos, ctx, k)
            }
        }
    }
}

fn seq(nodes: &[Node], pos: usize, ctx: &mut Ctx, k: &mut Cont) -> bool {
    match nodes {
        [] => k(pos, ctx),
        [last] => m(last, pos, ctx, k),
        [first, rest @ ..] => m(first, pos, ctx, &mut |e, c: &mut Ctx| {
            seq(rest, e, c, &mut *k)
        }),
    }
}

/// A repeated single character, without recursion per iteration.
fn repeat_single(
    node: &Node,
    min: u32,
    max: Option<u32>,
    greedy: bool,
    pos: usize,
    ctx: &mut Ctx,
    k: &mut Cont,
) -> bool {
    let max = max.map_or(usize::MAX, |m| m as usize);
    let min = min as usize;
    let mut ends = vec![pos];
    let mut at = pos;
    while ends.len() - 1 < max {
        if !greedy && ends.len() > min && k(at, ctx) {
            return true;
        }
        match char_at(ctx, at) {
            Some((c, len)) if single_matches(node, c, ctx.p) => {
                at += len;
                ends.push(at);
            }
            _ => break,
        }
    }
    if !greedy {
        return ends.len() - 1 == max && ends.len() > min && k(at, ctx);
    }
    if ends.len() - 1 < min {
        return false;
    }
    for &e in ends[min..].iter().rev() {
        if k(e, ctx) {
            return true;
        }
    }
    false
}

/// `RepeatMatcher` (ECMA-262 22.2.2.3.1).
#[allow(clippy::too_many_arguments)]
fn repeat(
    node: &Node,
    min: u32,
    max: Option<u32>,
    greedy: bool,
    groups: (usize, usize),
    pos: usize,
    ctx: &mut Ctx,
    k: &mut Cont,
) -> bool {
    if max == Some(0) {
        return k(pos, ctx);
    }
    let saved: Vec<Option<(usize, usize)>> = ctx.caps[groups.0..groups.1].to_vec();
    let once = |ctx: &mut Ctx, k: &mut Cont| -> bool {
        for c in &mut ctx.caps[groups.0..groups.1] {
            *c = None;
        }
        let found = m(node, pos, ctx, &mut |e, c: &mut Ctx| {
            if min == 0 && e == pos {
                return false;
            }
            repeat(
                node,
                min.saturating_sub(1),
                max.map(|m| m - 1),
                greedy,
                groups,
                e,
                c,
                &mut *k,
            )
        });
        if !found {
            ctx.caps[groups.0..groups.1].copy_from_slice(&saved);
        }
        found
    };
    if min > 0 {
        return once(ctx, k);
    }
    if !greedy {
        if k(pos, ctx) {
            return true;
        }
        return once(ctx, k);
    }
    if once(ctx, &mut *k) {
        return true;
    }
    k(pos, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec(source: &str, flags: &str, text: &str) -> Option<Vec<Option<String>>> {
        RegExp::of(source, flags).exec(text).map(|m| m.groups)
    }

    fn s(v: &[Option<&str>]) -> Option<Vec<Option<String>>> {
        Some(v.iter().map(|g| g.map(str::to_owned)).collect())
    }

    // Expected values are what Node 24 prints for each `exec`.
    #[test]
    fn matches_as_js_does() {
        assert_eq!(
            exec("^(#{1,6})\\s", "", "## a"),
            s(&[Some("## "), Some("##")])
        );
        assert_eq!(exec("(a)|b", "", "b"), s(&[Some("b"), None]));
        assert_eq!(
            exec("(z)((a+)?(b+)?(c))*", "", "zaacbbbcac"),
            s(&[
                Some("zaacbbbcac"),
                Some("z"),
                Some("ac"),
                Some("a"),
                None,
                Some("c")
            ])
        );
        assert_eq!(exec("(a*)*", "", "b"), s(&[Some(""), None]));
        assert_eq!(exec("(a*)b\\1+", "", "baaaac"), s(&[Some("b"), Some("")]));
        assert_eq!(
            exec("(?=(a+))a*b\\1", "", "baaabac"),
            s(&[Some("aba"), Some("a")])
        );
        assert_eq!(
            exec("(.*?)a(?!(a+)b\\2c)\\2(.*)", "", "baaabaac"),
            s(&[Some("baaabaac"), Some("ba"), None, Some("abaac")])
        );
        assert_eq!(
            exec("\\$([^$]+?)\\$", "", "x $a$ $b$"),
            s(&[Some("$a$"), Some("a")])
        );
        assert_eq!(
            exec(
                "^(\\s*)(?:[-*+]\\s)?\\s?(\\[(\\s|x)?\\])\\s",
                "i",
                "- [X] a"
            ),
            s(&[Some("- [X] "), Some(""), Some("[X]"), Some("X")])
        );
        assert_eq!(exec("a{,2}", "", "a{,2}"), s(&[Some("a{,2}")]));
        assert_eq!(exec("\\bfoo\\b", "", "a foo."), s(&[Some("foo")]));
        assert_eq!(exec("(?<=\\$)\\d+", "", "a$12"), s(&[Some("12")]));
        assert_eq!(
            exec("(?<y>\\d{4})-\\k<y>", "", "2020-2020"),
            s(&[Some("2020-2020"), Some("2020")])
        );
        assert_eq!(exec("[\\p{P}\\p{S}]", "u", "a€"), s(&[Some("€")]));
        assert_eq!(exec("^.$", "", "😀"), None);
        assert_eq!(exec("^.$", "u", "😀"), s(&[Some("😀")]));
        assert_eq!(exec("ſ", "i", "s"), None);
        assert_eq!(exec("[a-z]+", "i", "ABC"), s(&[Some("ABC")]));
    }

    #[test]
    fn global_flags_and_strings() {
        let re = RegExp::of("a", "g");
        assert!(re.test("aa"));
        assert_eq!(re.last_index(), 1);
        assert!(re.test("aa"));
        assert!(!re.test("aa"));
        assert_eq!(re.last_index(), 0);
        assert_eq!(
            RegExp::of("(\\*|\\^|\\+)", "g").replace("**^x", "\\$1"),
            "\\*\\*\\^x"
        );
        assert_eq!(RegExp::of("x*", "g").replace("abc", "-"), "-a-b-c-");
        assert_eq!(RegExp::of("😀", "u").search("a😀"), Some(1));
        assert_eq!(RegExp::of(",", "").split("a,b,,c"), ["a", "b", "", "c"]);
        assert_eq!(RegExp::of("\\s+", "").split(""), [""]);
        assert!(RegExp::new("(", "").is_err());
        assert!(RegExp::new("a", "gg").is_err());
        assert_eq!(RegExp::of("a", "yig").flags(), "giy");
    }
}
