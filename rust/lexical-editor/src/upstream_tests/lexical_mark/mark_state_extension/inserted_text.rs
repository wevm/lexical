//! "inserted text".
//!
//! The rule: inserted text gets the ids shared by the text right before and
//! right after the place it was inserted, plus, when it replaces a
//! selection, the ids all the replaced text had, plus its own ids (those of
//! pasted or otherwise inserted nodes). Typed text has no ids of its own.

use super::*;
use crate::clipboard::{
    SerializedSelectedNodes, generate_json_from_selected_nodes,
    generate_nodes_from_serialized_nodes, insert_generated_nodes,
};
use crate::history::{HistoryConfigOverrides, history_extension};
use crate::mark::remove_mark_id;
use crate::{
    CLEAR_HISTORY_COMMAND, DELETE_CHARACTER_COMMAND, INSERT_PARAGRAPH_COMMAND, UNDO_COMMAND,
    get_selection, insert_nodes, set_composition_key,
};
use std::cell::RefCell;

/// `setupWithHistory()`: on a root element, as the reconciler is part of
/// the repro (it syncs the paragraph's text format during the undo).
fn setup_with_history() -> LexicalEditorWithDispose {
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
    set_test_root_element(&editor);
    editor
}

/// `copy(editor, start, end)`: copies a character range of the first
/// paragraph.
fn copy(editor: &Editor, start: usize, end: usize) -> SerializedSelectedNodes {
    let json = Rc::new(RefCell::new(None));
    let (j, ed) = (json.clone(), editor.clone());
    update(editor, move || {
        *j.borrow_mut() = Some(generate_json_from_selected_nodes(
            &ed,
            Some(&select(start, end)),
        )?);
        Ok(())
    });
    json.borrow_mut().take().unwrap()
}

/// `paste(editor, json, $selectTarget)`.
fn paste(editor: &Editor, json: SerializedSelectedNodes, select_target: impl FnOnce() + 'static) {
    run(editor, select_target);
    let ed = editor.clone();
    update(editor, move || {
        let nodes: Vec<Value> = json.nodes.into_iter().map(Value::Object).collect();
        insert_generated_nodes(
            &ed,
            generate_nodes_from_serialized_nodes(&nodes)?,
            &get_selection().unwrap(),
        )
    });
}

fn select_first_child(start: usize, end: usize) {
    paragraph()
        .get_first_child()
        .unwrap()
        .select(Some(start), Some(end))
        .unwrap();
}

fn select_last_child(start: usize, end: usize) {
    paragraph()
        .get_children()
        .last()
        .unwrap()
        .select(Some(start), Some(end))
        .unwrap();
}

fn toggle_selection_format(format: &str) {
    if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
        selection.toggle_format(format);
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > typing next to a gap in a mark does not spread the mark over the gap
#[test]
fn typing_next_to_a_gap_in_a_mark_does_not_spread_the_mark_over_the_gap() {
    let editor = create_test_editor();
    update(&editor, || init_with("kjisk"));
    mark(&editor, 0, 5, "c4");
    update(&editor, || remove_mark_id("c4", Some(&select(1, 3))));
    assert_eq!(tree(&editor), r##"paragraph("k"[c4] "ji" "sk"[c4])"##);
    run(&editor, || select_first_child(1, 1));
    type_text(&editor, "C");
    assert_eq!(tree(&editor), r##"paragraph("k"[c4] "Cji" "sk"[c4])"##);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > typing with a toggled format next to a gap in a mark
#[test]
fn typing_with_a_toggled_format_next_to_a_gap_in_a_mark() {
    let editor = create_test_editor();
    update(&editor, || init_with("kjisk"));
    mark(&editor, 0, 5, "c4");
    update(&editor, || remove_mark_id("c4", Some(&select(1, 3))));
    run(&editor, || {
        select_at(3);
        toggle_selection_format("bold");
    });
    type_text(&editor, "C");
    assert_eq!(tree(&editor), r##"paragraph("k"[c4] "ji" "C" "sk"[c4])"##);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > a caret set programmatically at the start of a run
#[test]
fn a_caret_set_programmatically_at_the_start_of_a_run() {
    // The core moves a caret at offset 0 of a TextNode to the end of the
    // previous one before inserting: the text must still stay out of the
    // marks that start there. With a root element, as the core normalizes
    // the caret as it does for a caret read from the DOM
    let with_root = || {
        create_test_editor_with(Options {
            with_root: true,
            ..Options::default()
        })
    };
    let editor = with_root();
    update(&editor, || init_with("aek"));
    mark(&editor, 0, 1, "c2");
    mark(&editor, 0, 1, "c3");
    mark(&editor, 1, 3, "c2");
    mark(&editor, 1, 3, "c5");
    assert_eq!(tree(&editor), r##"paragraph("a"[c2,c3] "ek"[c2,c5])"##);
    let places: [fn(); 2] = [
        || select_last_child(0, 0),
        || {
            paragraph()
                .get_children()
                .last()
                .unwrap()
                .select_start()
                .unwrap();
        },
    ];
    for place in places {
        let copy_editor = with_root();
        let state = copy_editor
            .parse_editor_state_json(&editor.get_editor_state().to_json(false))
            .unwrap();
        copy_editor.set_editor_state(&state, None).unwrap();
        run(&copy_editor, place);
        type_text(&copy_editor, "B");
        assert_eq!(
            tree(&copy_editor),
            r##"paragraph("a"[c2,c3] "B"[c2] "ek"[c2,c5])"##
        );
    }
    // And at the end of the first run
    run(&editor, || select_first_child(1, 1));
    type_text(&editor, "B");
    assert_eq!(
        tree(&editor),
        r##"paragraph("a"[c2,c3] "B"[c2] "ek"[c2,c5])"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > text inserted in the same update that moves the caret to an edge
#[test]
fn text_inserted_in_the_same_update_that_moves_the_caret_to_an_edge() {
    let editor = create_test_editor();
    update(&editor, || init_with("abcdef"));
    mark(&editor, 0, 3, "c1");
    run(&editor, || {
        select_at(5);
    });
    update(&editor, || {
        select_first_child(3, 3);
        if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
            selection.insert_text("X")?;
        }
        Ok(())
    });
    assert_eq!(tree(&editor), r##"paragraph("abc"[c1] "Xdef")"##);
}

/// `setupEditorWithMark()`.
fn setup_editor_with_mark() -> LexicalEditorWithDispose {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    editor
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > text replacing a whole marked run keeps its ids, text replacing across its edge does not
#[test]
fn text_replacing_a_whole_marked_run_keeps_its_ids_text_replacing_across_its_edge_does_not() {
    let editor = setup_editor_with_mark();
    run(&editor, || {
        select(8, 15);
    });
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "X"[c1] " in Q3.")"##
    );
    let across = setup_editor_with_mark();
    run(&across, || {
        select(5, 11);
    });
    type_text(&across, "X");
    assert_eq!(
        tree(&across),
        r##"paragraph("RevenX" "w 4%"[c1] " in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > composition inside a run, in a new TextNode for a toggled format
#[test]
fn composition_inside_a_run_in_a_new_text_node_for_a_toggled_format() {
    let editor = create_test_editor();
    update(&editor, || init_with("abcdef"));
    mark(&editor, 0, 6, "c1");
    run(&editor, || {
        select_at(3);
        toggle_selection_format("bold");
    });
    let ed = (*editor).clone();
    update(&editor, move || dispatch_composition_start(&ed));
    update(&editor, || {
        let selection = get_selection()
            .filter(SelectionRef::is_range)
            .expect("Expected a RangeSelection");
        let node = selection.range().unwrap().anchor.get_node()?;
        assert!(node.is_composing());
        node.set_text_content("日");
        node.select(Some(1), Some(1)).map(|_| ())
    });
    run(&editor, || set_composition_key(None));
    assert_eq!(
        tree(&editor),
        r##"paragraph("abc"[c1] "日"[c1] "def"[c1])"##
    );
    assert!(read(&editor, || get_root().get_all_text_nodes()[1]
        .has_format("bold")));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > pasting several nodes inside a run: all of them join it
#[test]
fn pasting_several_nodes_inside_a_run_all_of_them_join_it() {
    let editor = create_test_editor();
    update(&editor, || {
        init_with("aaaa")?;
        paragraph().append(&[create_text_node("bbbb")])
    });
    mark(&editor, 1, 3, "x");
    mark(&editor, 4, 8, "c1");
    assert_eq!(tree(&editor), r##"paragraph("a" "aa"[x] "a" "bbbb"[c1])"##);
    let json = copy(&editor, 1, 4);
    paste(&editor, json, || {
        select_at(6);
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("a" "aa"[x] "a" "bb"[c1] "aa"[c1,x] "abb"[c1])"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > pasted text keeps its own ids at the edge of a run
#[test]
fn pasted_text_keeps_its_own_ids_at_the_edge_of_a_run() {
    // The same ids as the run it is pasted at the end of
    let editor = create_test_editor();
    update(&editor, || {
        init_with("abcd")?;
        let bold = create_text_node("zz");
        bold.toggle_format("bold");
        paragraph().append(&[bold])
    });
    mark(&editor, 0, 4, "c1");
    let json = copy(&editor, 2, 4);
    paste(&editor, json, || select_first_child(4, 4));
    assert_eq!(tree(&editor), r##"paragraph("abcdcd"[c1] "zz")"##);
    // Other ids, at the start of a run
    let other = create_test_editor();
    update(&other, || init_with("xy abcd"));
    mark(&other, 0, 2, "c2");
    mark(&other, 3, 7, "c1");
    let json = copy(&other, 0, 2);
    paste(&other, json, || select_last_child(0, 0));
    assert_eq!(
        tree(&other),
        r##"paragraph("xy"[c2] " " "xy"[c2] "abcd"[c1])"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > pasted marked text inside a run keeps its ids and joins the run
#[test]
fn pasted_marked_text_inside_a_run_keeps_its_ids_and_joins_the_run() {
    let editor = create_test_editor();
    update(&editor, || init_with("xy abcd"));
    mark(&editor, 0, 2, "c2");
    mark(&editor, 3, 7, "c1");
    let json = copy(&editor, 0, 2);
    paste(&editor, json, || {
        select_at(5);
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("xy"[c2] " " "ab"[c1] "xy"[c1,c2] "cd"[c1])"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > text inserted with insertNodes keeps its own ids
#[test]
fn text_inserted_with_insert_nodes_keeps_its_own_ids() {
    let editor = setup_editor_with_mark();
    update(&editor, || {
        select_at(15);
        let node = create_text_node("new");
        insert_nodes(vec![node.clone()])?;
        add_mark_id("c1", &node.select(Some(0), Some(3))?)?;
        Ok(())
    });
    // Equal ids at the edge: merged into the run, and kept
    update(&editor, || {
        select_at(18);
        let node = create_text_node("!");
        node.toggle_format("bold");
        insert_nodes(vec![node])
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%new"[c1] "!" " in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > undo after removing marked text restores the output
#[test]
fn undo_after_removing_marked_text_restores_the_output() {
    // A history setEditorState that other transforms also dirty: the output
    // must not depend on the shape of its dirty sets
    for bold in [false, true] {
        let editor = setup_with_history();
        update(&editor, move || {
            let node = create_text_node("xqCm");
            let root = get_root();
            root.clear()?;
            let paragraph = create_paragraph_node();
            paragraph.append(std::slice::from_ref(&node))?;
            root.append(&[paragraph])?;
            if bold {
                node.toggle_format("bold");
            }
            add_mark_id("c2", &node.select(Some(0), Some(4))?)?;
            Ok(())
        });
        editor.dispatch_command(&CLEAR_HISTORY_COMMAND, ()).unwrap();
        run(&editor, || select_first_child(3, 3));
        let ed = (*editor).clone();
        update(&editor, move || {
            ed.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
            Ok(())
        });
        run(&editor, || {
            paragraph_at(1)
                .get_first_child()
                .unwrap()
                .select(Some(0), Some(1))
                .unwrap();
        });
        let ed = (*editor).clone();
        update(&editor, move || {
            ed.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
            Ok(())
        });
        editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
        assert_eq!(
            tree(&editor),
            r##"paragraph("xqC"[c2]) / paragraph("m"[c2])"##
        );
        assert_eq!(registry_text(&editor), registry(&[("c2", &["m", "xqC"])]));
        update(&editor, || remove_mark_id("c2", None));
        assert_eq!(tree(&editor), r##"paragraph("xqC") / paragraph("m")"##);
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > a setEditorState of a state from elsewhere rescans the output
#[test]
fn a_set_editor_state_of_a_state_from_elsewhere_rescans_the_output() {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    let marked = editor.get_editor_state();
    update(&editor, || remove_mark_id("c1", None));
    assert_eq!(mark_id_node_keys_size(&editor), 0);
    // A transform that dirties another node during the setEditorState
    let unregister = editor.register_node_transform("root", |root| {
        for node in root.get_all_text_nodes() {
            if node.get_text_content() == " in Q3." {
                node.set_text_content(" in Q4.");
            }
        }
        Ok(())
    });
    editor.set_editor_state(&marked, None).unwrap();
    unregister();
    assert_eq!(registry_text(&editor), registry(&[("c1", &["grew 4%"])]));
}
