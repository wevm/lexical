//! `LexicalAutoLinkExtension.ts` (the `autolink` feature):
//! `createLinkMatcherWithRegExp`, `autoLinkUrlMatcher`,
//! `autoLinkEmailMatcher`, `AutoLinkConfig` and `registerAutoLink`.
//!
//! JS's regular expressions run on the `regex` crate, which this feature
//! alone brings in. Where the two engines differ, the patterns here spell
//! out what JS means (rust/PARITY.md):
//!
//! - Indices: `regex` reports UTF-8 byte offsets; a [`LinkMatcherResult`]'s
//!   `index` and `length` are UTF-16 code units, as JS's `match.index` and
//!   `match[0].length`.
//! - `\s`: JS's is ECMAScript's WhiteSpace and LineTerminator, which has
//!   U+FEFF and lacks U+0085; `regex`'s is Unicode's White_Space, the other
//!   way round. The patterns list JS's set ([`JS_WHITESPACE`]).
//! - `.`: JS's excludes the four line terminators; `regex`'s only `\n`.
//! - The `u` flag: a `u` regex (`URL_REGEX`) matches code points, as `regex`
//!   does. One without it (`EMAIL_REGEX`) matches UTF-16 code units, so a
//!   character outside the BMP counts twice towards `{1,64}`; it runs here
//!   on a string with one character per code unit ([`exec_code_units`]).
//! - `regex`'s leftmost-first `find` prefers what a backtracking engine
//!   does, so the patterns (no backreferences or lookaround) find the same
//!   match.

use super::link_extension::merge;
use super::link_node::{
    AutoLinkAttributes, TOGGLE_LINK_COMMAND, create_auto_link_node, is_auto_link_node, is_link_node,
};
use crate::editor::COMMAND_PRIORITY_LOW;
use crate::lexical_selection::{SelectionRef, Which, get_selection};
use crate::node::NodeRef;
use crate::nodes::line_break::LineBreakNode;
use crate::nodes::text::create_text_node;
use crate::{Editor, LexicalError, Unregister, utf16};
use regex::{Regex, RegexBuilder};
use std::fmt;
use std::rc::Rc;
use std::sync::LazyLock;

/// `ChangeHandler`: called when an autolink is created, updated or removed,
/// with the new `url` and the `prevUrl`; either is `None` (JS `null`) when
/// a link is added or removed.
pub type ChangeHandler = Rc<dyn Fn(Option<&str>, Option<&str>)>;

/// `LinkMatcherResult`. `index` and `length` are UTF-16 code units.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkMatcherResult {
    pub attributes: Option<AutoLinkAttributes>,
    pub index: usize,
    pub length: usize,
    pub text: String,
    pub url: String,
}

/// `LinkMatcher`: the first link in `text`, or `None` (JS `null`).
/// [`auto_link_url_matcher`] and [`auto_link_email_matcher`] are ones.
pub type LinkMatcher = Rc<dyn Fn(&str) -> Option<LinkMatcherResult>>;

/// `createLinkMatcherWithRegExp`'s `urlTransformer`: the URL of a matched
/// text.
pub type UrlTransformer = Rc<dyn Fn(&str) -> String>;

/// `excludeParents`' predicates: a text node in a parent one of them
/// returns true for is never linked.
pub type ExcludeParent = Rc<dyn Fn(&NodeRef) -> bool>;

/// What JS's `\s` matches, for use inside a class: ECMAScript's WhiteSpace
/// (tab, vertical tab, form feed, space, U+00A0, U+FEFF and the other
/// space separators, Zs) and LineTerminator (`\n`, `\r`, U+2028, U+2029).
/// It is the list Node prints for `/\s/`, and differs from `regex`'s `\s`
/// (Unicode White_Space) in U+FEFF, which only JS has, and U+0085, which
/// only `regex` has.
pub const JS_WHITESPACE: &str = r"\t\n\x0B\x0C\r\x20\u{A0}\u{1680}\u{2000}-\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}\u{FEFF}";

/// The size limit for compiling the patterns: `EMAIL_REGEX`'s nested
/// bounded repetitions (`{1,64}` within `{0,63}`) unroll to more than
/// `regex`'s default of 10 MiB.
const SIZE_LIMIT: usize = 64 << 20;

fn compile(pattern: &str) -> Regex {
    RegexBuilder::new(pattern)
        .size_limit(SIZE_LIMIT)
        .build()
        .expect("a valid pattern")
}

/// `URL_REGEX` (a `u` regex, which matches code points, as `regex` does).
static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    compile(
        r"((https?://(www\.)?)|(www\.))[-\p{L}\p{N}@:%._+~#=]{1,256}\.[\p{L}\p{N}]{1,6}(?:[-\p{L}\p{N}()@:%_+.~#?&//=]*[\p{L}\p{N}()@_~#?&//=])?",
    )
});

/// `EMAIL_REGEX`, which has no `u` flag, so it matches UTF-16 code units
/// ([`exec_code_units`]). JS reads `[` in a class as itself, where `regex`
/// starts a nested class, so it is escaped; `\s` is JS's ([`JS_WHITESPACE`]);
/// and `.` is JS's, which excludes every line terminator.
static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    let local = format!(r#"[^<>()\[\]\\.,;:{JS_WHITESPACE}@"]"#);
    compile(&format!(
        r#"(({local}{{1,64}}(\.{local}{{1,64}}){{0,63}})|("[^\n\r\u{{2028}}\u{{2029}}]{{1,255}}"))@((\[[0-9]{{1,3}}\.[0-9]{{1,3}}\.[0-9]{{1,3}}\.[0-9]{{1,3}}\])|(([a-zA-Z\-0-9]{{1,63}}\.){{1,127}}[a-zA-Z]{{2,63}}))"#
    ))
});

/// `PUNCTUATION_OR_SPACE`: `/[.,;\s]/`, the default `separatorRegex`.
static PUNCTUATION_OR_SPACE: LazyLock<Regex> =
    LazyLock::new(|| compile(&format!("[.,;{JS_WHITESPACE}]")));

/// `/[\s]/`, in `getTextNodesToMatch`.
static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| compile(&format!("[{JS_WHITESPACE}]")));

/// `/^\.[a-zA-Z]{2,}/`, in `startsWithTLD`.
static EMAIL_TLD: LazyLock<Regex> = LazyLock::new(|| compile(r"^\.[a-zA-Z]{2,}"));

/// `/^\.[a-zA-Z0-9]{1,}/`, in `startsWithTLD`.
static URL_TLD: LazyLock<Regex> = LazyLock::new(|| compile(r"^\.[a-zA-Z0-9]{1,}"));

/// What a surrogate code unit stands as in [`exec_code_units`]: a private
/// use character, which every class of `EMAIL_REGEX` treats as it treats a
/// surrogate (the negated classes and `.` match it, the others don't).
const SURROGATE_STAND_IN: char = '\u{E000}';

/// `regExp.exec(text)` for a regex without the `g` flag (which searches
/// from the start every time), by code point as a `u` regex: the match's
/// UTF-16 index and its text.
fn exec(reg_exp: &Regex, text: &str) -> Option<(usize, String)> {
    let m = reg_exp.find(text)?;
    Some((utf16::len(&text[..m.start()]), m.as_str().to_owned()))
}

/// `regExp.exec(text)` for a regex without the `u` flag, which matches
/// UTF-16 code units: `reg_exp` runs over a string with one character per
/// code unit, a surrogate standing as [`SURROGATE_STAND_IN`], so that its
/// bounds count code units as JS's do. A match that starts or ends inside a
/// surrogate pair keeps U+FFFD for the half it has (the unpaired surrogate
/// exception, rust/PARITY.md).
fn exec_code_units(reg_exp: &Regex, text: &str) -> Option<(usize, String)> {
    if text.is_ascii() {
        return exec(reg_exp, text);
    }
    let units: String = text
        .encode_utf16()
        .map(|unit| char::from_u32(unit.into()).unwrap_or(SURROGATE_STAND_IN))
        .collect();
    let m = reg_exp.find(&units)?;
    let index = units[..m.start()].chars().count();
    let length = m.as_str().chars().count();
    Some((index, utf16::slice(text, index, index + length)))
}

/// `createLinkMatcherWithRegExp(regExp, urlTransformer)`: a
/// [`LinkMatcher`] for the first match of `reg_exp`, whose URL is the
/// matched text as `url_transformer` rewrites it (unchanged without one, as
/// JS's default). `reg_exp` matches by code point, as a JS regex with the
/// `u` flag; JS's `g` and `y` flags, which make `exec` start where the last
/// match ended, have no counterpart.
pub fn create_link_matcher_with_reg_exp(
    reg_exp: Regex,
    url_transformer: Option<UrlTransformer>,
) -> LinkMatcher {
    Rc::new(move |text| {
        let (index, text) = exec(&reg_exp, text)?;
        Some(link_matcher_result(index, text, url_transformer.as_deref()))
    })
}

fn link_matcher_result(
    index: usize,
    text: String,
    url_transformer: Option<&dyn Fn(&str) -> String>,
) -> LinkMatcherResult {
    LinkMatcherResult {
        attributes: None,
        index,
        length: utf16::len(&text),
        url: url_transformer.map_or_else(|| text.clone(), |transform| transform(&text)),
        text,
    }
}

/// `autoLinkUrlMatcher`: URLs, with letters and numbers of any script
/// (`\p{L}`, `\p{N}`), balancing parentheses so that
/// `https://en.wikipedia.org/wiki/Fish_(disambiguation)` matches whole,
/// even inside prose parentheses. A URL without a scheme gets `https://`.
pub fn auto_link_url_matcher(text: &str) -> Option<LinkMatcherResult> {
    let (index, mut matched) = exec(&URL_REGEX, text)?;
    let mut depth: i64 = 0;
    for ch in matched.chars() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
        }
    }
    while depth < 0 && matched.ends_with(')') {
        matched.pop();
        depth += 1;
    }
    let url = if matched.starts_with("http") {
        matched.clone()
    } else {
        format!("https://{matched}")
    };
    Some(LinkMatcherResult {
        attributes: None,
        index,
        length: utf16::len(&matched),
        text: matched,
        url,
    })
}

/// `autoLinkEmailMatcher`: email addresses, whose URL is `mailto:` and the
/// address. JS makes it with `createLinkMatcherWithRegExp(EMAIL_REGEX, ...)`;
/// here it matches `EMAIL_REGEX` by UTF-16 code unit, as JS does for a
/// regex without the `u` flag, so a character outside the BMP counts twice
/// towards its bounds.
pub fn auto_link_email_matcher(text: &str) -> Option<LinkMatcherResult> {
    let (index, text) = exec_code_units(&EMAIL_REGEX, text)?;
    Some(link_matcher_result(
        index,
        text,
        Some(&|text: &str| format!("mailto:{text}")),
    ))
}

/// `AutoLinkConfig`. The default is `AutoLinkExtension`'s: no handlers, no
/// excluded parents, no matchers, and `/[.,;\s]/` (with JS's `\s`) as the
/// separators.
#[derive(Clone)]
pub struct AutoLinkConfig {
    pub change_handlers: Vec<ChangeHandler>,
    pub exclude_parents: Vec<ExcludeParent>,
    pub matchers: Vec<LinkMatcher>,
    /// `separatorRegex`: whether a character around a match counts as a
    /// separator when validating its boundaries. It is tested on one UTF-16
    /// code unit at a time (U+FFFD for half a surrogate pair), or on the
    /// string `undefined` past either end of a text, as JS tests
    /// `text[i]`.
    pub separator_regex: Regex,
}

impl Default for AutoLinkConfig {
    fn default() -> Self {
        AutoLinkConfig {
            change_handlers: Vec::new(),
            exclude_parents: Vec::new(),
            matchers: Vec::new(),
            separator_regex: PUNCTUATION_OR_SPACE.clone(),
        }
    }
}

impl fmt::Debug for AutoLinkConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AutoLinkConfig")
            .field("change_handlers", &self.change_handlers.len())
            .field("exclude_parents", &self.exclude_parents.len())
            .field("matchers", &self.matchers.len())
            .field("separator_regex", &self.separator_regex.as_str())
            .finish()
    }
}

/// `findFirstMatch(text, matchers)`.
fn find_first_match(text: &str, matchers: &[LinkMatcher]) -> Option<LinkMatcherResult> {
    matchers.iter().find_map(|matcher| matcher(text))
}

/// `text[index]`: the UTF-16 code unit at `index`, or `undefined`, which
/// `RegExp.test` reads as the string `"undefined"`.
fn char_at(text: &str, index: Option<usize>) -> String {
    match index {
        Some(i) if i < utf16::len(text) => utf16::slice(text, i, i + 1),
        _ => "undefined".to_owned(),
    }
}

/// `isSeparator(char, separatorRegex)`.
fn is_separator(char: &str, separator_regex: &Regex) -> bool {
    separator_regex.is_match(char)
}

/// `endsWithSeparator(textContent, separatorRegex)`.
fn ends_with_separator(text_content: &str, separator_regex: &Regex) -> bool {
    is_separator(
        &char_at(text_content, utf16::len(text_content).checked_sub(1)),
        separator_regex,
    )
}

/// `startsWithSeparator(textContent, separatorRegex)`.
fn starts_with_separator(text_content: &str, separator_regex: &Regex) -> bool {
    is_separator(&char_at(text_content, Some(0)), separator_regex)
}

/// `startsWithTLD(textContent, isEmail)`: whether the text can begin a
/// top-level domain, a full stop and then letters (or, for a URL, letters
/// and digits).
fn starts_with_tld(text_content: &str, is_email: bool) -> bool {
    if is_email {
        EMAIL_TLD.is_match(text_content)
    } else {
        URL_TLD.is_match(text_content)
    }
}

fn is_line_break_node(node: &NodeRef) -> bool {
    node.get_latest().is::<LineBreakNode>()
}

/// `isPreviousNodeValid(node, separatorRegex)`.
fn is_previous_node_valid(node: &NodeRef, separator_regex: &Regex) -> bool {
    let mut previous_node = node.get_previous_sibling();
    if let Some(previous) = &previous_node
        && previous.is_element()
    {
        previous_node = previous.get_last_descendant();
    }
    match previous_node {
        None => true,
        Some(previous) => {
            is_line_break_node(&previous)
                || (previous.is_text()
                    && ends_with_separator(&previous.get_text_content(), separator_regex))
        }
    }
}

/// `isNextNodeValid(node, separatorRegex)`.
fn is_next_node_valid(node: &NodeRef, separator_regex: &Regex) -> bool {
    let mut next_node = node.get_next_sibling();
    if let Some(next) = &next_node
        && next.is_element()
    {
        next_node = next.get_first_descendant();
    }
    match next_node {
        None => true,
        Some(next) => {
            is_line_break_node(&next)
                || (next.is_text()
                    && starts_with_separator(&next.get_text_content(), separator_regex))
        }
    }
}

/// JS reads `nodes[0]` of an empty array as `undefined`, and throws
/// calling a method on it.
fn undefined_node() -> LexicalError {
    LexicalError::new("Cannot read properties of undefined")
}

/// `isContentAroundIsValid(matchStart, matchEnd, separatorRegex, text,
/// nodes)`, with UTF-16 offsets into `text`.
fn is_content_around_is_valid(
    match_start: usize,
    match_end: usize,
    separator_regex: &Regex,
    text: &str,
    nodes: &[NodeRef],
) -> Result<bool, LexicalError> {
    let content_before_is_valid = if match_start > 0 {
        is_separator(&char_at(text, Some(match_start - 1)), separator_regex)
    } else {
        is_previous_node_valid(nodes.first().ok_or_else(undefined_node)?, separator_regex)
    };
    if !content_before_is_valid {
        return Ok(false);
    }
    Ok(if match_end < utf16::len(text) {
        is_separator(&char_at(text, Some(match_end)), separator_regex)
    } else {
        is_next_node_valid(nodes.last().ok_or_else(undefined_node)?, separator_regex)
    })
}

/// What `extractMatchingNodes` returns: `matchingOffset`,
/// `unmodifiedBeforeNodes`, `matchingNodes` and `unmodifiedAfterNodes`.
type MatchingNodes = (usize, Vec<NodeRef>, Vec<NodeRef>, Vec<NodeRef>);

/// `extractMatchingNodes(nodes, startIndex, endIndex)`.
fn extract_matching_nodes(
    nodes: &[NodeRef],
    start_index: usize,
    end_index: usize,
) -> MatchingNodes {
    let mut unmodified_before_nodes = Vec::new();
    let mut matching_nodes = Vec::new();
    let mut unmodified_after_nodes = Vec::new();
    let mut matching_offset = 0;

    let mut current_offset = 0;
    for current_node in nodes {
        let current_node_length = utf16::len(&current_node.get_text_content());
        let current_node_start = current_offset;
        let current_node_end = current_offset + current_node_length;

        if current_node_end <= start_index {
            unmodified_before_nodes.push(current_node.clone());
            matching_offset += current_node_length;
        } else if current_node_start >= end_index {
            unmodified_after_nodes.push(current_node.clone());
        } else {
            matching_nodes.push(current_node.clone());
        }
        current_offset += current_node_length;
    }
    (
        matching_offset,
        unmodified_before_nodes,
        matching_nodes,
        unmodified_after_nodes,
    )
}

/// The part `index` of a `splitText`, which JS destructures to `undefined`
/// when there is none.
fn part(parts: &[NodeRef], index: usize) -> Result<NodeRef, LexicalError> {
    parts.get(index).cloned().ok_or_else(undefined_node)
}

/// `textNode.setFormat(from.getFormat())`, then the detail and the style.
fn copy_text_format(text_node: &NodeRef, from: &NodeRef) -> Result<(), LexicalError> {
    let data = from.text_data();
    text_node.set_format(data.get_format());
    text_node.set_detail(data.get_detail())?;
    text_node.set_style(data.get_style());
    Ok(())
}

/// `$createAutoLinkNode_(nodes, startIndex, endIndex, match)`: wraps the
/// match, at those UTF-16 offsets into `nodes`' text, in a new autolink,
/// and returns the text node after it, if one was split off.
fn create_auto_link_node_(
    nodes: &[NodeRef],
    start_index: usize,
    end_index: usize,
    m: &LinkMatcherResult,
) -> Result<Option<NodeRef>, LexicalError> {
    let link_node = create_auto_link_node(&m.url, &m.attributes.clone().unwrap_or_default());
    if nodes.len() == 1 {
        let remaining_text_node = &nodes[0];
        let (link_text_node, remaining_text_node) = if start_index == 0 {
            let parts = remaining_text_node.split_text(&[end_index as i64])?;
            (part(&parts, 0)?, parts.get(1).cloned())
        } else {
            let parts = remaining_text_node.split_text(&[start_index as i64, end_index as i64])?;
            (part(&parts, 1)?, parts.get(2).cloned())
        };
        let text_node = create_text_node(&m.text);
        copy_text_format(&text_node, &link_text_node)?;
        link_node.append(&[text_node])?;
        link_text_node.replace(&link_node, false)?;
        return Ok(remaining_text_node);
    } else if nodes.len() > 1 {
        let first_text_node = &nodes[0];
        let mut offset = utf16::len(&first_text_node.get_text_content());
        let first_link_text_node = if start_index == 0 {
            first_text_node.clone()
        } else {
            part(&first_text_node.split_text(&[start_index as i64])?, 1)?
        };
        let mut link_nodes = Vec::new();
        let mut remaining_text_node = None;
        for current_node in &nodes[1..] {
            let current_node_length = utf16::len(&current_node.get_text_content());
            let current_node_start = offset;
            let current_node_end = offset + current_node_length;
            if current_node_start < end_index {
                if current_node_end <= end_index {
                    link_nodes.push(current_node.clone());
                } else {
                    let parts =
                        current_node.split_text(&[(end_index - current_node_start) as i64])?;
                    link_nodes.push(part(&parts, 0)?);
                    remaining_text_node = parts.get(1).cloned();
                }
            }
            offset += current_node_length;
        }
        let selection = get_selection();
        let selected_text_node = match &selection {
            Some(selection) => selection.get_nodes()?.into_iter().find(NodeRef::is_text),
            None => None,
        };
        let text_node = create_text_node(&first_link_text_node.get_text_content());
        copy_text_format(&text_node, &first_link_text_node)?;
        let mut children = vec![text_node.clone()];
        children.extend(link_nodes);
        link_node.append(&children)?;
        // it does not preserve caret position if caret was at the first text node
        // so we need to restore caret position
        if selected_text_node.as_ref() == Some(&first_link_text_node)
            && let Some(selection) = &selection
        {
            if selection.is_range() {
                let anchor = selection.point(Which::Anchor).expect("a range").offset;
                let focus = selection.point(Which::Focus).expect("a range").offset;
                text_node.select(Some(anchor), Some(focus))?;
            } else if selection.is_node() {
                let length = utf16::len(&text_node.get_text_content());
                text_node.select(Some(0), Some(length))?;
            }
        }
        first_link_text_node.replace(&link_node, false)?;
        return Ok(remaining_text_node);
    }
    Ok(None)
}

/// An autolink that isn't unlinked, which `registerAutoLink` keeps in step
/// with its text.
fn is_linked_auto_link(node: &NodeRef) -> bool {
    is_auto_link_node(node) && node.get_is_unlinked() != Some(true)
}

fn has_linked_auto_link_parent(node: &NodeRef) -> bool {
    node.get_parent()
        .is_some_and(|parent| is_linked_auto_link(&parent))
}

/// `$handleLinkCreation(nodes, matchers, onChange, separatorRegex)`: links
/// each match in the text of `nodes` whose surroundings are separators.
/// As in JS, the surroundings of a match after the first link are read at
/// its offset in the text before any was made (`initialText`).
fn handle_link_creation(
    nodes: &[NodeRef],
    matchers: &[LinkMatcher],
    on_change: &dyn Fn(Option<&str>, Option<&str>),
    separator_regex: &Regex,
) -> Result<(), LexicalError> {
    // Early return if any node is already part of an AutoLinkNode (idempotency check)
    if nodes.iter().any(has_linked_auto_link_parent) {
        return Ok(());
    }

    let mut current_nodes = nodes.to_vec();
    let initial_text: String = current_nodes
        .iter()
        .map(NodeRef::get_text_content)
        .collect();
    let mut text = initial_text.clone();
    let mut invalid_match_end = 0;

    while let Some(m) = find_first_match(&text, matchers) {
        let match_start = m.index;
        let match_length = m.length;
        let match_end = match_start + match_length;
        let is_valid = is_content_around_is_valid(
            invalid_match_end + match_start,
            invalid_match_end + match_end,
            separator_regex,
            &initial_text,
            &current_nodes,
        )?;

        if is_valid {
            let (matching_offset, _, matching_nodes, unmodified_after_nodes) =
                extract_matching_nodes(
                    &current_nodes,
                    invalid_match_end + match_start,
                    invalid_match_end + match_end,
                );

            // Skip if matching nodes are already part of an AutoLinkNode
            if matching_nodes.iter().any(has_linked_auto_link_parent) {
                invalid_match_end += match_end;
                text = utf16::slice_from(&text, match_end);
                continue;
            }

            let actual_match_start = invalid_match_end + match_start - matching_offset;
            let actual_match_end = invalid_match_end + match_end - matching_offset;
            let remaining_text_node =
                create_auto_link_node_(&matching_nodes, actual_match_start, actual_match_end, &m)?;
            current_nodes = match remaining_text_node {
                Some(remaining) => [vec![remaining], unmodified_after_nodes].concat(),
                None => unmodified_after_nodes,
            };
            on_change(Some(&m.url), None);
            invalid_match_end = 0;
        } else {
            invalid_match_end += match_end;
        }

        text = utf16::slice_from(&text, match_end);
    }
    Ok(())
}

/// `handleLinkEdit(linkNode, matchers, onChange, separatorRegex)`: unwraps
/// an autolink whose children aren't simple text, whose text is no longer
/// a whole match, or whose neighbors aren't separators; otherwise updates
/// its URL (and the matcher's attributes).
fn handle_link_edit(
    link_node: &NodeRef,
    matchers: &[LinkMatcher],
    on_change: &dyn Fn(Option<&str>, Option<&str>),
    separator_regex: &Regex,
) -> Result<(), LexicalError> {
    // Check children are simple text
    for child in link_node.get_children() {
        if !child.is_text() || !child.is_simple_text() {
            replace_with_children(link_node)?;
            on_change(None, link_node.get_url().as_deref());
            return Ok(());
        }
    }

    // Check text content fully matches
    let text = link_node.get_text_content();
    let m = match find_first_match(&text, matchers) {
        Some(m) if m.text == text => m,
        _ => {
            replace_with_children(link_node)?;
            on_change(None, link_node.get_url().as_deref());
            return Ok(());
        }
    };

    // Check neighbors
    if !is_previous_node_valid(link_node, separator_regex)
        || !is_next_node_valid(link_node, separator_regex)
    {
        replace_with_children(link_node)?;
        on_change(None, link_node.get_url().as_deref());
        return Ok(());
    }

    let url = link_node.get_url().unwrap_or_default();
    if url != m.url {
        link_node.set_url(&m.url);
        on_change(Some(&m.url), Some(&url));
    }

    if let Some(attributes) = &m.attributes {
        // JS compares the link's value (a string or null) with the
        // attribute, which is never equal when it is `undefined`, and sets
        // `attribute || null`.
        let refresh = |current: Option<String>,
                       value: &Option<Option<String>>,
                       set: &dyn Fn(Option<&str>)| {
            if value.as_ref() != Some(&current) {
                set(value.clone().flatten().filter(|v| !v.is_empty()).as_deref());
            }
        };
        refresh(link_node.get_rel(), &attributes.rel, &|v| {
            link_node.set_rel(v)
        });
        refresh(link_node.get_target(), &attributes.target, &|v| {
            link_node.set_target(v)
        });
        refresh(link_node.get_title(), &attributes.title, &|v| {
            link_node.set_title(v)
        });
    }
    Ok(())
}

/// `handleBadNeighbors(textNode, matchers, onChange, separatorRegex)`:
/// edits next to an autolink that make it invalid. Text that doesn't start
/// with a separator after it unwraps it, unless it continues its top-level
/// domain into a longer match, which it joins; text that doesn't end with
/// one before it unwraps it.
fn handle_bad_neighbors(
    text_node: &NodeRef,
    matchers: &[LinkMatcher],
    on_change: &dyn Fn(Option<&str>, Option<&str>),
    separator_regex: &Regex,
) -> Result<(), LexicalError> {
    let parent = text_node.get_parent();
    let previous_sibling = text_node.get_previous_sibling();
    let next_sibling = text_node.get_next_sibling();
    let text = text_node.get_text_content();

    // Skip if textNode is already part of an AutoLinkNode (idempotency check)
    // The handleLinkEdit on the parent will handle unwrapping if needed
    if parent.as_ref().is_some_and(is_linked_auto_link) {
        return Ok(());
    }

    // Handle case: textNode added AFTER a link, making link invalid
    if let Some(previous_sibling) = &previous_sibling
        && is_linked_auto_link(previous_sibling)
    {
        // Check if the textNode is still a sibling (hasn't been moved) to prevent loops
        if text_node.get_previous_sibling().as_ref() == Some(previous_sibling)
            && text_node.get_parent() == previous_sibling.get_parent()
        {
            // If text doesn't start with separator, link should be unwrapped
            // because non-separator after link makes the boundary invalid
            if !starts_with_separator(&text, separator_regex) {
                replace_with_children(previous_sibling)?;
                on_change(None, previous_sibling.get_url().as_deref());
                return Ok(());
            }

            // If text starts with separator, check if it's valid TLD continuation
            if starts_with_tld(&text, previous_sibling.is_email_uri()) {
                let combined_text = previous_sibling.get_text_content() + &text;
                if find_first_match(&combined_text, matchers)
                    .is_some_and(|m| m.text == combined_text)
                {
                    previous_sibling.append(std::slice::from_ref(text_node))?;
                    handle_link_edit(previous_sibling, matchers, on_change, separator_regex)?;
                    on_change(None, previous_sibling.get_url().as_deref());
                }
            }
            // If starts with separator but not valid TLD, do nothing (link stays valid)
        }
    }

    // Handle case: textNode added BEFORE a link, making link invalid
    if let Some(next_sibling) = &next_sibling
        && is_linked_auto_link(next_sibling)
        && !ends_with_separator(&text, separator_regex)
    {
        // Check if the nextSibling is still a sibling (hasn't been moved) to prevent loops
        if text_node.get_next_sibling().as_ref() == Some(next_sibling)
            && text_node.get_parent() == next_sibling.get_parent()
        {
            replace_with_children(next_sibling)?;
            on_change(None, next_sibling.get_url().as_deref());
        }
    }
    Ok(())
}

/// `replaceWithChildren(node)`: puts the element's children where it is and
/// removes it.
fn replace_with_children(node: &NodeRef) -> Result<Vec<NodeRef>, LexicalError> {
    let children = node.get_children();
    for child in children.iter().rev() {
        node.insert_after(child, true)?;
    }
    node.remove(false)?;
    Ok(children)
}

/// `getTextNodesToMatch(textNode)`: the text node and the simple text nodes
/// after it, up to the first with whitespace.
fn get_text_nodes_to_match(text_node: &NodeRef) -> Vec<NodeRef> {
    // check if next siblings are simple text nodes till a node contains a space separator
    let mut text_nodes_to_match = vec![text_node.clone()];
    let mut next_sibling = text_node.get_next_sibling();
    while let Some(next) = next_sibling
        && next.is_text()
        && next.is_simple_text()
    {
        text_nodes_to_match.push(next.clone());
        if WHITESPACE.is_match(&next.get_text_content()) {
            break;
        }
        next_sibling = next.get_next_sibling();
    }
    text_nodes_to_match
}

/// `registerAutoLink(editor, config)`: a `TextNode` transform that links
/// text the matchers match as it is typed and keeps autolinks in step with
/// edits, and a `TOGGLE_LINK_COMMAND` handler (above `registerLink`'s) that,
/// for `None`, flips `isUnlinked` on the autolinks around the selection and
/// lets the command go on. Returns what unregisters both.
pub fn register_auto_link(editor: &Editor, config: AutoLinkConfig) -> Unregister {
    let AutoLinkConfig {
        change_handlers,
        exclude_parents,
        matchers,
        separator_regex,
    } = config;
    let on_change = move |url: Option<&str>, prev_url: Option<&str>| {
        for handler in &change_handlers {
            handler(url, prev_url);
        }
    };
    merge(vec![
        editor.register_node_transform("text", move |text_node| {
            let parent = text_node.get_parent_or_throw()?;
            let previous = text_node.get_previous_sibling();
            if is_auto_link_node(&parent) {
                handle_link_edit(&parent, &matchers, &on_change, &separator_regex)?;
            } else if !is_link_node(&parent) && !exclude_parents.iter().any(|pred| pred(&parent)) {
                if text_node.is_simple_text()
                    && (starts_with_separator(&text_node.get_text_content(), &separator_regex)
                        || !previous.as_ref().is_some_and(is_auto_link_node))
                {
                    let text_nodes_to_match = get_text_nodes_to_match(text_node);
                    handle_link_creation(
                        &text_nodes_to_match,
                        &matchers,
                        &on_change,
                        &separator_regex,
                    )?;
                }

                handle_bad_neighbors(text_node, &matchers, &on_change, &separator_regex)?;
            }
            Ok(())
        }),
        editor.register_command(
            &TOGGLE_LINK_COMMAND,
            |payload, _| {
                let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
                    return Ok(false);
                };
                if payload.is_some() {
                    return Ok(false);
                }
                for node in selection.extract()? {
                    if let Some(parent) = node.get_parent()
                        && is_auto_link_node(&parent)
                    {
                        // invert the value
                        parent.set_is_unlinked(parent.get_is_unlinked() != Some(true));
                        parent.mark_dirty();
                    }
                }
                Ok(false)
            },
            // Has to be higher than TOGGLE_LINK_COMMAND in LinkExtension
            COMMAND_PRIORITY_LOW,
        ),
    ])
}
