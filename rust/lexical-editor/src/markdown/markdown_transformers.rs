//! `MarkdownTransformers.ts`: the transformer types, the transformers
//! upstream exports (`HEADING`, `QUOTE`, `CODE`, the lists, the text
//! formats and `LINK`), the markdown NodeStates, `normalizeMarkdown` and
//! `isTableRowDivider`.
//!
//! JS's transformers are objects compared by identity; here each is an
//! `Rc` of a struct with public fields, so a variant is made from a copy
//! (`{...LINK, importRegExp}` is `TextMatchTransformer { import_reg_exp,
//! ..(*link()).clone() }`), and the constants are functions that return the
//! same `Rc` on every call on a thread.

use super::import_text_format_transformer::scan_code_spans;
use super::markdown_export::is_selected;
use super::reg_exp::{RegExp, RegExpMatch};
use super::utils::{
    has_non_space, is_empty_paragraph, is_escaped, is_line_break_node, last_index_of, re, string,
    text_format, to_character_reference, trim, trim_start, unescape_text, units,
};
use crate::code::{create_code_node, is_code_node};
use crate::json::Value;
use crate::link::{LinkAttributes, create_link_node, is_auto_link_node, is_link_node};
use crate::list::{
    ListType, create_list_item_node, create_list_node, is_list_item_node, is_list_node,
};
use crate::nodes::line_break::create_line_break_node;
use crate::rich_text::{
    HeadingTagType, create_heading_node, create_quote_node, is_heading_node, is_quote_node,
};
use crate::utils::find_matching_parent;
use crate::{
    LexicalError, NodeRef, SelectionRef, StateConfig, StateValueConfig, create_state_with,
    create_text_node, get_state, set_state, utf16,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `Transformer`: one of the four kinds.
#[derive(Clone)]
pub enum Transformer {
    Element(Rc<ElementTransformer>),
    MultilineElement(Rc<MultilineElementTransformer>),
    TextFormat(Rc<TextFormatTransformer>),
    TextMatch(Rc<TextMatchTransformer>),
}

impl Transformer {
    /// `transformer.type`.
    pub fn transformer_type(&self) -> &'static str {
        match self {
            Transformer::Element(_) => "element",
            Transformer::MultilineElement(_) => "multiline-element",
            Transformer::TextFormat(_) => "text-format",
            Transformer::TextMatch(_) => "text-match",
        }
    }

    /// `transformer.dependencies`: the node types it needs, for those that
    /// have them.
    pub fn dependencies(&self) -> &[&'static str] {
        match self {
            Transformer::Element(t) => &t.dependencies,
            Transformer::MultilineElement(t) => &t.dependencies,
            Transformer::TextFormat(_) => &[],
            Transformer::TextMatch(t) => &t.dependencies,
        }
    }
}

impl From<Rc<ElementTransformer>> for Transformer {
    fn from(t: Rc<ElementTransformer>) -> Self {
        Transformer::Element(t)
    }
}

impl From<Rc<MultilineElementTransformer>> for Transformer {
    fn from(t: Rc<MultilineElementTransformer>) -> Self {
        Transformer::MultilineElement(t)
    }
}

impl From<Rc<TextFormatTransformer>> for Transformer {
    fn from(t: Rc<TextFormatTransformer>) -> Self {
        Transformer::TextFormat(t)
    }
}

impl From<Rc<TextMatchTransformer>> for Transformer {
    fn from(t: Rc<TextMatchTransformer>) -> Self {
        Transformer::TextMatch(t)
    }
}

/// `(node: ElementNode) => string`: exports an element's children.
pub type ExportChildren<'a> = &'a dyn Fn(&NodeRef) -> Result<String, LexicalError>;

/// `(node: TextNode, textContent: string) => string`: exports a text node's
/// formats around `textContent`.
pub type ExportFormat<'a> = &'a dyn Fn(&NodeRef, &str) -> Result<String, LexicalError>;

/// An element transformer's `export`: the markdown of `node`, or `None`
/// (JS `null`) to let the next transformer try. With a selection, only the
/// selected content is exported.
pub type ElementExport = Rc<
    dyn Fn(
        &NodeRef,
        ExportChildren<'_>,
        Option<&SelectionRef>,
    ) -> Result<Option<String>, LexicalError>,
>;

/// `ElementTransformer.replace(parentNode, children, match, isImport)`:
/// `false` cancels the transform (the next transformer is tried); JS's
/// `void` and `true` are both `true`.
pub type ElementReplace =
    Rc<dyn Fn(&NodeRef, &[NodeRef], &RegExpMatch, bool) -> Result<bool, LexicalError>>;

/// `ElementTransformer`: a block that a line's leading markdown makes,
/// such as a heading or a list item.
#[derive(Clone)]
pub struct ElementTransformer {
    pub dependencies: Vec<&'static str>,
    pub export: ElementExport,
    pub reg_exp: RegExp,
    pub replace: ElementReplace,
    /// `triggerOnEnter`: whether `registerMarkdownShortcuts` may run this
    /// transformer on Enter at the end of the line, without the space.
    pub trigger_on_enter: bool,
}

/// The arguments of `handleImportAfterStartMatch`.
pub struct MultilineImportArgs<'a> {
    pub lines: &'a [String],
    pub root_node: &'a NodeRef,
    pub start_line_index: usize,
    pub start_match: &'a RegExpMatch,
    pub transformer: &'a Rc<MultilineElementTransformer>,
}

/// What `handleImportAfterStartMatch` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultilineImportResult {
    /// `[imported, lastLineIndex]`.
    Handled(bool, usize),
    /// `null`: try the next multiline element transformer.
    Skip,
    /// `undefined`: the default behavior.
    Default,
}

/// `handleImportAfterStartMatch`.
pub type HandleImportAfterStartMatch =
    Rc<dyn Fn(&MultilineImportArgs<'_>) -> Result<MultilineImportResult, LexicalError>>;

/// `MultilineElementTransformer.replace(rootNode, children, startMatch,
/// endMatch, linesInBetween, isImport)`: `children` is given (and
/// `linesInBetween` is not) for a markdown shortcut; `false` cancels.
pub type MultilineReplace = Rc<
    dyn Fn(
        &NodeRef,
        Option<&[NodeRef]>,
        &RegExpMatch,
        Option<&RegExpMatch>,
        Option<Vec<String>>,
        bool,
    ) -> Result<bool, LexicalError>,
>;

/// `regExpEnd`: a regex, or `{optional: true, regExp}`.
#[derive(Clone, Debug)]
pub struct RegExpEnd {
    /// Whether the end match is optional: when none is found, the text from
    /// the start match to the end of the document matches.
    pub optional: bool,
    pub reg_exp: RegExp,
}

/// `MultilineElementTransformer`: a block that spans lines between a start
/// and an end match, such as a fenced code block.
#[derive(Clone)]
pub struct MultilineElementTransformer {
    pub handle_import_after_start_match: Option<HandleImportAfterStartMatch>,
    pub dependencies: Vec<&'static str>,
    pub export: Option<ElementExport>,
    pub reg_exp_start: RegExp,
    pub reg_exp_end: Option<RegExpEnd>,
    pub replace: MultilineReplace,
}

/// `TextFormatTransformer`: a text format between a pair of tags, such as
/// `**bold**`.
#[derive(Clone, Debug)]
pub struct TextFormatTransformer {
    pub format: Vec<&'static str>,
    pub tag: String,
    /// `intraword`; `Some(false)` keeps the tag from opening or closing
    /// inside a word.
    pub intraword: Option<bool>,
}

/// A text match transformer's `export`.
pub type TextMatchExport = Rc<
    dyn Fn(&NodeRef, ExportChildren<'_>, ExportFormat<'_>) -> Result<Option<String>, LexicalError>,
>;

/// `TextMatchTransformer.replace(node, match)`: may return a text node that
/// text formats are then imported in.
pub type TextMatchReplace =
    Rc<dyn Fn(&NodeRef, &RegExpMatch) -> Result<Option<NodeRef>, LexicalError>>;

/// `getEndIndex(node, match)`: `None` is JS `false`.
pub type GetEndIndex = Rc<dyn Fn(&NodeRef, &RegExpMatch) -> Result<Option<usize>, LexicalError>>;

/// `TextMatchTransformer`: inline text a regex matches, such as a link.
#[derive(Clone)]
pub struct TextMatchTransformer {
    pub dependencies: Vec<&'static str>,
    /// How a node exports, or `None` when this transformer does not export
    /// it. Export may call it with placeholder callbacks only to learn
    /// whether it writes the node, so it should have no side effects.
    pub export: Option<TextMatchExport>,
    /// What text is matched on import.
    pub import_reg_exp: Option<RegExp>,
    /// What text is matched for a markdown shortcut while typing.
    pub reg_exp: RegExp,
    pub replace: Option<TextMatchReplace>,
    pub get_end_index: Option<GetEndIndex>,
    /// The character whose typing triggers the transformer.
    pub trigger: Option<String>,
}

const ORDERED_LIST_REGEX: &str = r"^(\s*)(\d{1,})\.\s";
const UNORDERED_LIST_REGEX: &str = r"^(\s*)[-*+]\s";
const CHECK_LIST_REGEX: &str = r"^(\s*)(?:[-*+]\s)?\s?(\[(\s|x)?\])\s";
const HEADING_REGEX: &str = r"^(#{1,6})\s";
const QUOTE_REGEX: &str = r"^>\s";
const CODE_START_REGEX: &str = r"^([ \t]*`{3,})([\w-]+)?[ \t]?";
const CODE_END_REGEX: &str = r"^[ \t]*`{3,}$";
const CODE_SINGLE_LINE_REGEX: &str = r"^[ \t]*```[^`]+(?:(?:`{1,2}|`{4,})[^`]+)*```(?:[^`]|$)";
const TABLE_ROW_REG_EXP: &str = r"^(?:\|)(.+)(?:\|)\s?$";
const TAG_START_REGEX: &str = r"^<[a-z_][\w-]*(?:\s[^<>]*)?\/?>";
const TAG_END_REGEX: &str = r"^<\/[a-z_][\w-]*\s*>";

/// `isTableRowDivider(line)`: whether `line` is a table delimiter row such
/// as `| --- | :--: |`, the linear-time form of `/^(\| ?:?-*:? ?)+\|\s?$/`.
pub fn is_table_row_divider(line: &str) -> bool {
    let line = units(line);
    let at = |j: usize| line.get(j).copied();
    let is = |j: usize, c: u8| at(j) == Some(u16::from(c));
    if !is(0, b'|') {
        return false;
    }
    let length = line.len();
    let mut i = 1;
    let mut cells = 0;
    while i < length {
        let mut j = i;
        if is(j, b' ') {
            j += 1;
        }
        if is(j, b':') {
            j += 1;
        }
        while is(j, b'-') {
            j += 1;
        }
        if is(j, b':') {
            j += 1;
        }
        if is(j, b' ') {
            j += 1;
        }
        if !is(j, b'|') {
            break;
        }
        cells += 1;
        i = j + 1;
    }
    cells > 0 && (i == length || (i == length - 1 && super::utils::is_space(u32::from(line[i]))))
}

/// `ENDS_WITH(regex)`: `/(?:source)$/flags`.
fn ends_with(source: &str, flags: &str) -> RegExp {
    RegExp::of(&format!("(?:{source})$"), flags)
}

/// A string state with a `parse` and `resetOnCopyNode: true`.
fn string_state(key: &str, parse: impl Fn(&Value) -> String + 'static) -> StateConfig<String> {
    let mut config = StateValueConfig::new(parse);
    config.reset_on_copy_node = true;
    create_state_with(key, config)
}

thread_local! {
    static LIST_MARKER_STATE: StateConfig<String> = string_state("mdListMarker", |v| {
        match v.as_str() {
            Some(s) if re(r"^[-*+]$", "").test(s) => s.to_owned(),
            _ => "-".to_owned(),
        }
    });
    static CODE_FENCE_STATE: StateConfig<String> = string_state("mdCodeFence", |v| {
        match v.as_str() {
            Some(s) if re(r"^`{3,}$", "").test(s) => s.to_owned(),
            _ => "```".to_owned(),
        }
    });
    static CODE_META_STATE: StateConfig<String> =
        string_state("mdCodeMeta", |v| v.as_str().unwrap_or("").to_owned());
    static HARD_LINE_BREAK_STATE: StateConfig<String> = string_state("mdHardLineBreak", |v| {
        match v.as_str() {
            Some(s) if re(r"^(\\| {2,})$", "").test(s) => s.to_owned(),
            _ => String::new(),
        }
    });
}

/// `listMarkerState`: the bullet (`-`, `*` or `+`) a list was written with.
pub fn list_marker_state() -> StateConfig<String> {
    LIST_MARKER_STATE.with(Clone::clone)
}

/// `codeFenceState`: the fence (three or more backticks) a code block was
/// written with.
pub fn code_fence_state() -> StateConfig<String> {
    CODE_FENCE_STATE.with(Clone::clone)
}

/// `codeMetaState`: the info string's tail after a fenced code block's
/// language (`title="x"` in ```` ```js title="x" ````), which `CodeNode`
/// does not model.
pub fn code_meta_state() -> StateConfig<String> {
    CODE_META_STATE.with(Clone::clone)
}

/// `hardLineBreakState`: the marker (`\` or two or more spaces) of a hard
/// line break, on the line break node.
pub fn hard_line_break_state() -> StateConfig<String> {
    HARD_LINE_BREAK_STATE.with(Clone::clone)
}

/// `parseMarkdownHardLineBreak(line)`: the line without its hard line
/// break marker, and the marker, when it ends with one.
pub fn parse_markdown_hard_line_break(line: &str) -> Option<(String, String)> {
    if let Some(rest) = line.strip_suffix('\\') {
        // A trailing backslash is a hard line break only when it is not
        // itself escaped, so only an odd-length run counts.
        let backslashes = line.chars().rev().take_while(|c| *c == '\\').count();
        return (backslashes % 2 == 1).then(|| (rest.to_owned(), "\\".to_owned()));
    }
    let spaces = re(r"^(.*?\S)( {2,})$", "").exec(line)?;
    Some((spaces.str(1).to_owned(), spaces.str(2).to_owned()))
}

/// `hasNonWhitespaceContentOnLine(children, endIndex)`.
fn has_non_whitespace_content_on_line(children: &[NodeRef], end_index: usize) -> bool {
    for child in children[..end_index].iter().rev() {
        if is_line_break_node(child) {
            return false;
        }
        if has_non_space(&child.get_text_content()) {
            return true;
        }
    }
    false
}

/// `$extractMarkdownHardLineBreakMarker(previousNode)`.
fn extract_markdown_hard_line_break_marker(previous_node: &NodeRef) -> Option<String> {
    let children = previous_node.get_children();
    let last_child_index = children.len().checked_sub(1)?;
    let last_child = &children[last_child_index];
    if !last_child.is_text() {
        return None;
    }
    let last_text = last_child.get_text_content();
    if let Some((text, marker)) = parse_markdown_hard_line_break(&last_text) {
        last_child.set_text_content(&text);
        return Some(marker);
    }
    if re(r"^ {2,}$", "").test(&last_text)
        && has_non_whitespace_content_on_line(&children, last_child_index)
    {
        last_child.set_text_content("");
        return Some(last_text);
    }
    None
}

/// `$createMarkdownLineBreakNode(previousNode)`: a line break that keeps
/// the hard line break marker the end of `previous_node` holds.
pub fn create_markdown_line_break_node(previous_node: &NodeRef) -> Result<NodeRef, LexicalError> {
    let line_break = create_line_break_node();
    if let Some(marker) = extract_markdown_hard_line_break_marker(previous_node) {
        set_state(&line_break, &hard_line_break_state(), marker)?;
    }
    Ok(line_break)
}

/// `$isUnreplaceableBlock(parentNode)`: a quote holds inline content, so a
/// block shortcut typed in it would lose it.
fn is_unreplaceable_block(parent_node: &NodeRef) -> bool {
    is_quote_node(parent_node)
}

/// `stripFenceIndent(line, indent)`: removes up to `indent` spaces or tabs.
fn strip_fence_indent(line: &str, indent: usize) -> String {
    let u = units(line);
    let mut index = 0;
    while index < indent && matches!(u.get(index), Some(0x20 | 0x09)) {
        index += 1;
    }
    string(&u[index..])
}

/// `$setCodeMeta(parentNode, meta)`.
fn set_code_meta(parent_node: &NodeRef, meta: &str) -> Result<(), LexicalError> {
    if let Some(code_node) = parent_node.get_last_child()
        && !meta.is_empty()
        && is_code_node(&code_node)
    {
        set_state(&code_node, &code_meta_state(), meta.to_owned())?;
    }
    Ok(())
}

/// `createBlockNode(createNode)`'s replace.
fn create_block_node(
    node: NodeRef,
    parent_node: &NodeRef,
    children: &[NodeRef],
    is_import: bool,
) -> Result<bool, LexicalError> {
    node.append(children)?;
    parent_node.replace(&node, false)?;
    if !is_import {
        node.select(Some(0), Some(0))?;
    }
    Ok(true)
}

/// `LIST_INDENT_SIZE`: the spaces of one indentation level.
const LIST_INDENT_SIZE: usize = 4;

/// `getIndent(whitespaces)`.
fn get_indent(whitespaces: &str) -> usize {
    let tabs = whitespaces.chars().filter(|c| *c == '\t').count();
    let spaces = whitespaces.chars().filter(|c| *c == ' ').count();
    tabs + spaces / LIST_INDENT_SIZE
}

/// `getColumn(whitespaces)`: the column after `whitespaces`, with tabs
/// expanded to the next multiple of `LIST_INDENT_SIZE`.
fn get_column(whitespaces: &str) -> usize {
    let mut column = 0;
    for c in whitespaces.chars() {
        column += if c == '\t' {
            LIST_INDENT_SIZE - column % LIST_INDENT_SIZE
        } else {
            1
        };
    }
    column
}

thread_local! {
    /// `importListColumns`: the content columns of the list levels the
    /// lines placed so far have left open, or `None` outside an import.
    static IMPORT_LIST_COLUMNS: RefCell<Option<Vec<usize>>> = const { RefCell::new(None) };
    /// `importJoinsLooseLists`.
    static IMPORT_JOINS_LOOSE_LISTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `withListIndentColumns(joinLooseLists, fn)`: runs `f` with the columns
/// of one markdown import tracked across the lines it places, restoring
/// what was tracked around it.
pub(crate) fn with_list_indent_columns<T>(join_loose_lists: bool, f: impl FnOnce() -> T) -> T {
    let previous_columns = IMPORT_LIST_COLUMNS.with(|c| c.replace(Some(Vec::new())));
    let previous_joins = IMPORT_JOINS_LOOSE_LISTS.with(|c| c.replace(join_loose_lists));
    struct Restore(Option<Option<Vec<usize>>>, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            let columns = self.0.take().expect("columns to restore");
            IMPORT_LIST_COLUMNS.with(|c| *c.borrow_mut() = columns);
            IMPORT_JOINS_LOOSE_LISTS.with(|c| c.set(self.1));
        }
    }
    let _restore = Restore(Some(previous_columns), previous_joins);
    f()
}

/// `getColumnIndent(columns, whitespaces)`.
fn get_column_indent(columns: &[usize], whitespaces: &str) -> usize {
    let column = get_column(whitespaces);
    for i in (0..columns.len()).rev() {
        if column >= columns[i] {
            return i + 1;
        }
    }
    0
}

/// `getContentColumn(match, listType)`.
fn get_content_column(m: &RegExpMatch, list_type: ListType) -> usize {
    let mut prefix = m.str(0).to_owned();
    if list_type == ListType::Check {
        let rest = utf16::slice_from(&prefix, utf16::len(m.str(1)));
        if let Some(bullet) = re(r"^[-*+]\s", "").exec(&rest) {
            prefix = m.str(1).to_owned() + bullet.str(0);
        }
    }
    get_column(&prefix).min(get_column(m.str(1)) + LIST_INDENT_SIZE)
}

/// `setOpenColumn(columns, indent, match, listType)`.
fn set_open_column(columns: &mut Vec<usize>, indent: usize, m: &RegExpMatch, list_type: ListType) {
    for i in columns.len()..indent {
        columns.push((i + 1) * LIST_INDENT_SIZE);
    }
    columns.truncate(indent);
    columns.push(get_content_column(m, list_type));
}

/// `Number(match[2])`.
fn number(text: &str) -> f64 {
    text.parse().unwrap_or(f64::NAN)
}

/// `listReplace(listType)`'s replace.
fn list_replace(
    list_type: ListType,
    parent_node: &NodeRef,
    children: &[NodeRef],
    m: &RegExpMatch,
    is_import: bool,
) -> Result<bool, LexicalError> {
    if is_heading_node(parent_node) || (!is_import && is_unreplaceable_block(parent_node)) {
        return Ok(false);
    }
    let previous_node = parent_node.get_previous_sibling();
    let next_node = parent_node.get_next_sibling();
    let list_item = create_list_item_node(if list_type == ListType::Check {
        Some(re("^x$", "i").test(m.str(3)))
    } else {
        None
    });
    let first_match_char = trim(m.str(0)).chars().next().map(String::from);
    let list_marker = match &first_match_char {
        Some(c)
            if matches!(list_type, ListType::Bullet | ListType::Check)
                && matches!(c.as_str(), "-" | "*" | "+") =>
        {
            Some(c.clone())
        }
        _ => None,
    };
    // A block of another kind closes every level above this line; blank
    // lines only make the list loose (see upstream's comment).
    let mut columns = IMPORT_LIST_COLUMNS.with(|c| c.borrow_mut().take());
    let mut preceding_block = previous_node.clone();
    if let Some(columns) = columns.as_mut() {
        if IMPORT_JOINS_LOOSE_LISTS.with(std::cell::Cell::get) {
            while let Some(block) = preceding_block.clone().filter(is_empty_paragraph) {
                preceding_block = block.get_previous_sibling();
            }
        }
        if !preceding_block.as_ref().is_some_and(is_list_node) {
            columns.clear();
        }
    }
    let result = (|| -> Result<(), LexicalError> {
        let indent = match &columns {
            Some(c) if !c.is_empty() => get_column_indent(c, m.str(1)),
            _ => get_indent(m.str(1)),
        };
        if let Some(next) = next_node
            .as_ref()
            .filter(|n| is_list_node(n) && n.get_list_type() == Some(list_type))
        {
            match next.get_first_child() {
                Some(first) => {
                    first.insert_before(&list_item, true)?;
                }
                None => next.append(std::slice::from_ref(&list_item))?,
            }
            // The new item lands at index 0, so the typed number becomes
            // the list's start (#8677), unless it is indented.
            if list_type == ListType::Number && indent == 0 {
                next.set_list_start(number(m.str(2)));
            }
            parent_node.remove(false)?;
        } else if let Some(preceding) = preceding_block
            .as_ref()
            .filter(|p| is_list_node(p) && (p.get_list_type() == Some(list_type) || indent > 0))
        {
            preceding.append(std::slice::from_ref(&list_item))?;
            parent_node.remove(false)?;
        } else {
            let list = create_list_node(
                list_type,
                if list_type == ListType::Number {
                    number(m.str(2))
                } else {
                    1.0
                },
            );
            list.append(std::slice::from_ref(&list_item))?;
            parent_node.replace(&list, false)?;
        }
        list_item.append(children)?;
        if !is_import {
            list_item.select(Some(0), Some(0))?;
        }
        if indent > 0 {
            list_item.set_indent(indent as f64)?;
            retype_nested_list(&list_item, list_type, m)?;
        }
        // The marker belongs to the list the item ends up in.
        if let (Some(marker), Some(list_node)) = (&list_marker, list_item.get_parent())
            && is_list_node(&list_node)
        {
            set_state(&list_node, &list_marker_state(), marker.clone())?;
        }
        if let Some(columns) = columns.as_mut() {
            set_open_column(columns, indent, m, list_type);
        }
        Ok(())
    })();
    IMPORT_LIST_COLUMNS.with(|c| *c.borrow_mut() = columns);
    result.map(|()| true)
}

/// `$retypeNestedList(listItem, listType, match)`: moves an item nested by
/// `setIndent` into a list of its own type.
fn retype_nested_list(
    list_item: &NodeRef,
    list_type: ListType,
    m: &RegExpMatch,
) -> Result<(), LexicalError> {
    let Some(nested_list) = list_item.get_parent() else {
        return Ok(());
    };
    if !is_list_node(&nested_list) || nested_list.get_list_type() == Some(list_type) {
        return Ok(());
    }
    let Some(wrapper) = nested_list.get_parent().filter(is_list_item_node) else {
        return Ok(());
    };
    let retyped_list = create_list_node(
        list_type,
        if list_type == ListType::Number {
            number(m.str(2))
        } else {
            1.0
        },
    );
    let is_first = list_item.get_previous_sibling().is_none();
    retyped_list.append(std::slice::from_ref(list_item))?;
    let retyped_wrapper = create_list_item_node(None);
    retyped_wrapper.append(std::slice::from_ref(&retyped_list))?;
    if is_first {
        wrapper.insert_before(&retyped_wrapper, true)?;
    } else {
        wrapper.insert_after(&retyped_wrapper, true)?;
    }
    if nested_list.get_children_size() == 0 {
        wrapper.remove(false)?;
    }
    Ok(())
}

/// `$listExport(listNode, exportChildren, depth, selection)`.
fn list_export(
    list_node: &NodeRef,
    export_children: ExportChildren<'_>,
    depth: usize,
    selection: Option<&SelectionRef>,
) -> Result<String, LexicalError> {
    let mut output = Vec::new();
    let mut index = 0.0;
    for list_item_node in list_node.get_children() {
        if !is_list_item_node(&list_item_node) {
            continue;
        }
        if list_item_node.get_children_size() == 1
            && let Some(first_child) = list_item_node.get_first_child().filter(is_list_node)
        {
            let nested = list_export(&first_child, export_children, depth + 1, selection)?;
            if !nested.is_empty() {
                output.push(nested);
            }
            continue;
        }
        if let Some(selection) = selection {
            let mut any = false;
            for child in list_item_node.get_children() {
                if is_selected(&child, Some(selection))? {
                    any = true;
                    break;
                }
            }
            if !any {
                continue;
            }
        }
        let indent = " ".repeat(depth * LIST_INDENT_SIZE);
        let list_type = list_node.get_list_type().unwrap_or(ListType::Bullet);
        let list_marker = get_state(list_node, &list_marker_state())?;
        let prefix = match list_type {
            ListType::Number => format!(
                "{}. ",
                crate::json::number_to_string(list_node.get_list_start().unwrap_or(1.0) + index)
            ),
            ListType::Check => format!(
                "{list_marker} [{}] ",
                if list_item_node.get_checked() == Some(true) {
                    'x'
                } else {
                    ' '
                }
            ),
            ListType::Bullet => format!("{list_marker} "),
        };
        let mut children_text = export_children(&list_item_node)?;
        if list_type != ListType::Number {
            children_text = re(r"^(\s{0,3}\d+)(\.\s)", "").replace(&children_text, "$1\\$2");
        }
        output.push(indent + &prefix + &children_text);
        index += 1.0;
    }
    Ok(output.join("\n"))
}

fn list_transformer(list_type: ListType, reg_exp: &'static str) -> ElementTransformer {
    ElementTransformer {
        dependencies: vec!["list", "listitem"],
        export: Rc::new(|node, export_children, selection| {
            if is_list_node(node) {
                list_export(node, export_children, 0, selection).map(Some)
            } else {
                Ok(None)
            }
        }),
        reg_exp: re(
            reg_exp,
            if list_type == ListType::Check {
                "i"
            } else {
                ""
            },
        ),
        replace: Rc::new(move |parent, children, m, is_import| {
            list_replace(list_type, parent, children, m, is_import)
        }),
        trigger_on_enter: true,
    }
}

/// `CODE.replace`.
fn code_replace(
    root_node: &NodeRef,
    children: Option<&[NodeRef]>,
    start_match: &RegExpMatch,
    end_match: Option<&RegExpMatch>,
    lines_in_between: Option<Vec<String>>,
    is_import: bool,
) -> Result<bool, LexicalError> {
    let fence = match start_match.get(1).filter(|f| !f.is_empty()) {
        Some(f) => trim(f).to_owned(),
        None => "```".to_owned(),
    };
    let language = start_match.get(2).filter(|l| !l.is_empty());
    let fence_indent = start_match
        .get(1)
        .map_or(0, |f| utf16::len(f) - utf16::len(trim_start(f)));
    match (children, lines_in_between) {
        (None, Some(mut lines)) => {
            let code_block_node = create_code_node(language, None);
            let code = if lines.len() == 1 {
                if end_match.is_some() {
                    strip_fence_indent(&lines[0], fence_indent)
                } else {
                    lines[0].strip_prefix(' ').unwrap_or(&lines[0]).to_owned()
                }
            } else {
                if !lines.is_empty() {
                    if trim(&lines[0]).is_empty() {
                        lines.remove(0);
                    } else if let Some(rest) = lines[0].strip_prefix(' ') {
                        lines[0] = rest.to_owned();
                    }
                }
                while lines.last().is_some_and(String::is_empty) {
                    lines.pop();
                }
                lines
                    .iter()
                    .map(|line| strip_fence_indent(line, fence_indent))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            set_state(&code_block_node, &code_fence_state(), fence)?;
            let text_node = create_text_node(&code);
            code_block_node.append(&[text_node])?;
            root_node.append(&[code_block_node])?;
            Ok(true)
        }
        (Some(children), _) => {
            if !is_import && is_unreplaceable_block(root_node) {
                return Ok(false);
            }
            create_block_node(
                create_code_node(start_match.get(2), None),
                root_node,
                children,
                is_import,
            )
        }
        (None, None) => Ok(true),
    }
}

/// `CODE.handleImportAfterStartMatch`.
fn code_handle_import_after_start_match(
    args: &MultilineImportArgs<'_>,
) -> Result<MultilineImportResult, LexicalError> {
    let MultilineImportArgs {
        lines,
        root_node,
        start_line_index,
        start_match,
        ..
    } = *args;
    let fence = start_match.str(1);
    let fence_length = utf16::len(trim(fence));
    let current_line = &lines[start_line_index];
    let after_fence_index = start_match.index + utf16::len(fence);
    let after_fence = utf16::slice_from(current_line, after_fence_index);

    let single_line_end_regex = RegExp::of(&format!("`{{{fence_length},}}$"), "");
    if let Some(end_match) = single_line_end_regex.exec(&after_fence) {
        let after = units(&after_fence);
        let end = last_index_of(&after, &units(end_match.text())).unwrap_or(0);
        let content = string(&after[..end]);
        let mut fake_start_match = start_match.clone();
        if fake_start_match.groups.len() > 2 {
            fake_start_match.groups[2] = Some(String::new());
        }
        code_replace(
            root_node,
            None,
            &fake_start_match,
            Some(&end_match),
            Some(vec![content]),
            true,
        )?;
        return Ok(MultilineImportResult::Handled(true, start_line_index));
    }

    let multiline_end_regex = RegExp::of(&format!("^[ \\t]*`{{{fence_length},}}$"), "");
    let after_start = utf16::slice_from(current_line, start_match.match_len());
    for i in start_line_index + 1..lines.len() {
        let line = &lines[i];
        if let Some(end_match) = multiline_end_regex.exec(line) {
            let mut lines_in_between: Vec<String> = lines[start_line_index + 1..i].to_vec();
            // Only the info string's first word is the language; what follows
            // it is metadata. With no language, the remainder is content.
            let meta = if start_match.get(2).is_some_and(|l| !l.is_empty()) {
                trim(&after_start).to_owned()
            } else {
                String::new()
            };
            lines_in_between.insert(
                0,
                if meta.is_empty() {
                    after_start.clone()
                } else {
                    String::new()
                },
            );
            code_replace(
                root_node,
                None,
                start_match,
                Some(&end_match),
                Some(lines_in_between),
                true,
            )?;
            set_code_meta(root_node, &meta)?;
            return Ok(MultilineImportResult::Handled(true, i));
        }
    }

    let mut lines_in_between: Vec<String> = lines[start_line_index + 1..].to_vec();
    lines_in_between.insert(0, after_start);
    code_replace(
        root_node,
        None,
        start_match,
        None,
        Some(lines_in_between),
        true,
    )?;
    Ok(MultilineImportResult::Handled(true, lines.len() - 1))
}

/// `CODE.export`.
fn code_export(node: &NodeRef) -> Result<Option<String>, LexicalError> {
    if !is_code_node(node) {
        return Ok(None);
    }
    let text_content = node.get_text_content();
    let mut fence = get_state(node, &code_fence_state())?;
    if text_content.contains(&fence) {
        let backticks = re("`{3,}", "g").match_all(&text_content);
        if !backticks.is_empty() {
            let max_length = backticks
                .iter()
                .map(RegExpMatch::match_len)
                .max()
                .unwrap_or(0);
            fence = "`".repeat(max_length + 1);
        }
    }
    let language = node.get_language().unwrap_or_default();
    let meta = if language.is_empty() {
        String::new()
    } else {
        get_state(node, &code_meta_state())?
    };
    Ok(Some(format!(
        "{fence}{language}{}{}\n{fence}",
        if meta.is_empty() {
            String::new()
        } else {
            format!(" {meta}")
        },
        if text_content.is_empty() {
            String::new()
        } else {
            format!("\n{text_content}")
        },
    )))
}

/// What `getCodeSpanDelimiter` returns.
pub struct CodeSpanDelimiter {
    pub fence: String,
    pub padded: String,
}

/// `getCodeSpanDelimiter(content)`: a CommonMark fence and padded content
/// for an inline code span.
pub fn get_code_span_delimiter(content: &str) -> CodeSpanDelimiter {
    let mut longest_run = 0;
    let mut run = 0;
    for c in content.chars() {
        if c == '`' {
            run += 1;
            longest_run = longest_run.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat(longest_run + 1);
    let first = content.chars().next();
    let last = content.chars().last();
    let is_space = |c: Option<char>| c.is_some_and(|c| super::utils::is_space(c as u32));
    let needs_padding = content.is_empty()
        || content.contains('`')
        || (is_space(first) && is_space(last) && content.chars().any(|c| c != ' '));
    CodeSpanDelimiter {
        fence,
        padded: if needs_padding {
            format!(" {content} ")
        } else {
            content.to_owned()
        },
    }
}

/// `NOT_RAW_DESTINATION`: a URL with any of these goes in the pointy form.
const NOT_RAW_DESTINATION: &str = r"[\s\0-\x1f\x7f]";

/// `escapeLinkPart(value, syntax)`.
fn escape_link_part(value: &str, syntax: &RegExp) -> String {
    let value =
        re(r"&(?=#\d+;|#[Xx][\dA-Fa-f]+;|[A-Za-z][\dA-Za-z]*;)", "g").replace(value, "&#38;");
    let value = syntax.replace(&value, "\\$&");
    let value = value.replace('`', "\\`");
    re(r"[\r\n]", "g").replace_with(&value, |m| {
        to_character_reference(m.text().chars().next().map_or(0, |c| c as u32))
    })
}

/// `getLinkLabelOpener(text)`: the inner opener of a link label after a
/// literal prefix, `Some(-1)` for a balanced label, or `None` when
/// unmatched closers leave no recoverable opener.
pub fn get_link_label_opener(text: &str) -> Option<isize> {
    let u = units(text);
    let code_spans = scan_code_spans(&u);
    let mut code_index = 0;
    let mut brackets = u.clone();
    let mut i = 0;
    while i < u.len() {
        let c = u[i];
        let part_len = if c == u16::from(b'\\')
            && u.get(i + 1)
                .is_some_and(|n| !matches!(n, 0x0A | 0x0D | 0x2028 | 0x2029))
        {
            2
        } else if c == u16::from(b'[') || c == u16::from(b']') {
            1
        } else {
            i += 1;
            continue;
        };
        while code_index < code_spans.len() && i >= code_spans[code_index].end_index {
            code_index += 1;
        }
        let in_code = code_index < code_spans.len() && i >= code_spans[code_index].start_index;
        if in_code || c == u16::from(b'\\') {
            for b in &mut brackets[i..i + part_len] {
                *b = u16::from(b' ');
            }
        }
        i += part_len;
    }
    let mut openers: Vec<usize> = Vec::new();
    let mut unmatched_closer = false;
    for (index, c) in brackets.iter().enumerate() {
        if *c == u16::from(b'[') {
            openers.push(index);
        } else if *c == u16::from(b']') && openers.pop().is_none() {
            unmatched_closer = true;
        }
    }
    match openers.pop() {
        Some(o) => Some(o as isize),
        None if unmatched_closer => None,
        None => Some(-1),
    }
}

/// `unescapeLinkPrefix(text)`.
fn unescape_link_prefix(text: &str) -> String {
    let u = units(text);
    let mut output = String::new();
    let mut previous = 0;
    for span in scan_code_spans(&u) {
        output += &unescape_text(&string(&u[previous..span.start_index]));
        output += &string(&u[span.start_index..span.end_index]);
        previous = span.end_index;
    }
    output + &unescape_text(&string(&u[previous..]))
}

/// `LINK.export`.
fn link_export(
    node: &NodeRef,
    export_children: ExportChildren<'_>,
) -> Result<Option<String>, LexicalError> {
    if !is_link_node(node) || is_auto_link_node(node) {
        return Ok(None);
    }
    let text_content = export_children(node)?;
    let title = node.get_title();
    let raw_url = node.get_url().unwrap_or_default();
    let url = if raw_url.is_empty() || re(NOT_RAW_DESTINATION, "").test(&raw_url) {
        format!("<{}>", escape_link_part(&raw_url, &re(r"[\\<>]", "g")))
    } else {
        escape_link_part(&raw_url, &re(r"[\\()]|^<", "g"))
    };
    Ok(Some(match title.filter(|t| !t.is_empty()) {
        Some(title) => format!(
            "[{text_content}]({url} \"{}\")",
            escape_link_part(&title, &re(r#"[\\"]"#, "g"))
        ),
        None => format!("[{text_content}]({url})"),
    }))
}

/// `LINK.replace`.
fn link_replace(text_node: &NodeRef, m: &RegExpMatch) -> Result<Option<NodeRef>, LexicalError> {
    let source = units(&m.input);
    let start_index = m.index;
    if find_matching_parent(text_node, is_link_node).is_some()
        || is_escaped(&source, start_index)
        || is_escaped(&source, start_index + utf16::len(m.str(1)) + 1)
    {
        return Ok(None);
    }
    let link_text = m.str(1);
    // At most one destination shape matched, and either may be empty.
    let link_url = unescape_text(m.get(2).or(m.get(3)).unwrap_or(""));
    let raw_link_title = m.get(4).or(m.get(5)).or(m.get(6));
    let link_title = raw_link_title.map(unescape_text);
    let link_node = create_link_node(
        &link_url,
        &LinkAttributes {
            title: Some(link_title),
            ..LinkAttributes::default()
        },
    );
    let Some(open_bracket_index) = get_link_label_opener(link_text) else {
        return Ok(None);
    };
    let mut parsed_link_text = link_text.to_owned();
    let mut outside_link_text = String::new();
    if open_bracket_index != -1 {
        let o = open_bracket_index as usize;
        outside_link_text = "[".to_owned() + &unescape_link_prefix(&utf16::slice(link_text, 0, o));
        parsed_link_text = utf16::slice_from(link_text, o + 1);
    }
    // The text left outside the link carries the replaced node's format too.
    let format = text_format(text_node);
    let link_text_node = create_text_node(&parsed_link_text);
    link_text_node.set_format(format);
    link_node.append(std::slice::from_ref(&link_text_node))?;
    text_node.replace(&link_node, false)?;
    if !outside_link_text.is_empty() {
        let outside = create_text_node(&outside_link_text);
        outside.set_format(format);
        link_node.insert_before(&outside, true)?;
    }
    Ok(Some(link_text_node))
}

/// `LINK.importRegExp`'s source.
pub(crate) const LINK_IMPORT_REG_EXP: &str = r#"(?:\[(.+?)\])(?:\(\s*(?:(?:<((?:\\[^\n\r]|[^<>\n\r\\])*)>|((?!<)(?:\\[^\s]|\\(?=\s)|\((?:\\[^\s]|[^\s()\\]|\((?:\\[^\s]|[^\s()\\]|\((?:\\[^\s]|[^\s()\\])*\))*\))*\)|[^\s()\\])+))(?:\s+(?:"((?:\\[\s\S]|[^"\\])*)"|'((?:\\[\s\S]|[^'\\])*)'|\(((?:\\[\s\S]|[^()\\])*)\)))?\s*)?\))"#;
const LINK_REG_EXP: &str = r#"(?:\[([^[\]]*(?:\[[^[\]]*\][^[\]]*)*)\])(?:\(\s*(?:(?:<((?:\\[^\n\r]|[^<>\n\r\\])*)>|((?!<)(?:\\[^\s]|\\(?=\s)|\((?:\\[^\s]|[^\s()\\]|\((?:\\[^\s]|[^\s()\\]|\((?:\\[^\s]|[^\s()\\])*\))*\))*\)|[^\s()\\])+))(?:\s+(?:"((?:\\[\s\S]|[^"\\])*)"|'((?:\\[\s\S]|[^'\\])*)'|\(((?:\\[\s\S]|[^()\\])*)\)))?\s*)?\))$"#;

fn format_transformer(
    format: &[&'static str],
    tag: &str,
    intraword: Option<bool>,
) -> Rc<TextFormatTransformer> {
    Rc::new(TextFormatTransformer {
        format: format.to_vec(),
        tag: tag.to_owned(),
        intraword,
    })
}

struct Builtins {
    heading: Rc<ElementTransformer>,
    quote: Rc<ElementTransformer>,
    code: Rc<MultilineElementTransformer>,
    unordered_list: Rc<ElementTransformer>,
    check_list: Rc<ElementTransformer>,
    ordered_list: Rc<ElementTransformer>,
    inline_code: Rc<TextFormatTransformer>,
    highlight: Rc<TextFormatTransformer>,
    bold_italic_star: Rc<TextFormatTransformer>,
    bold_italic_underscore: Rc<TextFormatTransformer>,
    bold_star: Rc<TextFormatTransformer>,
    bold_underscore: Rc<TextFormatTransformer>,
    strikethrough: Rc<TextFormatTransformer>,
    italic_star: Rc<TextFormatTransformer>,
    italic_underscore: Rc<TextFormatTransformer>,
    link: Rc<TextMatchTransformer>,
}

fn builtins() -> Builtins {
    Builtins {
        heading: Rc::new(ElementTransformer {
            dependencies: vec!["heading"],
            export: Rc::new(|node, export_children, _| {
                if !is_heading_node(node) {
                    return Ok(None);
                }
                let level = node
                    .get_heading_tag()
                    .map_or(1, |t| t.as_str()[1..].parse::<usize>().unwrap_or(1));
                Ok(Some("#".repeat(level) + " " + &export_children(node)?))
            }),
            reg_exp: re(HEADING_REGEX, ""),
            replace: Rc::new(|parent, children, m, is_import| {
                if !is_import && is_unreplaceable_block(parent) {
                    return Ok(false);
                }
                let tag = HeadingTagType::parse(&format!("h{}", utf16::len(m.str(1))))
                    .expect("one to six #");
                create_block_node(create_heading_node(tag), parent, children, is_import)
            }),
            trigger_on_enter: true,
        }),
        quote: Rc::new(ElementTransformer {
            dependencies: vec!["quote"],
            export: Rc::new(|node, export_children, _| {
                if !is_quote_node(node) {
                    return Ok(None);
                }
                let text = export_children(node)?;
                Ok(Some(
                    text.split('\n')
                        .map(|line| format!("> {line}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ))
            }),
            reg_exp: re(QUOTE_REGEX, ""),
            replace: Rc::new(|parent, children, _m, is_import| {
                if is_import
                    && let Some(previous) = parent.get_previous_sibling().filter(is_quote_node)
                {
                    let mut nodes = vec![create_markdown_line_break_node(&previous)?];
                    nodes.extend_from_slice(children);
                    previous.splice(previous.get_children_size(), 0, &nodes)?;
                    parent.remove(false)?;
                    return Ok(true);
                }
                create_block_node(create_quote_node(false), parent, children, is_import)
            }),
            trigger_on_enter: true,
        }),
        code: Rc::new(MultilineElementTransformer {
            handle_import_after_start_match: Some(Rc::new(code_handle_import_after_start_match)),
            dependencies: vec!["code"],
            export: Some(Rc::new(|node, _, _| code_export(node))),
            reg_exp_start: re(CODE_START_REGEX, ""),
            reg_exp_end: Some(RegExpEnd {
                optional: true,
                reg_exp: re(CODE_END_REGEX, ""),
            }),
            replace: Rc::new(code_replace),
        }),
        unordered_list: Rc::new(list_transformer(ListType::Bullet, UNORDERED_LIST_REGEX)),
        check_list: Rc::new(list_transformer(ListType::Check, CHECK_LIST_REGEX)),
        ordered_list: Rc::new(list_transformer(ListType::Number, ORDERED_LIST_REGEX)),
        inline_code: format_transformer(&["code"], "`", None),
        highlight: format_transformer(&["highlight"], "==", None),
        bold_italic_star: format_transformer(&["bold", "italic"], "***", None),
        bold_italic_underscore: format_transformer(&["bold", "italic"], "___", Some(false)),
        bold_star: format_transformer(&["bold"], "**", None),
        bold_underscore: format_transformer(&["bold"], "__", Some(false)),
        strikethrough: format_transformer(&["strikethrough"], "~~", None),
        italic_star: format_transformer(&["italic"], "*", None),
        italic_underscore: format_transformer(&["italic"], "_", Some(false)),
        link: Rc::new(TextMatchTransformer {
            dependencies: vec!["link"],
            export: Some(Rc::new(|node, export_children, _| {
                link_export(node, export_children)
            })),
            import_reg_exp: Some(re(LINK_IMPORT_REG_EXP, "")),
            reg_exp: re(LINK_REG_EXP, ""),
            replace: Some(Rc::new(link_replace)),
            get_end_index: None,
            trigger: Some(")".to_owned()),
        }),
    }
}

thread_local! {
    static BUILTINS: Builtins = builtins();
}

macro_rules! builtin {
    ($(#[$doc:meta])* $name:ident: $ty:ty) => {
        $(#[$doc])*
        pub fn $name() -> Rc<$ty> {
            BUILTINS.with(|b| b.$name.clone())
        }
    };
}

builtin!(
    /// `HEADING`: `# ` to `###### ` make a heading.
    heading: ElementTransformer
);
builtin!(
    /// `QUOTE`: `> ` makes a quote.
    quote: ElementTransformer
);
builtin!(
    /// `CODE`: a fenced code block.
    code: MultilineElementTransformer
);
builtin!(
    /// `UNORDERED_LIST`: `- `, `* ` or `+ `.
    unordered_list: ElementTransformer
);
builtin!(
    /// `CHECK_LIST`: `- [ ] ` or `- [x] `. Not in `TRANSFORMERS`.
    check_list: ElementTransformer
);
builtin!(
    /// `ORDERED_LIST`: `1. `.
    ordered_list: ElementTransformer
);
builtin!(
    /// `INLINE_CODE`: `` `code` ``.
    inline_code: TextFormatTransformer
);
builtin!(
    /// `HIGHLIGHT`: `==highlight==`.
    highlight: TextFormatTransformer
);
builtin!(
    /// `BOLD_ITALIC_STAR`: `***bold italic***`.
    bold_italic_star: TextFormatTransformer
);
builtin!(
    /// `BOLD_ITALIC_UNDERSCORE`: `___bold italic___`.
    bold_italic_underscore: TextFormatTransformer
);
builtin!(
    /// `BOLD_STAR`: `**bold**`.
    bold_star: TextFormatTransformer
);
builtin!(
    /// `BOLD_UNDERSCORE`: `__bold__`.
    bold_underscore: TextFormatTransformer
);
builtin!(
    /// `STRIKETHROUGH`: `~~strikethrough~~`.
    strikethrough: TextFormatTransformer
);
builtin!(
    /// `ITALIC_STAR`: `*italic*`.
    italic_star: TextFormatTransformer
);
builtin!(
    /// `ITALIC_UNDERSCORE`: `_italic_`.
    italic_underscore: TextFormatTransformer
);
builtin!(
    /// `LINK`: `[text](url "title")`.
    link: TextMatchTransformer
);

/// `ELEMENT_TRANSFORMERS`.
pub fn element_transformers() -> Vec<Rc<ElementTransformer>> {
    vec![heading(), quote(), unordered_list(), ordered_list()]
}

/// `MULTILINE_ELEMENT_TRANSFORMERS`.
pub fn multiline_element_transformers() -> Vec<Rc<MultilineElementTransformer>> {
    vec![code()]
}

/// `TEXT_FORMAT_TRANSFORMERS`: code first, as it prevents any formats
/// inside, then longer tags before shorter ones.
pub fn text_format_transformers() -> Vec<Rc<TextFormatTransformer>> {
    vec![
        inline_code(),
        bold_italic_star(),
        bold_italic_underscore(),
        bold_star(),
        bold_underscore(),
        highlight(),
        italic_star(),
        italic_underscore(),
        strikethrough(),
    ]
}

/// `TEXT_MATCH_TRANSFORMERS`.
pub fn text_match_transformers() -> Vec<Rc<TextMatchTransformer>> {
    vec![link()]
}

/// `TRANSFORMERS`: every transformer above but `CHECK_LIST`.
pub fn transformers() -> Vec<Transformer> {
    let mut all: Vec<Transformer> = element_transformers().into_iter().map(Into::into).collect();
    all.extend(multiline_element_transformers().into_iter().map(Into::into));
    all.extend(text_format_transformers().into_iter().map(Into::into));
    all.extend(text_match_transformers().into_iter().map(Into::into));
    all
}

/// `normalizeMarkdown(input, shouldMergeAdjacentLines)`.
pub fn normalize_markdown(input: &str, should_merge_adjacent_lines: bool) -> String {
    let lines: Vec<&str> = input.split('\n').collect();
    let mut code_block_fence_length = 0;
    let mut sanitized_lines: Vec<String> = Vec::new();
    let test = |source: &'static str, flags: &'static str, text: &str| re(source, flags).test(text);

    for (i, raw_line) in lines.iter().enumerate() {
        let raw_line = *raw_line;
        let line = super::utils::trim_end(raw_line);
        let last_line = sanitized_lines.last().cloned();
        let hard_line_break = if i < lines.len() - 1 {
            parse_markdown_hard_line_break(raw_line)
        } else {
            None
        };
        let last_line_has_hard_line_break = last_line
            .as_deref()
            .is_some_and(|l| parse_markdown_hard_line_break(l).is_some());

        // Code blocks of ```single line``` don't toggle the code block.
        if test(CODE_SINGLE_LINE_REGEX, "", line) {
            sanitized_lines.push(line.to_owned());
            continue;
        }
        if code_block_fence_length == 0 {
            if let Some(open_match) = re(CODE_START_REGEX, "").exec(line) {
                code_block_fence_length = utf16::len(trim(open_match.str(1)));
                sanitized_lines.push(line.to_owned());
                continue;
            }
        } else {
            // Only a bare fence at least as long as the opening one closes.
            if test(CODE_END_REGEX, "", line) && utf16::len(trim(line)) >= code_block_fence_length {
                code_block_fence_length = 0;
                sanitized_lines.push(line.to_owned());
                continue;
            }
            sanitized_lines.push(raw_line.to_owned());
            continue;
        }

        let last = last_line.as_deref().unwrap_or("");
        if line.is_empty()
            || last_line.as_deref().is_none_or(str::is_empty)
            || test(HEADING_REGEX, "", last)
            || test(HEADING_REGEX, "", line)
            || test(QUOTE_REGEX, "", line)
            || test(ORDERED_LIST_REGEX, "", line)
            || test(UNORDERED_LIST_REGEX, "", line)
            || test(CHECK_LIST_REGEX, "i", line)
            || test(TABLE_ROW_REG_EXP, "", line)
            || is_table_row_divider(line)
            || last_line_has_hard_line_break
            || !should_merge_adjacent_lines
            || test(TAG_START_REGEX, "i", line)
            || test(TAG_END_REGEX, "i", line)
            || ends_with(TAG_END_REGEX, "i").test(last)
            || ends_with(TAG_START_REGEX, "i").test(last)
            || test(CODE_END_REGEX, "", last)
        {
            // When not merging, keep trailing whitespace (hard line break
            // markers); whitespace-only lines still collapse to ''.
            sanitized_lines.push(
                if (!should_merge_adjacent_lines && !line.is_empty()) || hard_line_break.is_some() {
                    raw_line.to_owned()
                } else {
                    line.to_owned()
                },
            );
        } else {
            let merged = last.to_owned()
                + " "
                + trim_start(if hard_line_break.is_none() {
                    line
                } else {
                    raw_line
                });
            *sanitized_lines.last_mut().expect("a last line") = merged;
        }
    }
    sanitized_lines.join("\n")
}
