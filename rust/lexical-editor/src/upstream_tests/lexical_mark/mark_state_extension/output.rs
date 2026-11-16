//! "output", "strict ids", "queries", "HTML" and "ids".
//!
//! The "DOM" and "isMarkIDShown" groups, and "strict ids > invalid ids
//! from JSON are dropped" (which reads the ids attribute of the element the
//! reconciler rendered), are skipped: the renderer half of
//! `MarkStateExtension.ts` isn't ported (rust/PARITY.md).

use super::*;
use crate::html::generate_html_from_nodes;
use crate::html::import::generate_nodes_from_dom_via_extension;
use crate::mark::{
    MARK_STATE_IDS_ATTRIBUTE, MARK_STATE_KEY, MarkIDsMode, create_mark_node,
    get_mark_ids_at_selection, migrate_mark_nodes_to_mark_state, remove_mark_id,
};
use crate::node_state::get_writable_node_state;

/// `editor.update(fn, {discrete: true})`, with its error.
fn try_update(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
) -> Result<(), LexicalError> {
    editor.update(
        f,
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        },
    )
}

/// `$getRoot().clear().append(...$generateNodesFromDOMViaExtension(dom))`
/// of `html` parsed with `DOMParser`.
fn import_html(editor: &Editor, html: String) {
    update(editor, move || {
        let dom = crate::dom::parse_from_string(&html);
        let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
        let root = get_root();
        root.clear()?;
        root.append(&nodes)
    });
}

fn html_of(editor: &Editor) -> String {
    let ed = editor.clone();
    read(editor, move || generate_html_from_nodes(&ed, None).unwrap())
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > output > after loading JSON, and $removeMarkID(id) through it
#[test]
fn after_loading_json_and_remove_mark_id_through_it() {
    let source = create_test_editor();
    update(&source, init);
    mark(&source, 8, 15, "c1");
    mark(&source, 0, 3, "c2");
    let json = source.get_editor_state().to_json(false).to_string();
    let editor = create_test_editor();
    let state = editor.parse_editor_state(&json).unwrap();
    editor.set_editor_state(&state, None).unwrap();
    assert_eq!(
        registry_text(&editor),
        registry(&[("c1", &["grew 4%"]), ("c2", &["Rev"])])
    );
    update(&editor, || remove_mark_id("c1", None));
    assert_eq!(registry_text(&editor), registry(&[("c2", &["Rev"])]));
    assert_eq!(
        tree(&editor),
        r##"paragraph("Rev"[c2] "enue grew 4% in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > output > drops the leaves of a removed paragraph, and only those
#[test]
fn drops_the_leaves_of_a_removed_paragraph_and_only_those() {
    let editor = create_test_editor();
    update(&editor, || {
        init()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("Two")])?;
        get_root().append(&[paragraph])
    });
    mark(&editor, 0, 3, "c1");
    update(&editor, || {
        let selection = create_range_selection();
        let node = paragraph_at(1).get_first_child_or_throw()?;
        selection.point_set(Which::Anchor, node.get_key(), 0, PointType::Text)?;
        selection.point_set(Which::Focus, node.get_key(), 3, PointType::Text)?;
        add_mark_id("c1", &selection)?;
        add_mark_id("c2", &selection)?;
        Ok(())
    });
    assert_eq!(
        registry_text(&editor),
        registry(&[("c1", &["Rev", "Two"]), ("c2", &["Two"])])
    );
    update(&editor, || paragraph_at(1).remove(false));
    assert_eq!(registry_text(&editor), registry(&[("c1", &["Rev"])]));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > output > disabled from the start: empty, and $removeMarkID(id) walks the document
#[test]
fn disabled_from_the_start_empty_and_remove_mark_id_walks_the_document() {
    let editor = create_test_editor_with(Options {
        disabled: true,
        ..Options::default()
    });
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    assert_eq!(mark_id_node_keys_size(&editor), 0);
    update(&editor, || remove_mark_id("c1", None));
    assert_eq!(tree(&editor), r##"paragraph("Revenue grew 4% in Q3.")"##);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > output > disabled with exportIDsToHTML: HTML export still writes the ids
#[test]
fn disabled_with_export_ids_to_html_html_export_still_writes_the_ids() {
    let editor = create_test_editor_with(Options {
        disabled: true,
        export_ids_to_html: true,
        ..Options::default()
    });
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    let html = html_of(&editor);
    assert!(
        html.contains(&format!("{MARK_STATE_IDS_ATTRIBUTE}=\"c1\"")),
        "{html}"
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > strict ids > invalid ids applied as NodeState or written with $setState are dropped
#[test]
fn invalid_ids_applied_as_node_state_or_written_with_set_state_are_dropped() {
    let editor = create_test_editor();
    update(&editor, init);
    update(&editor, || {
        let node = paragraph().get_first_child_or_throw()?;
        // How a state from elsewhere is applied
        get_writable_node_state(&node)?
            .borrow_mut()
            .update_from_unknown(
                MARK_STATE_KEY,
                Rc::new(Value::from(vec![
                    Value::from("b c"),
                    Value::from("x"),
                    Value::from(""),
                ])),
            );
        Ok(())
    });
    assert_eq!(tree(&editor), r##"paragraph("Revenue grew 4% in Q3."[x])"##);
    assert_eq!(
        registry_text(&editor),
        registry(&[("x", &["Revenue grew 4% in Q3."])])
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > strict ids > HTML import drops invalid tokens and keeps the valid ones
#[test]
fn html_import_drops_invalid_tokens_and_keeps_the_valid_ones() {
    let editor = create_test_editor();
    import_html(
        &editor,
        format!("<p><span {MARK_STATE_IDS_ATTRIBUTE}=\"  c1 \t  c2 \">x</span></p>"),
    );
    assert_eq!(tree(&editor), r##"paragraph("x"[c1,c2])"##);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > queries > $addMarkID returns the marked nodes, and a collapsed selection marks nothing
#[test]
fn add_mark_id_returns_the_marked_nodes_and_a_collapsed_selection_marks_nothing() {
    let editor = create_test_editor();
    update(&editor, init);
    update(&editor, || {
        assert!(add_mark_id("c1", &select_at(3))?.is_empty());
        let nodes = add_mark_id("c1", &select(8, 15))?;
        assert_eq!(
            nodes
                .iter()
                .map(NodeRef::get_text_content)
                .collect::<Vec<_>>(),
            ["grew 4%"]
        );
        // A collapsed selection removes nothing either
        remove_mark_id("c1", Some(&select_at(10)))
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " "grew 4%"[c1] " in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > queries > 'all' with nothing shared, and a backward selection
#[test]
fn all_with_nothing_shared_and_a_backward_selection() {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 0, 7, "c1");
    mark(&editor, 8, 15, "c2");
    read(&editor, || {
        let first = paragraph().get_children()[0].clone();
        let last = paragraph().get_children().last().unwrap().clone();
        let selection = create_range_selection();
        selection
            .point_set(Which::Anchor, last.get_key(), 0, PointType::Text)
            .unwrap();
        selection
            .point_set(Which::Focus, first.get_key(), 0, PointType::Text)
            .unwrap();
        assert!(selection.is_backward().unwrap());
        assert_eq!(
            get_mark_ids_at_selection(Some(&selection), MarkIDsMode::All).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            get_mark_ids_at_selection(Some(&selection), MarkIDsMode::Any).unwrap(),
            ["c1", "c2"]
        );
    });
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > queries > two editors with different markableNodes
#[test]
fn two_editors_with_different_markable_nodes() {
    // `[LineBreakNode]`: the class, as its guard (rust/PARITY.md)
    let line_breaks_only = create_test_editor_with(Options {
        markable_nodes: Some(vec![MarkableNodeMatch::new(|node: &NodeRef| {
            node.get_latest()
                .is::<crate::nodes::line_break::LineBreakNode>()
        })]),
        ..Options::default()
    });
    let defaults = create_test_editor();
    update(&line_breaks_only, init);
    update(&defaults, init);
    mark(&line_breaks_only, 8, 15, "c1");
    mark(&defaults, 8, 15, "c1");
    assert_eq!(
        tree(&line_breaks_only),
        r##"paragraph("Revenue grew 4% in Q3.")"##
    );
    assert_eq!(
        tree(&defaults),
        r##"paragraph("Revenue " "grew 4%"[c1] " in Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > HTML > ids are not exported by default
#[test]
fn ids_are_not_exported_by_default() {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    let html = html_of(&editor);
    assert!(html.contains("grew 4%"));
    assert!(!html.contains(MARK_STATE_IDS_ATTRIBUTE));
    assert!(!html.contains("c1"));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > HTML > export and import round trip the ids
#[test]
fn export_and_import_round_trip_the_ids() {
    let editor = create_test_editor_with(Options {
        export_ids_to_html: true,
        ..Options::default()
    });
    update(&editor, init);
    update(&editor, || select(10, 13).format_text("bold", None));
    mark(&editor, 8, 15, "c1");
    mark(&editor, 12, 19, "c2");
    let before = tree(&editor);
    let html = html_of(&editor);
    assert!(
        html.contains(&format!("{MARK_STATE_IDS_ATTRIBUTE}=\"c1 c2\"")),
        "{html}"
    );

    let target = create_test_editor();
    import_html(&target, html);
    assert_eq!(tree(&target), before);
    assert_eq!(registry_text(&target), registry_text(&editor));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > ids > ids must be non-empty and free of whitespace
#[test]
fn ids_must_be_non_empty_and_free_of_whitespace() {
    let editor = create_test_editor();
    update(&editor, init);
    for id in ["", "a b", "a\tb", "a\nb", " a"] {
        let error = try_update(&editor, move || {
            add_mark_id(id, &select(8, 15))?;
            Ok(())
        })
        .expect_err(id);
        assert!(error.to_string().contains("invalid mark id"), "{error}");
    }
    // Anything else is fine, and round trips through HTML
    let ids = ["a\"b", "a,b", "<x>", "__proto__", "é-1"];
    let exporting = create_test_editor_with(Options {
        export_ids_to_html: true,
        ..Options::default()
    });
    update(&exporting, init);
    for id in ids {
        mark(&exporting, 8, 15, id);
    }
    let html = html_of(&exporting);
    let target = create_test_editor();
    import_html(&target, html);
    let mut sorted: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
    sorted.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    assert_eq!(read(&target, || node_mark_ids_of_text("grew")), sorted);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > ids > migrating a MarkNode with an invalid id throws
#[test]
fn migrating_a_mark_node_with_an_invalid_id_throws() {
    let editor = create_test_editor();
    update(&editor, || {
        let mark = create_mark_node(&["a b".to_owned()]);
        mark.append(&[create_text_node("x")])?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[mark])?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    });
    let error = try_update(&editor, || {
        migrate_mark_nodes_to_mark_state(None)?;
        Ok(())
    })
    .expect_err("an invalid id");
    assert!(error.to_string().contains("invalid mark id"), "{error}");
}
