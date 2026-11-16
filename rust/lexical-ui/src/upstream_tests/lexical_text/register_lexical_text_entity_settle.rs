//! Ports of `packages/lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts`.
//!
//! registerLexicalTextEntity's transforms must settle wherever an edit leaves
//! text between text nodes of other formats (#9637), and settle on the same
//! entities as a fresh parse of the text they leave.

use super::support::{discrete, entity_class};
use crate::markdown::RegExp;
use crate::text::{EntityMatch, register_lexical_text_entity};
use crate::upstream_tests::support::panic_message;
use crate::{
    CreateEditorArgs, Editor, LexicalError, NodeClass, NodeRef, PointType, SelectionRef,
    UpdateOptions, Which, create, create_editor, create_paragraph_node, create_range_selection,
    create_text_node, get_root, set_composition_key, set_selection, utf16,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

entity_class!(KeywordNode, "test-keyword", false, false);
entity_class!(TagNode, "test-tag", false, true);

type GetMatch = Rc<dyn Fn(&str) -> Option<EntityMatch>>;

thread_local! {
    static KEYWORD: RegExp = RegExp::new("(^|[^a-z#])(cat|dog)(?=[^a-z]|$)", "").unwrap();
    static TAG: RegExp = RegExp::new(r"(^|\s)(#[a-z]+)", "").unwrap();
    static SPACED: RegExp = RegExp::new("^#[a-z]+$|#[a-z]+(?= )", "").unwrap();
}

/// `{end: m.index + m[1].length + m[2].length, start: m.index + m[1].length}`.
fn group_2(regex: &RegExp, text: &str) -> Option<EntityMatch> {
    let m = regex.exec(text)?;
    let start = m.index + utf16::len(m.str(1));
    Some(EntityMatch {
        start,
        end: start + utf16::len(m.str(2)),
    })
}

/// `{end: m.index + m[0].length, start: m.index}`.
fn whole(regex: &RegExp, text: &str) -> Option<EntityMatch> {
    let m = regex.exec(text)?;
    Some(EntityMatch {
        start: m.index,
        end: m.index + utf16::len(m.text()),
    })
}

/// A matcher of the whole match of `/source/flags`.
fn regex_whole(source: &str, flags: &str) -> GetMatch {
    let regex = RegExp::new(source, flags).unwrap();
    Rc::new(move |text| whole(&regex, text))
}

/// The issue's matcher: `cat` or `dog` with no letter next to them.
fn keyword_match(text: &str) -> Option<EntityMatch> {
    KEYWORD.with(|r| group_2(r, text))
}

/// A hashtag-like matcher: `#` and letters, after the start or a space.
fn tag_match(text: &str) -> Option<EntityMatch> {
    TAG.with(|r| group_2(r, text))
}

/// A hashtag when a space follows it, or when it is all of the text.
fn spaced(text: &str) -> Option<EntityMatch> {
    SPACED.with(|r| whole(r, text))
}

/// `dog` with no letter before or after it, where the letter before is looked
/// at but not taken into the match, like a regex lookbehind
/// (`/(?<![a-z])dog(?![a-z])/`).
fn lookbehind_match(text: &str, word: &str) -> Option<EntityMatch> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let word: Vec<u16> = word.encode_utf16().collect();
    let is_letter = |i: Option<usize>| {
        i.and_then(|i| units.get(i))
            .is_some_and(|&c| (65..=90).contains(&c) || (97..=122).contains(&c))
    };
    let index_of = |from: usize| {
        (from..=units.len().saturating_sub(word.len()))
            .find(|&i| i + word.len() <= units.len() && units[i..i + word.len()] == word[..])
    };
    let mut at = index_of(0);
    while let Some(i) = at {
        if !is_letter(i.checked_sub(1)) && !is_letter(Some(i + word.len())) {
            return Some(EntityMatch {
                start: i,
                end: i + word.len(),
            });
        }
        at = index_of(i + 1);
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Matcher {
    Keyword,
    Tag,
    Lookbehind,
}

const MATCHER_NAMES: [Matcher; 3] = [Matcher::Keyword, Matcher::Tag, Matcher::Lookbehind];

impl Matcher {
    fn get_match(self) -> GetMatch {
        match self {
            Matcher::Keyword => Rc::new(keyword_match),
            Matcher::Tag => Rc::new(tag_match),
            Matcher::Lookbehind => Rc::new(|text| lookbehind_match(text, "dog")),
        }
    }

    fn index(self) -> u32 {
        MATCHER_NAMES.iter().position(|&m| m == self).unwrap() as u32
    }
}

/// `createEditor({nodes, onError: e => { throw e; }})`.
fn new_editor(nodes: Vec<NodeClass>) -> Editor {
    create_editor(CreateEditorArgs {
        nodes,
        ..CreateEditorArgs::default()
    })
}

/// `registerLexicalTextEntity(editor, getMatch, Klass, node =>
/// $create(Klass).setTextContent(createText(node.getTextContent())))`, with
/// `setMode(mode)` when `mode` is given.
fn register<T: crate::LexicalNode + Default>(
    editor: &Editor,
    get_match: GetMatch,
    target_type: &'static str,
    create_text: impl Fn(String) -> String + 'static,
    mode: Option<&'static str>,
) {
    let _ = register_lexical_text_entity(
        editor,
        move |text| get_match(text),
        target_type,
        move |node| {
            let entity = create::<T>();
            entity.set_text_content(&create_text(node.get_text_content()));
            if let Some(mode) = mode {
                entity.set_mode(mode).unwrap();
            }
            entity
        },
    );
}

fn make_editor_with(matcher: Matcher, get_match: Option<GetMatch>) -> Editor {
    let editor = new_editor(vec![
        NodeClass::of::<KeywordNode>(),
        NodeClass::of::<TagNode>(),
    ]);
    if matcher == Matcher::Tag {
        let get_match = get_match.unwrap_or_else(|| matcher.get_match());
        register::<TagNode>(&editor, get_match, "test-tag", |t| t, None);
    } else {
        register::<KeywordNode>(&editor, matcher.get_match(), "test-keyword", |t| t, None);
    }
    editor
}

fn make_editor(matcher: Matcher) -> Editor {
    make_editor_with(matcher, None)
}

/// A run of text: `*` bold, `_` italic, `` ` `` code, then the text.
fn parse_run(run: &str) -> (Vec<&'static str>, String) {
    let mut formats = Vec::new();
    let mut rest = run;
    loop {
        match rest.chars().next() {
            Some('*') => formats.push("bold"),
            Some('_') => formats.push("italic"),
            Some('`') => formats.push("code"),
            _ => break,
        }
        rest = &rest[1..];
    }
    (formats, rest.to_owned())
}

fn create_run(run: &str) -> NodeRef {
    let (formats, text) = parse_run(run);
    let node = create_text_node(&text);
    for format in formats {
        node.toggle_format(format);
    }
    node
}

fn paragraph() -> NodeRef {
    get_root().get_first_child_or_throw().unwrap()
}

fn child(index: usize) -> NodeRef {
    paragraph().get_child_at_index(index).unwrap()
}

type Out = Vec<(String, String, i64)>;

fn out(entries: &[(&str, &str, i64)]) -> Out {
    entries
        .iter()
        .map(|&(t, x, f)| (t.to_owned(), x.to_owned(), f))
        .collect()
}

fn read_out() -> Out {
    paragraph()
        .get_children()
        .iter()
        .filter(|node| node.is_text())
        .map(|node| {
            (
                node.get_type().to_owned(),
                node.get_text_content(),
                node.text_data().get_format() as i64,
            )
        })
        .collect()
}

fn read(editor: &Editor) -> Out {
    editor.get_editor_state().read(read_out)
}

fn texts(out: &Out) -> String {
    out.iter().map(|(_, text, _)| text.as_str()).collect()
}

fn try_update(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
) -> Result<(), LexicalError> {
    editor.update(f, discrete())
}

fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    try_update(editor, f).unwrap();
}

fn try_set_up(editor: &Editor, runs: &[&str]) -> Result<(), LexicalError> {
    let runs: Vec<String> = runs.iter().map(|&r| r.to_owned()).collect();
    try_update(editor, move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&runs.iter().map(|r| create_run(r)).collect::<Vec<_>>())?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    })
}

fn set_up(editor: &Editor, runs: &[&str]) {
    try_set_up(editor, runs).unwrap();
}

fn select(node: &NodeRef, offset: usize) -> SelectionRef {
    let sel = create_range_selection();
    sel.point_set(Which::Anchor, node.get_key(), offset, PointType::Text)
        .unwrap();
    sel.point_set(Which::Focus, node.get_key(), offset, PointType::Text)
        .unwrap();
    set_selection(Some(sel.clone()));
    sel
}

/// Type into run `index` at `offset` of a paragraph made of `runs`.
fn type_into(runs: &[&str], index: usize, offset: usize, typed: &str, matcher: Matcher) -> Editor {
    let editor = make_editor(matcher);
    set_up(&editor, runs);
    let typed = typed.to_owned();
    update(&editor, move || {
        select(&child(index), offset).insert_text(&typed)
    });
    editor
}

/// The text of a paragraph as runs of one format each, adjacent runs of the
/// same format joined: what a fresh parse starts from.
fn to_runs(out: &Out) -> Vec<(String, i64)> {
    let mut runs: Vec<(String, i64)> = Vec::new();
    for (_, text, format) in out {
        match runs.last_mut() {
            Some(last) if last.1 == *format => last.0 += text,
            _ => runs.push((text.clone(), *format)),
        }
    }
    runs
}

/// The nodes the transforms settle on when they parse `runs` from scratch.
fn fresh_parse(runs: Vec<(String, i64)>, matcher: Matcher, get_match: Option<GetMatch>) -> Out {
    let editor = make_editor_with(matcher, get_match);
    update(&editor, move || {
        let paragraph = create_paragraph_node();
        paragraph.append(
            &runs
                .iter()
                .map(|(text, format)| {
                    let node = create_text_node(text);
                    node.set_format(*format as f64);
                    node
                })
                .collect::<Vec<_>>(),
        )?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    read(&editor)
}

fn expect_fresh_parse(editor: &Editor, matcher: Matcher) {
    let out = read(editor);
    assert_eq!(out, fresh_parse(to_runs(&out), matcher, None));
}

/// Runs `f`, and returns the error it gives (or the panic) as a message, or
/// `None`.
fn thrown(f: impl FnOnce() -> Result<(), LexicalError>) -> Option<String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(e.to_string().chars().take(80).collect()),
        Err(payload) => Some(panic_message(&*payload).chars().take(80).collect()),
    }
}

const B: i64 = 1; // IS_BOLD
const I: i64 = 2; // IS_ITALIC

use Matcher::{Keyword, Lookbehind, Tag};

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > bold "x" | "dogcat" | bold "y", a space after "dog": settles, no entity
#[test]
fn bold_x_dogcat_bold_y_a_space_after_dog_settles_no_entity() {
    let editor = type_into(&["*x", "dogcat", "*y"], 1, 3, " ", Keyword);
    assert_eq!(
        read(&editor),
        out(&[("text", "x", B), ("text", "dog cat", 0), ("text", "y", B)])
    );
    expect_fresh_parse(&editor, Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > bold "x" | "dogcat" | bold "dog", an emoji after "dog": settles, no entity
#[test]
fn bold_x_dogcat_bold_dog_an_emoji_after_dog_settles_no_entity() {
    let editor = type_into(&["*x", "dogcat", "*dog"], 1, 3, "😀", Keyword);
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "x", B),
            ("text", "dog😀cat", 0),
            ("text", "dog", B)
        ])
    );
    expect_fresh_parse(&editor, Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a later match in the same node is still an entity, and one touching the next node is not
#[test]
fn a_later_match_in_the_same_node_is_still_an_entity() {
    let editor = type_into(&["*x", "dogcat dog", "*y"], 1, 3, " ", Keyword);
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "x", B),
            ("text", "dog ", 0),
            ("test-keyword", "cat", 0),
            ("text", " dog", 0),
            ("text", "y", B),
        ])
    );
    expect_fresh_parse(&editor, Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > control: "dogcat" | bold "y", nothing before
#[test]
fn control_dogcat_bold_y_nothing_before() {
    let editor = type_into(&["dogcat", "*y"], 0, 3, " ", Keyword);
    assert_eq!(
        read(&editor),
        out(&[
            ("test-keyword", "dog", 0),
            ("text", " cat", 0),
            ("text", "y", B)
        ])
    );
    expect_fresh_parse(&editor, Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > control: bold " " | "dogcat" | bold "y", a space before
#[test]
fn control_bold_space_dogcat_bold_y_a_space_before() {
    let editor = type_into(&["* ", "dogcat", "*y"], 1, 3, " ", Keyword);
    assert_eq!(
        read(&editor),
        out(&[
            ("text", " ", B),
            ("test-keyword", "dog", 0),
            ("text", " cat", 0),
            ("text", "y", B),
        ])
    );
    expect_fresh_parse(&editor, Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a match that needs a lookbehind is checked against the previous node
#[test]
fn a_match_that_needs_a_lookbehind_is_checked_against_the_previous_node() {
    let touching = type_into(&["*x", "dogcat", "*y"], 1, 3, " ", Lookbehind);
    assert_eq!(
        read(&touching),
        out(&[("text", "x", B), ("text", "dog cat", 0), ("text", "y", B)])
    );
    let spaced = type_into(&["*x ", "dogcat", "*y"], 1, 3, " ", Lookbehind);
    assert_eq!(
        read(&spaced),
        out(&[
            ("text", "x ", B),
            ("test-keyword", "dog", 0),
            ("text", " cat", 0),
            ("text", "y", B),
        ])
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a hashtag after a formatted letter is not an entity, after a formatted space it is
#[test]
fn a_hashtag_after_a_formatted_letter_is_not_an_entity_after_a_formatted_space_it_is() {
    let letter = type_into(&["*a", "#tagx", "*."], 1, 4, " ", Tag);
    assert_eq!(
        read(&letter),
        out(&[("text", "a", B), ("text", "#tag x", 0), ("text", ".", B)])
    );
    expect_fresh_parse(&letter, Tag);
    let space = type_into(&["*a ", "#tagx", "*."], 1, 4, " ", Tag);
    assert_eq!(
        read(&space),
        out(&[
            ("text", "a ", B),
            ("test-tag", "#tag", 0),
            ("text", " x", 0),
            ("text", ".", B),
        ])
    );
    expect_fresh_parse(&space, Tag);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > of two matches next to each other, the second stays text, as before
#[test]
fn of_two_matches_next_to_each_other_the_second_stays_text() {
    let editor = type_into(&["*x ", "a#b", "*."], 1, 0, "#", Tag);
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "x ", B),
            ("test-tag", "#a", 0),
            ("text", "#b", 0),
            ("text", ".", B),
        ])
    );
    expect_fresh_parse(&editor, Tag);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > entity at the edges of a node, every format combination around it
#[test]
fn entity_at_the_edges_of_a_node_every_format_combination_around_it() {
    let formats = ["*", "_", "`", "*_"];
    for before in formats {
        for after in formats {
            for (left, right) in [("x", "y"), (" ", " "), ("x", " "), (" ", "y")] {
                for typed in [" ", "😀", "!", "z"] {
                    let runs = [
                        format!("{before}{left}"),
                        "dogcat".to_owned(),
                        format!("{after}{right}"),
                    ];
                    let runs: Vec<&str> = runs.iter().map(String::as_str).collect();
                    let editor =
                        std::panic::catch_unwind(|| type_into(&runs, 1, 3, typed, Keyword))
                            .unwrap_or_else(|_| panic!("{runs:?} {typed:?} threw"));
                    let out = read(&editor);
                    assert_eq!(texts(&out), format!("{left}dog{typed}cat{right}"));
                    expect_fresh_parse(&editor, Keyword);
                }
            }
        }
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > IME composition between two formatted nodes settles, and so does its end
#[test]
fn ime_composition_between_two_formatted_nodes_settles_and_so_does_its_end() {
    let editor = make_editor(Keyword);
    set_up(&editor, &["*x", "dogcat", "*y"]);
    update(&editor, || {
        let node = child(1);
        select(&node, 3);
        set_composition_key(Some(node.get_key()));
        Ok(())
    });
    for typed in ["、", " "] {
        assert_eq!(
            thrown(|| try_update(&editor, move || select(&child(1), 3).insert_text(typed))),
            None
        );
    }
    assert_eq!(
        thrown(|| try_update(&editor, || {
            set_composition_key(None);
            for node in paragraph().get_children() {
                node.mark_dirty();
            }
            Ok(())
        })),
        None
    );
    assert_eq!(texts(&read(&editor)), "xdog 、caty");
    expect_fresh_parse(&editor, Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > pasting formatted runs around a keyword settles
#[test]
fn pasting_formatted_runs_around_a_keyword_settles() {
    let editor = make_editor(Keyword);
    set_up(&editor, &["dogcat"]);
    assert_eq!(
        thrown(|| try_update(&editor, || {
            let node = paragraph().get_first_child_or_throw()?;
            select(&node, 3).insert_nodes(vec![
                create_run("*x"),
                create_run(" dog "),
                create_run("*y"),
            ])
        })),
        None
    );
    assert_eq!(texts(&read(&editor)), "dogx dog ycat");
    expect_fresh_parse(&editor, Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > bolding the text before an entity turns it back into text
#[test]
fn bolding_the_text_before_an_entity_turns_it_back_into_text() {
    let editor = make_editor(Keyword);
    set_up(&editor, &["xdog cat"]);
    update(&editor, || {
        let node = paragraph().get_first_child_or_throw()?;
        let parts = node.split_text(&[1])?;
        paragraph()
            .get_first_child_or_throw()?
            .toggle_format("bold");
        parts[1].mark_dirty();
        Ok(())
    });
    expect_fresh_parse(&editor, Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing a letter before an entity after text in another format turns it back into text
#[test]
fn typing_a_letter_before_an_entity_after_text_in_another_format_turns_it_back_into_text() {
    let editor = make_editor(Keyword);
    set_up(&editor, &["_😀", "dog", "*!"]);
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "😀", 2),
            ("test-keyword", "dog", 0),
            ("text", "!", B)
        ])
    );
    update(&editor, || select(&child(1), 0).insert_text("a"));
    assert_eq!(
        read(&editor),
        out(&[("text", "😀", 2), ("text", "adog", 0), ("text", "!", B)])
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an earlier match that ends where the text ends is handled as before
#[test]
fn an_earlier_match_that_ends_where_the_text_ends_is_handled_as_before() {
    // `<word> dog` at the end of the text, or else `dog` anywhere.
    let whole_re = RegExp::new("[a-z]+ dog$", "").unwrap();
    let phrase: GetMatch = Rc::new(move |text| {
        let w = whole(&whole_re, text);
        let word = keyword_match(text);
        match (w, word) {
            (Some(w), None) => Some(w),
            (Some(w), Some(word)) if w.start <= word.start => Some(w),
            _ => word,
        }
    });
    let editor = new_editor(vec![NodeClass::of::<KeywordNode>()]);
    register::<KeywordNode>(&editor, phrase, "test-keyword", |t| t, None);
    set_up(&editor, &["*z", "ab dog!"]);
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "z", B),
            ("text", "ab ", 0),
            ("test-keyword", "dog", 0),
            ("text", "!", 0),
        ])
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a matcher that needs the text after a match settles as before
#[test]
fn a_matcher_that_needs_the_text_after_a_match_settles_as_before() {
    // `dog` when " cat" follows it, or when it is all of the text.
    let editor = new_editor(vec![NodeClass::of::<KeywordNode>()]);
    register::<KeywordNode>(
        &editor,
        regex_whole("^dog$|dog(?= cat)", ""),
        "test-keyword",
        |t| t,
        None,
    );
    set_up(&editor, &["*x", "_a dog cat"]);
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "x", B),
            ("text", "a ", I),
            ("test-keyword", "dog", I),
            ("text", " cat", I),
        ])
    );
}

/// An editor with only `TagNode`, made by `spaced` (or `get_match`).
fn spaced_editor(get_match: GetMatch, mode: Option<&'static str>) -> Editor {
    let editor = new_editor(vec![NodeClass::of::<TagNode>()]);
    register::<TagNode>(&editor, get_match, "test-tag", |t| t, mode);
    editor
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a match at the start of a node that needs the text after it settles after a formatted node
#[test]
fn a_match_at_the_start_of_a_node_that_needs_the_text_after_it_settles_after_a_formatted_node() {
    // A hashtag when a space follows it, or when it is all of the text.
    let make = || spaced_editor(Rc::new(spaced), None);
    let typed = make();
    set_up(&typed, &["*x", "#tagx"]);
    update(&typed, || select(&child(1), 4).insert_text(" "));
    assert_eq!(
        read(&typed),
        out(&[("text", "x", B), ("test-tag", "#tag", 0), ("text", " x", 0)])
    );

    // Removing the space after the entity turns it back into text at once,
    // as a fresh parse of the text does.
    let removed = make();
    set_up(&removed, &["*g", "g#x "]);
    assert_eq!(
        read(&removed),
        out(&[
            ("text", "g", B),
            ("text", "g", 0),
            ("test-tag", "#x", 0),
            ("text", " ", 0),
        ])
    );
    update(&removed, || {
        paragraph().get_last_child_or_throw()?.remove(false)
    });
    assert_eq!(read(&removed), out(&[("text", "g", B), ("text", "g#x", 0)]));
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing before a formatted node with a token or segmented match leaves it as it was
#[test]
fn typing_before_a_formatted_node_with_a_token_or_segmented_match_leaves_it_as_it_was() {
    for mode in ["token", "segmented"] {
        let editor = spaced_editor(Rc::new(tag_match), Some(mode));
        set_up(&editor, &["#", "*#b "]);
        assert_eq!(
            thrown(|| try_update(&editor, || select(&child(0), 1).insert_text(" "))),
            None
        );
        assert_eq!(
            read(&editor),
            out(&[("text", "# ", 0), ("text", "#b ", B)]),
            "{mode}"
        );
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a node after an entity in the same format and style but another detail is not its source
#[test]
fn a_node_after_an_entity_in_the_same_format_and_style_but_another_detail_is_not_its_source() {
    let editor = spaced_editor(Rc::new(spaced), None);
    update(&editor, || {
        let space = create_text_node(" ");
        space.set_detail(2.0)?; // IS_UNMERGEABLE
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("#x"), space])?;
        get_root().append(&[paragraph])
    });
    assert_eq!(read(&editor)[0], out(&[("test-tag", "#x", 0)])[0]);
    update(&editor, || select(&child(0), 0).insert_text("g"));
    assert_eq!(read(&editor), out(&[("text", "g#x", 0), ("text", " ", 0)]));
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an entity after a node with an earlier match is checked too
#[test]
fn an_entity_after_a_node_with_an_earlier_match_is_checked_too() {
    let editor = spaced_editor(Rc::new(spaced), None);
    set_up(&editor, &["*x", "_z ", "#b "]);
    assert_eq!(read(&editor)[2], out(&[("test-tag", "#b", 0)])[0]);
    update(&editor, || {
        child(1).set_text_content("#a ");
        Ok(())
    });
    update(&editor, || {
        paragraph().get_last_child_or_throw()?.remove(false)
    });
    let out = read(&editor);
    assert_eq!(out, fresh_parse(to_runs(&out), Tag, Some(Rc::new(spaced))));
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > removing the text between an entity and a token after it turns the entity back into text
#[test]
fn removing_the_text_between_an_entity_and_a_token_after_it_turns_the_entity_back_into_text() {
    let congrats_re = RegExp::new("(^|[^a-z])(congrats)(?=[^a-z]|$)", "i").unwrap();
    let congrats: GetMatch = Rc::new(move |text| {
        let m = congrats_re.exec(text)?;
        let start = m.index + utf16::len(m.str(1));
        Some(EntityMatch {
            start,
            end: start + 8,
        })
    });
    let editor = new_editor(vec![NodeClass::of::<KeywordNode>()]);
    register::<KeywordNode>(&editor, congrats, "test-keyword", |t| t, None);
    set_up(&editor, &["_!", "*!congrats "]);
    update(&editor, || {
        let alice = create_run("*Alice");
        alice.set_mode("token")?;
        paragraph().append(&[alice])
    });
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "!", 2),
            ("text", "!", B),
            ("test-keyword", "congrats", B),
            ("text", " ", B),
            ("text", "Alice", B),
        ])
    );
    update(&editor, || child(3).remove(false));
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "!", 2),
            ("text", "!congrats", B),
            ("text", "Alice", B)
        ])
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an entity is not kept by text after it in another format
#[test]
fn an_entity_is_not_kept_by_text_after_it_in_another_format() {
    let editor = spaced_editor(Rc::new(spaced), None);
    set_up(&editor, &["#x", "* "]);
    assert_eq!(
        read(&editor),
        out(&[("test-tag", "#x", 0), ("text", " ", B)])
    );
    update(&editor, || select(&child(0), 0).insert_text("g"));
    // As a fresh parse of "g#x" and a bold " " makes it.
    assert_eq!(read(&editor), out(&[("text", "g#x", 0), ("text", " ", B)]));
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an entity is checked again in each update
#[test]
fn an_entity_is_checked_again_in_each_update() {
    let editor = spaced_editor(Rc::new(spaced), None);
    set_up(&editor, &["*#ab#ab ", "_a"]);
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "#ab", B),
            ("test-tag", "#ab", B),
            ("text", " ", B),
            ("text", "a", 2),
        ])
    );
    update(&editor, || select(&child(1), 2).insert_text("x"));
    update(&editor, || {
        child(1).toggle_format("bold");
        Ok(())
    });
    let out = read(&editor);
    assert_eq!(out, fresh_parse(to_runs(&out), Tag, Some(Rc::new(spaced))));
}

/// Types each character of `chars` into run 1, two before its end, and
/// returns the number of dirty leaves of each update.
fn dirty_leaves_typing(editor: &Editor, chars: &str) -> Vec<usize> {
    let dirty = Rc::new(RefCell::new(Vec::new()));
    let sink = dirty.clone();
    let _ = editor.register_update_listener(move |payload| {
        sink.borrow_mut().push(payload.dirty_leaves.len());
    });
    for c in chars.chars() {
        update(editor, move || {
            let node = child(1);
            let offset = node.get_text_content_size() - 2;
            select(&node, offset).insert_text(&c.to_string())
        });
    }
    dirty.take()
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing in text before a formatted hashtag marks only that text dirty, so history merges the keystrokes
#[test]
fn typing_in_text_before_a_formatted_hashtag_marks_only_that_text_dirty() {
    let editor = make_editor(Tag);
    set_up(&editor, &["*a", "#foo ", "_#bar"]);
    assert_eq!(dirty_leaves_typing(&editor, "xyz"), [1, 1, 1]);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing in text with a match before a formatted hashtag marks only that text dirty
#[cfg(feature = "hashtag")]
#[test]
fn typing_in_text_with_a_match_before_a_formatted_hashtag_marks_only_that_text_dirty() {
    // With @lexical/hashtag's own matcher.
    use crate::hashtag::{HashtagNode, create_hashtag_node, get_hashtag_match};
    let editor = new_editor(vec![NodeClass::of::<HashtagNode>()]);
    let _ = register_lexical_text_entity(&editor, get_hashtag_match, "hashtag", |node| {
        create_hashtag_node(&node.get_text_content())
    });
    set_up(&editor, &["*#x", "#y!ab#", "_#z"]);
    assert_eq!(read(&editor)[0], out(&[("hashtag", "#x", B)])[0]);
    assert_eq!(dirty_leaves_typing(&editor, "cde"), [1, 1, 1]);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing that makes a match at the start of the next node one leaves it text until that node changes
#[test]
fn typing_that_makes_a_match_at_the_start_of_the_next_node_one_leaves_it_text_until_that_node_changes()
 {
    let editor = make_editor(Tag);
    set_up(&editor, &["a", "*b", "#foo #bar"]);
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "a", 0),
            ("text", "b", B),
            ("text", "#foo ", 0),
            ("test-tag", "#bar", 0),
        ])
    );
    update(&editor, || {
        child(1).splice_text(1, 0, " ", false).map(|_| ())
    });
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "a", 0),
            ("text", "b ", B),
            ("text", "#foo ", 0),
            ("test-tag", "#bar", 0),
        ])
    );
    update(&editor, || {
        child(2).mark_dirty();
        Ok(())
    });
    assert_eq!(
        read(&editor),
        out(&[
            ("text", "a", 0),
            ("text", "b ", B),
            ("test-tag", "#foo", 0),
            ("text", " ", 0),
            ("test-tag", "#bar", 0),
        ])
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a token or segmented entity with text after it in its node settles
#[test]
fn a_token_or_segmented_entity_with_text_after_it_in_its_node_settles() {
    for mode in ["token", "segmented"] {
        let editor = spaced_editor(Rc::new(tag_match), Some(mode));
        set_up(&editor, &["#a b"]);
        assert_eq!(
            read(&editor),
            out(&[("test-tag", "#a", 0), ("text", " b", 0)])
        );
        update(&editor, || {
            select(&paragraph().get_last_child().unwrap(), 2).insert_text("c")
        });
        assert_eq!(
            read(&editor),
            out(&[("test-tag", "#a", 0), ("text", " bc", 0)]),
            "{mode}"
        );
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a token or segmented entity after a match left as text settles
#[test]
fn a_token_or_segmented_entity_after_a_match_left_as_text_settles() {
    for mode in ["token", "segmented"] {
        let editor = spaced_editor(Rc::new(tag_match), Some(mode));
        set_up(&editor, &[" DOG x DOG", "*_#ab ##ab"]);
        assert_eq!(
            thrown(|| try_update(&editor, || {
                let node = child(1);
                let selection = create_range_selection();
                selection.point_set(Which::Anchor, node.get_key(), 5, PointType::Text)?;
                selection.point_set(Which::Focus, node.get_key(), 7, PointType::Text)?;
                set_selection(Some(selection.clone()));
                selection.insert_text("x")
            })),
            None
        );
        // `#x` is turned back into text twice, then kept, and takes the `b`
        // its match extends into (3: bold and italic).
        assert_eq!(
            read(&editor),
            out(&[
                ("text", " DOG x DOG", 0),
                ("text", "#ab ", 3),
                ("test-tag", "#xb", 3),
            ]),
            "{mode}"
        );
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an update that fails does not count toward the next one
#[test]
fn an_update_that_fails_does_not_count_toward_the_next_one() {
    let editor = spaced_editor(Rc::new(tag_match), Some("token"));
    let failures = Rc::new(Cell::new(2));
    let count = failures.clone();
    let _ = editor.register_node_transform("text", move |node| {
        if count.get() > 0 && node.get_text_content() == "#foo x" {
            count.set(count.get() - 1);
            return Err(LexicalError::new("fail this update"));
        }
        Ok(())
    });
    editor
        .update(
            || {
                let a = create_text_node("a");
                a.set_format(1.0);
                let tag = create::<TagNode>();
                tag.set_text_content("#foo");
                tag.set_mode("token")?;
                let paragraph = create_paragraph_node();
                paragraph.append(&[a, tag, create_text_node(" x")])?;
                get_root().clear()?;
                get_root().append(&[paragraph])
            },
            UpdateOptions {
                discrete: true,
                skip_transforms: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    for _ in 0..3 {
        // The first two updates fail and are discarded.
        let _ = try_update(&editor, || {
            paragraph().get_last_child_or_throw()?.mark_dirty();
            Ok(())
        });
    }
    assert_eq!(failures.get(), 0);
    assert_eq!(
        read(&editor),
        out(&[("text", "a", B), ("text", "#foo x", 0)])
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > two registrations, one making token or segmented entities, settle
#[test]
fn two_registrations_one_making_token_or_segmented_entities_settle() {
    for mode in ["token", "segmented"] {
        for token_first in [true, false] {
            let editor = new_editor(vec![
                NodeClass::of::<KeywordNode>(),
                NodeClass::of::<TagNode>(),
            ]);
            let register_token = || {
                register::<TagNode>(
                    &editor,
                    regex_whole("[A-Z]+", ""),
                    "test-tag",
                    |t| t,
                    Some(mode),
                );
            };
            if token_first {
                register_token();
            }
            register::<KeywordNode>(
                &editor,
                Rc::new(|text: &str| keyword_match(&text.to_lowercase())),
                "test-keyword",
                |t| t,
                None,
            );
            if !token_first {
                register_token();
            }
            set_up(&editor, &["catDOGg"]);
            assert_eq!(
                read(&editor),
                out(&[("text", "cat", 0), ("test-tag", "DOG", 0), ("text", "g", 0)])
            );
            set_up(&editor, &["xa", "*d", "DOG!dog"]);
            let mut settled = out(&[("text", "d", B), ("test-tag", "DOG", 0)]);
            if token_first {
                settled.extend(out(&[("text", "!dog", 0)]));
            } else {
                settled.extend(out(&[("text", "!", 0), ("test-keyword", "dog", 0)]));
            }
            let mut expected = out(&[("text", "xa", 0)]);
            expected.extend(settled.clone());
            assert_eq!(read(&editor), expected, "{mode} {token_first}");
            update(&editor, || select(&child(0), 2).insert_text("x"));
            let mut expected = out(&[("text", "xax", 0)]);
            expected.extend(settled);
            assert_eq!(read(&editor), expected, "{mode} {token_first}");
        }
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a createNode that adds text or changes its case settles
#[test]
fn a_create_node_that_adds_text_or_changes_its_case_settles() {
    let settle = |runs: &[&str], create_text: fn(String) -> String| -> Out {
        let editor = new_editor(vec![NodeClass::of::<KeywordNode>()]);
        register::<KeywordNode>(
            &editor,
            regex_whole("dog|cat", ""),
            "test-keyword",
            create_text,
            None,
        );
        set_up(&editor, runs);
        read(&editor)
    };
    assert_eq!(
        settle(&["*z", "x", "_dog", "*!"], |text| text + " "),
        out(&[
            ("text", "z", B),
            ("text", "x", 0),
            ("test-keyword", "dog", I),
            ("text", " ", I),
            ("text", "!", B),
        ])
    );
    // `dog` before "cat", or `DOG`: the entity `createNode` makes is a
    // match after the bold "x" even though the text it replaces is one only
    // with the "cat" after it.
    let upper = new_editor(vec![NodeClass::of::<KeywordNode>()]);
    register::<KeywordNode>(
        &upper,
        regex_whole("dog(?=cat)|DOG", ""),
        "test-keyword",
        |text| text.to_uppercase(),
        None,
    );
    set_up(&upper, &["*x", "dogcat", "*y"]);
    assert_eq!(
        read(&upper),
        out(&[
            ("text", "x", B),
            ("test-keyword", "DOG", 0),
            ("text", "cat", 0),
            ("text", "y", B),
        ])
    );
    // The `DOG` made from "xdog" is no entity after the bold "z" either, so
    // it is turned back into text before it grows over the italic `DOG`
    // (before, it grew over it first, and that text lost its italic).
    assert_eq!(
        settle(&["*z", "xdog", "_dog cat", "*!"], |text| text
            .to_uppercase()),
        out(&[
            ("text", "z", B),
            ("text", "xDOG", 0),
            ("text", "DOG CAT", I),
            ("text", "!", B),
        ])
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing in a long text node before an entity calls getMatch a few times, however long it is
#[test]
fn typing_in_a_long_text_node_before_an_entity_calls_get_match_a_few_times() {
    for repeat in [2000, 8000] {
        let calls = Rc::new(Cell::new(0));
        let counter = calls.clone();
        let editor = new_editor(vec![NodeClass::of::<KeywordNode>()]);
        register::<KeywordNode>(
            &editor,
            Rc::new(move |text: &str| {
                counter.set(counter.get() + 1);
                keyword_match(&text.to_lowercase())
            }),
            "test-keyword",
            |t| t,
            None,
        );
        // "do" + "g" is a match that starts in the bold node, so the long node
        // is left as text, and the entity after it stays one.
        update(&editor, move || {
            let keyword = create::<KeywordNode>();
            keyword.set_text_content("dog");
            let paragraph = create_paragraph_node();
            paragraph.append(&[
                create_run("*do"),
                create_text_node(&format!("g{} ", " cat".repeat(repeat))),
                keyword,
            ])?;
            get_root().append(&[paragraph])
        });
        calls.set(0);
        update(&editor, || {
            child(1).splice_text(3, 0, "x", false).map(|_| ())
        });
        assert!(calls.get() <= 10, "{repeat}: {}", calls.get());
        assert_eq!(read(&editor)[2], out(&[("test-keyword", "dog", 0)])[0]);
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a createNode that changes the text still makes an entity, as before
#[test]
fn a_create_node_that_changes_the_text_still_makes_an_entity_as_before() {
    let editor = new_editor(vec![NodeClass::of::<KeywordNode>()]);
    register::<KeywordNode>(
        &editor,
        Rc::new(|text: &str| {
            lookbehind_match(text, "DOG").or_else(|| lookbehind_match(text, "dog"))
        }),
        "test-keyword",
        |text| text.to_lowercase(),
        None,
    );
    set_up(&editor, &["* ", "DOGcat", "*y"]);
    update(&editor, || select(&child(1), 3).insert_text(" "));
    assert_eq!(
        read(&editor),
        out(&[
            ("text", " ", B),
            ("test-keyword", "dog", 0),
            ("text", " cat", 0),
            ("text", "y", B),
        ])
    );
}

// On random layouts.

/// Mulberry32, so a failure names a seed that reproduces it.
fn rng(seed: u32) -> impl FnMut() -> f64 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6d2b_79f5);
        let mut t = a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

fn pick<T: Copy>(random: &mut impl FnMut() -> f64, xs: &[T]) -> T {
    xs[(random() * xs.len() as f64) as usize]
}

fn mark_all() -> Result<(), LexicalError> {
    for node in paragraph().get_children() {
        node.mark_dirty();
    }
    Ok(())
}

/// `process.env.TEXT_ENTITY_SETTLE_SEEDS ?? 2000`.
fn seeds() -> u32 {
    std::env::var("TEXT_ENTITY_SETTLE_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000)
}

fn pieces(matcher: Matcher) -> &'static [&'static str] {
    match matcher {
        Keyword => &["dog", "cat", " ", "x", "a", "#", "😀", "!", "do", "g"],
        Lookbehind => &["dog", " ", "x", "d", "og", "😀", "!"],
        Tag => &["#", "#a", "#ab", "a", "b", " ", "😀", "!", "x"],
    }
}
const TYPED: [&str; 9] = [" ", "😀", "a", "x", "#", "g", "d", "!", "\u{a0}"];
const FORMATS: [&str; 5] = ["", "*", "_", "*_", "`"];

fn random_runs(random: &mut impl FnMut() -> f64, matcher: Matcher) -> Vec<String> {
    let mut runs = Vec::new();
    let mut last_format: Option<&str> = None;
    let mut i = 2 + (random() * 3.0) as usize;
    while i > 0 {
        let mut format = pick(random, &FORMATS);
        if Some(format) == last_format {
            format = if format.is_empty() { "*" } else { "" };
        }
        last_format = Some(format);
        let mut text = String::new();
        let mut j = 1 + (random() * 4.0) as usize;
        while j > 0 {
            text += pick(random, pieces(matcher));
            j -= 1;
        }
        runs.push(format!("{format}{text}"));
        i -= 1;
    }
    runs
}

fn as_strs(runs: &[String]) -> Vec<&str> {
    runs.iter().map(String::as_str).collect()
}

/// A fresh parse settles, and transforming every node again leaves it as it
/// is.
fn random_layouts_parse_and_parse_again_the_same(matcher: Matcher) {
    let mut failures: Vec<String> = Vec::new();
    let mut seed = 0;
    while seed < seeds() && failures.len() < 5 {
        let mut random = rng(seed * 7 + matcher.index());
        let runs = random_runs(&mut random, matcher);
        let editor = make_editor(matcher);
        let label = format!("seed {seed} {runs:?}");
        seed += 1;
        if let Some(error) = thrown(|| try_set_up(&editor, &as_strs(&runs))) {
            failures.push(format!("{label}: {error}"));
            continue;
        }
        let parsed = read(&editor);
        if let Some(again) = thrown(|| try_update(&editor, mark_all)) {
            failures.push(format!("{label} again: {again}"));
        } else if read(&editor) != parsed {
            failures.push(format!("{label} again: {:?}", read(&editor)));
        }
    }
    assert_eq!(failures, Vec::<String>::new());
}

/// #9637's shape: a node with random text between two nodes of other
/// formats whose text has no match of its own, and starts none with it (a
/// hashtag's `#` would). Typing or deleting anywhere in it settles on the
/// entities a fresh parse of the result finds.
fn random_edits_between_two_formatted_nodes_settle_on_a_fresh_parse(matcher: Matcher) {
    let mut neighbours = vec!["x", "y", " ", "!", "😀", "a", "xa", " !"];
    if matcher != Tag {
        neighbours.push("#");
    }
    let mut failures: Vec<String> = Vec::new();
    let mut seed = 0;
    while seed < seeds() && failures.len() < 5 {
        let mut random = rng(seed * 7 + 3 + matcher.index());
        let middle = random_runs(&mut random, matcher).remove(0);
        let first = format!(
            "{}{}",
            pick(&mut random, &FORMATS[1..]),
            pick(&mut random, &neighbours)
        );
        let runs = vec![
            first,
            parse_run(&middle).1,
            format!(
                "{}{}",
                pick(&mut random, &FORMATS[1..]),
                pick(&mut random, &neighbours)
            ),
        ];
        let editor = make_editor(matcher);
        let label0 = format!("seed {seed} {runs:?}");
        seed += 1;
        if let Some(error) = thrown(|| try_set_up(&editor, &as_strs(&runs))) {
            failures.push(format!("{label0}: {error}"));
            continue;
        }
        let nodes = read(&editor);
        // An edit in one of the nodes the middle text became.
        let index = 1 + (random() * (nodes.len() - 2) as f64) as usize;
        let size = utf16::len(&nodes[index].1);
        let typing = size == 0 || random() < 0.7;
        let offset = (random() * (if typing { size + 1 } else { size }) as f64) as usize;
        let typed = pick(&mut random, &TYPED);
        let label = format!(
            "{label0} {nodes:?} {} at {index}:{offset}",
            if typing {
                format!("type {typed:?}")
            } else {
                "delete".to_owned()
            }
        );
        let error = thrown(|| {
            try_update(&editor, move || {
                let node = child(index);
                if typing {
                    select(&node, offset).insert_text(typed)
                } else {
                    node.splice_text(offset as i64, 1, "", false).map(|_| ())
                }
            })
        });
        if let Some(error) = error {
            failures.push(format!("{label}: {error}"));
            continue;
        }
        let out = read(&editor);
        let fresh = fresh_parse(to_runs(&out), matcher, None);
        if out != fresh {
            failures.push(format!("{label}: {out:?} != {fresh:?}"));
        }
    }
    assert_eq!(failures, Vec::<String>::new());
}

/// Any edit (typing or deleting anywhere, or bolding a node) settles, and so
/// does transforming every node again.
fn random_sessions_of_4_edits_anywhere_settle(matcher: Matcher) {
    let mut failures: Vec<String> = Vec::new();
    let mut seed = 0;
    while seed < seeds() && failures.len() < 5 {
        let mut random = rng(seed * 7 + 5 + matcher.index());
        let runs = random_runs(&mut random, matcher);
        let editor = make_editor(matcher);
        let this_seed = seed;
        seed += 1;
        if thrown(|| try_set_up(&editor, &as_strs(&runs))).is_some() {
            continue;
        }
        for edit in 0..4 {
            let nodes = read(&editor);
            if nodes.is_empty() {
                break;
            }
            let index = (random() * nodes.len() as f64) as usize;
            let size = utf16::len(&nodes[index].1);
            let op = random();
            let offset = (random() * (size + 1) as f64) as usize;
            let typed = pick(&mut random, &TYPED);
            let label = format!(
                "seed {this_seed} edit {edit} {nodes:?} op {op:.2} at {index}:{offset} {typed:?}"
            );
            let error = thrown(|| {
                try_update(&editor, move || {
                    let node = child(index);
                    if op < 0.6 {
                        select(&node, offset).insert_text(typed)
                    } else if op < 0.85 {
                        if size > 0 {
                            node.splice_text(offset.min(size - 1) as i64, 1, "", false)?;
                        }
                        Ok(())
                    } else {
                        node.toggle_format("bold");
                        Ok(())
                    }
                })
            });
            let again = error.or_else(|| thrown(|| try_update(&editor, mark_all)));
            if let Some(again) = again {
                failures.push(format!("{label}: {again}"));
                break;
            }
        }
    }
    assert_eq!(failures, Vec::<String>::new());
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > keyword: 2000 random layouts parse, and parse again the same
#[test]
fn keyword_random_layouts_parse_and_parse_again_the_same() {
    random_layouts_parse_and_parse_again_the_same(Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > keyword: 2000 random edits between two formatted nodes settle on a fresh parse
#[test]
fn keyword_random_edits_between_two_formatted_nodes_settle_on_a_fresh_parse() {
    random_edits_between_two_formatted_nodes_settle_on_a_fresh_parse(Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > keyword: 2000 random sessions of 4 edits anywhere settle
#[test]
fn keyword_random_sessions_of_4_edits_anywhere_settle() {
    random_sessions_of_4_edits_anywhere_settle(Keyword);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > tag: 2000 random layouts parse, and parse again the same
#[test]
fn tag_random_layouts_parse_and_parse_again_the_same() {
    random_layouts_parse_and_parse_again_the_same(Tag);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > tag: 2000 random edits between two formatted nodes settle on a fresh parse
#[test]
fn tag_random_edits_between_two_formatted_nodes_settle_on_a_fresh_parse() {
    random_edits_between_two_formatted_nodes_settle_on_a_fresh_parse(Tag);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > tag: 2000 random sessions of 4 edits anywhere settle
#[test]
fn tag_random_sessions_of_4_edits_anywhere_settle() {
    random_sessions_of_4_edits_anywhere_settle(Tag);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > lookbehind: 2000 random layouts parse, and parse again the same
#[test]
fn lookbehind_random_layouts_parse_and_parse_again_the_same() {
    random_layouts_parse_and_parse_again_the_same(Lookbehind);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > lookbehind: 2000 random edits between two formatted nodes settle on a fresh parse
#[test]
fn lookbehind_random_edits_between_two_formatted_nodes_settle_on_a_fresh_parse() {
    random_edits_between_two_formatted_nodes_settle_on_a_fresh_parse(Lookbehind);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > lookbehind: 2000 random sessions of 4 edits anywhere settle
#[test]
fn lookbehind_random_sessions_of_4_edits_anywhere_settle() {
    random_sessions_of_4_edits_anywhere_settle(Lookbehind);
}
