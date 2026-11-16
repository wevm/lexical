//! `DOMParser`: the WHATWG HTML tokenizer and the parts of the tree builder
//! HTML import meets (head, body, tables with foster parenting, lists, the
//! adoption agency and active formatting elements), ported from the Swift
//! port's `HTMLParser.swift`. The `html/parse` and `html/entities` vectors
//! pin it against jsdom's `DOMParser`. Not covered, as in Swift: templates,
//! framesets, the `<select>` and `<ruby>` insertion modes (their contents
//! parse as ordinary body content), and the case adjustments of SVG and
//! MathML names.

use super::html_entities::{HTML_LEGACY_ENTITY_NAMES, HTML_NAMED_ENTITIES};
use super::node::DomNode;
use std::collections::HashSet;

/// `new DOMParser().parseFromString(html, 'text/html')`.
pub fn parse_from_string(html: &str) -> DomNode {
    TreeBuilder::new(html).run()
}

/// The text a named character reference (without `&` and `;`) decodes to.
pub fn named_entity(name: &str) -> Option<&'static str> {
    HTML_NAMED_ENTITIES
        .binary_search_by(|(n, _)| (*n).cmp(name))
        .ok()
        .map(|i| HTML_NAMED_ENTITIES[i].1)
}

/// How many named character references there are.
pub fn named_entity_count() -> usize {
    HTML_NAMED_ENTITIES.len()
}

/// Whether a named character reference also decodes without its semicolon.
pub fn is_legacy_entity_name(name: &str) -> bool {
    HTML_LEGACY_ENTITY_NAMES.binary_search(&name).is_ok()
}

// Tokenizer

#[derive(Clone, Debug)]
struct Attribute {
    name: String,
    value: String,
}

#[derive(Clone, Debug)]
enum Token {
    Doctype(String),
    StartTag {
        name: String,
        attributes: Vec<Attribute>,
        self_closing: bool,
    },
    EndTag(String),
    Comment(String),
    Characters(String),
    Eof,
}

impl Token {
    fn is_start(&self, tag: &str) -> bool {
        matches!(self, Token::StartTag { name, .. } if name == tag)
    }

    fn is_end(&self, tag: &str) -> bool {
        matches!(self, Token::EndTag(name) if name == tag)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenizerState {
    Data,
    Rcdata,
    Rawtext,
    Plaintext,
}

/// Tab, LF, FF, CR and space.
fn is_html_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0C}' | '\r')
}

/// Code points 0x80–0x9F of a numeric reference, read as windows-1252.
fn windows1252(value: u32) -> Option<u32> {
    Some(match value {
        0x80 => 0x20AC,
        0x82 => 0x201A,
        0x83 => 0x0192,
        0x84 => 0x201E,
        0x85 => 0x2026,
        0x86 => 0x2020,
        0x87 => 0x2021,
        0x88 => 0x02C6,
        0x89 => 0x2030,
        0x8A => 0x0160,
        0x8B => 0x2039,
        0x8C => 0x0152,
        0x8E => 0x017D,
        0x91 => 0x2018,
        0x92 => 0x2019,
        0x93 => 0x201C,
        0x94 => 0x201D,
        0x95 => 0x2022,
        0x96 => 0x2013,
        0x97 => 0x2014,
        0x98 => 0x02DC,
        0x99 => 0x2122,
        0x9A => 0x0161,
        0x9B => 0x203A,
        0x9C => 0x0153,
        0x9E => 0x017E,
        0x9F => 0x0178,
        _ => return None,
    })
}

struct Tokenizer {
    input: Vec<char>,
    pos: usize,
    state: TokenizerState,
    /// The name of the element whose raw text is being read.
    last_start_tag: String,
}

impl Tokenizer {
    fn new(html: &str) -> Self {
        // Newlines are normalized to LF before tokenizing.
        let mut input = Vec::with_capacity(html.len());
        let mut previous_cr = false;
        for c in html.chars() {
            if c == '\r' {
                input.push('\n');
                previous_cr = true;
                continue;
            }
            if c == '\n' && previous_cr {
                previous_cr = false;
                continue;
            }
            previous_cr = false;
            input.push(c);
        }
        Self {
            input,
            pos: 0,
            state: TokenizerState::Data,
            last_start_tag: String::new(),
        }
    }

    fn peek(&self, offset: usize) -> Option<char> {
        self.input.get(self.pos + offset).copied()
    }

    fn string(&self, start: usize, end: usize) -> String {
        self.input[start..end].iter().collect()
    }

    fn next(&mut self) -> Token {
        while self.pos < self.input.len() {
            match self.state {
                TokenizerState::Plaintext => {
                    let text = self.string(self.pos, self.input.len());
                    self.pos = self.input.len();
                    return Token::Characters(text);
                }
                TokenizerState::Rawtext | TokenizerState::Rcdata => {
                    if let Some(token) = self.next_raw_text(self.state == TokenizerState::Rcdata) {
                        return token;
                    }
                }
                TokenizerState::Data => {
                    if self.is_tag_start(self.pos) {
                        if let Some(token) = self.tag_open() {
                            return token;
                        }
                    } else {
                        return Token::Characters(self.data_text());
                    }
                }
            }
        }
        Token::Eof
    }

    /// Whether a tag, comment or declaration starts at `i` (a `<` that is
    /// not literal text).
    fn is_tag_start(&self, i: usize) -> bool {
        if self.input[i] != '<' || i + 1 >= self.input.len() {
            return false;
        }
        let c = self.input[i + 1];
        if c.is_ascii_alphabetic() || c == '!' || c == '?' {
            return true;
        }
        c == '/' && i + 2 < self.input.len()
    }

    fn data_text(&mut self) -> String {
        let mut text = String::new();
        while self.pos < self.input.len() {
            let c = self.input[self.pos];
            if c == '<' && self.is_tag_start(self.pos) {
                break;
            }
            if c == '&' {
                self.consume_character_reference(&mut text, false);
                continue;
            }
            if c != '\0' {
                text.push(c);
            }
            self.pos += 1;
        }
        text
    }

    /// At `<`: the tag, comment or declaration, or `None` for one that is
    /// dropped (`</>`, a tag cut off by the end of input).
    fn tag_open(&mut self) -> Option<Token> {
        self.pos += 1;
        let c = self.input[self.pos];
        if c == '!' {
            self.pos += 1;
            return Some(self.markup_declaration());
        }
        if c == '?' {
            return Some(self.bogus_comment());
        }
        if c == '/' {
            self.pos += 1;
            let d = self.peek(0)?;
            if d == '>' {
                self.pos += 1;
                return None;
            }
            if d.is_ascii_alphabetic() {
                let (name, _, _) = self.tag()?;
                return Some(Token::EndTag(name));
            }
            return Some(self.bogus_comment());
        }
        let (name, attributes, self_closing) = self.tag()?;
        Some(Token::StartTag {
            name,
            attributes,
            self_closing,
        })
    }

    /// Text up to the next `>` as a comment.
    fn bogus_comment(&mut self) -> Token {
        let start = self.pos;
        while self.peek(0).is_some_and(|c| c != '>') {
            self.pos += 1;
        }
        let data: String = self.input[start..self.pos]
            .iter()
            .map(|&c| if c == '\0' { '\u{FFFD}' } else { c })
            .collect();
        if self.pos < self.input.len() {
            self.pos += 1;
        }
        Token::Comment(data)
    }

    fn matches_case_insensitive(&self, word: &str) -> bool {
        word.chars().enumerate().all(|(k, expected)| {
            self.peek(k)
                .is_some_and(|c| c.eq_ignore_ascii_case(&expected))
        })
    }

    /// After `<!`.
    fn markup_declaration(&mut self) -> Token {
        if self.peek(0) == Some('-') && self.peek(1) == Some('-') {
            self.pos += 2;
            return self.comment();
        }
        if self.matches_case_insensitive("doctype") {
            self.pos += 7;
            while self.peek(0).is_some_and(is_html_whitespace) {
                self.pos += 1;
            }
            let mut name = String::new();
            while let Some(c) = self.peek(0) {
                if is_html_whitespace(c) || c == '>' {
                    break;
                }
                name.push(c.to_ascii_lowercase());
                self.pos += 1;
            }
            while self.peek(0).is_some_and(|c| c != '>') {
                self.pos += 1;
            }
            if self.pos < self.input.len() {
                self.pos += 1;
            }
            return Token::Doctype(name);
        }
        self.bogus_comment()
    }

    /// After `<!--`.
    fn comment(&mut self) -> Token {
        if self.peek(0) == Some('>') {
            self.pos += 1;
            return Token::Comment(String::new());
        }
        if self.peek(0) == Some('-') && self.peek(1) == Some('>') {
            self.pos += 2;
            return Token::Comment(String::new());
        }
        let start = self.pos;
        while self.pos < self.input.len() {
            if self.input[self.pos] == '-' && self.peek(1) == Some('-') {
                if self.peek(2) == Some('>') {
                    let data = self.string(start, self.pos);
                    self.pos += 3;
                    return Token::Comment(data);
                }
                if self.peek(2) == Some('!') && self.peek(3) == Some('>') {
                    let data = self.string(start, self.pos);
                    self.pos += 4;
                    return Token::Comment(data);
                }
            }
            self.pos += 1;
        }
        Token::Comment(self.string(start, self.input.len()))
    }

    /// A tag's name and attributes, from its first name character through
    /// `>`; `None` when the input ends first.
    fn tag(&mut self) -> Option<(String, Vec<Attribute>, bool)> {
        let mut name = String::new();
        while let Some(c) = self.peek(0) {
            if is_html_whitespace(c) || c == '/' || c == '>' {
                break;
            }
            name.push(if c == '\0' {
                '\u{FFFD}'
            } else {
                c.to_ascii_lowercase()
            });
            self.pos += 1;
        }
        let mut attributes: Vec<Attribute> = Vec::new();
        let mut self_closing = false;
        while let Some(c) = self.peek(0) {
            if is_html_whitespace(c) {
                self.pos += 1;
                continue;
            }
            if c == '>' {
                self.pos += 1;
                return Some((name, attributes, self_closing));
            }
            if c == '/' {
                self.pos += 1;
                if self.peek(0) == Some('>') {
                    self_closing = true;
                }
                continue;
            }
            // An attribute: its first character may be `=`.
            let mut attribute_name = String::new();
            attribute_name.push(c.to_ascii_lowercase());
            self.pos += 1;
            while let Some(d) = self.peek(0) {
                if is_html_whitespace(d) || d == '/' || d == '>' || d == '=' {
                    break;
                }
                attribute_name.push(if d == '\0' {
                    '\u{FFFD}'
                } else {
                    d.to_ascii_lowercase()
                });
                self.pos += 1;
            }
            while self.peek(0).is_some_and(is_html_whitespace) {
                self.pos += 1;
            }
            let mut value = String::new();
            if self.peek(0) == Some('=') {
                self.pos += 1;
                while self.peek(0).is_some_and(is_html_whitespace) {
                    self.pos += 1;
                }
                match self.peek(0) {
                    Some(quote @ ('"' | '\'')) => {
                        self.pos += 1;
                        while let Some(d) = self.peek(0) {
                            if d == quote {
                                break;
                            }
                            self.attribute_value_char(d, &mut value);
                        }
                        if self.pos < self.input.len() {
                            self.pos += 1;
                        }
                    }
                    _ => {
                        while let Some(d) = self.peek(0) {
                            if is_html_whitespace(d) || d == '>' {
                                break;
                            }
                            self.attribute_value_char(d, &mut value);
                        }
                    }
                }
            }
            // A repeated attribute is dropped.
            if !attributes.iter().any(|a| a.name == attribute_name) {
                attributes.push(Attribute {
                    name: attribute_name,
                    value,
                });
            }
        }
        None
    }

    fn attribute_value_char(&mut self, d: char, value: &mut String) {
        if d == '&' {
            self.consume_character_reference(value, true);
        } else {
            value.push(if d == '\0' { '\u{FFFD}' } else { d });
            self.pos += 1;
        }
    }

    /// At `&`: appends what the character reference decodes to, or `&`.
    fn consume_character_reference(&mut self, out: &mut String, in_attribute: bool) {
        self.pos += 1;
        let Some(c) = self.peek(0) else {
            out.push('&');
            return;
        };
        if c == '#' {
            let mut i = self.pos + 1;
            let mut hex = false;
            if matches!(self.input.get(i), Some('x' | 'X')) {
                hex = true;
                i += 1;
            }
            let digits_start = i;
            let mut value: u32 = 0;
            while let Some(digit) = self
                .input
                .get(i)
                .and_then(|c| c.to_digit(if hex { 16 } else { 10 }))
            {
                value = value
                    .wrapping_mul(if hex { 16 } else { 10 })
                    .wrapping_add(digit)
                    .min(0x11_0000);
                i += 1;
            }
            if i == digits_start {
                out.push('&');
                return;
            }
            if self.input.get(i) == Some(&';') {
                i += 1;
            }
            self.pos = i;
            out.push(numeric_reference(value));
            return;
        }
        if !c.is_ascii_alphanumeric() {
            out.push('&');
            return;
        }
        let mut end = self.pos;
        while end < self.input.len()
            && self.input[end].is_ascii_alphanumeric()
            && end - self.pos < 40
        {
            end += 1;
        }
        let run = self.string(self.pos, end);
        if self.input.get(end) == Some(&';')
            && let Some(value) = named_entity(&run)
        {
            self.pos = end + 1;
            out.push_str(value);
            return;
        }
        // The longest name that may go without its semicolon.
        for length in (2..=(end - self.pos).min(6)).rev() {
            let name = self.string(self.pos, self.pos + length);
            let Some(value) = named_entity(&name).filter(|_| is_legacy_entity_name(&name)) else {
                continue;
            };
            if in_attribute
                && let Some(&following) = self.input.get(self.pos + length)
                && (following == '=' || following.is_ascii_alphanumeric())
            {
                out.push('&');
                return;
            }
            self.pos += length;
            out.push_str(value);
            return;
        }
        out.push('&');
    }

    /// Whether `</name` of the raw-text element starts at `i`.
    fn is_appropriate_end_tag(&self, i: usize) -> bool {
        if self.input[i] != '<' || self.input.get(i + 1) != Some(&'/') {
            return false;
        }
        let mut j = i + 2;
        for expected in self.last_start_tag.chars() {
            if self.input.get(j).map(char::to_ascii_lowercase) != Some(expected) {
                return false;
            }
            j += 1;
        }
        self.input
            .get(j)
            .is_some_and(|&c| is_html_whitespace(c) || c == '/' || c == '>')
    }

    /// Raw text or RCDATA up to the end tag of its element, or that end
    /// tag.
    fn next_raw_text(&mut self, decode: bool) -> Option<Token> {
        if self.is_appropriate_end_tag(self.pos) {
            self.pos += 2;
            self.state = TokenizerState::Data;
            let (name, _, _) = self.tag()?;
            return Some(Token::EndTag(name));
        }
        let mut text = String::new();
        while self.pos < self.input.len() {
            if self.input[self.pos] == '<' && self.is_appropriate_end_tag(self.pos) {
                break;
            }
            if decode && self.input[self.pos] == '&' {
                self.consume_character_reference(&mut text, false);
                continue;
            }
            let c = self.input[self.pos];
            text.push(if c == '\0' { '\u{FFFD}' } else { c });
            self.pos += 1;
        }
        Some(Token::Characters(text))
    }
}

fn numeric_reference(value: u32) -> char {
    if let Some(mapped) = windows1252(value) {
        return char::from_u32(mapped).expect("a windows-1252 character");
    }
    if value == 0 || value > 0x10_FFFF || (0xD800..=0xDFFF).contains(&value) {
        return '\u{FFFD}';
    }
    char::from_u32(value).unwrap_or('\u{FFFD}')
}

// Tree builder

const SPECIAL_ELEMENTS: &[&str] = &[
    "address",
    "applet",
    "area",
    "article",
    "aside",
    "base",
    "basefont",
    "bgsound",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dir",
    "div",
    "dl",
    "dt",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "iframe",
    "img",
    "input",
    "keygen",
    "li",
    "link",
    "listing",
    "main",
    "marquee",
    "menu",
    "meta",
    "nav",
    "noembed",
    "noframes",
    "noscript",
    "object",
    "ol",
    "p",
    "param",
    "plaintext",
    "pre",
    "script",
    "search",
    "section",
    "select",
    "source",
    "style",
    "summary",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
    "wbr",
    "xmp",
];

const SCOPE_BARRIERS: &[&str] = &[
    "applet", "caption", "html", "table", "td", "th", "marquee", "object", "template",
];

const IMPLIED_END_TAGS: &[&str] = &[
    "dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt", "rtc",
];

const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];

const FORMATTING_TAGS: &[&str] = &[
    "b", "big", "code", "em", "font", "i", "s", "small", "strike", "strong", "tt", "u",
];

const CLOSES_PARAGRAPH: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "center",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "header",
    "hgroup",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "search",
    "section",
    "summary",
    "ul",
];

/// [`CLOSES_PARAGRAPH`] with `button`, `listing` and `pre`, without `p`.
fn is_block_end_tag(name: &str) -> bool {
    (CLOSES_PARAGRAPH.contains(&name) && name != "p")
        || matches!(name, "button" | "listing" | "pre")
}

/// Start tags that leave foreign (SVG, MathML) content.
const FOREIGN_BREAKOUT: &[&str] = &[
    "b",
    "big",
    "blockquote",
    "body",
    "br",
    "center",
    "code",
    "dd",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "hr",
    "i",
    "img",
    "li",
    "listing",
    "menu",
    "meta",
    "nobr",
    "ol",
    "p",
    "pre",
    "ruby",
    "s",
    "small",
    "span",
    "strong",
    "strike",
    "sub",
    "sup",
    "table",
    "tt",
    "u",
    "ul",
    "var",
];

const TABLE_SECTIONS: &[&str] = &["tbody", "tfoot", "thead"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    InHeadNoscript,
    AfterHead,
    InBody,
    Text,
    InTable,
    InCaption,
    InColumnGroup,
    InTableBody,
    InRow,
    InCell,
    AfterBody,
    AfterAfterBody,
}

struct TreeBuilder {
    tokenizer: Tokenizer,
    document: DomNode,
    mode: Mode,
    original_mode: Mode,
    stack: Vec<DomNode>,
    /// The list of active formatting elements; `None` is a marker.
    formatting: Vec<Option<DomNode>>,
    head: Option<DomNode>,
    foster_parenting: bool,
    skip_next_newline: bool,
    foreign: HashSet<usize>,
    stopped: bool,
    /// No `<!DOCTYPE html>`: the quirks mode of a document without one.
    quirks: bool,
}

impl TreeBuilder {
    fn new(html: &str) -> Self {
        Self {
            tokenizer: Tokenizer::new(html),
            document: DomNode::new_document(),
            mode: Mode::Initial,
            original_mode: Mode::InBody,
            stack: Vec::new(),
            formatting: Vec::new(),
            head: None,
            foster_parenting: false,
            skip_next_newline: false,
            foreign: HashSet::new(),
            stopped: false,
            quirks: true,
        }
    }

    fn run(mut self) -> DomNode {
        while !self.stopped {
            let mut token = self.tokenizer.next();
            if self.skip_next_newline {
                self.skip_next_newline = false;
                if let Token::Characters(text) = &token
                    && let Some(rest) = text.strip_prefix('\n')
                {
                    if rest.is_empty() {
                        continue;
                    }
                    token = Token::Characters(rest.to_owned());
                }
            }
            let eof = matches!(token, Token::Eof);
            self.dispatch(token);
            if eof {
                break;
            }
        }
        self.document
    }

    fn current_node(&self) -> &DomNode {
        self.stack.last().expect("an open element")
    }

    fn current_name(&self) -> &str {
        self.current_node().local_name()
    }

    fn is_foreign(&self, element: &DomNode) -> bool {
        self.foreign.contains(&element.identity())
    }

    fn dispatch(&mut self, token: Token) {
        if self.stack.last().is_some_and(|n| self.is_foreign(n)) {
            match &token {
                Token::Characters(text) => {
                    self.insert_characters(text);
                    return;
                }
                Token::Comment(data) => {
                    self.insert_comment(data, None);
                    return;
                }
                Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                } => {
                    let is_font_breakout = name == "font"
                        && attributes
                            .iter()
                            .any(|a| matches!(a.name.as_str(), "color" | "face" | "size"));
                    if FOREIGN_BREAKOUT.contains(&name.as_str()) || is_font_breakout {
                        while self.stack.last().is_some_and(|n| self.is_foreign(n)) {
                            self.stack.pop();
                        }
                        self.process(token);
                    } else {
                        let (name, attributes, self_closing) =
                            (name.clone(), attributes.clone(), *self_closing);
                        self.insert_foreign(&name, &attributes, self_closing);
                    }
                    return;
                }
                Token::EndTag(name) => {
                    for index in (0..self.stack.len()).rev() {
                        let node = &self.stack[index];
                        if !self.is_foreign(node) {
                            break;
                        }
                        if node.local_name() == name {
                            self.stack.truncate(index);
                            return;
                        }
                    }
                }
                _ => {}
            }
        }
        self.process(token);
    }

    fn process(&mut self, token: Token) {
        match self.mode {
            Mode::Initial => self.initial(token),
            Mode::BeforeHtml => self.before_html(token),
            Mode::BeforeHead => self.before_head(token),
            Mode::InHead => self.in_head(token),
            Mode::InHeadNoscript => self.in_head_noscript(token),
            Mode::AfterHead => self.after_head(token),
            Mode::InBody => self.in_body(token),
            Mode::Text => self.text(token),
            Mode::InTable => self.in_table(token),
            Mode::InCaption => self.in_caption(token),
            Mode::InColumnGroup => self.in_column_group(token),
            Mode::InTableBody => self.in_table_body(token),
            Mode::InRow => self.in_row(token),
            Mode::InCell => self.in_cell(token),
            Mode::AfterBody => self.after_body(token),
            Mode::AfterAfterBody => self.after_after_body(token),
        }
    }

    /// Splits leading whitespace off a character token: hands it to
    /// `whitespace` and returns the rest, if any, as a token.
    fn split_whitespace(
        &mut self,
        token: Token,
        whitespace: impl FnOnce(&mut Self, &str),
    ) -> Option<Token> {
        let Token::Characters(text) = token else {
            return Some(token);
        };
        let index = text
            .find(|c: char| !is_html_whitespace(c))
            .unwrap_or(text.len());
        if index > 0 {
            whitespace(self, &text[..index]);
        }
        let rest = &text[index..];
        (!rest.is_empty()).then(|| Token::Characters(rest.to_owned()))
    }

    // Insertion

    fn create_element(&self, name: &str, attributes: &[Attribute]) -> DomNode {
        let element = DomNode::new_element(name.to_owned(), Some(&self.document));
        for attribute in attributes {
            element.set_attribute(&attribute.name, &attribute.value);
        }
        element
    }

    fn attributes_of(element: &DomNode) -> Vec<Attribute> {
        element
            .attributes()
            .into_iter()
            .map(|(name, value)| Attribute { name, value })
            .collect()
    }

    fn insertion_place(&self, target: Option<&DomNode>) -> (DomNode, Option<DomNode>) {
        let target = target.unwrap_or_else(|| self.current_node()).clone();
        if self.foster_parenting
            && matches!(
                target.local_name(),
                "table" | "tbody" | "tfoot" | "thead" | "tr"
            )
        {
            if let Some(table_index) = self.stack.iter().rposition(|e| e.local_name() == "table") {
                let table = &self.stack[table_index];
                if let Some(parent) = table.parent_node() {
                    return (parent, Some(table.clone()));
                }
                return (self.stack[table_index - 1].clone(), None);
            }
            return (self.stack[0].clone(), None);
        }
        (target, None)
    }

    fn insert_element(&mut self, name: &str, attributes: &[Attribute]) -> DomNode {
        let element = self.create_element(name, attributes);
        let (parent, before) = self.insertion_place(None);
        parent.insert_before(&element, before.as_ref());
        self.stack.push(element.clone());
        element
    }

    fn insert_void(&mut self, name: &str, attributes: &[Attribute]) {
        self.insert_element(name, attributes);
        self.stack.pop();
    }

    fn insert_foreign(&mut self, name: &str, attributes: &[Attribute], self_closing: bool) {
        let element = self.insert_element(name, attributes);
        self.foreign.insert(element.identity());
        if self_closing {
            self.stack.pop();
        }
    }

    fn insert_characters(&mut self, text: &str) {
        let text: String = text.chars().filter(|&c| c != '\0').collect();
        if text.is_empty() {
            return;
        }
        let (parent, before) = self.insertion_place(None);
        if parent.is_document() {
            return;
        }
        let previous = match &before {
            Some(before) => before.previous_sibling(),
            None => parent.last_child(),
        };
        match previous {
            Some(previous) if previous.is_text() => previous.push_data(&text),
            _ => {
                parent.insert_before(
                    &DomNode::new_text(text, Some(&self.document)),
                    before.as_ref(),
                );
            }
        }
    }

    fn insert_comment(&mut self, data: &str, into: Option<DomNode>) {
        let comment = DomNode::new_comment(data, Some(&self.document));
        match into {
            Some(parent) => {
                parent.append_child(&comment);
            }
            None => {
                let (parent, before) = self.insertion_place(None);
                parent.insert_before(&comment, before.as_ref());
            }
        }
    }

    fn start_raw_text(&mut self, name: &str, attributes: &[Attribute], state: TokenizerState) {
        self.insert_element(name, attributes);
        self.tokenizer.state = state;
        name.clone_into(&mut self.tokenizer.last_start_tag);
        self.original_mode = self.mode;
        self.mode = Mode::Text;
    }

    // Stack and scope

    fn in_scope(&self, names: &[&str], extra: &[&str]) -> bool {
        for element in self.stack.iter().rev() {
            let name = element.local_name();
            if names.contains(&name) {
                return true;
            }
            if SCOPE_BARRIERS.contains(&name) || extra.contains(&name) || self.is_foreign(element) {
                return false;
            }
        }
        false
    }

    fn in_button_scope(&self, name: &str) -> bool {
        self.in_scope(&[name], &["button"])
    }

    fn in_table_scope(&self, names: &[&str]) -> bool {
        for element in self.stack.iter().rev() {
            let name = element.local_name();
            if names.contains(&name) {
                return true;
            }
            if matches!(name, "html" | "table" | "template") {
                return false;
            }
        }
        false
    }

    fn element_in_scope(&self, target: &DomNode) -> bool {
        for element in self.stack.iter().rev() {
            if element == target {
                return true;
            }
            if SCOPE_BARRIERS.contains(&element.local_name()) {
                return false;
            }
        }
        false
    }

    fn generate_implied_end_tags(&mut self, except: Option<&str>) {
        while let Some(node) = self.stack.last() {
            let name = node.local_name();
            if !IMPLIED_END_TAGS.contains(&name) || Some(name) == except {
                break;
            }
            self.stack.pop();
        }
    }

    fn pop_until(&mut self, names: &[&str]) {
        while let Some(node) = self.stack.pop() {
            if names.contains(&node.local_name()) {
                return;
            }
        }
    }

    fn close_paragraph(&mut self) {
        if !self.in_button_scope("p") {
            return;
        }
        self.generate_implied_end_tags(Some("p"));
        self.pop_until(&["p"]);
    }

    fn clear_stack_back_to(&mut self, names: &[&str]) {
        while self
            .stack
            .last()
            .is_some_and(|n| !names.contains(&n.local_name()))
        {
            self.stack.pop();
        }
    }

    fn reset_insertion_mode(&mut self) {
        for index in (0..self.stack.len()).rev() {
            let last = index == 0;
            self.mode = match self.stack[index].local_name() {
                "td" | "th" if !last => Mode::InCell,
                "tr" => Mode::InRow,
                "tbody" | "thead" | "tfoot" => Mode::InTableBody,
                "caption" => Mode::InCaption,
                "colgroup" => Mode::InColumnGroup,
                "table" => Mode::InTable,
                "head" if !last => Mode::InHead,
                "body" => Mode::InBody,
                "html" => {
                    if self.head.is_none() {
                        Mode::BeforeHead
                    } else {
                        Mode::AfterHead
                    }
                }
                _ => {
                    if last {
                        self.mode = Mode::InBody;
                    }
                    continue;
                }
            };
            return;
        }
        self.mode = Mode::InBody;
    }

    // Active formatting elements

    fn push_formatting(&mut self, element: DomNode) {
        // At most three matching entries after the last marker.
        let mut matches: Vec<usize> = Vec::new();
        let attributes = element.attributes();
        for index in (0..self.formatting.len()).rev() {
            let Some(entry) = &self.formatting[index] else {
                break;
            };
            if entry.local_name() == element.local_name() && entry.attributes() == attributes {
                matches.push(index);
            }
        }
        if matches.len() >= 3 {
            let earliest = *matches.last().expect("matches");
            self.formatting.remove(earliest);
        }
        self.formatting.push(Some(element));
    }

    fn is_open(&self, element: &DomNode) -> bool {
        self.stack.contains(element)
    }

    fn reconstruct_formatting(&mut self) {
        let Some(Some(last)) = self.formatting.last() else {
            return;
        };
        if self.is_open(last) {
            return;
        }
        let mut index = self.formatting.len() - 1;
        while index > 0 {
            match &self.formatting[index - 1] {
                Some(entry) if !self.is_open(entry) => index -= 1,
                _ => break,
            }
        }
        for entry_index in index..self.formatting.len() {
            let entry = self.formatting[entry_index]
                .clone()
                .expect("an element after the last marker");
            let element = self.insert_element(entry.local_name(), &Self::attributes_of(&entry));
            self.formatting[entry_index] = Some(element);
        }
    }

    fn clear_formatting_to_marker(&mut self) {
        while let Some(entry) = self.formatting.pop() {
            if entry.is_none() {
                return;
            }
        }
    }

    fn formatting_element_after_marker(&self, name: &str) -> Option<usize> {
        for index in (0..self.formatting.len()).rev() {
            let entry = self.formatting[index].as_ref()?;
            if entry.local_name() == name {
                return Some(index);
            }
        }
        None
    }

    fn formatting_index_of(&self, element: &DomNode) -> Option<usize> {
        self.formatting
            .iter()
            .position(|e| e.as_ref() == Some(element))
    }

    /// The adoption agency algorithm for an end tag `subject`; `false` when
    /// the tag should be handled as any other end tag.
    fn adoption_agency(&mut self, subject: &str) -> bool {
        let current = self.current_node().clone();
        if current.local_name() == subject && self.formatting_index_of(&current).is_none() {
            self.stack.pop();
            return true;
        }
        for _ in 0..8 {
            let Some(formatting_index) = self.formatting_element_after_marker(subject) else {
                return false;
            };
            let formatting_element = self.formatting[formatting_index]
                .clone()
                .expect("a formatting element");
            let Some(stack_index) = self.stack.iter().position(|e| *e == formatting_element) else {
                self.formatting.remove(formatting_index);
                return true;
            };
            if !self.element_in_scope(&formatting_element) {
                return true;
            }
            let Some(furthest_index) = (stack_index + 1..self.stack.len())
                .find(|&i| SPECIAL_ELEMENTS.contains(&self.stack[i].local_name()))
            else {
                self.stack.truncate(stack_index);
                self.formatting.remove(formatting_index);
                return true;
            };
            let furthest_block = self.stack[furthest_index].clone();
            let common_ancestor = self.stack[stack_index - 1].clone();
            let mut bookmark = formatting_index;
            let mut last_node = furthest_block.clone();
            let mut node_index = furthest_index;
            let mut inner_loop = 0;
            loop {
                inner_loop += 1;
                node_index -= 1;
                let node = self.stack[node_index].clone();
                if node == formatting_element {
                    break;
                }
                let mut node_formatting_index = self.formatting_index_of(&node);
                if inner_loop > 3
                    && let Some(index) = node_formatting_index
                {
                    self.formatting.remove(index);
                    if index < bookmark {
                        bookmark -= 1;
                    }
                    node_formatting_index = None;
                }
                let Some(index) = node_formatting_index else {
                    self.stack.remove(node_index);
                    continue;
                };
                let replacement =
                    self.create_element(node.local_name(), &Self::attributes_of(&node));
                self.formatting[index] = Some(replacement.clone());
                self.stack[node_index] = replacement.clone();
                if last_node == furthest_block {
                    bookmark = index + 1;
                }
                replacement.append_child(&last_node);
                last_node = replacement;
            }
            let (parent, before) = self.insertion_place(Some(&common_ancestor));
            parent.insert_before(&last_node, before.as_ref());
            let new_element = self.create_element(
                formatting_element.local_name(),
                &Self::attributes_of(&formatting_element),
            );
            for child in furthest_block.child_nodes() {
                new_element.append_child(&child);
            }
            furthest_block.append_child(&new_element);
            if let Some(index) = self.formatting_index_of(&formatting_element) {
                self.formatting.remove(index);
                if index < bookmark {
                    bookmark -= 1;
                }
            }
            let at = bookmark.min(self.formatting.len());
            self.formatting.insert(at, Some(new_element.clone()));
            self.stack.retain(|e| *e != formatting_element);
            if let Some(index) = self.stack.iter().position(|e| *e == furthest_block) {
                self.stack.insert(index + 1, new_element);
            }
        }
        true
    }

    // Insertion modes

    fn initial(&mut self, token: Token) {
        match token {
            Token::Comment(data) => {
                let document = self.document.clone();
                self.insert_comment(&data, Some(document));
            }
            Token::Doctype(name) => {
                self.quirks = name != "html";
                self.mode = Mode::BeforeHtml;
            }
            token => {
                let Some(rest) = self.split_whitespace(token, |_, _| {}) else {
                    return;
                };
                self.mode = Mode::BeforeHtml;
                self.process(rest);
            }
        }
    }

    fn before_html(&mut self, token: Token) {
        match token {
            Token::Doctype(_) => {}
            Token::Comment(data) => {
                let document = self.document.clone();
                self.insert_comment(&data, Some(document));
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "html" => {
                let html = self.create_element("html", attributes);
                self.document.append_child(&html);
                self.stack.push(html);
                self.mode = Mode::BeforeHead;
            }
            Token::EndTag(ref name)
                if !matches!(name.as_str(), "head" | "body" | "html" | "br") => {}
            token => {
                let Some(rest) = self.split_whitespace(token, |_, _| {}) else {
                    return;
                };
                let html = self.create_element("html", &[]);
                self.document.append_child(&html);
                self.stack.push(html);
                self.mode = Mode::BeforeHead;
                self.process(rest);
            }
        }
    }

    fn before_head(&mut self, token: Token) {
        match token {
            Token::Doctype(_) => {}
            Token::Comment(data) => self.insert_comment(&data, None),
            ref t if t.is_start("html") => self.in_body(token),
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "head" => {
                self.head = Some(self.insert_element("head", attributes));
                self.mode = Mode::InHead;
            }
            Token::EndTag(ref name)
                if !matches!(name.as_str(), "head" | "body" | "html" | "br") => {}
            token => {
                let Some(rest) = self.split_whitespace(token, |_, _| {}) else {
                    return;
                };
                self.head = Some(self.insert_element("head", &[]));
                self.mode = Mode::InHead;
                self.process(rest);
            }
        }
    }

    /// The start tags the head handles, wherever they appear.
    fn head_start_tag(&mut self, name: &str, attributes: &[Attribute]) -> bool {
        match name {
            "base" | "basefont" | "bgsound" | "link" | "meta" => self.insert_void(name, attributes),
            "title" => self.start_raw_text(name, attributes, TokenizerState::Rcdata),
            "noframes" | "style" | "script" => {
                self.start_raw_text(name, attributes, TokenizerState::Rawtext);
            }
            _ => return false,
        }
        true
    }

    fn in_head(&mut self, token: Token) {
        match token {
            Token::Doctype(_) => {}
            Token::Comment(data) => self.insert_comment(&data, None),
            ref t if t.is_start("html") => self.in_body(token),
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "noscript" => {
                self.insert_element("noscript", attributes);
                self.mode = Mode::InHeadNoscript;
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if matches!(
                name.as_str(),
                "base"
                    | "basefont"
                    | "bgsound"
                    | "link"
                    | "meta"
                    | "title"
                    | "noframes"
                    | "style"
                    | "script"
            ) =>
            {
                self.head_start_tag(name, attributes);
            }
            ref t if t.is_start("head") => {}
            ref t if t.is_end("head") => {
                self.stack.pop();
                self.mode = Mode::AfterHead;
            }
            Token::EndTag(ref name) if !matches!(name.as_str(), "body" | "html" | "br") => {}
            token => {
                let Some(rest) = self.split_whitespace(token, |s, w| s.insert_characters(w)) else {
                    return;
                };
                self.stack.pop();
                self.mode = Mode::AfterHead;
                self.process(rest);
            }
        }
    }

    fn in_head_noscript(&mut self, token: Token) {
        match token {
            Token::Doctype(_) => {}
            ref t if t.is_start("html") => self.in_body(token),
            ref t if t.is_end("noscript") => {
                self.stack.pop();
                self.mode = Mode::InHead;
            }
            Token::Comment(_) => self.in_head(token),
            Token::StartTag { ref name, .. }
                if matches!(
                    name.as_str(),
                    "basefont" | "bgsound" | "link" | "meta" | "noframes" | "style"
                ) =>
            {
                self.in_head(token);
            }
            Token::StartTag { ref name, .. } if matches!(name.as_str(), "head" | "noscript") => {}
            Token::EndTag(ref name) if name != "br" => {}
            token => {
                let Some(rest) = self.split_whitespace(token, |s, w| s.insert_characters(w)) else {
                    return;
                };
                self.stack.pop();
                self.mode = Mode::InHead;
                self.process(rest);
            }
        }
    }

    fn after_head(&mut self, token: Token) {
        match token {
            Token::Doctype(_) => {}
            Token::Comment(data) => self.insert_comment(&data, None),
            ref t if t.is_start("html") => self.in_body(token),
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "body" => {
                self.insert_element("body", attributes);
                self.mode = Mode::InBody;
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if matches!(
                name.as_str(),
                "base"
                    | "basefont"
                    | "bgsound"
                    | "link"
                    | "meta"
                    | "noframes"
                    | "script"
                    | "style"
                    | "title"
            ) =>
            {
                let head = self.head.clone();
                if let Some(head) = &head {
                    self.stack.push(head.clone());
                }
                self.head_start_tag(name, attributes);
                if let Some(head) = &head {
                    self.stack.retain(|e| e != head);
                }
            }
            ref t if t.is_start("head") => {}
            Token::EndTag(ref name) if !matches!(name.as_str(), "body" | "html" | "br") => {}
            token => {
                let Some(rest) = self.split_whitespace(token, |s, w| s.insert_characters(w)) else {
                    return;
                };
                self.insert_element("body", &[]);
                self.mode = Mode::InBody;
                self.process(rest);
            }
        }
    }

    fn text(&mut self, token: Token) {
        match token {
            Token::Characters(text) => self.insert_characters(&text),
            Token::Eof => {
                self.stack.pop();
                self.mode = self.original_mode;
                self.process(token);
            }
            Token::EndTag(_) => {
                self.stack.pop();
                self.mode = self.original_mode;
            }
            _ => {}
        }
    }

    fn merge_attributes(attributes: &[Attribute], element: &DomNode) {
        for attribute in attributes {
            if !element.has_attribute(&attribute.name) {
                element.set_attribute(&attribute.name, &attribute.value);
            }
        }
    }

    fn in_body(&mut self, token: Token) {
        match token {
            Token::Characters(text) => {
                self.reconstruct_formatting();
                self.insert_characters(&text);
            }
            Token::Comment(data) => self.insert_comment(&data, None),
            Token::Doctype(_) => {}
            Token::Eof => self.stopped = true,
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => self.in_body_start_tag(&name, &attributes, self_closing),
            Token::EndTag(name) => self.in_body_end_tag(&name),
        }
    }

    fn in_body_start_tag(&mut self, name: &str, attributes: &[Attribute], self_closing: bool) {
        match name {
            "html" => Self::merge_attributes(attributes, &self.stack[0]),
            "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script" | "style"
            | "title" => {
                self.head_start_tag(name, attributes);
            }
            "body" => {
                if self.stack.len() > 1 && self.stack[1].local_name() == "body" {
                    Self::merge_attributes(attributes, &self.stack[1]);
                }
            }
            "frameset" => {}
            "template" => {
                self.insert_element(name, attributes);
                self.formatting.push(None);
            }
            _ if CLOSES_PARAGRAPH.contains(&name) => {
                self.close_paragraph();
                self.insert_element(name, attributes);
            }
            _ if HEADINGS.contains(&name) => {
                self.close_paragraph();
                if HEADINGS.contains(&self.current_name()) {
                    self.stack.pop();
                }
                self.insert_element(name, attributes);
            }
            "pre" | "listing" => {
                self.close_paragraph();
                self.insert_element(name, attributes);
                self.skip_next_newline = true;
            }
            "form" => {
                self.close_paragraph();
                self.insert_element(name, attributes);
            }
            "li" | "dd" | "dt" => {
                let closes: &[&str] = if name == "li" { &["li"] } else { &["dd", "dt"] };
                for node in self.stack.clone().iter().rev() {
                    let node_name = node.local_name();
                    if closes.contains(&node_name) {
                        self.generate_implied_end_tags(Some(node_name));
                        self.pop_until(&[node_name]);
                        break;
                    }
                    if SPECIAL_ELEMENTS.contains(&node_name)
                        && !matches!(node_name, "address" | "div" | "p")
                    {
                        break;
                    }
                }
                self.close_paragraph();
                self.insert_element(name, attributes);
            }
            "plaintext" => {
                self.close_paragraph();
                self.insert_element(name, attributes);
                self.tokenizer.state = TokenizerState::Plaintext;
            }
            "button" => {
                if self.in_scope(&["button"], &[]) {
                    self.generate_implied_end_tags(None);
                    self.pop_until(&["button"]);
                }
                self.reconstruct_formatting();
                self.insert_element(name, attributes);
            }
            "a" => {
                if let Some(index) = self.formatting_element_after_marker("a") {
                    let element = self.formatting[index].clone().expect("an element");
                    self.adoption_agency("a");
                    self.formatting.retain(|e| e.as_ref() != Some(&element));
                    self.stack.retain(|e| *e != element);
                }
                self.reconstruct_formatting();
                let element = self.insert_element(name, attributes);
                self.push_formatting(element);
            }
            _ if FORMATTING_TAGS.contains(&name) => {
                self.reconstruct_formatting();
                let element = self.insert_element(name, attributes);
                self.push_formatting(element);
            }
            "nobr" => {
                self.reconstruct_formatting();
                if self.in_scope(&["nobr"], &[]) {
                    self.adoption_agency("nobr");
                    self.reconstruct_formatting();
                }
                let element = self.insert_element(name, attributes);
                self.push_formatting(element);
            }
            "applet" | "marquee" | "object" => {
                self.reconstruct_formatting();
                self.insert_element(name, attributes);
                self.formatting.push(None);
            }
            "table" => {
                // In quirks mode a table stays inside an open paragraph.
                if !self.quirks {
                    self.close_paragraph();
                }
                self.insert_element(name, attributes);
                self.mode = Mode::InTable;
            }
            "area" | "br" | "embed" | "img" | "image" | "keygen" | "wbr" | "input" => {
                self.reconstruct_formatting();
                self.insert_void(if name == "image" { "img" } else { name }, attributes);
            }
            "param" | "source" | "track" => self.insert_void(name, attributes),
            "hr" => {
                self.close_paragraph();
                self.insert_void(name, attributes);
            }
            "textarea" => {
                self.start_raw_text(name, attributes, TokenizerState::Rcdata);
                self.skip_next_newline = true;
            }
            "xmp" => {
                self.close_paragraph();
                self.reconstruct_formatting();
                self.start_raw_text(name, attributes, TokenizerState::Rawtext);
            }
            "iframe" | "noembed" => self.start_raw_text(name, attributes, TokenizerState::Rawtext),
            "optgroup" | "option" => {
                if self.current_name() == "option" {
                    self.stack.pop();
                }
                self.reconstruct_formatting();
                self.insert_element(name, attributes);
            }
            "math" | "svg" => {
                self.reconstruct_formatting();
                self.insert_foreign(name, attributes, self_closing);
            }
            "caption" | "col" | "colgroup" | "frame" | "head" | "tbody" | "td" | "tfoot" | "th"
            | "thead" | "tr" => {}
            _ => {
                self.reconstruct_formatting();
                self.insert_element(name, attributes);
            }
        }
    }

    fn in_body_end_tag(&mut self, name: &str) {
        match name {
            "body" | "html" => {
                if !self.in_scope(&["body"], &[]) {
                    return;
                }
                self.mode = Mode::AfterBody;
                if name == "html" {
                    self.process(Token::EndTag(name.to_owned()));
                }
            }
            _ if is_block_end_tag(name) || name == "form" || name == "template" => {
                if !self.in_scope(&[name], &[]) {
                    return;
                }
                self.generate_implied_end_tags(None);
                self.pop_until(&[name]);
                if name == "template" {
                    self.clear_formatting_to_marker();
                }
            }
            "p" => {
                if !self.in_button_scope("p") {
                    self.insert_element("p", &[]);
                }
                self.close_paragraph();
            }
            "li" => {
                if !self.in_scope(&["li"], &["ol", "ul"]) {
                    return;
                }
                self.generate_implied_end_tags(Some("li"));
                self.pop_until(&["li"]);
            }
            "dd" | "dt" => {
                if !self.in_scope(&[name], &[]) {
                    return;
                }
                self.generate_implied_end_tags(Some(name));
                self.pop_until(&[name]);
            }
            _ if HEADINGS.contains(&name) => {
                if !self.in_scope(HEADINGS, &[]) {
                    return;
                }
                self.generate_implied_end_tags(None);
                self.pop_until(HEADINGS);
            }
            _ if name == "a" || name == "nobr" || FORMATTING_TAGS.contains(&name) => {
                if !self.adoption_agency(name) {
                    self.any_other_end_tag(name);
                }
            }
            "applet" | "marquee" | "object" => {
                if !self.in_scope(&[name], &[]) {
                    return;
                }
                self.generate_implied_end_tags(None);
                self.pop_until(&[name]);
                self.clear_formatting_to_marker();
            }
            "br" => {
                self.reconstruct_formatting();
                self.insert_void("br", &[]);
            }
            _ => self.any_other_end_tag(name),
        }
    }

    fn any_other_end_tag(&mut self, name: &str) {
        for index in (0..self.stack.len()).rev() {
            let node = self.stack[index].clone();
            if node.local_name() == name {
                self.generate_implied_end_tags(Some(name));
                if let Some(index) = self.stack.iter().rposition(|e| *e == node) {
                    self.stack.truncate(index);
                }
                return;
            }
            if SPECIAL_ELEMENTS.contains(&node.local_name()) {
                return;
            }
        }
    }

    /// Handles a token in body mode, with misplaced content moved in front
    /// of the table.
    fn foster_parent(&mut self, token: Token) {
        self.foster_parenting = true;
        self.in_body(token);
        self.foster_parenting = false;
    }

    fn in_table(&mut self, token: Token) {
        const BACK_TO: &[&str] = &["table", "template", "html"];
        match token {
            Token::Characters(ref text) => {
                if matches!(
                    self.current_name(),
                    "table" | "tbody" | "tfoot" | "thead" | "tr"
                ) && !text.chars().all(is_html_whitespace)
                {
                    self.foster_parent(token);
                } else {
                    self.insert_characters(text);
                }
            }
            Token::Comment(data) => self.insert_comment(&data, None),
            Token::Doctype(_) => {}
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "caption" => {
                self.clear_stack_back_to(BACK_TO);
                self.formatting.push(None);
                self.insert_element("caption", attributes);
                self.mode = Mode::InCaption;
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "colgroup" => {
                self.clear_stack_back_to(BACK_TO);
                self.insert_element("colgroup", attributes);
                self.mode = Mode::InColumnGroup;
            }
            ref t if t.is_start("col") => {
                self.clear_stack_back_to(BACK_TO);
                self.insert_element("colgroup", &[]);
                self.mode = Mode::InColumnGroup;
                self.process(token);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if TABLE_SECTIONS.contains(&name.as_str()) => {
                self.clear_stack_back_to(BACK_TO);
                self.insert_element(name, attributes);
                self.mode = Mode::InTableBody;
            }
            Token::StartTag { ref name, .. } if matches!(name.as_str(), "td" | "th" | "tr") => {
                self.clear_stack_back_to(BACK_TO);
                self.insert_element("tbody", &[]);
                self.mode = Mode::InTableBody;
                self.process(token);
            }
            ref t if t.is_start("table") => {
                if !self.in_table_scope(&["table"]) {
                    return;
                }
                self.pop_until(&["table"]);
                self.reset_insertion_mode();
                self.process(token);
            }
            Token::StartTag { ref name, .. } if matches!(name.as_str(), "style" | "script") => {
                self.in_head(token);
            }
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "input"
                && attributes
                    .iter()
                    .any(|a| a.name == "type" && a.value.to_lowercase() == "hidden") =>
            {
                self.insert_void("input", attributes);
            }
            ref t if t.is_end("table") => {
                if !self.in_table_scope(&["table"]) {
                    return;
                }
                self.pop_until(&["table"]);
                self.reset_insertion_mode();
            }
            Token::EndTag(ref name)
                if matches!(
                    name.as_str(),
                    "body"
                        | "caption"
                        | "col"
                        | "colgroup"
                        | "html"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) => {}
            Token::Eof => self.in_body(token),
            token => self.foster_parent(token),
        }
    }

    fn in_caption(&mut self, token: Token) {
        match token {
            ref t if t.is_end("caption") => {
                if !self.in_table_scope(&["caption"]) {
                    return;
                }
                self.generate_implied_end_tags(None);
                self.pop_until(&["caption"]);
                self.clear_formatting_to_marker();
                self.mode = Mode::InTable;
            }
            Token::StartTag { ref name, .. }
                if matches!(
                    name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                self.close_caption_and_reprocess(token);
            }
            ref t if t.is_end("table") => self.close_caption_and_reprocess(token),
            Token::EndTag(ref name)
                if matches!(
                    name.as_str(),
                    "body"
                        | "col"
                        | "colgroup"
                        | "html"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) => {}
            token => self.in_body(token),
        }
    }

    fn close_caption_and_reprocess(&mut self, token: Token) {
        if !self.in_table_scope(&["caption"]) {
            return;
        }
        self.generate_implied_end_tags(None);
        self.pop_until(&["caption"]);
        self.clear_formatting_to_marker();
        self.mode = Mode::InTable;
        self.process(token);
    }

    fn in_column_group(&mut self, token: Token) {
        match token {
            Token::Comment(data) => self.insert_comment(&data, None),
            Token::Doctype(_) => {}
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "col" => self.insert_void("col", attributes),
            ref t if t.is_end("colgroup") => {
                if self.current_name() != "colgroup" {
                    return;
                }
                self.stack.pop();
                self.mode = Mode::InTable;
            }
            ref t if t.is_end("col") => {}
            Token::Eof => self.in_body(token),
            token => {
                let Some(rest) = self.split_whitespace(token, |s, w| s.insert_characters(w)) else {
                    return;
                };
                if self.current_name() != "colgroup" {
                    return;
                }
                self.stack.pop();
                self.mode = Mode::InTable;
                self.process(rest);
            }
        }
    }

    fn in_table_body(&mut self, token: Token) {
        const BACK_TO: &[&str] = &["tbody", "tfoot", "thead", "template", "html"];
        match token {
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "tr" => {
                self.clear_stack_back_to(BACK_TO);
                self.insert_element("tr", attributes);
                self.mode = Mode::InRow;
            }
            Token::StartTag { ref name, .. } if matches!(name.as_str(), "th" | "td") => {
                self.clear_stack_back_to(BACK_TO);
                self.insert_element("tr", &[]);
                self.mode = Mode::InRow;
                self.process(token);
            }
            Token::EndTag(ref name) if TABLE_SECTIONS.contains(&name.as_str()) => {
                if !self.in_table_scope(&[name]) {
                    return;
                }
                self.clear_stack_back_to(BACK_TO);
                self.stack.pop();
                self.mode = Mode::InTable;
            }
            Token::StartTag { ref name, .. }
                if matches!(
                    name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead"
                ) =>
            {
                self.close_table_section_and_reprocess(token);
            }
            ref t if t.is_end("table") => self.close_table_section_and_reprocess(token),
            Token::EndTag(ref name)
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th" | "tr"
                ) => {}
            token => self.in_table(token),
        }
    }

    fn close_table_section_and_reprocess(&mut self, token: Token) {
        if !self.in_table_scope(TABLE_SECTIONS) {
            return;
        }
        self.clear_stack_back_to(&["tbody", "tfoot", "thead", "template", "html"]);
        self.stack.pop();
        self.mode = Mode::InTable;
        self.process(token);
    }

    fn in_row(&mut self, token: Token) {
        const BACK_TO: &[&str] = &["tr", "template", "html"];
        match token {
            Token::StartTag {
                ref name,
                ref attributes,
                ..
            } if name == "td" || name == "th" => {
                self.clear_stack_back_to(BACK_TO);
                self.insert_element(name, attributes);
                self.mode = Mode::InCell;
                self.formatting.push(None);
            }
            ref t if t.is_end("tr") => {
                if !self.in_table_scope(&["tr"]) {
                    return;
                }
                self.clear_stack_back_to(BACK_TO);
                self.stack.pop();
                self.mode = Mode::InTableBody;
            }
            Token::StartTag { ref name, .. }
                if matches!(
                    name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead" | "tr"
                ) =>
            {
                self.close_row_and_reprocess(token);
            }
            ref t if t.is_end("table") => self.close_row_and_reprocess(token),
            Token::EndTag(ref name) if TABLE_SECTIONS.contains(&name.as_str()) => {
                if !self.in_table_scope(&[name]) {
                    return;
                }
                self.close_row_and_reprocess(token);
            }
            Token::EndTag(ref name)
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th"
                ) => {}
            token => self.in_table(token),
        }
    }

    fn close_row_and_reprocess(&mut self, token: Token) {
        if !self.in_table_scope(&["tr"]) {
            return;
        }
        self.clear_stack_back_to(&["tr", "template", "html"]);
        self.stack.pop();
        self.mode = Mode::InTableBody;
        self.process(token);
    }

    fn close_cell(&mut self) {
        self.generate_implied_end_tags(None);
        self.pop_until(&["td", "th"]);
        self.clear_formatting_to_marker();
        self.mode = Mode::InRow;
    }

    fn in_cell(&mut self, token: Token) {
        match token {
            Token::EndTag(ref name) if name == "td" || name == "th" => {
                if !self.in_table_scope(&[name]) {
                    return;
                }
                self.close_cell();
            }
            Token::StartTag { ref name, .. }
                if matches!(
                    name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                if !self.in_table_scope(&["td", "th"]) {
                    return;
                }
                self.close_cell();
                self.process(token);
            }
            Token::EndTag(ref name)
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html"
                ) => {}
            Token::EndTag(ref name)
                if matches!(name.as_str(), "table" | "tbody" | "tfoot" | "thead" | "tr") =>
            {
                if !self.in_table_scope(&[name]) {
                    return;
                }
                self.close_cell();
                self.process(token);
            }
            token => self.in_body(token),
        }
    }

    fn after_body(&mut self, token: Token) {
        match token {
            Token::Comment(data) => {
                let html = self.stack[0].clone();
                self.insert_comment(&data, Some(html));
            }
            Token::Doctype(_) => {}
            ref t if t.is_end("html") => self.mode = Mode::AfterAfterBody,
            Token::Eof => self.stopped = true,
            ref t if t.is_start("html") => self.in_body(token),
            token => {
                let Some(rest) = self.split_whitespace(token, |s, w| {
                    s.in_body(Token::Characters(w.to_owned()));
                }) else {
                    return;
                };
                self.mode = Mode::InBody;
                self.process(rest);
            }
        }
    }

    fn after_after_body(&mut self, token: Token) {
        match token {
            Token::Comment(data) => {
                let document = self.document.clone();
                self.insert_comment(&data, Some(document));
            }
            Token::Doctype(_) => self.in_body(token),
            ref t if t.is_start("html") => self.in_body(token),
            Token::Eof => self.stopped = true,
            token => {
                let Some(rest) = self.split_whitespace(token, |s, w| {
                    s.in_body(Token::Characters(w.to_owned()));
                }) else {
                    return;
                };
                self.mode = Mode::InBody;
                self.process(rest);
            }
        }
    }
}
