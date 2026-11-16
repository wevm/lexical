//! Ports of `packages/lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts`.
//!
//! Converting or normalizing blocks keeps the ids of every character: no
//! text is inserted or deleted, so the document read as a sequence of
//! (character, ids) pairs is the same before and after, whichever block
//! types are involved, wherever the comments are and wherever the selection
//! is.
//!
//! The `highlighted-code` source and target highlight the code with
//! `code_highlighter_extension` in place of `CodePrismExtension`, over
//! [`PrismLikeTokenizer`]: Prism's grammars aren't ported (rust/PARITY.md),
//! and for the text these tests highlight, Prism's JavaScript grammar gives
//! plain text and a `constant` token for an uppercase word (a typed `X`), as
//! Swift's port does.
//!
//! Each upstream `test.each` case (a source and a target) is a Rust test,
//! generated from the upstream lists.

use crate::code::{
    CodeHighlighterConfigOverrides, CodeLanguageState, CodeToken, DEFAULT_CODE_LANGUAGE, Tokenizer,
    TokenizerRef, code_extension, code_highlighter_extension, create_code_node,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{LinkAttributes, create_link_node, link_extension};
use crate::list::{
    ListType, create_list_item_node, create_list_node, insert_list, is_list_item_node,
    is_list_node, list_extension, remove_list,
};
use crate::mark::{add_mark_id, get_node_mark_ids, mark_state_extension};
use crate::rich_text::{HeadingTagType, create_heading_node, create_quote_node};
use crate::selection::set_blocks_type;
use crate::{
    CONTROLLED_TEXT_INSERTION_COMMAND, Editor, InitialEditorStateType, LexicalError,
    LexicalExtension, NodeKey, NodeRef, PointType, SelectionRef, UpdateOptions, Which,
    config_extension, create_paragraph_node, create_range_selection, create_text_node,
    define_extension, get_node_by_key, get_root, get_selection, set_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    Paragraph,
    Heading,
    Quote,
    Code,
    HighlightedCode,
    Bullet,
    Number,
    Check,
    NestedBullet,
    NestedNumber,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Paragraph,
    Heading,
    Quote,
    Code,
    HighlightedCode,
    Bullet,
    Number,
    Check,
    NoList,
}

/// The converted text; `beta` is a link in the `link` content.
const TEXT: &str = "alpha beta gamma ";
const TEXT_LENGTH: usize = TEXT.len();

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Content {
    Plain,
    Link,
}
const CONTENTS: [Content; 2] = [Content::Plain, Content::Link];

/// Comments as [id, start, end) offsets in TEXT, in upstream key order.
/// `[id, start, end)` of each comment of a marks case.
type Comments = &'static [(&'static str, usize, usize)];

const MARKS: [(&str, Comments); 4] = [
    ("all", &[("c1", 0, TEXT_LENGTH)]),
    ("overlapping", &[("c1", 3, 8), ("c2", 7, 14)]),
    ("part", &[("c1", 6, 10)]),
    ("partial", &[("c1", 7, 9)]),
];

/// - `Alone`: nothing else in the document.
/// - `LinedUp`: a list before it, of the target's type, whose only item is
///   `gamma ` (the end of the converted text): a list conversion merges
///   with it.
/// - `MarkedBefore`: that list's item is `old gamma ` with `c4`, and the
///   last character of the converted text has `c4` too.
/// - `MarkedAfter`: a list of the target's type after it, with `c5` on its
///   text, and `c5` on the first character of the converted text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Context {
    Alone,
    LinedUp,
    MarkedBefore,
    MarkedAfter,
}
const CONTEXTS: [Context; 4] = [
    Context::Alone,
    Context::LinedUp,
    Context::MarkedBefore,
    Context::MarkedAfter,
];

/// Collapsed selections at offsets in TEXT, a range over it, and a range
/// from it into the neighbour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SelectionAt {
    At(usize),
    Range,
    Across,
}
const SELECTIONS: [SelectionAt; 6] = [
    SelectionAt::At(0),
    SelectionAt::At(8),
    SelectionAt::At(16),
    SelectionAt::At(17),
    SelectionAt::Range,
    SelectionAt::Across,
];

/// What Prism's JavaScript grammar makes of the text these tests highlight:
/// plain text, but a `constant` token for a word of capitals
/// (`/\b[A-Z](?:[A-Z_]|\dx?)*\b/`).
struct PrismLikeTokenizer;

impl Tokenizer for PrismLikeTokenizer {
    fn default_language(&self) -> Option<&str> {
        Some(DEFAULT_CODE_LANGUAGE)
    }
    fn language_state(&self, language: &str) -> CodeLanguageState {
        if language == DEFAULT_CODE_LANGUAGE {
            CodeLanguageState::Loaded
        } else {
            CodeLanguageState::Unsupported
        }
    }
    fn tokenize(&self, code: &str, _language: &str) -> Vec<CodeToken> {
        let mut tokens = Vec::new();
        let mut plain = String::new();
        let mut rest = code;
        while let Some(first) = rest.chars().next() {
            let word_len = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let (run, is_constant) = if word_len > 0 {
                let word = &rest[..word_len];
                (
                    word,
                    first.is_ascii_uppercase()
                        && word
                            .chars()
                            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()),
                )
            } else {
                (&rest[..first.len_utf8()], false)
            };
            if is_constant {
                if !plain.is_empty() {
                    tokens.push(CodeToken::Text(std::mem::take(&mut plain)));
                }
                tokens.push(CodeToken::token("constant", run));
            } else {
                plain.push_str(run);
            }
            rest = &rest[run.len()..];
        }
        if !plain.is_empty() {
            tokens.push(CodeToken::Text(plain));
        }
        tokens
    }
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

fn read<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read(f).unwrap()
}

fn build_editor(highlighting: bool) -> LexicalEditorWithDispose {
    let code = if highlighting {
        config_extension(
            &code_highlighter_extension(),
            [CodeHighlighterConfigOverrides {
                tokenizer: Some(TokenizerRef(Rc::new(PrismLikeTokenizer))),
                ..CodeHighlighterConfigOverrides::default()
            }],
        )
    } else {
        code_extension().into()
    };
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            list_extension().into(),
            link_extension().into(),
            code,
            mark_state_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("block-conversion", ())
    })
    .into()])
    .unwrap()
}

/// `$sequence()`: the document as `character:ids` for every character, in
/// order.
fn sequence() -> Vec<String> {
    sequence_of(&get_root())
}

fn sequence_of(node: &NodeRef) -> Vec<String> {
    if node.is_text() {
        let ids = get_node_mark_ids(node).unwrap().join(",");
        return node
            .get_text_content()
            .chars()
            .map(|c| format!("{c}:{ids}"))
            .collect();
    }
    if node.is_element() {
        return node.get_children().iter().flat_map(sequence_of).collect();
    }
    Vec::new()
}

fn is_code_source(source: Source) -> bool {
    matches!(source, Source::Code | Source::HighlightedCode)
}

fn list_type_of(target: Target) -> ListType {
    match target {
        Target::Number => ListType::Number,
        Target::Check => ListType::Check,
        _ => ListType::Bullet,
    }
}

fn link(url: &str) -> NodeRef {
    create_link_node(url, &LinkAttributes::default())
}

fn with_children(node: NodeRef, children: &[NodeRef]) -> NodeRef {
    node.append(children).unwrap();
    node
}

fn content(content: Content) -> Vec<NodeRef> {
    match content {
        Content::Link => vec![
            create_text_node("alpha "),
            with_children(link("https://example.org"), &[create_text_node("beta")]),
            create_text_node(" gamma "),
        ],
        Content::Plain => vec![create_text_node(TEXT)],
    }
}

/// `$appendSource(source, children)`: appends the source block to the root,
/// holding `children`, and returns the block that holds them.
fn append_source(source: Source, children: &[NodeRef]) -> NodeRef {
    let root = get_root();
    match source {
        Source::Paragraph
        | Source::Heading
        | Source::Quote
        | Source::Code
        | Source::HighlightedCode => {
            let block = match source {
                Source::Paragraph => create_paragraph_node(),
                Source::Heading => create_heading_node(HeadingTagType::H1),
                Source::Quote => create_quote_node(false),
                _ => create_code_node(None, None),
            };
            root.append(&[with_children(block.clone(), children)])
                .unwrap();
            block
        }
        Source::Bullet | Source::Number | Source::Check => {
            let list_type = match source {
                Source::Bullet => ListType::Bullet,
                Source::Number => ListType::Number,
                _ => ListType::Check,
            };
            let item = with_children(create_list_item_node(None), children);
            root.append(&[with_children(
                create_list_node(list_type, 1.0),
                std::slice::from_ref(&item),
            )])
            .unwrap();
            item
        }
        Source::NestedBullet | Source::NestedNumber => {
            let list_type = if source == Source::NestedBullet {
                ListType::Bullet
            } else {
                ListType::Number
            };
            let item = with_children(create_list_item_node(None), children);
            root.append(&[with_children(
                create_list_node(list_type, 1.0),
                &[
                    with_children(create_list_item_node(None), &[create_text_node("outer")]),
                    with_children(
                        create_list_item_node(None),
                        &[with_children(
                            create_list_node(list_type, 1.0),
                            std::slice::from_ref(&item),
                        )],
                    ),
                ],
            )])
            .unwrap();
            item
        }
    }
}

/// `$locate(block, offset)`: the TextNode and offset at `offset` in the
/// text of `block`.
fn locate(block: &NodeRef, offset: usize) -> (NodeRef, usize) {
    let mut remaining = offset;
    let texts = block.get_all_text_nodes();
    for text in &texts {
        let size = text.get_text_content_size();
        if remaining < size {
            return (text.clone(), remaining);
        }
        remaining -= size;
    }
    let last = texts.last().unwrap().clone();
    let size = last.get_text_content_size();
    (last, size)
}

/// `$select([anchorBlock, anchorOffset], [focusBlock, focusOffset])`.
fn select(anchor: (&NodeRef, usize), focus: (&NodeRef, usize)) -> SelectionRef {
    let selection = create_range_selection();
    let (anchor_node, anchor_at) = locate(anchor.0, anchor.1);
    let (focus_node, focus_at) = locate(focus.0, focus.1);
    selection
        .point_set(
            Which::Anchor,
            anchor_node.get_key(),
            anchor_at,
            PointType::Text,
        )
        .unwrap();
    selection
        .point_set(
            Which::Focus,
            focus_node.get_key(),
            focus_at,
            PointType::Text,
        )
        .unwrap();
    set_selection(Some(selection.clone()));
    selection
}

/// `$mark(block, id, start, end)`.
fn mark(block: &NodeRef, id: &str, start: usize, end: usize) {
    add_mark_id(id, &select((block, start), (block, end))).unwrap();
}

/// `$convert(target)`.
fn convert(target: Target) -> Result<(), LexicalError> {
    let set_blocks =
        |create: fn() -> NodeRef| set_blocks_type(get_selection().as_ref(), create, None);
    match target {
        Target::Paragraph => set_blocks(create_paragraph_node),
        Target::Heading => set_blocks(|| create_heading_node(HeadingTagType::H2)),
        Target::Quote => set_blocks(|| create_quote_node(false)),
        Target::Code | Target::HighlightedCode => set_blocks(|| create_code_node(None, None)),
        Target::Bullet | Target::Number | Target::Check => insert_list(list_type_of(target)),
        Target::NoList => remove_list(),
    }
}

/// `$getBlockOf(node)`: the nearest block of `node`, or `node` itself.
fn get_block_of(node: &NodeRef) -> NodeRef {
    let mut current = node.clone();
    while !(current.is_element() && !current.is_inline()) {
        current = current.get_parent_or_throw().unwrap();
    }
    current
}

/// `$label(block)`: the type of a block, with the list type for a list
/// item.
fn label(block: &NodeRef) -> String {
    match block.get_parent() {
        Some(parent) if is_list_item_node(block) && is_list_node(&parent) => {
            format!("listitem:{}", parent.get_list_type().unwrap().as_str())
        }
        _ => block.get_type().to_owned(),
    }
}

/// `expectedLabel(from, target)`: the label of the block that holds the
/// converted text after `target`, for text in a block labeled `from`.
fn expected_label(from: &str, target: Target) -> String {
    match target {
        Target::Bullet | Target::Number | Target::Check => {
            format!("listitem:{}", list_type_of(target).as_str())
        }
        Target::NoList => {
            if from.starts_with("listitem:") {
                "paragraph".to_owned()
            } else {
                from.to_owned()
            }
        }
        Target::HighlightedCode => "code".to_owned(),
        Target::Paragraph => "paragraph".to_owned(),
        Target::Heading => "heading".to_owned(),
        Target::Quote => "quote".to_owned(),
        Target::Code => "code".to_owned(),
    }
}

/// `$findTextBlock(text, partial)`: the block whose own text is `text`, or
/// includes it with `partial`.
fn find_text_block(text: &str, partial: bool) -> Option<NodeRef> {
    let mut texts: Vec<(NodeRef, String)> = Vec::new();
    for node in get_root().get_all_text_nodes() {
        let block = get_block_of(&node);
        match texts
            .iter_mut()
            .find(|(b, _)| b.get_key() == block.get_key())
        {
            Some((_, content)) => content.push_str(&node.get_text_content()),
            None => texts.push((block, node.get_text_content())),
        }
    }
    texts
        .into_iter()
        .find(|(_, content)| {
            if partial {
                content.contains(text)
            } else {
                content == text
            }
        })
        .map(|(block, _)| block)
}

/// `$type()`.
fn type_x() -> Result<(), LexicalError> {
    let selection = get_selection().unwrap();
    assert!(selection.range().is_some_and(|r| r.is_collapsed()));
    selection.insert_text("X")
}

#[derive(Clone, Copy, Debug)]
struct Case {
    content: Content,
    context: Context,
    marks: usize,
    selection: SelectionAt,
    /// Type `X` at the caret in the update that converts.
    typed: bool,
}

/// `cases()`.
fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for content in CONTENTS {
        for marks in 0..MARKS.len() {
            for context in CONTEXTS {
                for selection in SELECTIONS {
                    if selection == SelectionAt::Across && context == Context::Alone {
                        continue;
                    }
                    let case = Case {
                        content,
                        context,
                        marks,
                        selection,
                        typed: false,
                    };
                    out.push(case);
                    if !matches!(selection, SelectionAt::Range | SelectionAt::Across) {
                        out.push(Case {
                            typed: true,
                            ..case
                        });
                    }
                }
            }
        }
    }
    out
}

/// What `build` returns: sets the selection that the case converts.
type Built = Rc<dyn Fn()>;

/// `build(editor, target, c, $appendBlock)`: the document for `c`, with the
/// converted block made by `append_block`. The comments are added in an
/// update of their own, after code highlighting has made its tokens.
fn build(
    editor: &Editor,
    target: Target,
    c: Case,
    append_block: impl FnOnce() -> NodeRef + 'static,
) -> Built {
    let keys: Rc<RefCell<(NodeKey, NodeKey)>> = Rc::default();
    let k = keys.clone();
    update(editor, move || {
        let root = get_root();
        root.clear()?;
        let neighbour = create_list_item_node(None);
        if matches!(c.context, Context::LinedUp | Context::MarkedBefore) {
            neighbour.append(&[create_text_node(if c.context == Context::LinedUp {
                "gamma "
            } else {
                "old gamma "
            })])?;
            root.append(&[with_children(
                create_list_node(list_type_of(target), 1.0),
                std::slice::from_ref(&neighbour),
            )])?;
        }
        let block = append_block();
        if c.context == Context::MarkedAfter {
            neighbour.append(&[create_text_node("delta")])?;
            root.append(&[with_children(
                create_list_node(list_type_of(target), 1.0),
                std::slice::from_ref(&neighbour),
            )])?;
        }
        *k.borrow_mut() = (block.get_key().to_owned(), neighbour.get_key().to_owned());
        Ok(())
    });
    let k = keys.clone();
    update(editor, move || {
        let (block_key, neighbour_key) = k.borrow().clone();
        let block = get_node_by_key(&block_key).unwrap();
        assert!(block.is_element());
        for (id, start, end) in MARKS[c.marks].1 {
            mark(&block, id, *start, *end);
        }
        if matches!(c.context, Context::MarkedBefore | Context::MarkedAfter) {
            let neighbour = get_node_by_key(&neighbour_key).unwrap();
            assert!(neighbour.is_element());
            if c.context == Context::MarkedBefore {
                mark(&neighbour, "c4", 0, "old gamma ".len());
                mark(&block, "c4", TEXT_LENGTH - 1, TEXT_LENGTH);
            } else {
                mark(&neighbour, "c5", 0, "delta".len());
                mark(&block, "c5", 0, 1);
            }
        }
        Ok(())
    });
    Rc::new(move || {
        let (block_key, neighbour_key) = keys.borrow().clone();
        let at = get_node_by_key(&block_key).unwrap();
        assert!(at.is_element());
        match c.selection {
            SelectionAt::Range => {
                select((&at, 0), (&at, TEXT_LENGTH));
            }
            SelectionAt::Across => {
                let other = get_node_by_key(&neighbour_key).unwrap();
                assert!(other.is_element());
                if c.context == Context::MarkedAfter {
                    select((&at, 3), (&other, 2));
                } else {
                    select((&other, 2), (&at, 3));
                }
            }
            SelectionAt::At(offset) => {
                select((&at, offset), (&at, offset));
            }
        }
    })
}

fn describe_case(source: Source, target: Target, c: &Case) -> String {
    format!(
        "{source:?} -> {target:?}, {:?}, marks {}, {:?}, selection {:?}{}",
        c.content,
        MARKS[c.marks].0,
        c.context,
        c.selection,
        if c.typed { ", typed" } else { "" }
    )
}

/// `keepsIDs(before, after, inHighlightedCode)`: whether the characters of
/// `after` are those of `before` with the same ids. A code highlighting
/// token is not split where its ids change, and gets every id its text had,
/// so in highlighted code each character only has to keep every id it had.
fn keeps_ids(before: &[String], after: &[String], in_highlighted_code: bool) -> bool {
    if !in_highlighted_code {
        return after == before;
    }
    let split = |unit: &str| {
        let at = unit.find(':').unwrap();
        (unit[..at].to_owned(), unit[at + 1..].to_owned())
    };
    after.len() == before.len()
        && after.iter().zip(before).all(|(unit, prev)| {
            let (c, ids) = split(unit);
            let (prev_c, prev_ids) = split(prev);
            let ids: Vec<&str> = ids.split(',').collect();
            c == prev_c && (prev_ids.is_empty() || prev_ids.split(',').all(|id| ids.contains(&id)))
        })
}

/// The `from %s` > `to %s` test.
fn from_source_to_target(source: Source, target: Target) {
    let highlighting = source == Source::HighlightedCode || target == Target::HighlightedCode;
    let editor = build_editor(highlighting);
    let mut failures: Vec<String> = Vec::new();
    for c in cases() {
        if is_code_source(source) && c.content == Content::Link {
            continue;
        }
        if highlighting
            && matches!(c.context, Context::MarkedBefore | Context::MarkedAfter)
            && (is_code_source(source) || c.selection == SelectionAt::Across)
        {
            // Comment ids on part of a code highlighting token, and text that a
            // range conversion merges into a code block, are re-tokenized
            // without them (#9491)
            continue;
        }
        if c.typed && highlighting && c.content == Content::Link {
            // Highlighting that unwraps the link puts the caret back at a wrong
            // offset (counted from the start of the code), so the typed text
            // is not right before the caret, as the inserted-text rule needs
            continue;
        }
        let built = build(&editor, target, c, move || {
            append_source(source, &content(c.content))
        });
        let before = read(&editor, sequence);
        let from = read(&editor, || {
            label(&find_text_block(TEXT, false).unwrap_or_else(get_root))
        });
        update(&editor, move || {
            built();
            convert(target)?;
            if c.typed {
                type_x()?;
            }
            Ok(())
        });
        let (after, found) = read(&editor, || {
            let block = find_text_block("alpha", true);
            (
                sequence(),
                block.map_or_else(|| "missing".to_owned(), |b| label(&b)),
            )
        });
        let mut problems = Vec::new();
        if found != expected_label(&from, target) {
            problems.push(format!("  converted {from} to {found}"));
        }
        // The typed text gets the ids of the inserted-text rule where it can
        // compare the changed blocks with the previous state, and otherwise
        // those of the TextNode it lands in, so only the other characters are
        // checked
        let kept: Vec<String> = if c.typed {
            after
                .iter()
                .filter(|unit| !unit.starts_with("X:"))
                .cloned()
                .collect()
        } else {
            after.clone()
        };
        if c.typed && kept.len() != after.len() - 1 {
            problems.push("  X was not typed once".to_owned());
        }
        if !keeps_ids(
            &before,
            &kept,
            highlighting && expected_label(&from, target) == "code",
        ) {
            problems.push(format!("  before {}", before.join(" ")));
        }
        if !problems.is_empty() {
            failures.push(format!(
                "{}:\n{}\n  after  {}",
                describe_case(source, target, &c),
                problems.join("\n"),
                after.join(" ")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The `pasted %s` > `to %s` test: the source block is copied with its
/// comments, pasted into an empty document, and the pasted blocks are
/// converted: the pasted text keeps its ids through both.
fn pasted_source_to_target(source: Source, target: Target) {
    let highlighting = source == Source::HighlightedCode || target == Target::HighlightedCode;
    let from = build_editor(highlighting);
    let mut failures: Vec<String> = Vec::new();
    for c in cases() {
        if c.context != Context::Alone
            || c.typed
            || (is_code_source(source) && c.content == Content::Link)
        {
            continue;
        }
        // A fresh editor: pasting into the document of the previous case in
        // the update that clears it would compare the pasted text with that
        // document's equal text
        let editor = build_editor(highlighting);
        build(&from, target, c, move || {
            append_source(source, &content(c.content))
        });
        let json: Rc<RefCell<Vec<crate::json::Value>>> = Rc::default();
        let (j, from_editor) = (json.clone(), (*from).clone());
        update(&from, move || {
            let root = get_root();
            let selection = root.select(Some(0), Some(root.get_children_size()))?;
            *j.borrow_mut() = crate::clipboard::generate_json_from_selected_nodes(
                &from_editor,
                Some(&selection),
            )?
            .nodes
            .into_iter()
            .map(crate::json::Value::Object)
            .collect();
            Ok(())
        });
        let copied = read(&from, sequence);
        let to_editor = (*editor).clone();
        update(&editor, move || {
            let paragraph = create_paragraph_node();
            get_root().append(std::slice::from_ref(&paragraph))?;
            crate::clipboard::insert_generated_nodes(
                &to_editor,
                crate::clipboard::generate_nodes_from_serialized_nodes(&json.borrow())?,
                &paragraph.select(Some(0), Some(0))?,
            )
        });
        let pasted = read(&editor, sequence);
        let found_label =
            || find_text_block(TEXT, false).map_or_else(|| "missing".to_owned(), |b| label(&b));
        let pasted_label = read(&editor, found_label);
        update(&editor, move || {
            let block = find_text_block(TEXT, false).expect("the pasted text is in one block");
            match c.selection {
                SelectionAt::Range => select((&block, 0), (&block, TEXT_LENGTH)),
                SelectionAt::At(offset) => select((&block, offset), (&block, offset)),
                SelectionAt::Across => unreachable!("across needs a neighbour"),
            };
            convert(target)
        });
        let after = read(&editor, sequence);
        let found = read(&editor, found_label);
        if pasted != copied
            || !keeps_ids(
                &copied,
                &after,
                highlighting && expected_label(&pasted_label, target) == "code",
            )
            || found != expected_label(&pasted_label, target)
        {
            failures.push(format!(
                "{}:\n  copied {}\n  pasted {}\n  after  {}, converted to {found}",
                describe_case(source, target, &c),
                copied.join(" "),
                pasted.join(" "),
                after.join(" ")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// The tests of `describe.each(SOURCES)('from %s')` and
// `describe.each(SOURCES)('pasted %s')`, each with `test.each(TARGETS)`,
// generated from the upstream lists.
// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to paragraph
#[test]
fn from_paragraph_to_paragraph() {
    from_source_to_target(Source::Paragraph, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to heading
#[test]
fn from_paragraph_to_heading() {
    from_source_to_target(Source::Paragraph, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to quote
#[test]
fn from_paragraph_to_quote() {
    from_source_to_target(Source::Paragraph, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to code
#[test]
fn from_paragraph_to_code() {
    from_source_to_target(Source::Paragraph, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to highlighted-code
#[test]
fn from_paragraph_to_highlighted_code() {
    from_source_to_target(Source::Paragraph, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to bullet
#[test]
fn from_paragraph_to_bullet() {
    from_source_to_target(Source::Paragraph, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to number
#[test]
fn from_paragraph_to_number() {
    from_source_to_target(Source::Paragraph, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to check
#[test]
fn from_paragraph_to_check() {
    from_source_to_target(Source::Paragraph, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to no-list
#[test]
fn from_paragraph_to_no_list() {
    from_source_to_target(Source::Paragraph, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to paragraph
#[test]
fn from_heading_to_paragraph() {
    from_source_to_target(Source::Heading, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to heading
#[test]
fn from_heading_to_heading() {
    from_source_to_target(Source::Heading, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to quote
#[test]
fn from_heading_to_quote() {
    from_source_to_target(Source::Heading, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to code
#[test]
fn from_heading_to_code() {
    from_source_to_target(Source::Heading, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to highlighted-code
#[test]
fn from_heading_to_highlighted_code() {
    from_source_to_target(Source::Heading, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to bullet
#[test]
fn from_heading_to_bullet() {
    from_source_to_target(Source::Heading, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to number
#[test]
fn from_heading_to_number() {
    from_source_to_target(Source::Heading, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to check
#[test]
fn from_heading_to_check() {
    from_source_to_target(Source::Heading, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to no-list
#[test]
fn from_heading_to_no_list() {
    from_source_to_target(Source::Heading, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to paragraph
#[test]
fn from_quote_to_paragraph() {
    from_source_to_target(Source::Quote, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to heading
#[test]
fn from_quote_to_heading() {
    from_source_to_target(Source::Quote, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to quote
#[test]
fn from_quote_to_quote() {
    from_source_to_target(Source::Quote, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to code
#[test]
fn from_quote_to_code() {
    from_source_to_target(Source::Quote, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to highlighted-code
#[test]
fn from_quote_to_highlighted_code() {
    from_source_to_target(Source::Quote, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to bullet
#[test]
fn from_quote_to_bullet() {
    from_source_to_target(Source::Quote, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to number
#[test]
fn from_quote_to_number() {
    from_source_to_target(Source::Quote, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to check
#[test]
fn from_quote_to_check() {
    from_source_to_target(Source::Quote, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to no-list
#[test]
fn from_quote_to_no_list() {
    from_source_to_target(Source::Quote, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to paragraph
#[test]
fn from_code_to_paragraph() {
    from_source_to_target(Source::Code, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to heading
#[test]
fn from_code_to_heading() {
    from_source_to_target(Source::Code, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to quote
#[test]
fn from_code_to_quote() {
    from_source_to_target(Source::Code, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to code
#[test]
fn from_code_to_code() {
    from_source_to_target(Source::Code, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to highlighted-code
#[test]
fn from_code_to_highlighted_code() {
    from_source_to_target(Source::Code, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to bullet
#[test]
fn from_code_to_bullet() {
    from_source_to_target(Source::Code, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to number
#[test]
fn from_code_to_number() {
    from_source_to_target(Source::Code, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to check
#[test]
fn from_code_to_check() {
    from_source_to_target(Source::Code, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to no-list
#[test]
fn from_code_to_no_list() {
    from_source_to_target(Source::Code, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to paragraph
#[test]
fn from_highlighted_code_to_paragraph() {
    from_source_to_target(Source::HighlightedCode, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to heading
#[test]
fn from_highlighted_code_to_heading() {
    from_source_to_target(Source::HighlightedCode, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to quote
#[test]
fn from_highlighted_code_to_quote() {
    from_source_to_target(Source::HighlightedCode, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to code
#[test]
fn from_highlighted_code_to_code() {
    from_source_to_target(Source::HighlightedCode, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to highlighted-code
#[test]
fn from_highlighted_code_to_highlighted_code() {
    from_source_to_target(Source::HighlightedCode, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to bullet
#[test]
fn from_highlighted_code_to_bullet() {
    from_source_to_target(Source::HighlightedCode, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to number
#[test]
fn from_highlighted_code_to_number() {
    from_source_to_target(Source::HighlightedCode, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to check
#[test]
fn from_highlighted_code_to_check() {
    from_source_to_target(Source::HighlightedCode, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to no-list
#[test]
fn from_highlighted_code_to_no_list() {
    from_source_to_target(Source::HighlightedCode, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to paragraph
#[test]
fn from_bullet_to_paragraph() {
    from_source_to_target(Source::Bullet, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to heading
#[test]
fn from_bullet_to_heading() {
    from_source_to_target(Source::Bullet, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to quote
#[test]
fn from_bullet_to_quote() {
    from_source_to_target(Source::Bullet, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to code
#[test]
fn from_bullet_to_code() {
    from_source_to_target(Source::Bullet, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to highlighted-code
#[test]
fn from_bullet_to_highlighted_code() {
    from_source_to_target(Source::Bullet, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to bullet
#[test]
fn from_bullet_to_bullet() {
    from_source_to_target(Source::Bullet, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to number
#[test]
fn from_bullet_to_number() {
    from_source_to_target(Source::Bullet, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to check
#[test]
fn from_bullet_to_check() {
    from_source_to_target(Source::Bullet, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to no-list
#[test]
fn from_bullet_to_no_list() {
    from_source_to_target(Source::Bullet, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to paragraph
#[test]
fn from_number_to_paragraph() {
    from_source_to_target(Source::Number, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to heading
#[test]
fn from_number_to_heading() {
    from_source_to_target(Source::Number, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to quote
#[test]
fn from_number_to_quote() {
    from_source_to_target(Source::Number, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to code
#[test]
fn from_number_to_code() {
    from_source_to_target(Source::Number, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to highlighted-code
#[test]
fn from_number_to_highlighted_code() {
    from_source_to_target(Source::Number, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to bullet
#[test]
fn from_number_to_bullet() {
    from_source_to_target(Source::Number, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to number
#[test]
fn from_number_to_number() {
    from_source_to_target(Source::Number, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to check
#[test]
fn from_number_to_check() {
    from_source_to_target(Source::Number, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to no-list
#[test]
fn from_number_to_no_list() {
    from_source_to_target(Source::Number, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to paragraph
#[test]
fn from_check_to_paragraph() {
    from_source_to_target(Source::Check, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to heading
#[test]
fn from_check_to_heading() {
    from_source_to_target(Source::Check, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to quote
#[test]
fn from_check_to_quote() {
    from_source_to_target(Source::Check, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to code
#[test]
fn from_check_to_code() {
    from_source_to_target(Source::Check, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to highlighted-code
#[test]
fn from_check_to_highlighted_code() {
    from_source_to_target(Source::Check, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to bullet
#[test]
fn from_check_to_bullet() {
    from_source_to_target(Source::Check, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to number
#[test]
fn from_check_to_number() {
    from_source_to_target(Source::Check, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to check
#[test]
fn from_check_to_check() {
    from_source_to_target(Source::Check, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to no-list
#[test]
fn from_check_to_no_list() {
    from_source_to_target(Source::Check, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to paragraph
#[test]
fn from_nested_bullet_to_paragraph() {
    from_source_to_target(Source::NestedBullet, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to heading
#[test]
fn from_nested_bullet_to_heading() {
    from_source_to_target(Source::NestedBullet, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to quote
#[test]
fn from_nested_bullet_to_quote() {
    from_source_to_target(Source::NestedBullet, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to code
#[test]
fn from_nested_bullet_to_code() {
    from_source_to_target(Source::NestedBullet, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to highlighted-code
#[test]
fn from_nested_bullet_to_highlighted_code() {
    from_source_to_target(Source::NestedBullet, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to bullet
#[test]
fn from_nested_bullet_to_bullet() {
    from_source_to_target(Source::NestedBullet, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to number
#[test]
fn from_nested_bullet_to_number() {
    from_source_to_target(Source::NestedBullet, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to check
#[test]
fn from_nested_bullet_to_check() {
    from_source_to_target(Source::NestedBullet, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to no-list
#[test]
fn from_nested_bullet_to_no_list() {
    from_source_to_target(Source::NestedBullet, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to paragraph
#[test]
fn from_nested_number_to_paragraph() {
    from_source_to_target(Source::NestedNumber, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to heading
#[test]
fn from_nested_number_to_heading() {
    from_source_to_target(Source::NestedNumber, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to quote
#[test]
fn from_nested_number_to_quote() {
    from_source_to_target(Source::NestedNumber, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to code
#[test]
fn from_nested_number_to_code() {
    from_source_to_target(Source::NestedNumber, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to highlighted-code
#[test]
fn from_nested_number_to_highlighted_code() {
    from_source_to_target(Source::NestedNumber, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to bullet
#[test]
fn from_nested_number_to_bullet() {
    from_source_to_target(Source::NestedNumber, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to number
#[test]
fn from_nested_number_to_number() {
    from_source_to_target(Source::NestedNumber, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to check
#[test]
fn from_nested_number_to_check() {
    from_source_to_target(Source::NestedNumber, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to no-list
#[test]
fn from_nested_number_to_no_list() {
    from_source_to_target(Source::NestedNumber, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to paragraph
#[test]
fn pasted_paragraph_to_paragraph() {
    pasted_source_to_target(Source::Paragraph, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to heading
#[test]
fn pasted_paragraph_to_heading() {
    pasted_source_to_target(Source::Paragraph, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to quote
#[test]
fn pasted_paragraph_to_quote() {
    pasted_source_to_target(Source::Paragraph, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to code
#[test]
fn pasted_paragraph_to_code() {
    pasted_source_to_target(Source::Paragraph, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to highlighted-code
#[test]
fn pasted_paragraph_to_highlighted_code() {
    pasted_source_to_target(Source::Paragraph, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to bullet
#[test]
fn pasted_paragraph_to_bullet() {
    pasted_source_to_target(Source::Paragraph, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to number
#[test]
fn pasted_paragraph_to_number() {
    pasted_source_to_target(Source::Paragraph, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to check
#[test]
fn pasted_paragraph_to_check() {
    pasted_source_to_target(Source::Paragraph, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to no-list
#[test]
fn pasted_paragraph_to_no_list() {
    pasted_source_to_target(Source::Paragraph, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to paragraph
#[test]
fn pasted_heading_to_paragraph() {
    pasted_source_to_target(Source::Heading, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to heading
#[test]
fn pasted_heading_to_heading() {
    pasted_source_to_target(Source::Heading, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to quote
#[test]
fn pasted_heading_to_quote() {
    pasted_source_to_target(Source::Heading, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to code
#[test]
fn pasted_heading_to_code() {
    pasted_source_to_target(Source::Heading, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to highlighted-code
#[test]
fn pasted_heading_to_highlighted_code() {
    pasted_source_to_target(Source::Heading, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to bullet
#[test]
fn pasted_heading_to_bullet() {
    pasted_source_to_target(Source::Heading, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to number
#[test]
fn pasted_heading_to_number() {
    pasted_source_to_target(Source::Heading, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to check
#[test]
fn pasted_heading_to_check() {
    pasted_source_to_target(Source::Heading, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to no-list
#[test]
fn pasted_heading_to_no_list() {
    pasted_source_to_target(Source::Heading, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to paragraph
#[test]
fn pasted_quote_to_paragraph() {
    pasted_source_to_target(Source::Quote, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to heading
#[test]
fn pasted_quote_to_heading() {
    pasted_source_to_target(Source::Quote, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to quote
#[test]
fn pasted_quote_to_quote() {
    pasted_source_to_target(Source::Quote, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to code
#[test]
fn pasted_quote_to_code() {
    pasted_source_to_target(Source::Quote, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to highlighted-code
#[test]
fn pasted_quote_to_highlighted_code() {
    pasted_source_to_target(Source::Quote, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to bullet
#[test]
fn pasted_quote_to_bullet() {
    pasted_source_to_target(Source::Quote, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to number
#[test]
fn pasted_quote_to_number() {
    pasted_source_to_target(Source::Quote, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to check
#[test]
fn pasted_quote_to_check() {
    pasted_source_to_target(Source::Quote, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to no-list
#[test]
fn pasted_quote_to_no_list() {
    pasted_source_to_target(Source::Quote, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to paragraph
#[test]
fn pasted_code_to_paragraph() {
    pasted_source_to_target(Source::Code, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to heading
#[test]
fn pasted_code_to_heading() {
    pasted_source_to_target(Source::Code, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to quote
#[test]
fn pasted_code_to_quote() {
    pasted_source_to_target(Source::Code, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to code
#[test]
fn pasted_code_to_code() {
    pasted_source_to_target(Source::Code, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to highlighted-code
#[test]
fn pasted_code_to_highlighted_code() {
    pasted_source_to_target(Source::Code, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to bullet
#[test]
fn pasted_code_to_bullet() {
    pasted_source_to_target(Source::Code, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to number
#[test]
fn pasted_code_to_number() {
    pasted_source_to_target(Source::Code, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to check
#[test]
fn pasted_code_to_check() {
    pasted_source_to_target(Source::Code, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to no-list
#[test]
fn pasted_code_to_no_list() {
    pasted_source_to_target(Source::Code, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to paragraph
#[test]
fn pasted_highlighted_code_to_paragraph() {
    pasted_source_to_target(Source::HighlightedCode, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to heading
#[test]
fn pasted_highlighted_code_to_heading() {
    pasted_source_to_target(Source::HighlightedCode, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to quote
#[test]
fn pasted_highlighted_code_to_quote() {
    pasted_source_to_target(Source::HighlightedCode, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to code
#[test]
fn pasted_highlighted_code_to_code() {
    pasted_source_to_target(Source::HighlightedCode, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to highlighted-code
#[test]
fn pasted_highlighted_code_to_highlighted_code() {
    pasted_source_to_target(Source::HighlightedCode, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to bullet
#[test]
fn pasted_highlighted_code_to_bullet() {
    pasted_source_to_target(Source::HighlightedCode, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to number
#[test]
fn pasted_highlighted_code_to_number() {
    pasted_source_to_target(Source::HighlightedCode, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to check
#[test]
fn pasted_highlighted_code_to_check() {
    pasted_source_to_target(Source::HighlightedCode, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to no-list
#[test]
fn pasted_highlighted_code_to_no_list() {
    pasted_source_to_target(Source::HighlightedCode, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to paragraph
#[test]
fn pasted_bullet_to_paragraph() {
    pasted_source_to_target(Source::Bullet, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to heading
#[test]
fn pasted_bullet_to_heading() {
    pasted_source_to_target(Source::Bullet, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to quote
#[test]
fn pasted_bullet_to_quote() {
    pasted_source_to_target(Source::Bullet, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to code
#[test]
fn pasted_bullet_to_code() {
    pasted_source_to_target(Source::Bullet, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to highlighted-code
#[test]
fn pasted_bullet_to_highlighted_code() {
    pasted_source_to_target(Source::Bullet, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to bullet
#[test]
fn pasted_bullet_to_bullet() {
    pasted_source_to_target(Source::Bullet, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to number
#[test]
fn pasted_bullet_to_number() {
    pasted_source_to_target(Source::Bullet, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to check
#[test]
fn pasted_bullet_to_check() {
    pasted_source_to_target(Source::Bullet, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to no-list
#[test]
fn pasted_bullet_to_no_list() {
    pasted_source_to_target(Source::Bullet, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to paragraph
#[test]
fn pasted_number_to_paragraph() {
    pasted_source_to_target(Source::Number, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to heading
#[test]
fn pasted_number_to_heading() {
    pasted_source_to_target(Source::Number, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to quote
#[test]
fn pasted_number_to_quote() {
    pasted_source_to_target(Source::Number, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to code
#[test]
fn pasted_number_to_code() {
    pasted_source_to_target(Source::Number, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to highlighted-code
#[test]
fn pasted_number_to_highlighted_code() {
    pasted_source_to_target(Source::Number, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to bullet
#[test]
fn pasted_number_to_bullet() {
    pasted_source_to_target(Source::Number, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to number
#[test]
fn pasted_number_to_number() {
    pasted_source_to_target(Source::Number, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to check
#[test]
fn pasted_number_to_check() {
    pasted_source_to_target(Source::Number, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to no-list
#[test]
fn pasted_number_to_no_list() {
    pasted_source_to_target(Source::Number, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to paragraph
#[test]
fn pasted_check_to_paragraph() {
    pasted_source_to_target(Source::Check, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to heading
#[test]
fn pasted_check_to_heading() {
    pasted_source_to_target(Source::Check, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to quote
#[test]
fn pasted_check_to_quote() {
    pasted_source_to_target(Source::Check, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to code
#[test]
fn pasted_check_to_code() {
    pasted_source_to_target(Source::Check, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to highlighted-code
#[test]
fn pasted_check_to_highlighted_code() {
    pasted_source_to_target(Source::Check, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to bullet
#[test]
fn pasted_check_to_bullet() {
    pasted_source_to_target(Source::Check, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to number
#[test]
fn pasted_check_to_number() {
    pasted_source_to_target(Source::Check, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to check
#[test]
fn pasted_check_to_check() {
    pasted_source_to_target(Source::Check, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to no-list
#[test]
fn pasted_check_to_no_list() {
    pasted_source_to_target(Source::Check, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to paragraph
#[test]
fn pasted_nested_bullet_to_paragraph() {
    pasted_source_to_target(Source::NestedBullet, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to heading
#[test]
fn pasted_nested_bullet_to_heading() {
    pasted_source_to_target(Source::NestedBullet, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to quote
#[test]
fn pasted_nested_bullet_to_quote() {
    pasted_source_to_target(Source::NestedBullet, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to code
#[test]
fn pasted_nested_bullet_to_code() {
    pasted_source_to_target(Source::NestedBullet, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to highlighted-code
#[test]
fn pasted_nested_bullet_to_highlighted_code() {
    pasted_source_to_target(Source::NestedBullet, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to bullet
#[test]
fn pasted_nested_bullet_to_bullet() {
    pasted_source_to_target(Source::NestedBullet, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to number
#[test]
fn pasted_nested_bullet_to_number() {
    pasted_source_to_target(Source::NestedBullet, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to check
#[test]
fn pasted_nested_bullet_to_check() {
    pasted_source_to_target(Source::NestedBullet, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to no-list
#[test]
fn pasted_nested_bullet_to_no_list() {
    pasted_source_to_target(Source::NestedBullet, Target::NoList);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to paragraph
#[test]
fn pasted_nested_number_to_paragraph() {
    pasted_source_to_target(Source::NestedNumber, Target::Paragraph);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to heading
#[test]
fn pasted_nested_number_to_heading() {
    pasted_source_to_target(Source::NestedNumber, Target::Heading);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to quote
#[test]
fn pasted_nested_number_to_quote() {
    pasted_source_to_target(Source::NestedNumber, Target::Quote);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to code
#[test]
fn pasted_nested_number_to_code() {
    pasted_source_to_target(Source::NestedNumber, Target::Code);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to highlighted-code
#[test]
fn pasted_nested_number_to_highlighted_code() {
    pasted_source_to_target(Source::NestedNumber, Target::HighlightedCode);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to bullet
#[test]
fn pasted_nested_number_to_bullet() {
    pasted_source_to_target(Source::NestedNumber, Target::Bullet);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to number
#[test]
fn pasted_nested_number_to_number() {
    pasted_source_to_target(Source::NestedNumber, Target::Number);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to check
#[test]
fn pasted_nested_number_to_check() {
    pasted_source_to_target(Source::NestedNumber, Target::Check);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to no-list
#[test]
fn pasted_nested_number_to_no_list() {
    pasted_source_to_target(Source::NestedNumber, Target::NoList);
}

/// `$getRoot().getAllTextNodes().map(text => text:ids)`.
fn text_marks() -> Vec<String> {
    get_root()
        .get_all_text_nodes()
        .iter()
        .map(|text| {
            format!(
                "{}:{}",
                text.get_text_content(),
                get_node_mark_ids(text).unwrap().join(",")
            )
        })
        .collect()
}

fn last_child() -> NodeRef {
    let last = get_root().get_last_child().unwrap();
    assert!(last.is_element());
    last
}

// #9490: the converted paragraph merges into the list above it, whose
// item's text lines up with the end of the paragraph's text
// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a paragraph turned into a list item keeps the comment on a link
#[test]
fn a_paragraph_turned_into_a_list_item_keeps_the_comment_on_a_link() {
    let editor = build_editor(false);
    update(&editor, || {
        let commented = create_text_node("abcd");
        get_root().append(&[
            with_children(
                create_list_node(ListType::Number, 1.0),
                &[with_children(
                    create_list_item_node(None),
                    &[create_text_node("echo ")],
                )],
            ),
            with_children(
                create_paragraph_node(),
                &[
                    with_children(
                        link("https://example.org"),
                        std::slice::from_ref(&commented),
                    ),
                    create_text_node(" Z echo "),
                ],
            ),
        ])?;
        add_mark_id("c0", &commented.select(Some(0), Some(4))?)?;
        Ok(())
    });
    let before = read(&editor, sequence);
    update(&editor, || {
        last_child()
            .get_all_text_nodes()
            .last()
            .unwrap()
            .select(Some(4), Some(4))?;
        insert_list(ListType::Number)
    });
    assert_eq!(read(&editor, || get_root().get_children_size()), 1);
    assert_eq!(read(&editor, sequence), before);
}

// #9490: the text of the paragraph lines up with the marked item above
// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a paragraph turned into a list item does not take the comment of the item above
#[test]
fn a_paragraph_turned_into_a_list_item_does_not_take_the_comment_of_the_item_above() {
    let editor = build_editor(false);
    update(&editor, || {
        let item = create_text_node("old echo ");
        let space = create_text_node(" ");
        get_root().append(&[
            with_children(
                create_list_node(ListType::Bullet, 1.0),
                &[with_children(
                    create_list_item_node(None),
                    std::slice::from_ref(&item),
                )],
            ),
            with_children(
                create_paragraph_node(),
                &[create_text_node("new echo"), space.clone()],
            ),
        ])?;
        add_mark_id(
            "c1",
            &item.select(Some(0), Some(item.get_text_content_size()))?,
        )?;
        add_mark_id("c1", &space.select(Some(0), Some(1))?)?;
        Ok(())
    });
    let before = read(&editor, sequence);
    update(&editor, || {
        last_child().get_all_text_nodes()[0].select_end()?;
        insert_list(ListType::Bullet)
    });
    assert_eq!(read(&editor, sequence), before);
}

// Code highlighting turns its tokens back into TextNodes when the code
// block becomes a paragraph; with the code selected, the inserted-text
// rule does not run, so the new TextNodes keep the ids on their own
// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a comment on code text survives turning the code block into a paragraph
#[test]
fn a_comment_on_code_text_survives_turning_the_code_block_into_a_paragraph() {
    let editor = build_editor(true);
    update(&editor, || {
        get_root().append(&[with_children(
            create_code_node(None, None),
            &[create_text_node(TEXT)],
        )])
    });
    update(&editor, || {
        let code = get_root().get_first_child().unwrap();
        assert!(code.is_element());
        mark(&code, "c1", 0, TEXT_LENGTH);
        select((&code, 0), (&code, TEXT_LENGTH));
        Ok(())
    });
    let before = read(&editor, sequence);
    update(&editor, || convert(Target::Paragraph));
    let after = read(&editor, || {
        let mut out = vec![
            get_root()
                .get_first_child_or_throw()
                .unwrap()
                .get_type()
                .to_owned(),
        ];
        out.extend(sequence());
        out
    });
    let mut expected = vec!["paragraph".to_owned()];
    expected.extend(before.iter().cloned());
    assert_eq!(after, expected);
    assert_eq!(before[0], "a:c1");
}

// A lone list item turned into another block with the caret at its end
// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a list item turned into a paragraph keeps its comment
// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a list item turned into a heading keeps its comment
// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a list item turned into a quote keeps its comment
#[test]
fn a_list_item_turned_into_a_block_keeps_its_comment() {
    for target in [Target::Paragraph, Target::Heading, Target::Quote] {
        let editor = build_editor(false);
        update(&editor, || {
            let text = create_text_node("alpha beta");
            get_root().append(&[with_children(
                create_list_node(ListType::Bullet, 1.0),
                &[with_children(
                    create_list_item_node(None),
                    std::slice::from_ref(&text),
                )],
            )])?;
            add_mark_id("c1", &text.select(Some(6), Some(10))?)?;
            Ok(())
        });
        update(&editor, move || {
            get_root()
                .get_all_text_nodes()
                .last()
                .unwrap()
                .select_end()?;
            convert(target)
        });
        let got = read(&editor, || {
            let mut out = vec![
                get_root()
                    .get_first_child_or_throw()
                    .unwrap()
                    .get_type()
                    .to_owned(),
            ];
            out.extend(text_marks());
            out
        });
        assert_eq!(
            got,
            [expected_label("", target).as_str(), "alpha :", "beta:c1"],
            "{target:?}"
        );
    }
}

// A node that code moves is not inserted text, unlike a new node
// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > moved text > a node moved into a marked run keeps its ids, a new node joins the run
#[test]
fn a_node_moved_into_a_marked_run_keeps_its_ids_a_new_node_joins_the_run() {
    let editor = build_editor(false);
    update(&editor, || {
        let a = create_text_node("aaa");
        let b = create_text_node("bbb");
        b.toggle_format("italic");
        let moved = create_text_node("moved");
        moved.toggle_format("bold");
        get_root().append(&[
            with_children(create_paragraph_node(), &[a.clone(), b.clone()]),
            with_children(create_paragraph_node(), &[moved]),
        ])?;
        add_mark_id("c1", &a.select(Some(0), Some(3))?)?;
        add_mark_id("c1", &b.select(Some(0), Some(3))?)?;
        Ok(())
    });
    update(&editor, || {
        let created = create_text_node("new");
        created.toggle_format("underline");
        get_root().get_all_text_nodes()[0].insert_after(&created, true)?;
        created.select_end()?;
        Ok(())
    });
    assert_eq!(
        read(&editor, text_marks),
        ["aaa:c1", "new:c1", "bbb:c1", "moved:"]
    );
    update(&editor, || {
        let texts = get_root().get_all_text_nodes();
        let (created, moved) = (&texts[1], &texts[3]);
        created.insert_after(moved, true)?;
        moved.select_end()?;
        Ok(())
    });
    assert_eq!(
        read(&editor, text_marks),
        ["aaa:c1", "new:c1", "moved:", "bbb:c1"]
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > moved text > a node moved into the text that replaces a selection: the new text keeps the replaced ids
#[test]
fn a_node_moved_into_the_text_that_replaces_a_selection() {
    let editor = build_editor(false);
    update(&editor, || {
        let replaced = create_text_node("zzz");
        let keep = create_text_node("keep");
        keep.toggle_format("bold");
        get_root().append(&[
            with_children(create_paragraph_node(), &[keep]),
            with_children(create_paragraph_node(), std::slice::from_ref(&replaced)),
        ])?;
        add_mark_id("c1", &replaced.select(Some(0), Some(3))?)?;
        replaced.select(Some(0), Some(3))?;
        Ok(())
    });
    let inner = (*editor).clone();
    update(&editor, move || {
        inner.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, "Q".to_owned())?;
        let texts = get_root().get_all_text_nodes();
        let (moved, inserted) = (&texts[0], &texts[1]);
        inserted.insert_before(moved, true)?;
        inserted.select_end()?;
        Ok(())
    });
    assert_eq!(read(&editor, text_marks), ["keep:", "Q:c1"]);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > moved text > a selection replaced in the update that turns its list item into a paragraph
#[test]
fn a_selection_replaced_in_the_update_that_turns_its_list_item_into_a_paragraph() {
    let editor = build_editor(false);
    update(&editor, || {
        let a = create_text_node("abc");
        let b = create_text_node("def");
        b.toggle_format("bold");
        get_root().append(&[with_children(
            create_list_node(ListType::Bullet, 1.0),
            &[with_children(
                create_list_item_node(None),
                &[a.clone(), b.clone()],
            )],
        )])?;
        add_mark_id("c1", &a.select(Some(0), Some(3))?)?;
        add_mark_id("c1", &b.select(Some(0), Some(3))?)?;
        a.select_end()?;
        Ok(())
    });
    let inner = (*editor).clone();
    update(&editor, move || {
        convert(Target::Paragraph)?;
        let texts = get_root().get_all_text_nodes();
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, texts[0].get_key(), 1, PointType::Text)?;
        selection.point_set(Which::Focus, texts[1].get_key(), 2, PointType::Text)?;
        set_selection(Some(selection));
        inner.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, "Q".to_owned())?;
        Ok(())
    });
    assert_eq!(read(&editor, sequence), ["a:c1", "Q:c1", "f:c1"]);
}
