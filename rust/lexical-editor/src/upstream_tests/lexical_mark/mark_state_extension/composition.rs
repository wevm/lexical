//! "composition over a selection".
//!
//! `startComposition` dispatches `COMPOSITION_START_COMMAND` with a
//! `timeStamp` that never counts as just after a keydown, so JS's
//! `$handleCompositionStart` prepares a composition target only where its
//! other conditions hold; [`dispatch_composition_start`] does that part,
//! which the port's core leaves out.

use super::*;
use crate::{
    COMMAND_PRIORITY_HIGH, COMPOSITION_START_COMMAND, get_selection, set_composition_key, set_slot,
};

/// `startComposition(editor)`.
fn start_composition(editor: &Editor) {
    let ed = editor.clone();
    update(editor, move || dispatch_composition_start(&ed));
    assert!(editor.is_composing());
}

/// The UTF-16 code unit at `at` of `text` (`text[at]`).
fn unit_at(text: &str, at: usize) -> Option<u16> {
    text.encode_utf16().nth(at)
}

const ZWSP: u16 = 0x200B;
const NBSP: u16 = 0x00A0;

/// `compose(editor, composed)`: the input method writes `composed` at the
/// caret, over the placeholder.
fn compose(editor: &Editor, composed: &'static str) {
    update(editor, move || {
        let selection = get_selection()
            .filter(SelectionRef::is_range)
            .expect("no selection");
        let anchor = selection.range().unwrap().anchor;
        let target = anchor.get_node()?;
        let content = target.get_text_content();
        let mut at = anchor.offset;
        if unit_at(&content, at) != Some(ZWSP) && at > 0 && unit_at(&content, at - 1) == Some(ZWSP)
        {
            at -= 1;
        }
        let delete = if unit_at(&content, at) == Some(ZWSP) {
            1
        } else {
            0
        };
        target.splice_text(at as i64, delete, composed, true)?;
        Ok(())
    });
}

/// `setupMarked(text, marks, [start, end])`.
fn setup_marked(
    text: &'static str,
    marks: &[(usize, usize, &str)],
    (start, end): (usize, usize),
) -> LexicalEditorWithDispose {
    let editor = create_test_editor();
    update(&editor, move || init_with(text));
    for (mark_start, mark_end, id) in marks {
        mark(&editor, *mark_start, *mark_end, id);
    }
    run(&editor, move || {
        select(start, end);
    });
    editor
}

/// label, text, marked, replaced, composed, result
type ReplacingCase = (
    &'static str,
    &'static str,
    (usize, usize),
    (usize, usize),
    &'static str,
    &'static str,
);

const REPLACING: [ReplacingCase; 4] = [
    (
        "the end of a run, by longer text",
        "abcd",
        (0, 4),
        (2, 4),
        "cde",
        r##"paragraph("abcde"[c1])"##,
    ),
    (
        "the end of a run followed by plain text, by longer text",
        "abcd xy",
        (0, 4),
        (2, 4),
        "cde",
        r##"paragraph("abcde"[c1] " xy")"##,
    ),
    (
        "a word inside a run",
        "x teh y",
        (0, 7),
        (2, 5),
        "the",
        r##"paragraph("x the y"[c1])"##,
    ),
    (
        "text only partly in the run",
        "abcd xy",
        (0, 4),
        (2, 6),
        "zz",
        r##"paragraph("ab"[c1] "zzy")"##,
    ),
];

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing the end of a run, by longer text gives the text the ids all of the replaced text had
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing the end of a run followed by plain text, by longer text gives the text the ids all of the replaced text had
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing a word inside a run gives the text the ids all of the replaced text had
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing text only partly in the run gives the text the ids all of the replaced text had
#[test]
fn replacing_gives_the_text_the_ids_all_of_the_replaced_text_had() {
    for (label, text, (mark_start, mark_end), replaced, composed, result) in REPLACING {
        let editor = setup_marked(text, &[(mark_start, mark_end, "c1")], replaced);
        start_composition(&editor);
        compose(&editor, composed);
        run(&editor, || set_composition_key(None));
        assert_eq!((label, tree(&editor)), (label, result.to_owned()));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Where {
    Children,
    Slot,
}

/// The block (`$paragraph()`) or its `title` slot.
fn slotted_parent(place: Where) -> NodeRef {
    let block = paragraph();
    match place {
        Where::Slot => get_slot(&block, "title").unwrap(),
        Where::Children => block,
    }
}

/// `setupSlotted(where)`: "abcd xy" with c1 on "abcd", in the block
/// (`where`) the test names.
fn setup_slotted(place: Where) -> LexicalEditorWithDispose {
    let editor = create_test_editor();
    update(&editor, move || {
        let title = create_paragraph_node();
        title.append(&[create_text_node(if place == Where::Slot {
            "abcd xy"
        } else {
            "Title"
        })])?;
        let block = crate::create::<TitledParagraphNode>();
        block.append(&[create_text_node(if place == Where::Slot {
            "Body"
        } else {
            "abcd xy"
        })])?;
        set_slot(&block, "title", &title)?;
        let root = get_root();
        root.clear()?;
        root.append(&[block])
    });
    // The text node holding "abcd xy" (a host's getAllTextNodes lists its
    // slots' text first, so it is picked directly)
    let target = move || slotted_parent(place).get_first_child_or_throw().unwrap();
    update(&editor, move || {
        add_mark_id("c1", &target().select(Some(0), Some(4))?)?;
        Ok(())
    });
    update(&editor, move || {
        target().select(Some(2), Some(4)).map(|_| ())
    });
    editor
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > in a block with a named slot > replacing the end of a run in its children gives the text the ids all of the replaced text had
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > in a block with a named slot > replacing the end of a run in its slot gives the text the ids all of the replaced text had
#[test]
fn replacing_the_end_of_a_run_in_a_block_with_a_named_slot() {
    for place in [Where::Children, Where::Slot] {
        let editor = setup_slotted(place);
        start_composition(&editor);
        compose(&editor, "cde");
        run(&editor, || set_composition_key(None));
        let marked = read(&editor, move || {
            slotted_parent(place)
                .get_children()
                .iter()
                .filter(|node| get_node_mark_ids(node).unwrap().iter().any(|id| id == "c1"))
                .map(NodeRef::get_text_content)
                .collect::<String>()
        });
        assert_eq!((place, marked.as_str()), (place, "abcde"));
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing the end of a run inside an inline element that adds text of its own gives the text the ids all of the replaced text had
#[test]
fn replacing_the_end_of_a_run_inside_an_inline_element_that_adds_text_of_its_own() {
    let editor = create_test_editor();
    update(&editor, || {
        let inline = crate::create::<BracketedInlineNode>();
        inline.append(&[create_text_node("abcd xy")])?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[inline])?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    });
    let text = || get_root().get_all_text_nodes()[0].clone();
    update(&editor, move || {
        add_mark_id("c1", &text().select(Some(0), Some(4))?)?;
        Ok(())
    });
    update(&editor, move || text().select(Some(2), Some(4)).map(|_| ()));
    start_composition(&editor);
    compose(&editor, "cde");
    run(&editor, || set_composition_key(None));
    let marked = read(&editor, || {
        get_root()
            .get_all_text_nodes()
            .iter()
            .filter(|node| get_node_mark_ids(node).unwrap().iter().any(|id| id == "c1"))
            .map(NodeRef::get_text_content)
            .collect::<String>()
    });
    assert_eq!(marked, "abcde");
}

/// `composeOverPlaceholder(editor, composed)`: composes `composed` over
/// the placeholder at the caret, if there is one.
fn compose_over_placeholder(editor: &Editor, composed: &'static str) {
    update(editor, move || {
        let selection = get_selection()
            .filter(SelectionRef::is_range)
            .expect("no selection");
        let anchor = selection.range().unwrap().anchor;
        let target = anchor.get_node()?;
        let content = target.get_text_content();
        let at = anchor.offset;
        let before = if at > 0 {
            unit_at(&content, at - 1)
        } else {
            None
        };
        if before == Some(ZWSP) || before == Some(NBSP) {
            target.splice_text(at as i64 - 1, 1, composed, true)?;
        } else {
            let delete = if unit_at(&content, at) == Some(ZWSP) {
                1
            } else {
                0
            };
            target.splice_text(at as i64, delete, composed, true)?;
        }
        Ok(())
    });
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection replaced by nothing (a start that removes the selection) at the composition start gives the composed text the ids all of it had
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection replaced by a zero-width space at the composition start gives the composed text the ids all of it had
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection replaced by a no-break space at the composition start gives the composed text the ids all of it had
#[test]
fn a_selection_replaced_at_the_composition_start_gives_the_composed_text_the_ids_all_of_it_had() {
    for (label, placeholder) in [
        ("nothing (a start that removes the selection)", ""),
        ("a zero-width space", "\u{200B}"),
        ("a no-break space", "\u{00A0}"),
    ] {
        let editor = setup_marked("abcd", &[(0, 4, "c1")], (2, 4));
        let _unregister = editor.register_command(
            &CONTROLLED_TEXT_INSERTION_COMMAND,
            move |payload, _| {
                let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
                    return Ok(false);
                };
                if payload != "\u{200B}" || placeholder == "\u{200B}" {
                    return Ok(false);
                }
                if placeholder.is_empty() {
                    selection.remove_text()?;
                } else {
                    selection.insert_text(placeholder)?;
                }
                Ok(true)
            },
            COMMAND_PRIORITY_HIGH,
        );
        start_composition(&editor);
        compose_over_placeholder(&editor, "cde");
        run(&editor, || set_composition_key(None));
        // A no-break space the composition kept is not the point here: the
        // composed text and the text before it have the run's id
        assert_eq!(
            (label, tree(&editor).replace('\u{00A0}', "")),
            (label, r#"paragraph("abcde"[c1])"#.to_owned())
        );
    }
}

/// `collapseOnCompositionStart(editor)`: collapses the selection to its
/// focus when a composition starts.
fn collapse_on_composition_start(editor: &Editor) {
    let _unregister = editor.register_command(
        &COMPOSITION_START_COMMAND,
        |_, _| {
            if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
                let focus = selection.range().unwrap().focus;
                selection.point_set(Which::Anchor, &focus.key, focus.offset, focus.point_type)?;
            }
            Ok(false)
        },
        COMMAND_PRIORITY_HIGH,
    );
}

/// The ids of the text node whose text includes "X", or `None`.
fn composed_ids(editor: &Editor) -> Option<Vec<String>> {
    read(editor, || {
        get_root()
            .get_all_text_nodes()
            .into_iter()
            .find(|text| text.get_text_content().contains('X'))
            .map(|node| get_node_mark_ids(&node).unwrap())
    })
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection of an inline decorator without text that a handler kept was not replaced
#[test]
fn a_selection_of_an_inline_decorator_without_text_that_a_handler_kept_was_not_replaced() {
    let editor = create_test_editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[
            create_text_node("ab"),
            crate::create::<EmptyInlineDecoratorNode>(),
            create_text_node("cd"),
        ])?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    });
    let select_decorator = || {
        let key = paragraph().get_key().to_owned();
        let selection = create_range_selection();
        selection
            .point_set(Which::Anchor, &key, 1, PointType::Element)
            .unwrap();
        selection
            .point_set(Which::Focus, &key, 2, PointType::Element)
            .unwrap();
        set_selection(Some(selection.clone()));
        selection
    };
    update(&editor, move || {
        add_mark_id("c1", &select_decorator())?;
        Ok(())
    });
    run(&editor, move || {
        select_decorator();
    });
    collapse_on_composition_start(&editor);
    start_composition(&editor);
    compose(&editor, "X");
    run(&editor, || set_composition_key(None));
    assert_eq!(composed_ids(&editor), Some(Vec::new()));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection that is a no-break space, kept by a start that wrote nothing, was not replaced
#[test]
fn a_selection_that_is_a_no_break_space_kept_by_a_start_that_wrote_nothing_was_not_replaced() {
    let editor = setup_marked("a\u{00A0}b", &[(0, 3, "c1"), (1, 2, "c2")], (1, 2));
    collapse_on_composition_start(&editor);
    // The start writes nothing (as a browser that composes in place)
    let _unregister = editor.register_command(
        &CONTROLLED_TEXT_INSERTION_COMMAND,
        |payload, _| Ok(payload == "\u{200B}"),
        COMMAND_PRIORITY_HIGH,
    );
    start_composition(&editor);
    compose(&editor, "X");
    run(&editor, || set_composition_key(None));
    assert_eq!(composed_ids(&editor), Some(vec!["c1".to_owned()]));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection that a handler collapsed before the composition started was not replaced
#[test]
fn a_selection_that_a_handler_collapsed_before_the_composition_started_was_not_replaced() {
    let editor = setup_marked("abcd", &[(0, 4, "c1")], (2, 4));
    collapse_on_composition_start(&editor);
    start_composition(&editor);
    compose(&editor, "X");
    run(&editor, || set_composition_key(None));
    assert_eq!(tree(&editor), r##"paragraph("abcd"[c1] "X")"##);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection that a handler of the composition placeholder kept was not replaced
#[test]
fn a_selection_that_a_handler_of_the_composition_placeholder_kept_was_not_replaced() {
    let editor = setup_marked("x teh y", &[(0, 7, "c1"), (2, 4, "c2")], (2, 5));
    // Quotes the selection instead of replacing it with the placeholder
    let _unregister = editor.register_command(
        &CONTROLLED_TEXT_INSERTION_COMMAND,
        |payload, _| {
            let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
                return Ok(false);
            };
            let range = selection.range().unwrap();
            if payload != "\u{200B}" || range.is_collapsed() {
                return Ok(false);
            }
            let (start, end) = if selection.is_backward()? {
                (range.focus.clone(), range.anchor.clone())
            } else {
                (range.anchor.clone(), range.focus.clone())
            };
            let start_node = start.get_node()?;
            let end_node = end.get_node()?;
            let (start_at, end_at) = (start.offset, end.offset);
            end_node.splice_text(end_at as i64, 0, "\"", false)?;
            start_node.splice_text(start_at as i64, 0, "\"", false)?;
            let offset = end_at
                + if start_node.get_key() == end_node.get_key() {
                    2
                } else {
                    1
                };
            end_node.select(Some(offset), Some(offset))?;
            Ok(true)
        },
        COMMAND_PRIORITY_HIGH,
    );
    start_composition(&editor);
    run(&editor, || set_composition_key(None));
    assert_eq!(
        tree(&editor),
        r##"paragraph("x \""[c1] "te"[c1,c2] "h\" y"[c1])"##
    );
}
