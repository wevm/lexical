//! "add, remove and query", "structural edits" and "typing at the edges of
//! a mark".
//!
//! The autocorrection that upstream dispatches as an
//! `insertReplacementText` InputEvent payload of
//! `CONTROLLED_TEXT_INSERTION_COMMAND` is dispatched as that `beforeinput`
//! event (`BEFORE_INPUT_COMMAND`), which the core turns into the command
//! with the event's text: Rust's command takes the text only.

use super::*;
use crate::caret::{CaretDirection, PointCaret, get_text_point_caret};
use crate::clipboard::{
    generate_json_from_selected_nodes, generate_nodes_from_serialized_nodes, insert_generated_nodes,
};
use crate::history::{HistoryConfigOverrides, history_extension};
use crate::mark::{MarkIDsMode, get_mark_ids_at_selection, remove_mark_id};
use crate::nodes::line_break::create_line_break_node;
use crate::{
    BEFORE_INPUT_COMMAND, COLLABORATION_TAG, COMPOSITION_START_COMMAND, CompositionEvent,
    DELETE_CHARACTER_COMMAND, DROP_COMMAND, DragEvent, HISTORIC_TAG, INSERT_PARAGRAPH_COMMAND,
    InputEvent, LexicalNodeReplacement, PASTE_COMMAND, REDO_COMMAND, RootElement, RootElementRef,
    UNDO_COMMAND, add_update_tag, get_selection, insert_nodes, set_composition_key,
};
use std::cell::RefCell;

/// `setup()`: `P` with `c1` on "grew 4%".
fn setup() -> LexicalEditorWithDispose {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    editor
}

fn child(index: usize) -> NodeRef {
    paragraph().get_children()[index].clone()
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > overlapping marks share TextNodes regardless of order
#[test]
fn overlapping_marks_share_text_nodes_regardless_of_order() {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    mark(&editor, 12, 19, "c2");
    let expected = r##"paragraph("Revenue " "grew"[c1] " 4%"[c1,c2] " in "[c2] "Q3.")"##;
    assert_eq!(tree(&editor), expected);

    let reversed = create_test_editor();
    update(&reversed, init);
    mark(&reversed, 12, 19, "c2");
    mark(&reversed, 8, 15, "c1");
    assert_eq!(tree(&reversed), expected);

    // One set-valued NodeState key, a sorted array in JSON
    let states = paragraph_child_states(&editor);
    assert_eq!(
        states,
        vec![
            None,
            mark_ids_state(&["c1"]),
            mark_ids_state(&["c1", "c2"]),
            mark_ids_state(&["c2"]),
            None,
        ]
    );
    let paragraph_json = |e: &Editor| {
        e.get_editor_state()
            .to_json(false)
            .get("root")
            .unwrap()
            .get("children")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .clone()
    };
    assert_eq!(paragraph_json(&reversed), paragraph_json(&editor));
    assert_eq!(
        registry_text(&editor),
        registry(&[("c1", &[" 4%", "grew"]), ("c2", &[" 4%", " in "])])
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > the selection still covers the marked text
#[test]
fn the_selection_still_covers_the_marked_text() {
    let editor = create_test_editor();
    update(&editor, init);
    run(&editor, || {
        add_mark_id("c1", &select(8, 15)).unwrap();
        let selection = get_selection().unwrap();
        assert_eq!(selection.get_text_content().unwrap(), "grew 4%");
    });
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > $getMarkIDsAtSelection
#[test]
fn get_mark_ids_at_selection_test() {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    mark(&editor, 12, 19, "c2");
    run(&editor, || {
        let at = |s: SelectionRef, mode| get_mark_ids_at_selection(Some(&s), mode).unwrap();
        let any = MarkIDsMode::Any;
        assert_eq!(at(select_at(3), any), Vec::<String>::new());
        assert_eq!(at(select_at(10), any), ["c1"]);
        assert_eq!(at(select_at(13), any), ["c1", "c2"]);
        assert_eq!(at(select_at(17), any), ["c2"]);
        // A caret right before a marked run (end of "Revenue ") is in it
        assert_eq!(at(select_at(8), any), ["c1"]);
        // Defaults to the current selection
        select_at(13);
        assert_eq!(
            get_mark_ids_at_selection(get_selection().as_ref(), any).unwrap(),
            ["c1", "c2"]
        );
        assert_eq!(at(select(10, 17), any), ["c1", "c2"]);
        assert_eq!(at(select(10, 17), MarkIDsMode::All), Vec::<String>::new());
        assert_eq!(at(select(12, 15), MarkIDsMode::All), ["c1", "c2"]);
        // Leaves the selection only touches at an edge do not count
        assert_eq!(at(select(0, 8), any), Vec::<String>::new());
        assert_eq!(at(select(19, 22), any), Vec::<String>::new());
    });
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > the helpers return new arrays that the caller may change
#[test]
fn the_helpers_return_new_arrays_that_the_caller_may_change() {
    // A Rust Vec is the caller's own, so a change never reaches the stored
    // ids (`Object.isFrozen` has no counterpart); the port checks that the
    // stored ids are unchanged.
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    mark(&editor, 12, 19, "c2");
    run(&editor, || {
        let overlap = get_root()
            .get_all_text_nodes()
            .into_iter()
            .find(|n| n.get_text_content() == " 4%")
            .expect("expected the overlap");
        let results = vec![
            get_node_mark_ids(&overlap).unwrap(),
            get_mark_ids_at_selection(Some(&select_at(13)), MarkIDsMode::Any).unwrap(),
            get_mark_ids_at_selection(Some(&select(10, 17)), MarkIDsMode::Any).unwrap(),
            // One leaf: the result is not the stored value
            get_mark_ids_at_selection(Some(&select(12, 15)), MarkIDsMode::All).unwrap(),
        ];
        for mut ids in results {
            assert_eq!(ids, ["c1", "c2"]);
            ids.push("x".to_owned());
            ids.reverse();
        }
        assert_eq!(get_node_mark_ids(&overlap).unwrap(), ["c1", "c2"]);
        assert_eq!(
            get_mark_ids_at_selection(Some(&select(12, 15)), MarkIDsMode::All).unwrap(),
            ["c1", "c2"]
        );
    });
    assert_eq!(
        registry_text(&editor),
        registry(&[("c1", &[" 4%", "grew"]), ("c2", &[" 4%", " in "])])
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > removing an id merges the text back
#[test]
fn removing_an_id_merges_the_text_back() {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    mark(&editor, 12, 19, "c2");
    update(&editor, || remove_mark_id("c1", None));
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue grew" " 4% in "[c2] "Q3.")"##
    );
    assert_eq!(registry_text(&editor), registry(&[("c2", &[" 4% in "])]));
    update(&editor, || remove_mark_id("c2", None));
    assert_eq!(tree(&editor), r##"paragraph("Revenue grew 4% in Q3.")"##);
    assert_eq!(mark_id_node_keys_size(&editor), 0);
    // No NodeState is left in the JSON
    assert!(
        !editor
            .get_editor_state()
            .to_json(false)
            .to_string()
            .contains("\"$\"")
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > removing an id from part of its range
#[test]
fn removing_an_id_from_part_of_its_range() {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 19, "c1");
    update(&editor, || remove_mark_id("c1", Some(&select(12, 15))));
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew"[c1] " 4%" " in "[c1] "Q3.")"##
    );
    assert_eq!(
        registry_text(&editor),
        registry(&[("c1", &[" in ", "grew"])])
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > removing an id without the active registry walks the document
#[test]
fn removing_an_id_without_the_active_registry_walks_the_document() {
    let editor = create_test_editor_with(Options {
        disabled: true,
        ..Options::default()
    });
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    mark(&editor, 12, 19, "c2");
    update(&editor, || remove_mark_id("c1", None));
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue grew" " 4% in "[c2] "Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > add and remove in the same update
#[test]
fn add_and_remove_in_the_same_update() {
    let editor = create_test_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id("c1", &select(8, 15))?;
        remove_mark_id("c1", None)
    });
    assert_eq!(tree(&editor), r##"paragraph("Revenue grew 4% in Q3.")"##);
    assert_eq!(mark_id_node_keys_size(&editor), 0);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > line breaks, inline decorators and links
#[test]
fn line_breaks_inline_decorators_and_links() {
    let editor = create_test_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[
            create_text_node("ab"),
            create_line_break_node(),
            create_text_node("cd"),
            create_chip_node(),
            create_text_node("ef"),
        ])?;
        root.append(&[paragraph])
    });
    update(&editor, || {
        let children = paragraph().get_children();
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, children[0].get_key(), 1, PointType::Text)?;
        selection.point_set(Which::Focus, children[4].get_key(), 1, PointType::Text)?;
        add_mark_id("c1", &selection)?;
        Ok(())
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("a" "b"[c1] br[c1] "cd"[c1] test-chip[c1] "e"[c1] "f")"##
    );

    let linked = create_test_editor();
    update(&linked, init);
    update(&linked, || {
        select(8, 15);
        crate::link::toggle_link(Some("https://example.com"), &Default::default())
    });
    mark(&linked, 8, 19, "c1");
    assert_eq!(
        tree(&linked),
        r##"paragraph("Revenue " link("grew 4%"[c1]) " in "[c1] "Q3.")"##
    );
    // A comment on part of a link marks only that part
    mark(&linked, 0, 10, "c2");
    assert_eq!(
        tree(&linked),
        r##"paragraph("Revenue "[c2] link("gr"[c1,c2] "ew 4%"[c1]) " in "[c1] "Q3.")"##
    );
    update(&linked, || {
        select(8, 15);
        crate::link::toggle_link(None, &Default::default())
    });
    assert_eq!(
        tree(&linked),
        r##"paragraph("Revenue "[c2] "gr"[c1,c2] "ew 4% in "[c1] "Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > structural edits > Enter splits a marked run and Backspace joins it again
#[test]
fn enter_splits_a_marked_run_and_backspace_joins_it_again() {
    let editor = setup();
    let ed = (*editor).clone();
    update(&editor, move || {
        select_at(12);
        ed.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
        Ok(())
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew"[c1]) / paragraph(" 4%"[c1] " in Q3.")"##
    );
    assert_eq!(
        registry_text(&editor),
        registry(&[("c1", &[" 4%", "grew"])])
    );
    let ed = (*editor).clone();
    update(&editor, move || {
        paragraph_at(1).select_start()?;
        ed.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
        Ok(())
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%"[c1] " in Q3.")"##
    );
    assert_eq!(registry_text(&editor), registry(&[("c1", &["grew 4%"])]));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > structural edits > Enter at the edge of a run keeps it whole
#[test]
fn enter_at_the_edge_of_a_run_keeps_it_whole() {
    let editor = setup();
    let ed = (*editor).clone();
    update(&editor, move || {
        select_at(15);
        ed.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
        Ok(())
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%"[c1]) / paragraph(" in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > structural edits > deleting across the boundaries of a run
#[test]
fn deleting_across_the_boundaries_of_a_run() {
    let editor = setup();
    // Delete "ue gr" across the start of the run
    update(&editor, || select(5, 10).remove_text());
    assert_eq!(
        tree(&editor),
        r##"paragraph("Reven" "ew 4%"[c1] " in Q3.")"##
    );
    // Delete the last character of the run
    update(&editor, || select(9, 10).remove_text());
    assert_eq!(
        tree(&editor),
        r##"paragraph("Reven" "ew 4"[c1] " in Q3.")"##
    );
    // Deleting the whole run removes the id
    update(&editor, || select(4, 11).remove_text());
    assert_eq!(tree(&editor), r##"paragraph("Reven Q3.")"##);
    assert_eq!(mark_id_node_keys_size(&editor), 0);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > structural edits > marked and unmarked text do not merge, equal ids do
#[test]
fn marked_and_unmarked_text_do_not_merge_equal_ids_do() {
    let editor = create_test_editor();
    update(&editor, || init_with("aabbcc"));
    mark(&editor, 0, 2, "x");
    mark(&editor, 2, 4, "y");
    assert_eq!(tree(&editor), r##"paragraph("aa"[x] "bb"[y] "cc")"##);
    mark(&editor, 4, 6, "y");
    assert_eq!(tree(&editor), r##"paragraph("aa"[x] "bbcc"[y])"##);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text typed at the end of a run is not marked
#[test]
fn text_typed_at_the_end_of_a_run_is_not_marked() {
    let editor = setup();
    run(&editor, || {
        select_at(15);
    });
    type_text(&editor, "X");
    type_text(&editor, "Y");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%"[c1] "XY in Q3.")"##
    );
    run(&editor, || {
        let selection = get_selection().unwrap();
        let range = selection.range().unwrap();
        let text = range.anchor.get_node().unwrap().get_text_content();
        assert_eq!(&text[..2], "XY");
        assert_eq!(range.anchor.offset, 2);
    });
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text typed at the start of a run is not marked
#[test]
fn text_typed_at_the_start_of_a_run_is_not_marked() {
    let editor = setup();
    // The caret at the start of the marked node (not the end of "Revenue ")
    update(&editor, || child(1).select(Some(0), Some(0)).map(|_| ()));
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue X" "grew 4%"[c1] " in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > a run at the start of a paragraph
#[test]
fn a_run_at_the_start_of_a_paragraph() {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 0, 7, "c1");
    update(&editor, || paragraph().select_start().map(|_| ()));
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("X" "Revenue"[c1] " grew 4% in Q3.")"##
    );
    // and at the end of a paragraph
    mark(&editor, 20, 23, "c2");
    update(&editor, || paragraph().select_end().map(|_| ()));
    type_text(&editor, "Y");
    assert_eq!(
        tree(&editor),
        r##"paragraph("X" "Revenue"[c1] " grew 4% in " "Q3."[c2] "Y")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > the class that replaces TextNode is split like TextNode, like MarkNode
#[test]
fn the_class_that_replaces_text_node_is_split_like_text_node_like_mark_node() {
    // With or without withKlass
    for with_klass in [true, false] {
        for with_mark_node in [false, true] {
            let editor = editor_with(
                with_mark_node,
                false,
                vec![
                    NodeClass::of::<ReplacementTextNode>(),
                    LexicalNodeReplacement {
                        replace: NodeClass::of::<crate::nodes::text::TextNode>(),
                        with: Rc::new(|node: &NodeRef| {
                            let replacement = crate::create::<ReplacementTextNode>();
                            replacement.set_text_content(&node.get_text_content());
                            Ok(replacement)
                        }),
                        with_klass: with_klass.then(NodeClass::of::<ReplacementTextNode>),
                    }
                    .into(),
                ],
            );
            update(&editor, || init_with("abcdef"));
            run(&editor, move || {
                mark_selection(&select(0, 3), with_mark_node)
            });
            run(&editor, || {
                select_at(3);
            });
            type_text(&editor, "X");
            update(&editor, || {
                paragraph().get_all_text_nodes()[0]
                    .select(Some(0), Some(0))
                    .map(|_| ())
            });
            type_text(&editor, "Y");
            // The core does not merge TextNodes of another type
            assert_eq!(
                tree(&editor),
                if with_mark_node {
                    r##"paragraph("Y" mark[c1]("abc") "X" "def")"##
                } else {
                    r##"paragraph("Y" "abc"[c1] "X" "def")"##
                }
            );
            assert!(read(&editor, || {
                paragraph()
                    .get_all_text_nodes()
                    .iter()
                    .all(|n| n.get_latest().is::<ReplacementTextNode>())
            }));
        }
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text inserted after a marked hashtag is not marked, like MarkNode
#[test]
fn text_inserted_after_a_marked_hashtag_is_not_marked_like_mark_node() {
    // The hashtag ends where the inserted text starts, and its transform
    // splits that text off with the ids of the hashtag: those are not the
    // inserted text's own, however it was inserted
    let cases = [
        (
            "typed",
            " ",
            r##"paragraph("see " "#foo"[c1] "  bar")"##,
            r##"paragraph("see " mark[c1]("#foo") "  bar")"##,
        ),
        (
            "pasted",
            " tail",
            r##"paragraph("see " "#foo"[c1] " tail bar")"##,
            r##"paragraph("see " mark[c1]("#foo") " tail bar")"##,
        ),
        (
            "paste event",
            ", x",
            r##"paragraph("see " "#foo"[c1] ", x bar")"##,
            r##"paragraph("see " mark[c1]("#foo") ", x bar")"##,
        ),
        (
            "code",
            ", x #bar y",
            r##"paragraph("see " "#foo"[c1] ", x " "#bar" " y bar")"##,
            r##"paragraph("see " mark[c1]("#foo") ", x " "#bar" " y bar")"##,
        ),
    ];
    for (how, text, expected, expected_with_mark_node) in cases {
        for with_mark_node in [false, true] {
            let editor = editor_with(with_mark_node, true, vec![]);
            if how == "paste event" {
                set_test_root_element(&editor);
            }
            update(&editor, || init_with("see #foo bar"));
            run(&editor, move || {
                mark_selection(&select(4, 8), with_mark_node)
            });
            run(&editor, || {
                select_at(8);
            });
            match how {
                "typed" => type_text(&editor, text),
                "pasted" | "paste event" => {
                    editor
                        .dispatch_command(&PASTE_COMMAND, paste_event(text, None, None))
                        .unwrap();
                }
                _ => {
                    let text = text.to_owned();
                    update(&editor, move || select_at(8).insert_text(&text));
                }
            }
            assert_eq!(
                (how, tree(&editor)),
                (
                    how,
                    (if with_mark_node {
                        expected_with_mark_node
                    } else {
                        expected
                    })
                    .to_owned()
                )
            );
        }
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > a hashtag inserted at the edge of a marked run is not marked, like MarkNode
#[test]
fn a_hashtag_inserted_at_the_edge_of_a_marked_run_is_not_marked_like_mark_node() {
    // The inserted text lands in the marked node, and the hashtag
    // transform splits the hashtag off with a copy of that node's ids
    // (which the hashtag keeps): those are not the inserted text's own
    let cases = [
        (
            "pasted after",
            3,
            " #tag",
            r##"paragraph("see"[c1] " " "#tag" " more")"##,
            r##"paragraph(mark[c1]("see") " " "#tag" " more")"##,
        ),
        (
            "code after",
            3,
            " #tag",
            r##"paragraph("see"[c1] " " "#tag" " more")"##,
            r##"paragraph(mark[c1]("see") " " "#tag" " more")"##,
        ),
        (
            "code before",
            0,
            "#tag ",
            // As on main: the space lands in the text with "see"
            r##"paragraph("#tag" " see"[c1] " more")"##,
            r##"paragraph("#tag" " " mark[c1]("see") " more")"##,
        ),
    ];
    for (how, at, text, expected, expected_with_mark_node) in cases {
        for with_mark_node in [false, true] {
            let editor = editor_with(with_mark_node, true, vec![]);
            update(&editor, || init_with("see more"));
            run(&editor, move || {
                mark_selection(&select(0, 3), with_mark_node)
            });
            run(&editor, move || {
                select_at(at);
            });
            if how.starts_with("pasted") {
                editor
                    .dispatch_command(&PASTE_COMMAND, paste_event(text, None, None))
                    .unwrap();
            } else {
                update(&editor, move || select_at(at).insert_text(text));
            }
            assert_eq!(
                (how, with_mark_node, tree(&editor)),
                (
                    how,
                    with_mark_node,
                    (if with_mark_node {
                        expected_with_mark_node
                    } else {
                        expected
                    })
                    .to_owned()
                )
            );
        }
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > pasted Lexical data keeps its ids after a marked hashtag
#[test]
fn pasted_lexical_data_keeps_its_ids_after_a_marked_hashtag() {
    // Pasted or inserted content with the same ids as the hashtag is not
    // taken for text split off it: its ids are a value of its own, not the
    // hashtag's
    for how in ["paste", "code"] {
        let editor = editor_with(false, true, vec![]);
        update(&editor, || init_with("see #foo bar"));
        mark(&editor, 4, 12, "c1");
        let json = Rc::new(RefCell::new(None));
        let (j, ed) = (json.clone(), (*editor).clone());
        update(&editor, move || {
            *j.borrow_mut() = Some(generate_json_from_selected_nodes(
                &ed,
                Some(&select(8, 12)),
            )?);
            remove_mark_id("c1", Some(&select(8, 12)))
        });
        run(&editor, || {
            select_at(8);
        });
        let json = json.borrow_mut().take().unwrap();
        if how == "paste" {
            editor
                .dispatch_command(
                    &PASTE_COMMAND,
                    paste_event(" bar", Some(&json.to_json().to_string()), None),
                )
                .unwrap();
        } else {
            let ed = (*editor).clone();
            update(&editor, move || {
                let nodes: Vec<Value> = json.nodes.into_iter().map(Value::Object).collect();
                insert_generated_nodes(
                    &ed,
                    generate_nodes_from_serialized_nodes(&nodes)?,
                    &get_selection().unwrap(),
                )
            });
        }
        assert_eq!(
            (how, tree(&editor)),
            (
                how,
                r##"paragraph("see " "#foo"[c1] " bar"[c1] " bar")"##.to_owned()
            )
        );
    }
}

/// A root that resolves every point to the end of the `#foo` hashtag's
/// text, as the mocked `caretRangeFromPoint` does.
struct HashtagEndRoot;

impl RootElement for HashtagEndRoot {
    fn caret_from_point(&self, _x: f64, _y: f64) -> Option<PointCaret> {
        let hashtag = paragraph()
            .get_all_text_nodes()
            .into_iter()
            .find(|n| n.get_text_content() == "#foo")?;
        get_text_point_caret(&hashtag, CaretDirection::Next, 4).ok()
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text dropped from another editor after a marked hashtag keeps its ids, like MarkNode
#[test]
fn text_dropped_from_another_editor_after_a_marked_hashtag_keeps_its_ids_like_mark_node() {
    // The root resolves the drop point, as the mocked `caretRangeFromPoint`
    // resolves it to offset 4 of the hashtag's DOM text
    for with_mark_node in [false, true] {
        let editor = editor_with(with_mark_node, true, vec![]);
        editor
            .set_root_element(Some(RootElementRef(Rc::new(HashtagEndRoot))))
            .unwrap();
        // " more"[c1] in another editor, as Lexical data
        update(&editor, || init_with(" more"));
        run(&editor, move || {
            mark_selection(&select(0, 5), with_mark_node)
        });
        let json = Rc::new(RefCell::new(String::new()));
        let (j, ed) = (json.clone(), (*editor).clone());
        update(&editor, move || {
            *j.borrow_mut() = generate_json_from_selected_nodes(&ed, Some(&select(0, 5)))?
                .to_json()
                .to_string();
            Ok(())
        });
        update(&editor, || init_with("see #foo bar"));
        run(&editor, move || {
            mark_selection(&select(4, 8), with_mark_node)
        });
        // Dropped at the end of the hashtag: a drop runs no paste command
        let json = json.borrow().clone();
        let mut event = DragEvent::new(
            "drop",
            Some(data_transfer(&[
                // Dragged from an editor that is not on the page
                (
                    "application/x-lexical-drag",
                    r##"{"editorKey":"another-editor"}"##,
                ),
                ("application/x-lexical-editor", &json),
                ("text/plain", " more"),
            ])),
        );
        event.mouse.client_x = 1.0;
        event.mouse.client_y = 1.0;
        assert!(
            editor
                .dispatch_command(&DROP_COMMAND, Rc::new(event))
                .unwrap()
        );
        assert_eq!(
            tree(&editor),
            if with_mark_node {
                r##"paragraph("see " mark[c1]("#foo") mark[c1](" more") " bar")"##
            } else {
                r##"paragraph("see " "#foo"[c1] " more"[c1] " bar")"##
            }
        );
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text that code inserts after a hashtag and marks in the same update keeps its ids
#[test]
fn text_that_code_inserts_after_a_hashtag_and_marks_in_the_same_update_keeps_its_ids() {
    // Marked together with the hashtag, like MarkNode
    for with_mark_node in [false, true] {
        let editor = editor_with(with_mark_node, true, vec![]);
        update(&editor, || init_with("see #foo bar"));
        update(&editor, move || {
            select_at(8);
            insert_nodes(vec![create_text_node(" Q")])?;
            mark_selection(&select(4, 10), with_mark_node);
            select_at(10);
            Ok(())
        });
        assert_eq!(
            tree(&editor),
            if with_mark_node {
                r##"paragraph("see " mark[c1]("#foo" " Q") " bar")"##
            } else {
                r##"paragraph("see " "#foo"[c1] " Q"[c1] " bar")"##
            }
        );
    }
    // A run of nodes inserted after a marked hashtag, each marked by code
    let editor = editor_with(false, true, vec![]);
    update(&editor, || init_with("see #foo"));
    mark(&editor, 4, 8, "c1");
    update(&editor, || {
        let nodes = vec![
            create_text_node(" "),
            crate::hashtag::create_hashtag_node("#bar"),
            create_text_node(" baz"),
        ];
        select_at(8);
        insert_nodes(nodes.clone())?;
        for node in nodes {
            let selection = create_range_selection();
            selection.point_set(Which::Anchor, node.get_key(), 0, PointType::Text)?;
            selection.point_set(
                Which::Focus,
                node.get_key(),
                node.get_text_content_size(),
                PointType::Text,
            )?;
            add_mark_id("c1", &selection)?;
        }
        Ok(())
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("see " "#foo"[c1] " "[c1] "#bar"[c1] " baz"[c1])"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > pasted text whose HTML is the same text is not marked after a marked hashtag, like MarkNode
#[test]
fn pasted_text_whose_html_is_the_same_text_is_not_marked_after_a_marked_hashtag_like_mark_node() {
    // The beforeinput of the autocorrection is dispatched as
    // BEFORE_INPUT_COMMAND, which reaches CONTROLLED_TEXT_INSERTION_COMMAND
    // with its text (see the module comment)
    for how in ["paste", "beforeinput"] {
        for with_mark_node in [false, true] {
            let editor = editor_with(with_mark_node, true, vec![]);
            update(&editor, || init_with("see #foo bar"));
            run(&editor, move || {
                mark_selection(&select(4, 8), with_mark_node)
            });
            run(&editor, || {
                select_at(8);
            });
            if how == "paste" {
                editor
                    .dispatch_command(&PASTE_COMMAND, paste_event(", x", None, Some(", x")))
                    .unwrap();
            } else {
                let mut event = InputEvent::new("insertReplacementText");
                event.data_transfer = Some(data_transfer(&[
                    ("text/html", ", x"),
                    ("text/plain", ", x"),
                ]));
                editor
                    .dispatch_command(&BEFORE_INPUT_COMMAND, Rc::new(event))
                    .unwrap();
            }
            assert_eq!(
                (how, tree(&editor)),
                (
                    how,
                    if with_mark_node {
                        r##"paragraph("see " mark[c1]("#foo") ", x bar")"##
                    } else {
                        r##"paragraph("see " "#foo"[c1] ", x bar")"##
                    }
                    .to_owned()
                )
            );
        }
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text typed inside a run is marked
#[test]
fn text_typed_inside_a_run_is_marked() {
    let editor = setup();
    run(&editor, || {
        select_at(11);
    });
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "greXw 4%"[c1] " in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text replacing a selection inside a run is marked
#[test]
fn text_replacing_a_selection_inside_a_run_is_marked() {
    let editor = setup();
    run(&editor, || {
        select(9, 11);
    });
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "gXw 4%"[c1] " in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > ids shared with the neighbor continue across the edge
#[test]
fn ids_shared_with_the_neighbor_continue_across_the_edge() {
    let editor = setup();
    mark(&editor, 12, 19, "c2");
    // Caret at the end of "grew"[c1], before " 4%"[c1,c2]
    update(&editor, || child(1).select(Some(4), Some(4)).map(|_| ()));
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grewX"[c1] " 4%"[c1,c2] " in "[c2] "Q3.")"##
    );
    // Caret at the end of " 4%"[c1,c2], before " in "[c2]: c1 ends here
    update(&editor, || child(2).select(Some(3), Some(3)).map(|_| ()));
    type_text(&editor, "Y");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grewX"[c1] " 4%"[c1,c2] "Y in "[c2] "Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text inserted natively into the DOM text
#[test]
fn text_inserted_natively_into_the_dom_text() {
    // What the native beforeinput path ends up doing: the DOM text of the
    // marked node changes and is read back into it with the caret after it
    let editor = setup();
    run(&editor, || {
        select_at(15);
    });
    update(&editor, || {
        let marked = child(1);
        marked.set_text_content("grew 4%X");
        marked.select(Some(8), Some(8)).map(|_| ())
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%"[c1] "X in Q3.")"##
    );
}

/// `editor.dispatchCommand(COMPOSITION_START_COMMAND, new
/// CompositionEvent('compositionstart', {data: ''}))`, inside an update.
fn start_composition(editor: &Editor) {
    let ed = editor.clone();
    update(editor, move || {
        ed.dispatch_command(
            &COMPOSITION_START_COMMAND,
            Rc::new(CompositionEvent::new("compositionstart", "")),
        )?;
        Ok(())
    });
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > composition at the end of a run
#[test]
fn composition_at_the_end_of_a_run() {
    let editor = setup();
    run(&editor, || {
        select_at(15);
    });
    start_composition(&editor);
    assert!(editor.is_composing());
    // The IME updates the DOM text of the composing node
    for composed in ["に", "日本"] {
        update(&editor, move || {
            let marked = child(1);
            assert!(marked.is_composing());
            marked.set_text_content(&format!("grew 4%{composed}"));
            let end = 7 + crate::utf16::len(composed);
            marked.select(Some(end), Some(end)).map(|_| ())
        });
        // While composing the text stays in the node the IME writes to
        assert_eq!(
            tree(&editor),
            format!(r##"paragraph("Revenue " "grew 4%{composed}"[c1] " in Q3.")"##)
        );
    }
    run(&editor, || set_composition_key(None));
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%"[c1] "日本 in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > composition inside a run
#[test]
fn composition_inside_a_run() {
    let editor = setup();
    run(&editor, || {
        select_at(11);
    });
    start_composition(&editor);
    update(&editor, || {
        let marked = child(1);
        marked.set_text_content("gre日w 4%");
        marked.select(Some(4), Some(4)).map(|_| ())
    });
    run(&editor, || set_composition_key(None));
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "gre日w 4%"[c1] " in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > replayed and historic updates are left alone
#[test]
fn replayed_and_historic_updates_are_left_alone() {
    for tag in [COLLABORATION_TAG, HISTORIC_TAG] {
        let editor = setup();
        run(&editor, || {
            select_at(15);
        });
        update(&editor, move || {
            add_update_tag(tag);
            let marked = child(1);
            marked.set_text_content("grew 4%X");
            marked.select(Some(8), Some(8)).map(|_| ())
        });
        assert_eq!(
            tree(&editor),
            r##"paragraph("Revenue " "grew 4%X"[c1] " in Q3.")"##
        );
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > a caret that was not at the edge does not split
#[test]
fn a_caret_that_was_not_at_the_edge_does_not_split() {
    let editor = setup();
    run(&editor, || {
        select_at(3);
    });
    // e.g. a programmatic append while the caret is elsewhere
    run(&editor, || child(1).set_text_content("grew 4%X"));
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%X"[c1] " in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > pasting at the end of a run
#[test]
fn pasting_at_the_end_of_a_run() {
    let editor = setup();
    update(&editor, || {
        select_at(15);
        insert_nodes(vec![create_text_node("pasted")])
    });
    assert_eq!(
        read(&editor, || node_mark_ids_of_text("pasted")),
        Vec::<String>::new()
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text typed with a different format inside a run is marked
#[test]
fn text_typed_with_a_different_format_inside_a_run_is_marked() {
    let editor = setup();
    run(&editor, || {
        select_at(11);
        if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
            selection.toggle_format("bold");
        }
    });
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "gre"[c1] "X"[c1] "w 4%"[c1] " in Q3.")"##
    );
    assert!(read(&editor, || get_root().get_all_text_nodes()[2]
        .has_format("bold")));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text pasted inside a run is marked
#[test]
fn text_pasted_inside_a_run_is_marked() {
    let editor = setup();
    update(&editor, || {
        select_at(11);
        let node = create_text_node("pasted");
        node.toggle_format("italic");
        insert_nodes(vec![node])
    });
    assert_eq!(read(&editor, || node_mark_ids_of_text("pasted")), ["c1"]);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > undo restores the marks and the output
#[test]
fn undo_restores_the_marks_and_the_output() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            config_extension(
                &history_extension(),
                [HistoryConfigOverrides {
                    delay: Some(0.0),
                    ..Default::default()
                }],
            ),
            mark_state_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    assert_eq!(registry_text(&editor), registry(&[("c1", &["grew 4%"])]));
    run(&editor, || {
        select_at(15);
    });
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%"[c1] "X in Q3.")"##
    );
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%"[c1] " in Q3.")"##
    );
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(tree(&editor), r##"paragraph("Revenue grew 4% in Q3.")"##);
    assert_eq!(mark_id_node_keys_size(&editor), 0);
    editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
    assert_eq!(registry_text(&editor), registry(&[("c1", &["grew 4%"])]));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > disabled: typed text joins the run
#[test]
fn disabled_typed_text_joins_the_run() {
    let editor = create_test_editor_with(Options {
        disabled: true,
        ..Options::default()
    });
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    run(&editor, || {
        select_at(15);
    });
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%X"[c1] " in Q3.")"##
    );
    assert_eq!(mark_id_node_keys_size(&editor), 0);
}
