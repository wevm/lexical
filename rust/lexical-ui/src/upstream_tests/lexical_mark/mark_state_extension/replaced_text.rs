//! "replaced and re-created text".
//!
//! Rust's `CONTROLLED_TEXT_INSERTION_COMMAND` takes the text. The
//! autocorrection that upstream dispatches as an `insertReplacementText`
//! InputEvent payload of it is dispatched as that `beforeinput` event
//! (`BEFORE_INPUT_COMMAND`), which the core turns into the command with the
//! event's text, in the same update. JS forgets the replaced selection at
//! the next microtask (`await Promise.resolve()`); the port forgets it once
//! the editor has another pending state (rust/PARITY.md), so the awaits
//! have no counterpart.

use super::*;
use crate::mark::remove_mark_id;
use crate::{
    BEFORE_INPUT_COMMAND, COLLABORATION_TAG, COMMAND_PRIORITY_HIGH, COMPOSITION_START_COMMAND,
    CompositionEvent, DELETE_CHARACTER_COMMAND, InputEvent, PASTE_COMMAND, add_update_tag,
    get_selection, set_composition_key,
};

#[derive(Clone, Copy)]
enum Payload {
    Text(&'static str),
    /// `replacement(text)`: an autocorrection.
    Replacement(&'static str),
}

/// Dispatches the insertion of `payload`.
fn dispatch_insertion(editor: &Editor, payload: Payload) -> Result<(), LexicalError> {
    match payload {
        Payload::Text(text) => {
            editor.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, text.to_owned())?;
        }
        Payload::Replacement(text) => {
            let mut event = InputEvent::new("insertReplacementText");
            event.data_transfer = Some(data_transfer(&[("text/plain", text)]));
            editor.dispatch_command(&BEFORE_INPUT_COMMAND, Rc::new(event))?;
        }
    }
    Ok(())
}

/// `replaceText(withMarkNode, text, [markStart, markEnd], [start, end],
/// payload, sameUpdate)`: marks `[markStart, markEnd)` of `text` with c1,
/// as MarkState ids or as a MarkNode, then replaces `[start, end)` with
/// `payload`. The selection is committed first, as when the user selects
/// the text, or set in the same update, as a beforeinput event sets it
/// from its target range.
fn replace_text(
    with_mark_node: bool,
    text: &'static str,
    (mark_start, mark_end): (usize, usize),
    (start, end): (usize, usize),
    payload: Payload,
    same_update: bool,
) -> String {
    let editor = create_test_editor();
    update(&editor, move || init_with(text));
    run(&editor, move || {
        let selection = select(mark_start, mark_end);
        if with_mark_node {
            wrap_selection_in_mark_node(&selection, false, "c1", None).unwrap();
        } else {
            add_mark_id("c1", &selection).unwrap();
        }
    });
    if !same_update {
        run(&editor, move || {
            select(start, end);
        });
    }
    let ed = (*editor).clone();
    update(&editor, move || {
        if same_update {
            select(start, end);
        }
        dispatch_insertion(&ed, payload)
    });
    tree(&editor)
}

type ReplaceCase = (
    &'static str,
    &'static str,
    (usize, usize),
    (usize, usize),
    Payload,
    &'static str,
    &'static str,
);

/// label, text, marked, replaced, inserted, MarkState, MarkNode
const REPLACE_CASES: [ReplaceCase; 7] = [
    (
        "a word inside a run, by its first letter",
        "x teh y",
        (0, 7),
        (2, 5),
        Payload::Text("t"),
        r##"paragraph("x t y"[c1])"##,
        r##"paragraph(mark[c1]("x t y"))"##,
    ),
    (
        "a word inside a run, by its last letter",
        "x teh y",
        (0, 7),
        (2, 5),
        Payload::Text("h"),
        r##"paragraph("x h y"[c1])"##,
        r##"paragraph(mark[c1]("x h y"))"##,
    ),
    (
        "a word inside a run, by the corrected word",
        "x teh y",
        (0, 7),
        (2, 5),
        Payload::Text("the"),
        r##"paragraph("x the y"[c1])"##,
        r##"paragraph(mark[c1]("x the y"))"##,
    ),
    (
        "a word inside a run, by an autocorrection",
        "x teh y",
        (0, 7),
        (2, 5),
        Payload::Replacement("the"),
        r##"paragraph("x the y"[c1])"##,
        r##"paragraph(mark[c1]("x the y"))"##,
    ),
    // MarkNode loses the mark when all of its text is replaced (the empty
    // MarkNode is removed); the ids are kept, so retyping a commented word
    // keeps the comment
    (
        "a whole run, by its first letter",
        "a grew b",
        (2, 6),
        (2, 6),
        Payload::Text("g"),
        r##"paragraph("a " "g"[c1] " b")"##,
        r##"paragraph("a g b")"##,
    ),
    (
        "a whole run, by an autocorrection",
        "a recieve b",
        (2, 9),
        (2, 9),
        Payload::Replacement("receive"),
        r##"paragraph("a " "receive"[c1] " b")"##,
        r##"paragraph("a receive b")"##,
    ),
    (
        "across the start of a run, by text that starts with the same letter",
        "abcde",
        (2, 5),
        (1, 3),
        Payload::Text("bX"),
        r##"paragraph("abX" "de"[c1])"##,
        r##"paragraph("abX" mark[c1]("de"))"##,
    ),
];

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a word inside a run, by its first letter
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a word inside a run, by its last letter
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a word inside a run, by the corrected word
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a word inside a run, by an autocorrection
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a whole run, by its first letter
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a whole run, by an autocorrection
// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing across the start of a run, by text that starts with the same letter
#[test]
fn replacing() {
    for (label, text, marked, range, payload, state, mark_node) in REPLACE_CASES {
        for same_update in [false, true] {
            assert_eq!(
                (
                    label,
                    replace_text(false, text, marked, range, payload, same_update)
                ),
                (label, state.to_owned())
            );
            assert_eq!(
                (
                    label,
                    replace_text(true, text, marked, range, payload, same_update)
                ),
                (label, mark_node.to_owned())
            );
        }
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > text replaced by code that dispatches no command keeps the letters it kept
#[test]
fn text_replaced_by_code_that_dispatches_no_command_keeps_the_letters_it_kept() {
    // Without the selection the insertion command replaced, the smallest
    // edit tells the kept letters from the inserted ones: "t" and "e" are
    // kept with their ids, "h" is inserted between them
    let editor = create_test_editor();
    update(&editor, || init_with("x teh y"));
    mark(&editor, 0, 7, "c1");
    mark(&editor, 2, 3, "c2");
    assert_eq!(
        tree(&editor),
        r##"paragraph("x "[c1] "t"[c1,c2] "eh y"[c1])"##
    );
    update(&editor, || select(2, 5).insert_text("the"));
    assert_eq!(
        tree(&editor),
        r##"paragraph("x "[c1] "t"[c1,c2] "he y"[c1])"##
    );
    // The same edit through the command replaces the selection: the new
    // text gets the ids all of the replaced text had
    let typed = create_test_editor();
    update(&typed, || init_with("x teh y"));
    mark(&typed, 0, 7, "c1");
    mark(&typed, 2, 3, "c2");
    run(&typed, || {
        select(2, 5);
    });
    type_text(&typed, "the");
    assert_eq!(tree(&typed), r##"paragraph("x the y"[c1])"##);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > the replaced selection is known when the command is dispatched outside an update
#[test]
fn the_replaced_selection_is_known_when_the_command_is_dispatched_outside_an_update() {
    // Each priority level of a command dispatched outside an update runs in
    // an update of its own, and the DOM paste event dispatches
    // PASTE_COMMAND that way ("paste event" dispatches it so, on an editor
    // with a root element; see the module comment of mod.rs)
    let outside_cases = [
        (
            "abcde",
            (2, 5),
            (1, 3),
            "bX",
            r##"paragraph("abX" "de"[c1])"##,
        ),
        (
            "abcde",
            (0, 3),
            (2, 4),
            "cX",
            r##"paragraph("ab"[c1] "cXe")"##,
        ),
        (
            "x teh y",
            (0, 7),
            (2, 5),
            "t",
            r##"paragraph("x t y"[c1])"##,
        ),
        (
            "a grew b",
            (2, 6),
            (2, 6),
            "g",
            r##"paragraph("a " "g"[c1] " b")"##,
        ),
    ];
    for (text, marked, (start, end), payload, expected) in outside_cases {
        for how in ["command", "paste", "paste event"] {
            let editor = create_test_editor_with(Options {
                with_root: how == "paste event",
                ..Options::default()
            });
            update(&editor, move || init_with(text));
            mark(&editor, marked.0, marked.1, "c1");
            run(&editor, move || {
                select(start, end);
            });
            if how == "command" {
                editor
                    .dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, payload.to_owned())
                    .unwrap();
            } else {
                editor
                    .dispatch_command(&PASTE_COMMAND, paste_event(payload, None, None))
                    .unwrap();
            }
            assert_eq!((how, tree(&editor)), (how, expected.to_owned()));
        }
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > the selection of a command that changed nothing is not used by a later update
#[test]
fn the_selection_of_a_command_that_changed_nothing_is_not_used_by_a_later_update() {
    let editor = create_test_editor();
    let _unregister = editor.register_command(
        &CONTROLLED_TEXT_INSERTION_COMMAND,
        |_, _| Ok(true),
        COMMAND_PRIORITY_HIGH,
    );
    update(&editor, || init_with("abcde"));
    mark(&editor, 2, 5, "c1");
    run(&editor, || {
        select(1, 3);
    });
    editor
        .dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, "bX".to_owned())
        .unwrap();
    assert_eq!(tree(&editor), r##"paragraph("ab" "cde"[c1])"##);
    // Code that dispatches no command: the smallest edit
    update(&editor, || {
        if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
            selection.insert_text("bX")?;
        }
        Ok(())
    });
    assert_eq!(tree(&editor), r##"paragraph("ab" "Xde"[c1])"##);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > an id removed in the same update is not given back
#[test]
fn an_id_removed_in_the_same_update_is_not_given_back() {
    for range in [false, true] {
        let editor = create_entity_editor(false);
        update(&editor, || init_with("see foo bar"));
        mark_all(&editor, false);
        run(&editor, || {
            select_at(4);
        });
        let ed = (*editor).clone();
        update(&editor, move || {
            ed.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, "#".to_owned())?;
            let selection = range.then(|| select(4, 8));
            remove_mark_id("c1", selection.as_ref())
        });
        assert_eq!(
            tree(&editor),
            if range {
                r##"paragraph("see "[c1] "#foo" " bar"[c1])"##
            } else {
                r##"paragraph("see " "#foo" " bar")"##
            }
        );
        assert_eq!(
            registry_text(&editor),
            if range {
                registry(&[("c1", &[" bar", "see "])])
            } else {
                registry(&[])
            }
        );
    }
    // Nor to text that replaces a selection, before or after the removal
    for remove_first in [false, true] {
        let editor = create_test_editor();
        update(&editor, || init_with("x teh y"));
        mark(&editor, 0, 7, "c1");
        let ed = (*editor).clone();
        update(&editor, move || {
            if remove_first {
                remove_mark_id("c1", None)?;
            }
            select(2, 5);
            ed.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, "the".to_owned())?;
            if !remove_first {
                remove_mark_id("c1", None)?;
            }
            Ok(())
        });
        assert_eq!(tree(&editor), r##"paragraph("x the y")"##);
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > a long TextNode is compared around the change
#[test]
fn a_long_text_node_is_compared_around_the_change() {
    let long = "abcdefghij".repeat(2000);
    let editor = create_test_editor();
    update(&editor, move || init_with(&long));
    mark(&editor, 10000, 10010, "c1");
    // At either edge, inside, and a letter equal to the one after the edge
    for (at, text, run_text) in [
        (10010, "X", r##""abcdefghij"[c1] "Xabc"##),
        (10000, "Y", r##"Y" "abcdefghij"[c1] "Xabc"##),
        (10007, "Z", r##"Y" "abcdefZghij"[c1] "Xabc"##),
        (10012, "X", r##"Y" "abcdefZghij"[c1] "XXabc"##),
    ] {
        run(&editor, move || {
            select_at(at);
        });
        type_text(&editor, text);
        assert!(tree(&editor).contains(run_text), "{run_text}");
    }
    // A selection replaced across the end of the mark
    run(&editor, || {
        select(10011, 10013);
    });
    type_text(&editor, "jQ");
    assert!(tree(&editor).contains(r##"Y" "abcdefZghi"[c1] "jQXabc"##));
    assert_eq!(read(&editor, || get_root().get_text_content_size()), 20004);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > no edit splits a surrogate pair
#[test]
fn no_edit_splits_a_surrogate_pair() {
    let editor = create_test_editor();
    update(&editor, || init_with("x\u{1F600}\u{1F642}y"));
    mark(&editor, 1, 5, "c1");
    // An emoji typed between the marked emoji joins the run
    run(&editor, || {
        select_at(3);
    });
    type_text(&editor, "\u{1F603}");
    assert_eq!(
        tree(&editor),
        "paragraph(\"x\" \"\u{1F600}\u{1F603}\u{1F642}\"[c1] \"y\")"
    );
    // Emoji replaced without a command by one that shares their first code
    // unit: the kept text is found by code point
    let other = create_test_editor();
    update(&other, || init_with("x\u{1F600}\u{1F642}y"));
    mark(&other, 1, 3, "c1");
    update(&other, || select(1, 5).insert_text("\u{1F603}"));
    assert_eq!(tree(&other), "paragraph(\"x\u{1F603}y\")");
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > a handler that wraps the selection instead of replacing it keeps its ids
#[test]
fn a_handler_that_wraps_the_selection_instead_of_replacing_it_keeps_its_ids() {
    let editor = create_test_editor();
    update(&editor, || init_with("x teh y"));
    mark(&editor, 0, 7, "c1");
    mark(&editor, 2, 4, "c2");
    assert_eq!(
        tree(&editor),
        r##"paragraph("x "[c1] "te"[c1,c2] "h y"[c1])"##
    );
    let _unregister = editor.register_command(
        &CONTROLLED_TEXT_INSERTION_COMMAND,
        |payload, _| {
            let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
                return Ok(false);
            };
            let range = selection.range().unwrap();
            if payload != "\"" || range.is_collapsed() {
                return Ok(false);
            }
            let (start, end) = if selection.is_backward()? {
                (range.focus.clone(), range.anchor.clone())
            } else {
                (range.anchor.clone(), range.focus.clone())
            };
            let start_node = start.get_node()?;
            let end_node = end.get_node()?;
            let (start_offset, end_offset) = (start.offset, end.offset);
            end_node.splice_text(end_offset as i64, 0, "\"", false)?;
            start_node.splice_text(start_offset as i64, 0, "\"", false)?;
            let after = end_offset
                + if start_node.get_key() == end_node.get_key() {
                    2
                } else {
                    1
                };
            end_node.select(Some(after), Some(after))?;
            Ok(true)
        },
        COMMAND_PRIORITY_HIGH,
    );
    run(&editor, || {
        select(2, 5);
    });
    type_text(&editor, "\"");
    assert_eq!(
        tree(&editor),
        format!(
            r##"paragraph({}[c1] "te"[c1,c2] {}[c1])"##,
            quoted("x \""),
            quoted("h\" y")
        )
    );
}

/// `/@[a-z]+/.exec(text)`: the UTF-16 index and length of the match.
fn mention_match(text: &str) -> Option<(usize, usize)> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut i = 0;
    while i < units.len() {
        if units[i] == u16::from(b'@') {
            let mut j = i + 1;
            while j < units.len() && (u16::from(b'a')..=u16::from(b'z')).contains(&units[j]) {
                j += 1;
            }
            if j > i + 1 {
                return Some((i, j - i));
            }
        }
        i += 1;
    }
    None
}

/// `createEntityEditor(withMarkNode)`: hashtags, and a mentions transform
/// that replaces "@name" by a new token node; with a root element, as
/// deleting a character can read the DOM.
fn create_entity_editor(with_mark_node: bool) -> LexicalEditorWithDispose {
    let mut dependencies: Vec<AnyLexicalExtensionArgument> = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::hashtag::hashtag_extension().into(),
        mark_extension().into(),
    ];
    if !with_mark_node {
        dependencies.push(mark_state_extension().into());
    }
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies,
        nodes: vec![NodeClass::of::<MentionNode>()],
        // Like a mentions plugin: "@name" is replaced by a new token node
        register: Some(Rc::new(|entity_editor: &Editor, _, _| {
            Ok(entity_editor.register_node_transform("text", |node| {
                let Some((index, len)) = node
                    .is_simple_text()
                    .then(|| mention_match(&node.get_text_content()))
                    .flatten()
                else {
                    return Ok(());
                };
                let parts = node.split_text(&[index as i64, (index + len) as i64])?;
                let target = &parts[if index == 0 { 0 } else { 1 }];
                target.replace(&create_mention_node(&target.get_text_content()), false)?;
                Ok(())
            }))
        })),
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

/// `markAll(editor, withMarkNode)`.
fn mark_all(editor: &Editor, with_mark_node: bool) {
    run(editor, move || {
        let selection = select(0, paragraph().get_text_content_size());
        mark_selection(&selection, with_mark_node);
    });
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > a hashtag created from marked text keeps its ids, like MarkNode
#[test]
fn a_hashtag_created_from_marked_text_keeps_its_ids_like_mark_node() {
    for with_mark_node in [false, true] {
        let editor = create_entity_editor(with_mark_node);
        update(&editor, || init_with("see foo bar"));
        mark_all(&editor, with_mark_node);
        run(&editor, || {
            select_at(4);
        });
        type_text(&editor, "#");
        assert_eq!(
            tree(&editor),
            if with_mark_node {
                r##"paragraph(mark[c1]("see " "#foo" " bar"))"##
            } else {
                r##"paragraph("see "[c1] "#foo"[c1] " bar"[c1])"##
            }
        );
        assert_eq!(
            read(&editor, || {
                paragraph()
                    .get_all_text_nodes()
                    .iter()
                    .map(|n| n.get_type())
                    .collect::<Vec<_>>()
            }),
            ["text", "hashtag", "text"]
        );
        // Deleting the "#" turns it back into text, which keeps them too
        run(&editor, || {
            select(4, 5);
        });
        let ed = (*editor).clone();
        update(&editor, move || {
            ed.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
            Ok(())
        });
        assert_eq!(
            tree(&editor),
            if with_mark_node {
                r##"paragraph(mark[c1]("see foo bar"))"##
            } else {
                r##"paragraph("see foo bar"[c1])"##
            }
        );
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > a token node created from marked text keeps its ids, like MarkNode
#[test]
fn a_token_node_created_from_marked_text_keeps_its_ids_like_mark_node() {
    for with_mark_node in [false, true] {
        let editor = create_entity_editor(with_mark_node);
        update(&editor, || init_with("hi bob x"));
        mark_all(&editor, with_mark_node);
        run(&editor, || {
            select_at(3);
        });
        type_text(&editor, "@");
        assert_eq!(
            tree(&editor),
            if with_mark_node {
                r##"paragraph(mark[c1]("hi " "@bob" " x"))"##
            } else {
                r##"paragraph("hi "[c1] "@bob"[c1] " x"[c1])"##
            }
        );
        assert_eq!(
            read(&editor, || {
                paragraph()
                    .get_all_text_nodes()
                    .iter()
                    .map(|n| format!("{}:{}", n.get_type(), n.text_data().get_mode()))
                    .collect::<Vec<_>>()
            }),
            ["text:normal", "test-mention:token", "text:normal"]
        );
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > an atomic node created from text with different ids gets all of them
#[test]
fn an_atomic_node_created_from_text_with_different_ids_gets_all_of_them() {
    // "#" typed at the start of a marked paragraph is at the edge of the
    // mark, but the hashtag it makes is one node: it keeps the ids of "foo"
    let editor = create_entity_editor(false);
    update(&editor, || init_with("foo"));
    mark_all(&editor, false);
    run(&editor, || {
        select_at(0);
    });
    type_text(&editor, "#");
    assert_eq!(tree(&editor), r##"paragraph("#foo"[c1])"##);
    assert_eq!(
        read(&editor, || paragraph()
            .get_first_child()
            .unwrap()
            .get_type()),
        "hashtag"
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > typing inside a segmented node in a marked run keeps its ids, like MarkNode
#[test]
fn typing_inside_a_segmented_node_in_a_marked_run_keeps_its_ids_like_mark_node() {
    for with_mark_node in [false, true] {
        let editor = create_test_editor();
        update(&editor, || {
            let root = get_root();
            root.clear()?;
            let segmented = create_text_node("Alice Smith");
            segmented.set_mode("segmented")?;
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("hi "), segmented, create_text_node(" ok")])?;
            root.append(&[paragraph])
        });
        mark_all(&editor, with_mark_node);
        run(&editor, || {
            select_at(8);
        });
        type_text(&editor, "X");
        assert_eq!(
            tree(&editor),
            if with_mark_node {
                r##"paragraph(mark[c1]("hi AliceX Smith ok"))"##
            } else {
                r##"paragraph("hi AliceX Smith ok"[c1])"##
            }
        );
    }
}

/// `compose(editor, taggedEdit)`.
fn compose(editor: &Editor, tagged_edit: impl FnOnce() + 'static) {
    run(editor, || {
        select_at(15);
    });
    let ed = editor.clone();
    update(editor, move || {
        ed.dispatch_command(
            &COMPOSITION_START_COMMAND,
            Rc::new(CompositionEvent::new("compositionstart", "")),
        )?;
        Ok(())
    });
    update(editor, || {
        let marked = paragraph().get_child_at_index(1).unwrap();
        marked.set_text_content("grew 4%に");
        marked.select(Some(8), Some(8)).map(|_| ())
    });
    run(editor, move || {
        add_update_tag(COLLABORATION_TAG);
        tagged_edit();
    });
    update(editor, || {
        let marked = paragraph().get_child_at_index(1).unwrap();
        let text = marked.get_text_content().replacen('に', "日本", 1);
        marked.set_text_content(&text);
        let end = crate::utf16::len(&text);
        marked.select(Some(end), Some(end)).map(|_| ())
    });
    run(editor, || set_composition_key(None));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > an update tagged COLLABORATION_TAG editing the paragraph of a composition
#[test]
fn an_update_tagged_collaboration_tag_editing_the_paragraph_of_a_composition() {
    // Comparing with the paragraph from before the composition would take
    // the "P" of the tagged update for composed text and change its ids, so
    // the composed text is left in the node it was composed in
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    compose(&editor, || {
        let marked = paragraph().get_child_at_index(1).unwrap();
        marked.set_text_content("grew 4%Pに");
        marked.select(Some(9), Some(9)).unwrap();
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%P日本"[c1] " in Q3.")"##
    );
    // An edit in another paragraph does not change how it is compared
    let other = create_test_editor();
    update(&other, || {
        init()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("z")])?;
        get_root().append(&[paragraph])
    });
    mark(&other, 8, 15, "c1");
    compose(&other, || {
        let first = paragraph_at(1).get_first_child_or_throw().unwrap();
        assert!(first.is_text());
        first.set_text_content("zz");
    });
    assert_eq!(
        tree(&other),
        r##"paragraph("Revenue " "grew 4%"[c1] "日本 in Q3.") / paragraph("zz")"##
    );
}
