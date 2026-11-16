//! Ports of `packages/lexical/src/__tests__/unit/LexicalSelection.test.ts`.
//!
//! Where upstream reads the mounted editor's `innerHTML`, these compare the
//! committed tree ([`shape`]), which the reconciler renders one to one.

#![cfg(all(
    feature = "extension",
    feature = "link",
    feature = "list",
    feature = "rich-text"
))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{LinkAttributes, create_link_node};
use crate::list::{ListType, create_list_item_node, create_list_node, is_list_node};
use crate::upstream_tests::support::{
    TestBlockableDecoratorNode, TestInlineElementNode, TestShadowRootNode,
    create_test_blockable_decorator_node, create_test_shadow_root_node, mount_in_jsdom,
    set_is_inline, shape, update,
};
use crate::{
    LexicalError, LexicalExtension, NodeClass, NodeRef, PointType, SelectionRef, Which,
    create_paragraph_node, create_range_selection, create_text_node, define_extension, get_root,
    get_selection, set_composition_key, set_selection,
};
use std::rc::Rc;

/// `selectionTestExtension`'s parts, so a test can add to them.
fn selection_test_extension(name: &str, extra: Vec<NodeClass>) -> LexicalEditorWithDispose {
    let mut nodes = vec![
        NodeClass::of::<TestBlockableDecoratorNode>(),
        NodeClass::of::<TestInlineElementNode>(),
        NodeClass::of::<TestShadowRootNode>(),
    ];
    nodes.extend(extra);
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::link::link_extension().into(),
            crate::list::list_extension().into(),
        ],
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `buildEditorFromExtensions(selectionTestExtension)`.
fn build() -> LexicalEditorWithDispose {
    selection_test_extension("@test/selection", Vec::new())
}

/// The `LexicalSelection tests` editor: built and mounted (`beforeEach`).
fn mounted() -> LexicalEditorWithDispose {
    let editor = build();
    mount_in_jsdom(&editor);
    editor
}

fn range(selection: &SelectionRef) -> crate::RangeSelection {
    selection.range().expect("a RangeSelection")
}

fn current() -> SelectionRef {
    get_selection()
        .filter(SelectionRef::is_range)
        .expect("Expected RangeSelection")
}

fn paragraph_of(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

fn link_of(text: &str) -> NodeRef {
    let link = create_link_node("https://", &LinkAttributes::default());
    link.append(&[create_text_node(text)]).unwrap();
    link
}

fn texts_of(nodes: &[NodeRef]) -> Vec<String> {
    nodes.iter().map(NodeRef::get_text_content).collect()
}

fn block_decorator() -> NodeRef {
    set_is_inline(&create_test_blockable_decorator_node(), false)
}

// LexicalSelection tests > Inserting text either side of inline elements

#[derive(Clone, Copy)]
enum Mode {
    Start,
    Mid,
    End,
}

#[derive(Clone, Copy)]
enum Method {
    InsertText,
    InsertNodes,
}

/// `setup(mode)`.
fn setup_inline(editor: &crate::Editor, mode: Mode) {
    update(editor, move || {
        let root = get_root();
        root.clear()?;
        let children = match mode {
            Mode::Start => vec![link_of("a"), create_text_node("b")],
            Mode::Mid => vec![create_text_node("a"), link_of("b"), create_text_node("c")],
            Mode::End => vec![create_text_node("a"), link_of("b")],
        };
        root.append(&[paragraph_of(&children)])
    });
    assert_eq!(
        shape(editor),
        match mode {
            Mode::End => "<paragraph>[a]<link>[b]</link></paragraph>",
            Mode::Mid => "<paragraph>[a]<link>[b]</link>[c]</paragraph>",
            Mode::Start => "<paragraph><link>[a]</link>[b]</paragraph>",
        }
    );
}

/// `$insertTextOrNodes(selection, method)`.
fn insert_text_or_nodes(selection: &SelectionRef, method: Method) -> Result<(), LexicalError> {
    match method {
        // Mirrors what LexicalClipboard does when pasting inline plain text.
        Method::InsertText => selection.insert_text("x"),
        // Mirrors pasting inline rich text.
        Method::InsertNodes => selection.insert_nodes(vec![paragraph_of(&[create_text_node("x")])]),
    }
}

/// Selects `(anchor, focus)` in the paragraph's first (`first`) or last
/// child, inserts with `method`, and compares the result with `expected`.
fn insert_beside_inline(mode: Mode, first: bool, offset: usize, method: Method, expected: &str) {
    let editor = mounted();
    setup_inline(&editor, mode);
    update(&editor, move || {
        let paragraph = get_root().get_first_child_or_throw()?;
        let node = if first {
            paragraph.get_first_child_or_throw()?
        } else {
            paragraph.get_last_child_or_throw()?
        };
        let selection = node.select(Some(offset), Some(offset))?;
        insert_text_or_nodes(&selection, method)
    });
    assert_eq!(shape(&editor), expected);
    editor.dispose();
}

const BEFORE_START: &str = "<paragraph>[x]<link>[a]</link>[b]</paragraph>";
const BEFORE_MID: &str = "<paragraph>[ax]<link>[b]</link>[c]</paragraph>";
const BEFORE_END: &str = "<paragraph>[ax]<link>[b]</link></paragraph>";
const AFTER_START: &str = "<paragraph><link>[a]</link>[xb]</paragraph>";
const AFTER_MID: &str = "<paragraph>[a]<link>[b]</link>[xc]</paragraph>";
const AFTER_END: &str = "<paragraph>[a]<link>[b]</link>[x]</paragraph>";

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > Start-of-paragraph inline elements > Can insert text before a start-of-paragraph inline element, using insertText
#[test]
fn insert_text_before_a_start_of_paragraph_inline_element_using_insert_text() {
    insert_beside_inline(Mode::Start, true, 0, Method::InsertText, BEFORE_START);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > Start-of-paragraph inline elements > Can insert text before a start-of-paragraph inline element, using insertNodes
#[test]
fn insert_text_before_a_start_of_paragraph_inline_element_using_insert_nodes() {
    insert_beside_inline(Mode::Start, true, 0, Method::InsertNodes, BEFORE_START);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > Mid-paragraph inline elements > Can insert text before a mid-paragraph inline element, using insertText
#[test]
fn insert_text_before_a_mid_paragraph_inline_element_using_insert_text() {
    insert_beside_inline(Mode::Mid, true, 1, Method::InsertText, BEFORE_MID);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > Mid-paragraph inline elements > Can insert text before a mid-paragraph inline element, using insertNodes
#[test]
fn insert_text_before_a_mid_paragraph_inline_element_using_insert_nodes() {
    insert_beside_inline(Mode::Mid, true, 1, Method::InsertNodes, BEFORE_MID);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > End-of-paragraph inline elements > Can insert text before an end-of-paragraph inline element, using insertText
#[test]
fn insert_text_before_an_end_of_paragraph_inline_element_using_insert_text() {
    insert_beside_inline(Mode::End, true, 1, Method::InsertText, BEFORE_END);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > End-of-paragraph inline elements > Can insert text before an end-of-paragraph inline element, using insertNodes
#[test]
fn insert_text_before_an_end_of_paragraph_inline_element_using_insert_nodes() {
    insert_beside_inline(Mode::End, true, 1, Method::InsertNodes, BEFORE_END);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > Start-of-paragraph inline elements > Can insert text after a start-of-paragraph inline element, using insertText
#[test]
fn insert_text_after_a_start_of_paragraph_inline_element_using_insert_text() {
    insert_beside_inline(Mode::Start, false, 0, Method::InsertText, AFTER_START);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > Start-of-paragraph inline elements > Can insert text after a start-of-paragraph inline element, using insertNodes
#[test]
fn insert_text_after_a_start_of_paragraph_inline_element_using_insert_nodes() {
    insert_beside_inline(Mode::Start, false, 0, Method::InsertNodes, AFTER_START);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > Mid-paragraph inline elements > Can insert text after a mid-paragraph inline element, using insertText
#[test]
fn insert_text_after_a_mid_paragraph_inline_element_using_insert_text() {
    insert_beside_inline(Mode::Mid, false, 0, Method::InsertText, AFTER_MID);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > Mid-paragraph inline elements > Can insert text after a mid-paragraph inline element, using insertNodes
#[test]
fn insert_text_after_a_mid_paragraph_inline_element_using_insert_nodes() {
    insert_beside_inline(Mode::Mid, false, 0, Method::InsertNodes, AFTER_MID);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > End-of-paragraph inline elements > Can insert text after an end-of-paragraph inline element, using insertText
#[test]
fn insert_text_after_an_end_of_paragraph_inline_element_using_insert_text() {
    insert_beside_inline(Mode::End, false, 1, Method::InsertText, AFTER_END);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > End-of-paragraph inline elements > Can insert text after an end-of-paragraph inline element, using insertNodes
#[test]
fn insert_text_after_an_end_of_paragraph_inline_element_using_insert_nodes() {
    insert_beside_inline(Mode::End, false, 1, Method::InsertNodes, AFTER_END);
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > insertText() > inserts into existing paragraph node when selection is on parent of paragraph
#[test]
fn inserts_into_existing_paragraph_node_when_selection_is_on_parent_of_paragraph() {
    let editor = mounted();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[create_paragraph_node()])?;
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, "root", 0, PointType::Element)?;
        selection.point_set(Which::Focus, "root", 0, PointType::Element)?;
        set_selection(Some(selection.clone()));
        selection.insert_text("text")?;
        assert_eq!(root.get_children_size(), 1);
        assert_eq!(root.get_text_content(), "text");
        Ok(())
    });
    editor.dispose();
}

// LexicalSelection tests > removeText

/// The `removeText` fixtures: a paragraph of a `first` and a `second` text
/// node, the one named by `token`/`segmented` in that mode.
#[derive(Clone, Copy)]
enum Fixture {
    /// `leading text`, then `token text` in token mode.
    TrailingToken,
    /// `token text` in token mode, then `trailing text`.
    LeadingToken,
    /// `leading text`, then `segmented text` in segmented mode.
    TrailingSegmented,
}

/// What a `removeText` case selects: the whole of `first`, the whole of
/// `second`, or a text range from `first` at one offset to `second` at
/// another (`None`: its end).
#[derive(Clone, Copy)]
enum Sel {
    First,
    Second,
    Range(usize, Option<usize>),
}

/// Where the caret lands: on the paragraph at 0, on `first` at an offset
/// (`None`: its end), on `second` at 0, or on the only text node left at 0.
#[derive(Clone, Copy)]
enum Caret {
    Paragraph,
    First(Option<usize>),
    Second,
    Remaining,
}

struct RemoveTextCase {
    fixture: Fixture,
    select: Sel,
    first_attached: bool,
    second_attached: bool,
    texts: &'static [&'static str],
    caret: Caret,
}

fn remove_text(case: RemoveTextCase) {
    let editor = mounted();
    let (first, second, paragraph) = crate::upstream_tests::support::update_with(&editor, {
        let fixture = case.fixture;
        move || {
            let (first, second) = match fixture {
                Fixture::TrailingToken => {
                    let t = create_text_node("token text");
                    t.set_mode("token")?;
                    (create_text_node("leading text"), t)
                }
                Fixture::LeadingToken => {
                    let t = create_text_node("token text");
                    t.set_mode("token")?;
                    (t, create_text_node("trailing text"))
                }
                Fixture::TrailingSegmented => {
                    let t = create_text_node("segmented text");
                    t.set_mode("segmented")?;
                    (create_text_node("leading text"), t)
                }
            };
            let paragraph = paragraph_of(&[first.clone(), second.clone()]);
            get_root().clear()?;
            get_root().append(std::slice::from_ref(&paragraph))?;
            Ok((first, second, paragraph))
        }
    });
    let (f, s, p) = (first.clone(), second.clone(), paragraph.clone());
    update(&editor, move || {
        let sel = match case.select {
            Sel::First => f.select(Some(0), Some(f.get_text_content_size()))?,
            Sel::Second => s.select(Some(0), Some(s.get_text_content_size()))?,
            Sel::Range(start, end) => {
                let sel = f.select(Some(0), Some(0))?;
                sel.set_text_node_range(
                    &f,
                    start,
                    &s,
                    end.unwrap_or_else(|| s.get_text_content_size()),
                )?;
                assert!(get_selection().unwrap().ptr_eq(&sel));
                sel
            }
        };
        sel.remove_text()?;
        assert!(get_selection().unwrap().ptr_eq(&sel));
        assert_eq!(f.is_attached(), case.first_attached);
        assert_eq!(s.is_attached(), case.second_attached);
        let all = get_root().get_all_text_nodes();
        assert_eq!(texts_of(&all), case.texts);
        let selection = range(&current());
        assert!(selection.is_collapsed());
        let (key, offset) = match case.caret {
            Caret::Paragraph => (p.get_key().to_owned(), 0),
            Caret::First(offset) => (
                f.get_key().to_owned(),
                offset.unwrap_or_else(|| f.get_text_content_size()),
            ),
            Caret::Second => (s.get_key().to_owned(), 0),
            Caret::Remaining => (all[0].get_key().to_owned(), 0),
        };
        assert_eq!(selection.anchor.key, key);
        assert_eq!(selection.anchor.offset, offset);
        Ok(())
    });
    if let Fixture::TrailingSegmented = case.fixture
        && let Sel::Range(4, Some(10)) = case.select
    {
        editor
            .read_with_mode(crate::EditorReadMode::Latest, || {
                // These get merged in reconciliation.
                assert_eq!(texts_of(&get_root().get_all_text_nodes()), ["leadtext"]);
                assert!(first.is_attached());
            })
            .unwrap();
    }
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove all text
#[test]
fn remove_text_trailing_token_remove_all_text() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingToken,
        select: Sel::Range(0, None),
        first_attached: false,
        second_attached: false,
        texts: &[],
        caret: Caret::Paragraph,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove initial TextNode
#[test]
fn remove_text_trailing_token_remove_initial_text_node() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingToken,
        select: Sel::First,
        first_attached: false,
        second_attached: true,
        texts: &["token text"],
        caret: Caret::Second,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove trailing token TextNode
#[test]
fn remove_text_trailing_token_remove_trailing_token_text_node() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingToken,
        select: Sel::Second,
        first_attached: true,
        second_attached: false,
        texts: &["leading text"],
        caret: Caret::First(None),
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove initial TextNode and partial token TextNode
#[test]
fn remove_text_trailing_token_remove_initial_and_partial_token() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingToken,
        select: Sel::Range(0, Some("token ".len())),
        first_attached: false,
        // Expecting no node since it was token.
        second_attached: false,
        texts: &[],
        caret: Caret::Paragraph,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove partial initial TextNode and partial token TextNode
#[test]
fn remove_text_trailing_token_remove_partial_initial_and_partial_token() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingToken,
        select: Sel::Range("lead".len(), Some("token ".len())),
        first_attached: true,
        second_attached: false,
        // The token node is completely removed.
        texts: &["lead"],
        caret: Caret::First(Some("lead".len())),
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove all text
#[test]
fn remove_text_leading_token_remove_all_text() {
    remove_text(RemoveTextCase {
        fixture: Fixture::LeadingToken,
        select: Sel::Range(0, None),
        first_attached: false,
        second_attached: false,
        texts: &[],
        caret: Caret::Paragraph,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove trailing TextNode
#[test]
fn remove_text_leading_token_remove_trailing_text_node() {
    remove_text(RemoveTextCase {
        fixture: Fixture::LeadingToken,
        select: Sel::Second,
        first_attached: true,
        second_attached: false,
        texts: &["token text"],
        caret: Caret::First(None),
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove leading token TextNode
#[test]
fn remove_text_leading_token_remove_leading_token_text_node() {
    remove_text(RemoveTextCase {
        fixture: Fixture::LeadingToken,
        select: Sel::First,
        first_attached: false,
        second_attached: true,
        texts: &["trailing text"],
        caret: Caret::Second,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove partial leading token TextNode and trailing TextNode
#[test]
fn remove_text_leading_token_remove_partial_token_and_trailing() {
    remove_text(RemoveTextCase {
        fixture: Fixture::LeadingToken,
        select: Sel::Range("token ".len(), None),
        // Expecting no node since it was token.
        first_attached: false,
        second_attached: false,
        texts: &[],
        caret: Caret::Paragraph,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove partial token TextNode and partial trailing TextNode
#[test]
fn remove_text_leading_token_remove_partial_token_and_partial_trailing() {
    remove_text(RemoveTextCase {
        fixture: Fixture::LeadingToken,
        select: Sel::Range("token ".len(), Some("trail".len())),
        first_attached: false,
        second_attached: true,
        // The token node is completely removed.
        texts: &["ing text"],
        caret: Caret::Second,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove all text
#[test]
fn remove_text_trailing_segmented_remove_all_text() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingSegmented,
        select: Sel::Range(0, None),
        first_attached: false,
        second_attached: false,
        texts: &[],
        caret: Caret::Paragraph,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove initial TextNode
#[test]
fn remove_text_trailing_segmented_remove_initial_text_node() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingSegmented,
        select: Sel::First,
        first_attached: false,
        second_attached: true,
        texts: &["segmented text"],
        caret: Caret::Second,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove trailing segmented TextNode
#[test]
fn remove_text_trailing_segmented_remove_trailing_segmented_text_node() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingSegmented,
        select: Sel::Second,
        first_attached: true,
        second_attached: false,
        texts: &["leading text"],
        caret: Caret::First(None),
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove initial TextNode and partial segmented TextNode
#[test]
fn remove_text_trailing_segmented_remove_initial_and_partial_segmented() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingSegmented,
        select: Sel::Range(0, Some("segmented ".len())),
        first_attached: false,
        // Expecting a new node since it was segmented.
        second_attached: false,
        texts: &["text"],
        caret: Caret::Remaining,
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove partial initial TextNode and partial segmented TextNode
#[test]
fn remove_text_trailing_segmented_remove_partial_initial_and_partial_segmented() {
    remove_text(RemoveTextCase {
        fixture: Fixture::TrailingSegmented,
        select: Sel::Range("lead".len(), Some("segmented ".len())),
        first_attached: true,
        second_attached: false,
        // These get merged in reconciliation.
        texts: &["lead", "text"],
        caret: Caret::First(Some("lead".len())),
    });
}

// Segmented node composition (#5065)

fn segmented(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.set_mode("segmented").unwrap();
    node
}

fn note_state() -> crate::StateConfig<String> {
    crate::create_state("note", |v| v.as_str().unwrap_or_default().to_owned())
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText during composition preserves node key
#[test]
fn insert_text_during_composition_preserves_node_key() {
    let editor = build();
    update(&editor, || {
        let segmented = segmented("JohnSmith");
        get_root().clear()?;
        get_root().append(&[paragraph_of(std::slice::from_ref(&segmented))])?;
        let key = segmented.get_key().to_owned();
        set_composition_key(Some(&key));
        segmented.select(Some(4), Some(4))?.insert_text("")?;
        assert_eq!(segmented.get_key(), key);
        assert!(!segmented.is_segmented());
        assert_eq!(segmented.get_text_content(), "JohnSmith");
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText without composition replaces segmented node
#[test]
fn insert_text_without_composition_replaces_segmented_node() {
    let editor = build();
    update(&editor, || {
        let segmented = segmented("JohnSmith");
        get_root().clear()?;
        get_root().append(&[paragraph_of(std::slice::from_ref(&segmented))])?;
        segmented.select(Some(4), Some(4))?.insert_text("")?;
        assert!(!segmented.is_attached());
        let all = get_root().get_all_text_nodes();
        assert_eq!(all.len(), 1);
        assert_ne!(all[0].get_key(), segmented.get_key());
        assert_eq!(all[0].get_text_content(), "JohnSmith");
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText without composition types into a segmented node
#[test]
fn insert_text_without_composition_types_into_a_segmented_node() {
    let editor = build();
    update(&editor, || {
        let segmented = segmented("JohnSmith");
        get_root().clear()?;
        get_root().append(&[paragraph_of(std::slice::from_ref(&segmented))])?;
        segmented.select(Some(4), Some(4))?.insert_text("X")?;
        assert!(!segmented.is_attached());
        let all = get_root().get_all_text_nodes();
        assert_eq!(all.len(), 1);
        assert!(!all[0].is_segmented());
        assert_eq!(all[0].get_text_content(), "JohnXSmith");
        let selection = range(&current());
        assert!(selection.is_collapsed());
        assert_eq!(selection.anchor.key, all[0].get_key());
        assert_eq!(selection.anchor.offset, 5);
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText without composition keeps the NodeState and detail of the segmented node
#[test]
fn insert_text_without_composition_keeps_the_node_state_and_detail_of_the_segmented_node() {
    let note = note_state();
    let editor = build();
    update(&editor, move || {
        let segmented = segmented("JohnSmith");
        segmented.toggle_unmergeable();
        crate::set_state(&segmented, &note, "c1".to_owned())?;
        get_root().clear()?;
        get_root().append(&[paragraph_of(std::slice::from_ref(&segmented))])?;
        segmented.select(Some(4), Some(4))?.insert_text("X")?;
        let all = get_root().get_all_text_nodes();
        assert_eq!(all.len(), 1);
        assert!(!all[0].is_segmented());
        assert_eq!(all[0].get_text_content(), "JohnXSmith");
        assert_eq!(crate::get_state(&all[0], &note)?, "c1");
        assert!(all[0].is_unmergeable());
        Ok(())
    });
    editor.dispose();
}

/// `ReplacementTextNode` (`test-replacement-text`): a `TextNode` subclass.
#[derive(Clone, Debug, Default)]
struct ReplacementTextNode {
    base: crate::NodeBase,
    text: crate::TextData,
}

impl crate::LexicalNode for ReplacementTextNode {
    fn get_type(&self) -> &'static str {
        "test-replacement-text"
    }
    fn base(&self) -> &crate::NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut crate::NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&crate::TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut crate::TextData> {
        Some(&mut self.text)
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > a TextNode replacement keeps the NodeState it sets, and takes the rest
#[test]
fn a_text_node_replacement_keeps_the_node_state_it_sets_and_takes_the_rest() {
    let note = note_state();
    let other = crate::create_state("other", |v| v.as_str().unwrap_or_default().to_owned());
    let n = note.clone();
    let editor = crate::upstream_tests::support::create_test_editor(crate::CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<ReplacementTextNode>(),
            crate::LexicalNodeReplacement {
                replace: NodeClass::of::<crate::nodes::text::TextNode>(),
                with: Rc::new(move |node| {
                    let replacement = crate::create::<ReplacementTextNode>();
                    replacement.set_text_content(&node.get_text_content());
                    crate::set_state(&replacement, &n, "replacement".to_owned())
                }),
                with_klass: Some(NodeClass::of::<ReplacementTextNode>()),
            }
            .into(),
        ],
        ..crate::CreateEditorArgs::default()
    });
    update(&editor, move || {
        let segmented = crate::create::<ReplacementTextNode>();
        segmented.set_text_content("JohnSmith");
        segmented.set_mode("segmented")?;
        crate::set_state(&segmented, &note, "c1".to_owned())?;
        crate::set_state(&segmented, &other, "o1".to_owned())?;
        get_root().clear()?;
        get_root().append(&[paragraph_of(std::slice::from_ref(&segmented))])?;
        segmented.select(Some(4), Some(4))?.insert_text("X")?;
        let text = get_root().get_all_text_nodes()[0].clone();
        assert_eq!(text.get_text_content(), "JohnXSmith");
        assert_eq!(crate::get_state(&text, &note)?, "replacement");
        assert_eq!(crate::get_state(&text, &other)?, "o1");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > removing text from inside a segmented node to the next block keeps its NodeState and detail
#[test]
fn removing_text_from_inside_a_segmented_node_to_the_next_block_keeps_its_node_state_and_detail() {
    let note = note_state();
    let editor = build();
    update(&editor, move || {
        let segmented = segmented("$AB");
        segmented.toggle_unmergeable();
        crate::set_state(&segmented, &note, "c1".to_owned())?;
        let next = create_text_node("z");
        get_root().clear()?;
        get_root().append(&[
            paragraph_of(std::slice::from_ref(&segmented)),
            paragraph_of(std::slice::from_ref(&next)),
        ])?;
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, segmented.get_key(), 2, PointType::Text)?;
        selection.point_set(Which::Focus, next.get_key(), 0, PointType::Text)?;
        set_selection(Some(selection.clone()));
        selection.remove_text()?;
        let nodes = get_root().get_all_text_nodes();
        assert_eq!(get_root().get_text_content(), "$Az");
        let kept = nodes
            .iter()
            .find(|n| n.get_text_content().starts_with("$A"))
            .expect("the kept node");
        assert_eq!(crate::get_state(kept, &note)?, "c1");
        assert!(kept.is_unmergeable());
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText during composition preserves format and style
#[test]
fn insert_text_during_composition_preserves_format_and_style() {
    let editor = build();
    update(&editor, || {
        let segmented = segmented("JohnSmith");
        segmented.set_format_type("bold");
        get_root().clear()?;
        get_root().append(&[paragraph_of(std::slice::from_ref(&segmented))])?;
        let key = segmented.get_key().to_owned();
        set_composition_key(Some(&key));
        let sel = segmented.select(Some(4), Some(4))?;
        sel.set_format(1.0); // bold
        sel.insert_text("")?;
        assert!(segmented.is_attached());
        assert_eq!(segmented.get_key(), key);
        assert_eq!(segmented.text_data().get_format(), 1.0);
        Ok(())
    });
    editor.dispose();
}

// Non-collapsed selection + composition preserves node identity

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Non-collapsed selection + composition preserves node identity > spliceText into existing node instead of creating new one
#[test]
fn splice_text_into_existing_node_instead_of_creating_new_one() {
    let editor = build();
    update(&editor, || {
        let text_node = create_text_node("Hello World");
        get_root().clear()?;
        get_root().append(&[paragraph_of(std::slice::from_ref(&text_node))])?;
        let key = text_node.get_key().to_owned();
        set_composition_key(Some(&key));
        text_node.select(Some(5), Some(11))?.insert_text("!")?;
        assert_eq!(text_node.get_key(), key);
        assert_eq!(text_node.get_text_content(), "Hello!");
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Non-collapsed selection + composition preserves node identity > without composition creates a new node
#[test]
fn without_composition_creates_a_new_node() {
    let editor = build();
    update(&editor, || {
        let text_node = create_text_node("Hello World");
        get_root().clear()?;
        get_root().append(&[paragraph_of(std::slice::from_ref(&text_node))])?;
        let key = text_node.get_key().to_owned();
        text_node.select(Some(5), Some(11))?.insert_text("!")?;
        let all = get_root().get_all_text_nodes();
        assert_eq!(texts_of(&all), ["Hello", "!"]);
        assert_ne!(all[1].get_key(), key);
        Ok(())
    });
    editor.dispose();
}

// Regression tests for #6701

/// `InlineElementNode` (`inline-element-node`).
#[derive(Clone, Debug, Default)]
struct InlineElementNode {
    base: crate::NodeBase,
    element: crate::ElementData,
}

impl crate::LexicalNode for InlineElementNode {
    fn get_type(&self) -> &'static str {
        "inline-element-node"
    }
    fn base(&self) -> &crate::NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut crate::NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&crate::ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut crate::ElementData> {
        Some(&mut self.element)
    }
    fn is_inline(&self) -> bool {
        true
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #6701 > insertNodes fails an invariant when there is no Block ancestor
#[test]
fn insert_nodes_fails_an_invariant_when_there_is_no_block_ancestor() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        nodes: vec![NodeClass::of::<InlineElementNode>()],
        ..LexicalExtension::<(), (), ()>::new("@test/6701", ())
    })
    .into()])
    .unwrap();
    let result = editor.update(
        || {
            let text_node = create_text_node("test");
            let inline = crate::create::<InlineElementNode>();
            inline.append(std::slice::from_ref(&text_node))?;
            get_root().clear()?;
            get_root().append(&[inline])?;
            text_node
                .select(None, None)?
                .insert_nodes(vec![create_text_node("more text")])
        },
        crate::UpdateOptions {
            discrete: true,
            ..crate::UpdateOptions::default()
        },
    );
    let message = result.expect_err("an invariant").message().to_owned();
    assert!(
        message
            .contains("Expected node TextNode of type text to have a block ElementNode ancestor"),
        "{message}"
    );
    editor.dispose();
}

// Regression tests for #8707

/// Seeds the root with a shadow root of `children`, runs `select` on it,
/// then inserts with `insert` at the selection, in a second update.
fn in_shadow_root(
    editor: &crate::Editor,
    children: fn() -> Vec<NodeRef>,
    offset: usize,
    insert: fn(&SelectionRef) -> Result<(), LexicalError>,
) {
    update(editor, move || {
        let shadow = create_test_shadow_root_node();
        shadow.append(&children())?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&shadow))?;
        // Block cursor: a collapsed element point on the shadow root.
        shadow.select(Some(offset), Some(offset))?;
        Ok(())
    });
    update(editor, move || insert(&current()));
}

fn insert_block_decorator(selection: &SelectionRef) -> Result<(), LexicalError> {
    selection.insert_nodes(vec![block_decorator()])
}

fn shadow_children() -> Vec<NodeRef> {
    let shadow = get_root().get_first_child_or_throw().unwrap();
    assert_eq!(
        shadow.get_type(),
        "test_shadow_root",
        "Expected shadow root"
    );
    shadow.get_children()
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block decorator after the block cursor at the end of a shadow root
#[test]
fn inserts_a_block_decorator_after_the_block_cursor_at_the_end_of_a_shadow_root() {
    let editor = build();
    in_shadow_root(
        &editor,
        || vec![block_decorator()],
        1,
        insert_block_decorator,
    );
    editor
        .read(|| {
            // The decorator landed in the same shadow root as the block
            // cursor, not the outer document root.
            assert_eq!(get_root().get_children_size(), 1);
            let children = shadow_children();
            assert_eq!(children.len(), 2);
            assert!(children.iter().all(NodeRef::is_decorator));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block decorator before the block cursor at the start of a shadow root
#[test]
fn inserts_a_block_decorator_before_the_block_cursor_at_the_start_of_a_shadow_root() {
    let editor = build();
    let existing_key = crate::upstream_tests::support::update_with(&editor, || {
        let shadow = create_test_shadow_root_node();
        let existing = block_decorator();
        shadow.append(std::slice::from_ref(&existing))?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&shadow))?;
        // Block cursor: collapsed element point before the decorator.
        shadow.select(Some(0), Some(0))?;
        Ok(existing.get_key().to_owned())
    });
    update(&editor, || insert_block_decorator(&current()));
    editor
        .read(|| {
            let children = shadow_children();
            assert_eq!(children.len(), 2);
            assert!(children.iter().all(NodeRef::is_decorator));
            // Inserted before the pre-existing decorator.
            assert_eq!(children[1].get_key(), existing_key);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block decorator into an empty shadow root
#[test]
fn inserts_a_block_decorator_into_an_empty_shadow_root() {
    let editor = build();
    in_shadow_root(&editor, Vec::new, 0, insert_block_decorator);
    editor
        .read(|| {
            let children = shadow_children();
            assert_eq!(children.len(), 1);
            assert!(children[0].is_decorator());
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block element at the block cursor inside a shadow root
#[test]
fn inserts_a_block_element_at_the_block_cursor_inside_a_shadow_root() {
    let editor = build();
    in_shadow_root(
        &editor,
        || vec![block_decorator()],
        1,
        |selection| selection.insert_nodes(vec![paragraph_of(&[create_text_node("inserted")])]),
    );
    editor
        .read(|| {
            let children = shadow_children();
            assert_eq!(children.len(), 2);
            assert!(children[0].is_decorator());
            assert_eq!(children[1].get_type(), "paragraph");
            assert_eq!(children[1].get_text_content(), "inserted");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > insertParagraph at an element point on a shadow root seeds into that shadow root
#[test]
fn insert_paragraph_at_an_element_point_on_a_shadow_root_seeds_into_that_shadow_root() {
    let editor = build();
    in_shadow_root(
        &editor,
        || vec![block_decorator()],
        1,
        |selection| {
            let paragraph = selection
                .insert_paragraph()?
                .expect("Expected a paragraph to be inserted");
            assert_eq!(
                paragraph.get_parent().map(|p| p.get_key().to_owned()),
                get_root().get_first_child().map(|p| p.get_key().to_owned())
            );
            Ok(())
        },
    );
    editor
        .read(|| {
            assert_eq!(get_root().get_children_size(), 1);
            let children = shadow_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[1].get_type(), "paragraph");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block decorator at a root element point without wrapping it in a paragraph
#[test]
fn inserts_a_block_decorator_at_a_root_element_point_without_wrapping_it_in_a_paragraph() {
    let editor = build();
    update(&editor, || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[create_text_node("existing")])])?;
        // Element point directly on the root, after the paragraph.
        get_root().select(Some(1), Some(1))?;
        Ok(())
    });
    update(&editor, || insert_block_decorator(&current()));
    editor
        .read(|| {
            let children = get_root().get_children();
            // The decorator is a direct child of root; no empty paragraph
            // wrapper was created for it.
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].get_type(), "paragraph");
            assert!(children[1].is_decorator());
        })
        .unwrap();
    editor.dispose();
}

// Regression tests for #8075

fn forward_delete(editor: &crate::Editor) {
    update(editor, || current().delete_character(false));
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8075 > forward delete removes an empty block before a shadow root
#[test]
fn forward_delete_removes_an_empty_block_before_a_shadow_root() {
    let editor = build();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let shadow = create_test_shadow_root_node();
        shadow.append(&[paragraph_of(&[create_text_node("inside")])])?;
        get_root().clear()?;
        get_root().append(&[paragraph.clone(), shadow])?;
        paragraph.select_start()?;
        Ok(())
    });
    forward_delete(&editor);
    editor
        .read(|| {
            let children = get_root().get_children();
            // The empty paragraph is gone and the shadow root is untouched.
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].get_type(), "test_shadow_root");
            assert_eq!(children[0].get_text_content(), "inside");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8075 > forward delete keeps a non-empty block before a shadow root
#[test]
fn forward_delete_keeps_a_non_empty_block_before_a_shadow_root() {
    let editor = build();
    update(&editor, || {
        let paragraph = paragraph_of(&[create_text_node("before")]);
        let shadow = create_test_shadow_root_node();
        shadow.append(&[paragraph_of(&[create_text_node("inside")])])?;
        get_root().clear()?;
        get_root().append(&[paragraph.clone(), shadow])?;
        paragraph.select_end()?;
        Ok(())
    });
    forward_delete(&editor);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].get_text_content(), "before");
            assert_eq!(children[1].get_type(), "test_shadow_root");
            assert_eq!(children[1].get_text_content(), "inside");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8075 > forward delete from an empty block at the end of a shadow root does nothing
#[test]
fn forward_delete_from_an_empty_block_at_the_end_of_a_shadow_root_does_nothing() {
    let editor = build();
    update(&editor, || {
        let inner = create_paragraph_node();
        let first = create_test_shadow_root_node();
        first.append(std::slice::from_ref(&inner))?;
        let second = create_test_shadow_root_node();
        second.append(&[paragraph_of(&[create_text_node("next")])])?;
        get_root().clear()?;
        get_root().append(&[first, second])?;
        inner.select_start()?;
        Ok(())
    });
    forward_delete(&editor);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].get_type(), "test_shadow_root");
            // The empty block still belongs to the first shadow root; the
            // following shadow root was not pulled into it.
            assert_eq!(children[0].get_children_size(), 1);
            assert_eq!(children[0].get_text_content(), "");
            assert_eq!(children[1].get_text_content(), "next");
        })
        .unwrap();
    editor.dispose();
}

// Regression tests for #9095

/// `insertBlockTestExtension`: `selectionTestExtension` and `QuoteNode`.
fn build_insert_block() -> LexicalEditorWithDispose {
    selection_test_extension(
        "@test/selection-insert-block",
        vec![NodeClass::of::<crate::rich_text::QuoteNode>()],
    )
}

/// `setupParagraph(editor, text, caretOffset)`.
fn setup_paragraph(editor: &crate::Editor, text: &'static str, caret_offset: usize) {
    update(editor, move || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[create_text_node(text)])])?;
        // `$placeCaretInFirstText(offset)`.
        let paragraph = get_root().get_first_child_or_throw()?;
        assert_eq!(paragraph.get_type(), "paragraph", "Expected a paragraph");
        let text = paragraph.get_first_child_or_throw()?;
        assert!(text.is_text(), "Expected a text node");
        text.select(Some(caret_offset), Some(caret_offset))?;
        Ok(())
    });
}

/// `insertBlocks(editor, $makeNodes)`.
fn insert_blocks(editor: &crate::Editor, make_nodes: fn() -> Vec<NodeRef>) {
    update(editor, move || current().insert_nodes(make_nodes()));
}

/// `$makeList()`.
fn make_list() -> Vec<NodeRef> {
    let item = create_list_item_node(None);
    item.append(&[create_text_node("item")]).unwrap();
    let list = create_list_node(ListType::Bullet, 1.0);
    list.append(&[item]).unwrap();
    vec![list]
}

/// Each root child as `paragraph:<text>`, `decorator` or `list:<text>`.
fn blocks(editor: &crate::Editor) -> Vec<String> {
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|c| {
                    if c.is_decorator() {
                        "decorator".to_owned()
                    } else if is_list_node(c) {
                        format!("list:{}", c.get_text_content())
                    } else {
                        format!("{}:{}", c.get_type(), c.get_text_content())
                    }
                })
                .collect()
        })
        .unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a block decorator at the end of a paragraph leaves no empty paragraph
#[test]
fn pasting_a_block_decorator_at_the_end_of_a_paragraph_leaves_no_empty_paragraph() {
    let editor = build_insert_block();
    setup_paragraph(&editor, "hello", 5);
    insert_blocks(&editor, || vec![block_decorator()]);
    assert_eq!(blocks(&editor), ["paragraph:hello", "decorator"]);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a block decorator in the middle of a paragraph keeps the trailing text
#[test]
fn pasting_a_block_decorator_in_the_middle_of_a_paragraph_keeps_the_trailing_text() {
    let editor = build_insert_block();
    setup_paragraph(&editor, "helloworld", 5);
    insert_blocks(&editor, || vec![block_decorator()]);
    assert_eq!(
        blocks(&editor),
        ["paragraph:hello", "decorator", "paragraph:world"]
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a block decorator at the start of a paragraph keeps the paragraph after it
#[test]
fn pasting_a_block_decorator_at_the_start_of_a_paragraph_keeps_the_paragraph_after_it() {
    let editor = build_insert_block();
    setup_paragraph(&editor, "hello", 0);
    insert_blocks(&editor, || vec![block_decorator()]);
    assert_eq!(blocks(&editor), ["decorator", "paragraph:hello"]);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a list at the end of a paragraph leaves no empty paragraph
#[test]
fn pasting_a_list_at_the_end_of_a_paragraph_leaves_no_empty_paragraph() {
    let editor = build_insert_block();
    setup_paragraph(&editor, "hello", 5);
    insert_blocks(&editor, make_list);
    assert_eq!(blocks(&editor), ["paragraph:hello", "list:item"]);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a list in the middle of a paragraph keeps the trailing text
#[test]
fn pasting_a_list_in_the_middle_of_a_paragraph_keeps_the_trailing_text() {
    let editor = build_insert_block();
    setup_paragraph(&editor, "helloworld", 5);
    insert_blocks(&editor, make_list);
    assert_eq!(
        blocks(&editor),
        ["paragraph:hello", "list:item", "paragraph:world"]
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a list at the start of a paragraph keeps the paragraph after it
#[test]
fn pasting_a_list_at_the_start_of_a_paragraph_keeps_the_paragraph_after_it() {
    let editor = build_insert_block();
    setup_paragraph(&editor, "hello", 0);
    insert_blocks(&editor, make_list);
    assert_eq!(blocks(&editor), ["list:item", "paragraph:hello"]);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a quote at the end of a paragraph merges into it (control)
#[test]
fn pasting_a_quote_at_the_end_of_a_paragraph_merges_into_it() {
    let editor = build_insert_block();
    setup_paragraph(&editor, "hello", 5);
    insert_blocks(&editor, || {
        let quote = crate::rich_text::create_quote_node(false);
        quote.append(&[create_text_node("quoted")]).unwrap();
        vec![quote]
    });
    assert_eq!(blocks(&editor), ["paragraph:helloquoted"]);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a paragraph at the end of a paragraph merges into it (control)
#[test]
fn pasting_a_paragraph_at_the_end_of_a_paragraph_merges_into_it() {
    let editor = build_insert_block();
    setup_paragraph(&editor, "hello", 5);
    insert_blocks(&editor, || vec![paragraph_of(&[create_text_node("world")])]);
    assert_eq!(blocks(&editor), ["paragraph:helloworld"]);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > typing after pasting a block decorator at the end creates a paragraph on demand
#[test]
fn typing_after_pasting_a_block_decorator_at_the_end_creates_a_paragraph_on_demand() {
    let editor = build_insert_block();
    setup_paragraph(&editor, "hello", 5);
    insert_blocks(&editor, || vec![block_decorator()]);
    update(&editor, || current().insert_text("x"));
    assert_eq!(
        blocks(&editor),
        ["paragraph:hello", "decorator", "paragraph:x"]
    );
    editor.dispose();
}

// getNodes() and extract()

/// The `getNodes() and extract()` document's nodes.
#[derive(Clone)]
struct Doc {
    paragraph_node: NodeRef,
    paragraph_text: NodeRef,
    link_node: NodeRef,
    link_text: NodeRef,
    list_node: NodeRef,
    list_item_text1: NodeRef,
    list_item_text2: NodeRef,
    list_item1: NodeRef,
    list_item2: NodeRef,
    empty_paragraph: NodeRef,
}

impl Doc {
    /// The document order every select-all case expects, before
    /// `empty_paragraph`.
    fn up_to_list(&self) -> Vec<NodeRef> {
        vec![
            self.paragraph_text.clone(),
            self.link_node.clone(),
            self.link_text.clone(),
            // The parent paragraphNode comes after its children because the
            // selection started inside of it at paragraphText.
            self.paragraph_node.clone(),
            self.list_node.clone(),
            self.list_item1.clone(),
            self.list_item_text1.clone(),
            self.list_item2.clone(),
            self.list_item_text2.clone(),
        ]
    }

    fn all(&self) -> Vec<NodeRef> {
        let mut nodes = self.up_to_list();
        nodes.push(self.empty_paragraph.clone());
        nodes
    }
}

/// The `beforeEach`: the editor and its document.
fn get_nodes_setup() -> (LexicalEditorWithDispose, Doc) {
    let editor = build();
    let doc = crate::upstream_tests::support::update_with(&editor, || {
        let doc = Doc {
            paragraph_text: create_text_node("paragraph text"),
            link_text: create_text_node("link text"),
            link_node: create_link_node("", &LinkAttributes::default()),
            paragraph_node: create_paragraph_node(),
            list_item_text1: create_text_node("item 1"),
            list_item_text2: create_text_node("item 2"),
            list_item1: create_list_item_node(None),
            list_item2: create_list_item_node(None),
            list_node: create_list_node(ListType::Bullet, 1.0),
            empty_paragraph: create_paragraph_node(),
        };
        doc.link_node.append(std::slice::from_ref(&doc.link_text))?;
        doc.paragraph_node
            .append(&[doc.paragraph_text.clone(), doc.link_node.clone()])?;
        doc.list_item1
            .append(std::slice::from_ref(&doc.list_item_text1))?;
        doc.list_item2
            .append(std::slice::from_ref(&doc.list_item_text2))?;
        doc.list_node
            .append(&[doc.list_item1.clone(), doc.list_item2.clone()])?;
        get_root().clear()?;
        get_root().append(&[
            doc.paragraph_node.clone(),
            doc.list_node.clone(),
            doc.empty_paragraph.clone(),
        ])?;
        Ok(doc)
    });
    (editor, doc)
}

fn keys(nodes: &[NodeRef]) -> Vec<String> {
    nodes.iter().map(|n| n.get_key().to_owned()).collect()
}

/// `expect(selection).toMatchObject({anchor, focus})`.
fn expect_points(
    selection: &SelectionRef,
    anchor: (&NodeRef, usize, PointType),
    focus: (&NodeRef, usize, PointType),
) {
    let r = range(selection);
    for (point, (node, offset, point_type)) in [(&r.anchor, anchor), (&r.focus, focus)] {
        assert_eq!(point.key, node.get_key());
        assert_eq!(point.offset, offset);
        assert_eq!(point.point_type, point_type);
    }
}

/// Runs `f` with the document in a discrete update.
fn with_doc(f: impl FnOnce(&Doc) -> Result<(), LexicalError> + 'static) {
    let (editor, doc) = get_nodes_setup();
    update(&editor, move || f(&doc));
    editor.dispose();
}

fn root_node() -> NodeRef {
    get_root()
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with test document
#[test]
fn get_nodes_select_all_with_test_document() {
    with_doc(|d| {
        let selection = crate::select_all()?;
        // Normalized to the text nodes.
        expect_points(
            &selection,
            (&d.paragraph_text, 0, PointType::Text),
            (&d.empty_paragraph, 0, PointType::Element),
        );
        assert_eq!(keys(&selection.get_nodes()?), keys(&d.all()));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with leading inline decorator
#[test]
fn get_nodes_select_all_with_leading_inline_decorator() {
    with_doc(|d| {
        let leading = create_test_blockable_decorator_node();
        d.paragraph_node
            .splice(0, 0, std::slice::from_ref(&leading))?;
        let selection = crate::select_all()?;
        expect_points(
            &selection,
            (&d.paragraph_node, 0, PointType::Element),
            (&d.empty_paragraph, 0, PointType::Element),
        );
        let mut want = vec![leading];
        want.extend(d.all());
        assert_eq!(keys(&selection.get_nodes()?), keys(&want));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with trailing inline decorator
#[test]
fn get_nodes_select_all_with_trailing_inline_decorator() {
    with_doc(|d| {
        let trailing = create_test_blockable_decorator_node();
        d.empty_paragraph
            .splice(0, 0, std::slice::from_ref(&trailing))?;
        let selection = crate::select_all()?;
        expect_points(
            &selection,
            (&d.paragraph_text, 0, PointType::Text),
            (&d.empty_paragraph, 1, PointType::Element),
        );
        let mut want = d.all();
        want.push(trailing);
        assert_eq!(keys(&selection.get_nodes()?), keys(&want));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with leading empty inline element
#[test]
fn get_nodes_select_all_with_leading_empty_inline_element() {
    with_doc(|d| {
        let leading = crate::upstream_tests::support::create_test_inline_element_node();
        d.paragraph_node
            .splice(0, 0, std::slice::from_ref(&leading))?;
        let selection = crate::select_all()?;
        expect_points(
            &selection,
            (&leading, 0, PointType::Element),
            (&d.empty_paragraph, 0, PointType::Element),
        );
        let mut want = vec![leading];
        want.extend(d.all());
        assert_eq!(keys(&selection.get_nodes()?), keys(&want));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with trailing empty inline element
#[test]
fn get_nodes_select_all_with_trailing_empty_inline_element() {
    with_doc(|d| {
        let trailing = crate::upstream_tests::support::create_test_inline_element_node();
        d.empty_paragraph
            .splice(0, 0, std::slice::from_ref(&trailing))?;
        let selection = crate::select_all()?;
        expect_points(
            &selection,
            (&d.paragraph_text, 0, PointType::Text),
            (&trailing, 0, PointType::Element),
        );
        let mut want = d.all();
        want.push(trailing);
        assert_eq!(keys(&selection.get_nodes()?), keys(&want));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > after removing empty paragraph
#[test]
fn get_nodes_select_all_after_removing_empty_paragraph() {
    with_doc(|d| {
        d.empty_paragraph.remove(false)?;
        let selection = crate::select_all()?;
        expect_points(
            &selection,
            (&d.paragraph_text, 0, PointType::Text),
            (
                &d.list_item_text2,
                d.list_item_text2.get_text_content_size(),
                PointType::Text,
            ),
        );
        assert_eq!(keys(&selection.get_nodes()?), keys(&d.up_to_list()));
        Ok(())
    });
}

/// A range selection, not set as the editor's, between two points.
fn range_between(
    anchor: (&str, usize, PointType),
    focus: (&str, usize, PointType),
) -> Result<SelectionRef, LexicalError> {
    let selection = create_range_selection();
    selection.point_set(Which::Anchor, anchor.0, anchor.1, anchor.2)?;
    selection.point_set(Which::Focus, focus.0, focus.1, focus.2)?;
    Ok(selection)
}

fn select_all_manually() -> Result<SelectionRef, LexicalError> {
    range_between(
        ("root", 0, PointType::Element),
        ("root", root_node().get_children_size(), PointType::Element),
    )
}

fn first_text_to_empty_paragraph(d: &Doc) -> Result<SelectionRef, LexicalError> {
    range_between(
        (d.paragraph_text.get_key(), 0, PointType::Text),
        (d.empty_paragraph.get_key(), 0, PointType::Element),
    )
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select all without normalization
#[test]
fn get_nodes_manual_select_all_without_normalization() {
    with_doc(|d| {
        let selection = select_all_manually()?;
        assert_eq!(keys(&selection.get_nodes()?), keys(&d.all()));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select all from first text to last empty paragraph
#[test]
fn get_nodes_manual_select_all_from_first_text_to_last_empty_paragraph() {
    with_doc(|d| {
        let selection = first_text_to_empty_paragraph(d)?;
        assert_eq!(keys(&selection.get_nodes()?), keys(&d.all()));
        Ok(())
    });
}

/// Puts two inline decorators in the empty paragraph.
fn two_decorators(d: &Doc) -> Result<(NodeRef, NodeRef), LexicalError> {
    let leading = create_test_blockable_decorator_node();
    let trailing = create_test_blockable_decorator_node();
    d.empty_paragraph
        .splice(0, 0, &[leading.clone(), trailing.clone()])?;
    Ok((leading, trailing))
}

fn collapsed_in_empty_paragraph(d: &Doc, offset: usize) -> Result<(), LexicalError> {
    let (_, trailing) = two_decorators(d)?;
    let p = d.empty_paragraph.get_key();
    let selection = range_between(
        (p, offset, PointType::Element),
        (p, offset, PointType::Element),
    )?;
    assert!(range(&selection).is_collapsed());
    expect_points(
        &selection,
        (&d.empty_paragraph, offset, PointType::Element),
        (&d.empty_paragraph, offset, PointType::Element),
    );
    // The bias is towards the right, or the last descendant when nothing
    // is to the right.
    assert_eq!(keys(&selection.get_nodes()?), keys(&[trailing]));
    Ok(())
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select with focus collapsed between inline decorators
#[test]
fn get_nodes_manual_select_with_focus_collapsed_between_inline_decorators() {
    with_doc(|d| collapsed_in_empty_paragraph(d, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select with focus collapsed after inline decorator
#[test]
fn get_nodes_manual_select_with_focus_collapsed_after_inline_decorator() {
    with_doc(|d| collapsed_in_empty_paragraph(d, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select with focus between inline decorators
#[test]
fn get_nodes_manual_select_with_focus_between_inline_decorators() {
    with_doc(|d| {
        let (leading, _) = two_decorators(d)?;
        let selection = range_between(
            (d.paragraph_text.get_key(), 0, PointType::Text),
            (d.empty_paragraph.get_key(), 1, PointType::Element),
        )?;
        expect_points(
            &selection,
            (&d.paragraph_text, 0, PointType::Text),
            (&d.empty_paragraph, 1, PointType::Element),
        );
        let mut want = d.all();
        want.push(leading);
        assert_eq!(keys(&selection.get_nodes()?), keys(&want));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > select only the paragraph (not normalized)
#[test]
fn get_nodes_select_only_the_paragraph() {
    with_doc(|d| {
        let size = d.paragraph_node.get_children_size();
        let selection = d.paragraph_node.select(Some(0), Some(size))?;
        expect_points(
            &selection,
            (&d.paragraph_node, 0, PointType::Element),
            (&d.paragraph_node, size, PointType::Element),
        );
        // The selection doesn't visit outside of the paragraph.
        assert_eq!(
            keys(&selection.get_nodes()?),
            keys(&[
                d.paragraph_text.clone(),
                d.link_node.clone(),
                d.link_text.clone()
            ])
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > select around the paragraph (not normalized)
#[test]
fn get_nodes_select_around_the_paragraph() {
    with_doc(|d| {
        let index = d.paragraph_node.get_index_within_parent().unwrap();
        let selection = range_between(
            ("root", index, PointType::Element),
            ("root", index + 1, PointType::Element),
        )?;
        let root = root_node();
        expect_points(
            &selection,
            (&root, 0, PointType::Element),
            (&root, 1, PointType::Element),
        );
        // The selection shouldn't visit outside of the paragraph.
        assert_eq!(
            keys(&selection.get_nodes()?),
            keys(&[
                d.paragraph_text.clone(),
                d.link_node.clone(),
                d.link_text.clone(),
                d.paragraph_node.clone(),
            ])
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > selection collapsed inside an empty element
#[test]
fn get_nodes_selection_collapsed_inside_an_empty_element() {
    with_doc(|d| {
        let p = d.empty_paragraph.get_key();
        let selection = range_between((p, 0, PointType::Element), (p, 0, PointType::Element))?;
        // The selection includes the node it is collapsed inside.
        assert_eq!(
            keys(&selection.get_nodes()?),
            keys(std::slice::from_ref(&d.empty_paragraph))
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > select an empty ListItemNode (collapsed)
#[test]
fn get_nodes_select_an_empty_list_item_node() {
    with_doc(|d| {
        let empty_list_item = create_list_item_node(None);
        d.list_item2.insert_before(&empty_list_item, true)?;
        let k = empty_list_item.get_key();
        let selection = range_between((k, 0, PointType::Element), (k, 0, PointType::Element))?;
        expect_points(
            &selection,
            (&empty_list_item, 0, PointType::Element),
            (&empty_list_item, 0, PointType::Element),
        );
        assert_eq!(keys(&selection.get_nodes()?), keys(&[empty_list_item]));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > Manual select all without normalization
#[test]
fn extract_manual_select_all_without_normalization() {
    with_doc(|d| {
        let selection = select_all_manually()?;
        let extracted = selection.extract()?;
        assert_eq!(keys(&extracted), keys(&d.all()));
        assert_eq!(keys(&selection.get_nodes()?), keys(&extracted));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > Manual select all from first text to last empty paragraph
#[test]
fn extract_manual_select_all_from_first_text_to_last_empty_paragraph() {
    with_doc(|d| {
        let selection = first_text_to_empty_paragraph(d)?;
        let extracted = selection.extract()?;
        assert_eq!(keys(&extracted), keys(&d.all()));
        assert_eq!(keys(&selection.get_nodes()?), keys(&extracted));
        Ok(())
    });
}

/// `extract()` of `select(start, end)` on a text node: one text node with
/// `want`, which `getNodes()` then returns.
fn extract_partial(text: fn(&Doc) -> NodeRef, start: usize, end: usize, want: &'static str) {
    with_doc(move |d| {
        let selection = text(d).select(Some(start), Some(end))?;
        let extracted = selection.extract()?;
        assert_eq!(extracted.len(), 1);
        assert!(extracted[0].is_text());
        assert_eq!(extracted[0].get_text_content(), want);
        assert_eq!(keys(&selection.get_nodes()?), keys(&extracted));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select partial TextNode extracts paragraph text
#[test]
fn extract_select_partial_text_node_extracts_paragraph_text() {
    extract_partial(|d| d.paragraph_text.clone(), 2, 8, "ragrap");
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select partial TextNode extracts link text
#[test]
fn extract_select_partial_text_node_extracts_link_text() {
    extract_partial(|d| d.link_text.clone(), 1, 4, "ink");
}

/// `[__text, key]` of the nodes: text nodes by their text (JS's
/// `objectContaining({__text})`), others by key.
fn described(nodes: &[NodeRef]) -> Vec<String> {
    nodes
        .iter()
        .map(|n| {
            if n.is_text() {
                format!("text:{}", n.get_text_content())
            } else {
                format!("key:{}", n.get_key())
            }
        })
        .collect()
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select multiple partial TextNode extracts text
#[test]
fn extract_select_multiple_partial_text_node_extracts_text() {
    with_doc(|d| {
        let selection = create_range_selection();
        selection.set_text_node_range(&d.paragraph_text, 10, &d.link_text, 4)?;
        let extracted = selection.extract()?;
        assert_eq!(
            described(&extracted),
            [
                "text:text".to_owned(),
                format!("key:{}", d.link_node.get_key()),
                "text:link".to_owned()
            ]
        );
        assert_eq!(keys(&selection.get_nodes()?), keys(&extracted));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select last offset TextNode as first node removes node
#[test]
fn extract_select_last_offset_text_node_as_first_node_removes_node() {
    with_doc(|d| {
        let selection = create_range_selection();
        selection.set_text_node_range(
            &d.paragraph_text,
            d.paragraph_text.get_text_content_size(),
            &d.link_text,
            4,
        )?;
        let before_nodes = selection.get_nodes()?;
        let extracted = selection.extract()?;
        assert_eq!(
            described(&extracted),
            [
                format!("key:{}", d.link_node.get_key()),
                "text:link".to_owned()
            ]
        );
        // The identity of the linkText does not change since the first
        // node is re-used.
        assert_eq!(keys(&selection.get_nodes()?), keys(&before_nodes));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select 0 offset TextNode as last node removes node
#[test]
fn extract_select_0_offset_text_node_as_last_node_removes_node() {
    with_doc(|d| {
        let selection = create_range_selection();
        selection.set_text_node_range(&d.paragraph_text, 4, &d.link_text, 0)?;
        let before_nodes = selection.get_nodes()?;
        assert_eq!(
            described(&selection.extract()?),
            [
                "text:graph text".to_owned(),
                format!("key:{}", d.link_node.get_key())
            ]
        );
        // The identity is not paragraphText anymore because that is the
        // left side outside of the extraction.
        let mut want = vec![d.paragraph_text.get_next_sibling().unwrap()];
        want.extend(before_nodes[1..].iter().cloned());
        assert_eq!(keys(&selection.get_nodes()?), keys(&want));
        Ok(())
    });
}

/// A discrete update of a fresh `selectionTestExtension` editor.
fn in_fresh_editor(f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    let editor = build();
    update(&editor, f);
    editor.dispose();
}

fn is_bold(node: &NodeRef) -> bool {
    assert!(node.is_text());
    crate::nodes::text::to_int32(node.text_data().get_format()) & crate::constants::IS_BOLD != 0
}

fn bold_text(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_format("bold");
    node
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression #7081 > Firefox selection & paste before linebreak
#[test]
fn firefox_selection_and_paste_before_linebreak() {
    in_fresh_editor(|| {
        let text_node = create_text_node("XXXX");
        text_node.set_style("color: --color-test");
        let paragraph_node = paragraph_of(&[
            create_text_node("ID: "),
            text_node.clone(),
            crate::nodes::line_break::create_line_break_node(),
            create_text_node("aa"),
        ]);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph_node))?;
        let selection = text_node.select(Some(0), None)?;
        selection.point_set(
            Which::Focus,
            paragraph_node.get_key(),
            1 + text_node.get_index_within_parent().unwrap(),
            PointType::Element,
        )?;
        selection.insert_text("123")?;
        let styled_child = paragraph_node
            .get_children()
            .into_iter()
            .find(|c| c.is_text() && c.text_data().get_style() == "color: --color-test")
            .expect("a styled child");
        assert_eq!(styled_child.get_text_content(), "123");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression #7173 > Can insertNodes of multiple blocks with a target of an initial empty block and the entire next block
#[test]
fn insert_nodes_of_multiple_blocks_over_an_initial_empty_block_and_the_next_block() {
    in_fresh_editor(|| {
        let empty_paragraph = create_paragraph_node();
        let replaced_paragraph = paragraph_of(&[create_text_node("replaced!")]);
        get_root().clear()?;
        get_root().append(&[empty_paragraph, replaced_paragraph])?;
        let selection = crate::select_all()?;
        let inserted_nodes = vec![
            paragraph_of(&[create_text_node("p1")]),
            paragraph_of(&[create_text_node("p2")]),
        ];
        selection.insert_nodes(inserted_nodes.clone())?;
        assert_eq!(keys(&get_root().get_children()), keys(&inserted_nodes));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression #3181 > Point.isBefore edge case with mixed TextNode & ElementNode and matching descendants
#[test]
fn point_is_before_edge_case_with_mixed_text_and_element_points() {
    in_fresh_editor(|| {
        let token = |text: &str| {
            let node = create_text_node(text);
            node.set_mode("token").unwrap();
            node
        };
        let target_text = token("target");
        let paragraph = paragraph_of(&[token("a"), token("b"), target_text.clone()]);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        let selection = paragraph.select(Some(2), Some(2))?;
        selection.point_set(Which::Focus, target_text.get_key(), 1, PointType::Text)?;
        expect_points(
            &selection,
            (&paragraph, 2, PointType::Element),
            (&target_text, 1, PointType::Text),
        );
        let r = range(&selection);
        let caret_range = crate::caret_range_from_selection(&r)?;
        assert_eq!(
            crate::compare_point_caret_next(
                &crate::get_caret_in_direction(&caret_range.anchor, crate::CaretDirection::Next)?,
                &crate::get_caret_in_direction(&caret_range.focus, crate::CaretDirection::Next)?,
            )?,
            std::cmp::Ordering::Less
        );
        assert!(r.anchor.is_before(&r.focus)?);
        assert!(!r.focus.is_before(&r.anchor)?);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression #8067 > Formatting issue when replacing text with format
#[test]
fn formatting_issue_when_replacing_text_with_format() {
    in_fresh_editor(|| {
        let paragraph = paragraph_of(&[bold_text("hello"), create_text_node(" world!")]);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        crate::select_all()?.insert_text("hello")?;
        let child = paragraph.get_children()[0].clone();
        assert_eq!(child.get_text_content(), "hello");
        assert!(is_bold(&child));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText with backward selection inherits first node format > backward selection across bold+plain inherits bold
#[test]
fn backward_selection_across_bold_and_plain_inherits_bold() {
    in_fresh_editor(|| {
        let bold_node = bold_text("hello");
        let plain_node = create_text_node(" world");
        let paragraph = paragraph_of(&[bold_node.clone(), plain_node.clone()]);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        // Backward: anchor at the end of plain, focus at the start of bold.
        let selection = plain_node.select(Some(6), Some(6))?;
        selection.set_text_node_range(&plain_node, 6, &bold_node, 0)?;
        selection.insert_text("X")?;
        let child = paragraph.get_children()[0].clone();
        assert_eq!(child.get_text_content(), "X");
        assert!(is_bold(&child));
        Ok(())
    });
}

fn token_text(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.set_mode("token").unwrap();
    node
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText needsRedirect paths > token node at middle offset replaces entire node
#[test]
fn token_node_at_middle_offset_replaces_entire_node() {
    in_fresh_editor(|| {
        let token = token_text("abcdef");
        let paragraph = paragraph_of(std::slice::from_ref(&token));
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        token.select(Some(3), Some(3))?.insert_text("X")?;
        let children = paragraph.get_children();
        assert_eq!(children.len(), 1);
        assert!(children[0].is_text());
        assert_eq!(children[0].get_text_content(), "X");
        assert!(!token.is_attached());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText needsRedirect paths > offset 0 on token reuses insertable previous sibling
#[test]
fn offset_0_on_token_reuses_insertable_previous_sibling() {
    in_fresh_editor(|| {
        let prev = create_text_node("before");
        let token = token_text("locked");
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[prev.clone(), token.clone()])])?;
        token.select(Some(0), Some(0))?;
        current().insert_text("X")?;
        assert_eq!(prev.get_text_content(), "beforeX");
        assert!(token.is_attached());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText needsRedirect paths > offset at end of token reuses insertable next sibling
#[test]
fn offset_at_end_of_token_reuses_insertable_next_sibling() {
    in_fresh_editor(|| {
        let token = token_text("locked");
        let next = create_text_node("after");
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[token.clone(), next.clone()])])?;
        token.select(Some(6), Some(6))?;
        current().insert_text("X")?;
        assert_eq!(next.get_text_content(), "Xafter");
        assert!(token.is_attached());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText formatDiffers on empty text node > applies format in-place on empty anchor then splices text
#[test]
fn applies_format_in_place_on_empty_anchor_then_splices_text() {
    in_fresh_editor(|| {
        let empty = create_text_node("");
        let paragraph = paragraph_of(std::slice::from_ref(&empty));
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        let sel = empty.select(Some(0), Some(0))?;
        sel.set_format(f64::from(crate::constants::IS_BOLD));
        sel.insert_text("typed")?;
        assert!(empty.is_attached());
        assert_eq!(empty.get_text_content(), "typed");
        assert!(is_bold(&empty));
        assert_eq!(paragraph.get_children_size(), 1);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::RangeSelection.isBackward() caching (#5825) > caches the result and invalidates on Point mutations
#[test]
fn is_backward_caches_the_result_and_invalidates_on_point_mutations() {
    in_fresh_editor(|| {
        let root = get_root();
        root.clear()?;
        let text = create_text_node("hello");
        root.append(&[paragraph_of(std::slice::from_ref(&text))])?;
        let selection = text.select(Some(0), Some(5))?;
        let cached = || range(&selection).cached_is_backward;
        assert_eq!(cached(), None);
        // The first call computes and caches.
        assert!(!selection.is_backward()?);
        assert_eq!(cached(), Some(false));
        // Hits the cache.
        assert!(!selection.is_backward()?);
        // setTextNodeRange sets both points, invalidating the cache.
        selection.set_text_node_range(&text, 5, &text, 0)?;
        assert_eq!(cached(), None);
        assert!(selection.is_backward()?);
        assert_eq!(cached(), Some(true));
        Ok(())
    });
}

// $wrapInlineNodes regression

fn child_types(node: &NodeRef) -> Vec<&'static str> {
    node.get_children().iter().map(NodeRef::get_type).collect()
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$wrapInlineNodes regression > Wraps all inline nodes, preserving first linebreak if contain a block element
#[test]
fn wraps_all_inline_nodes_preserving_first_linebreak_if_contain_a_block_element() {
    in_fresh_editor(|| {
        get_root().clear()?;
        let selection = crate::select_all()?;
        let bold = create_text_node("p1");
        bold.set_format_type("bold");
        let inline_nodes = vec![
            crate::nodes::line_break::create_line_break_node(),
            create_text_node("p1"),
            bold,
        ];
        let mut nodes = inline_nodes.clone();
        nodes.push(create_paragraph_node());
        selection.insert_nodes(nodes)?;
        let children = get_root().get_children();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].get_type(), "paragraph");
        assert_eq!(keys(&children[0].get_children()), keys(&inline_nodes));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$wrapInlineNodes regression > Collapses a lone linebreak run into an empty paragraph at the end of a non-empty paragraph
#[test]
fn collapses_a_lone_linebreak_run_into_an_empty_paragraph_at_the_end_of_a_non_empty_paragraph() {
    in_fresh_editor(|| {
        let root = get_root();
        root.clear()?;
        let paragraph = paragraph_of(&[create_text_node("abc")]);
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?.insert_nodes(vec![
            crate::nodes::line_break::create_line_break_node(),
            paragraph_of(&[create_text_node("x")]),
        ])?;
        let children = root.get_children();
        assert_eq!(children.len(), 3);
        assert!(children.iter().all(|c| c.get_type() == "paragraph"));
        assert_eq!(children[0].get_text_content(), "abc");
        assert_eq!(children[1].get_children_size(), 0);
        assert_eq!(children[2].get_text_content(), "x");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$wrapInlineNodes regression > Preserves a linebreak followed by inline content when merging into a non-empty paragraph
#[test]
fn preserves_a_linebreak_followed_by_inline_content_when_merging_into_a_non_empty_paragraph() {
    in_fresh_editor(|| {
        let root = get_root();
        root.clear()?;
        let paragraph = paragraph_of(&[create_text_node("abc")]);
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?.insert_nodes(vec![
            crate::nodes::line_break::create_line_break_node(),
            create_text_node("tail"),
            paragraph_of(&[create_text_node("x")]),
        ])?;
        let children = root.get_children();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].get_type(), "paragraph");
        assert_eq!(child_types(&children[0]), ["text", "linebreak", "text"]);
        assert_eq!(children[0].get_text_content(), "abc\ntail");
        assert_eq!(children[1].get_type(), "paragraph");
        assert_eq!(children[1].get_text_content(), "x");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$wrapInlineNodes regression > Collapses a lone trailing linebreak after a block into an empty paragraph
#[test]
fn collapses_a_lone_trailing_linebreak_after_a_block_into_an_empty_paragraph() {
    in_fresh_editor(|| {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?.insert_nodes(vec![
            paragraph_of(&[create_text_node("x")]),
            crate::nodes::line_break::create_line_break_node(),
        ])?;
        let children = root.get_children();
        assert_eq!(children.len(), 2);
        assert!(children.iter().all(|c| c.get_type() == "paragraph"));
        assert_eq!(children[0].get_text_content(), "x");
        assert_eq!(children[1].get_children_size(), 0);
        Ok(())
    });
}

// $formatText toggle direction (#6935) and $setTextFormat (#5518)

/// A paragraph of `texts`, each bold where its flag says, selected with
/// `$selectAll()`; runs `f` on the selection, then returns each child's
/// bold and italic.
fn format_all(
    texts: &'static [(&'static str, bool)],
    f: fn(&SelectionRef) -> Result<(), LexicalError>,
    want: &'static [(bool, bool)],
) {
    in_fresh_editor(move || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        for &(text, bold) in texts {
            let node = create_text_node(text);
            if bold {
                node.set_format(f64::from(crate::constants::IS_BOLD));
            }
            paragraph.append(&[node])?;
        }
        root.append(std::slice::from_ref(&paragraph))?;
        let selection = crate::select_all()?;
        f(&selection)?;
        let got: Vec<(bool, bool)> = paragraph
            .get_children()
            .iter()
            .map(|c| {
                let format = crate::nodes::text::to_int32(c.text_data().get_format());
                (
                    format & crate::constants::IS_BOLD != 0,
                    format & crate::constants::IS_ITALIC != 0,
                )
            })
            .collect();
        assert_eq!(got, want);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$formatText toggle direction (#6935) > uses selection.format (AND) instead of first node for toggle direction
#[test]
fn format_text_uses_selection_format_instead_of_first_node_for_toggle_direction() {
    format_all(
        &[("bold", true), ("plain", false)],
        |selection| {
            assert!(!selection.has_format("bold"));
            selection.format_text("bold", None)
        },
        &[(true, false), (true, false)],
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$formatText toggle direction (#6935) > toggling off works when all nodes share the format
#[test]
fn format_text_toggling_off_works_when_all_nodes_share_the_format() {
    format_all(
        &[("aaa", true), ("bbb", true)],
        |selection| {
            // onSelectionChange computes this as the AND of the text
            // nodes; set it as the test does.
            selection.set_format(f64::from(crate::constants::IS_BOLD));
            assert!(selection.has_format("bold"));
            selection.format_text("bold", None)
        },
        &[(false, false), (false, false)],
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$formatText toggle direction (#6935) > explicit alignWithFormat bypasses selection.format reference
#[test]
fn format_text_explicit_align_with_format_bypasses_selection_format() {
    format_all(
        &[("bold", true), ("plain", false)],
        |selection| selection.format_text("bold", Some(f64::from(crate::constants::IS_BOLD))),
        &[(true, false), (true, false)],
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$setTextFormat (#5518) > sets bold to true on mixed formatting
#[test]
fn set_text_format_sets_bold_to_true_on_mixed_formatting() {
    format_all(
        &[("bold", true), ("plain", false)],
        |selection| selection.set_text_format(&[("bold", true)]),
        &[(true, false), (true, false)],
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$setTextFormat (#5518) > sets bold to false on all-bold text
#[test]
fn set_text_format_sets_bold_to_false_on_all_bold_text() {
    format_all(
        &[("aaa", true), ("bbb", true)],
        |selection| {
            selection.set_format(f64::from(crate::constants::IS_BOLD));
            selection.set_text_format(&[("bold", false)])
        },
        &[(false, false), (false, false)],
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::$setTextFormat (#5518) > sets multiple formats at once
#[test]
fn set_text_format_sets_multiple_formats_at_once() {
    format_all(
        &[("hello", false)],
        |selection| selection.set_text_format(&[("bold", true), ("italic", true)]),
        &[(true, true)],
    );
}

// insertNodes() selection

/// `$pasteAtStartAndDescribeSelection(blocks)`.
fn paste_at_start_and_describe_selection(blocks: Vec<NodeRef>) -> Result<String, LexicalError> {
    let text = create_text_node("asdf");
    get_root().clear()?;
    get_root().append(&[paragraph_of(std::slice::from_ref(&text))])?;
    text.select(Some(0), Some(0))?;
    current().insert_nodes(blocks)?;
    let after = range(&current());
    assert!(after.is_collapsed());
    Ok(format!(
        "{}:{:?}@{}",
        after.anchor.point_type.as_str(),
        after.anchor.get_node()?.get_text_content(),
        after.anchor.offset
    ))
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::insertNodes() selection > leaves the caret where pasted content ending in an empty block joins
#[test]
fn insert_nodes_leaves_the_caret_where_content_ending_in_an_empty_block_joins() {
    in_fresh_editor(|| {
        let description = paste_at_start_and_describe_selection(vec![
            paragraph_of(&[create_text_node("AA")]),
            paragraph_of(&[create_text_node("BB")]),
            create_paragraph_node(),
        ])?;
        assert_eq!(texts_of(&get_root().get_children()), ["AA", "BB", "asdf"]);
        assert_eq!(description, r#"text:"asdf"@0"#);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelection.test.ts::insertNodes() selection > leaves the caret where pasted content ending in a text block joins
#[test]
fn insert_nodes_leaves_the_caret_where_content_ending_in_a_text_block_joins() {
    in_fresh_editor(|| {
        let description = paste_at_start_and_describe_selection(vec![
            paragraph_of(&[create_text_node("AA")]),
            paragraph_of(&[create_text_node("BB")]),
        ])?;
        assert_eq!(texts_of(&get_root().get_children()), ["AA", "BBasdf"]);
        assert_eq!(description, r#"text:"BB"@2"#);
        Ok(())
    });
}
