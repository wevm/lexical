//! "links", "migration and compatibility", "ids in JSON are a set",
//! "slots" and "markable nodes".
//!
//! "markable nodes > by default TextNode, LineBreakNode and inline
//! decorators, including DecoratorTextNode" is skipped: it also reads the
//! ids attribute the renderer writes on the reconciled element
//! (rust/parity/skips.json).

use super::*;
use crate::link::toggle_link;
use crate::mark::{
    MarkIDsMode, create_mark_node, get_mark_ids_at_selection, migrate_mark_nodes_to_mark_state,
    remove_mark_id,
};
use crate::nodes::line_break::create_line_break_node;
use crate::{create_node_selection, get_selection, set_slot};

/// `$toggleLink('https://example.com')` of [8, 12).
fn link_grew() -> Result<(), LexicalError> {
    select(8, 12);
    toggle_link(Some("https://example.com"), &Default::default())
}

/// `setupLinked()`: "Revenue " link("grew"[c1]) " 4% in"[c1] " Q3."
fn setup_linked() -> LexicalEditorWithDispose {
    let editor = create_test_editor();
    update(&editor, init);
    update(&editor, link_grew);
    mark(&editor, 8, 18, "c1");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " link("grew"[c1]) " 4% in"[c1] " Q3.")"##
    );
    editor
}

fn select_child(index: usize, offset: usize) {
    paragraph().get_children()[index]
        .select(Some(offset), Some(offset))
        .unwrap();
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > typing at the start of a run right after a marked link
#[test]
fn typing_at_the_start_of_a_run_right_after_a_marked_link() {
    let editor = setup_linked();
    run(&editor, || select_child(2, 0));
    type_text(&editor, "X");
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " link("grew"[c1]) "X 4% in"[c1] " Q3.")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > typing at the end of marked link text
#[test]
fn typing_at_the_end_of_marked_link_text() {
    let editor = setup_linked();
    run(&editor, || {
        paragraph().get_all_text_nodes()[1]
            .select(Some(4), Some(4))
            .unwrap();
    });
    type_text(&editor, "X");
    assert_eq!(read(&editor, || node_mark_ids_of_text("X")), ["c1"]);
    assert_eq!(
        read(&editor, || {
            paragraph()
                .get_all_text_nodes()
                .iter()
                .map(NodeRef::get_text_content)
                .collect::<String>()
        }),
        "Revenue grewX 4% in Q3."
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > typing with a toggled format at the link boundary
#[test]
fn typing_with_a_toggled_format_at_the_link_boundary() {
    let editor = setup_linked();
    run(&editor, || {
        select_child(2, 0);
        if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
            selection.toggle_format("bold");
        }
    });
    type_text(&editor, "X");
    assert_eq!(read(&editor, || node_mark_ids_of_text("X")), ["c1"]);
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > typing after the end of a marked link that ends the mark
#[test]
fn typing_after_the_end_of_a_marked_link_that_ends_the_mark() {
    let editor = create_test_editor();
    update(&editor, init);
    update(&editor, link_grew);
    mark(&editor, 8, 12, "c1");
    run(&editor, || select_child(2, 0));
    type_text(&editor, "X");
    assert_eq!(
        read(&editor, || node_mark_ids_of_text("X")),
        Vec::<String>::new()
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > the caret after a marked link is in the mark
#[test]
fn the_caret_after_a_marked_link_is_in_the_mark() {
    let editor = create_test_editor();
    update(&editor, init);
    update(&editor, link_grew);
    mark(&editor, 8, 12, "c1");
    run(&editor, || {
        // End of "Revenue ", right before the link
        paragraph().get_all_text_nodes()[0]
            .select(Some(8), Some(8))
            .unwrap();
        assert_eq!(
            get_mark_ids_at_selection(get_selection().as_ref(), MarkIDsMode::Any).unwrap(),
            ["c1"]
        );
    });
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > migration and compatibility > a MarkNode document migrates to the same tree
#[test]
fn a_mark_node_document_migrates_to_the_same_tree() {
    let editor = create_test_editor();
    update(&editor, init);
    // Nested MarkNodes (no nested element resolver registered)
    run(&editor, || {
        wrap_selection_in_mark_node(&select(8, 15), false, "c1", None).unwrap();
    });
    run(&editor, || {
        wrap_selection_in_mark_node(&select(12, 19), false, "c2", None).unwrap();
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue " mark[c1]("grew" mark[c2](" 4%")) mark[c2](" in ") "Q3.")"##
    );
    let count = Rc::new(std::cell::Cell::new(0));
    let c = count.clone();
    update(&editor, move || {
        c.set(migrate_mark_nodes_to_mark_state(None)?);
        Ok(())
    });
    assert_eq!(count.get(), 3);

    let direct = create_test_editor();
    update(&direct, init);
    mark(&direct, 8, 15, "c1");
    mark(&direct, 12, 19, "c2");
    assert_eq!(tree(&editor), tree(&direct));
    assert_eq!(registry_text(&editor), registry_text(&direct));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > migration and compatibility > a MarkNode with several ids migrates
#[test]
fn a_mark_node_with_several_ids_migrates() {
    let editor = create_test_editor();
    update(&editor, || {
        let mark = create_mark_node(&["x".to_owned(), "y".to_owned()]);
        mark.append(&[create_text_node("b")])?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("a"), mark])?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    });
    update(&editor, || {
        migrate_mark_nodes_to_mark_state(None).map(|_| ())
    });
    assert_eq!(tree(&editor), r##"paragraph("a" "b"[x,y])"##);
}

/// `editor.setEditorState(editor.parseEditorState(json))`.
fn load(editor: &Editor, json: &str) {
    let state = editor.parse_editor_state(json).unwrap();
    editor.set_editor_state(&state, None).unwrap();
}

fn json_of(editor: &Editor) -> String {
    editor.get_editor_state().to_json(false).to_string()
}

/// `buildEditorFromExtensions` of `[RichTextExtension]` with `nodes`.
fn rich_text_editor(nodes: Vec<NodeClass>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        nodes,
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > migration and compatibility > an editor without the extension keeps the ids
#[test]
fn an_editor_without_the_extension_keeps_the_ids() {
    let editor = create_test_editor();
    update(&editor, init);
    mark(&editor, 8, 15, "c1");
    mark(&editor, 12, 19, "c2");
    let json = json_of(&editor);

    let plain = rich_text_editor(vec![]);
    load(&plain, &json);
    update(&plain, || {
        paragraph().select_start()?;
        get_selection().unwrap().insert_text("Q4 ")
    });
    let round_trip = create_test_editor();
    load(&round_trip, &json_of(&plain));
    assert_eq!(
        tree(&round_trip),
        r##"paragraph("Q4 Revenue " "grew"[c1] " 4%"[c1,c2] " in "[c2] "Q3.")"##
    );
    // The output is rebuilt from a loaded editor state
    assert_eq!(
        registry_text(&round_trip),
        registry(&[("c1", &[" 4%", "grew"]), ("c2", &[" 4%", " in "])])
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > ids in JSON are a set: unordered input is saved sorted and without duplicates
#[test]
fn ids_in_json_are_a_set_unordered_input_is_saved_sorted_and_without_duplicates() {
    let editor = create_test_editor();
    update(&editor, init);
    // `paragraph.children[0].$ = {markIds: ['c2', 'c1', 'c2', 7]}`
    let json = json_of(&editor);
    let text = r##""text":"Revenue grew 4% in Q3.""##;
    assert!(json.contains(text), "{json}");
    let json = json.replacen(
        text,
        &format!(r##""$":{{"markIds":["c2","c1","c2",7]}},{text}"##),
        1,
    );
    let loaded = create_test_editor();
    load(&loaded, &json);
    assert_eq!(
        tree(&loaded),
        r##"paragraph("Revenue grew 4% in Q3."[c1,c2])"##
    );
    assert_eq!(
        registry_text(&loaded),
        registry(&[
            ("c1", &["Revenue grew 4% in Q3."]),
            ("c2", &["Revenue grew 4% in Q3."])
        ])
    );
    let saved = json_of(&loaded);
    assert_eq!(
        paragraph_child_states(&loaded)[0],
        mark_ids_state(&["c1", "c2"])
    );
    // Stable across another round trip
    let again = create_test_editor();
    load(&again, &saved);
    assert_eq!(json_of(&again), saved);
}

/// `$initFigure()`: paragraph(P) / figure{caption paragraph} /
/// paragraph("After it.")
fn init_figure() -> Result<(), LexicalError> {
    init()?;
    let figure = crate::create::<FigureNode>();
    let after = create_paragraph_node();
    after.append(&[create_text_node("After it.")])?;
    get_root().append(&[figure.clone(), after])?;
    let caption = create_paragraph_node();
    caption.append(&[create_text_node("A caption")])?;
    set_slot(&figure, "caption", &caption)?;
    Ok(())
}

/// `$selectAcrossFigure()`.
fn select_across_figure() -> SelectionRef {
    let (first, _) = locate(&paragraph_at(0), 8, false);
    let (last, _) = locate(&paragraph_at(2), 5, false);
    let selection = create_range_selection();
    selection
        .point_set(Which::Anchor, first.get_key(), 8, PointType::Text)
        .unwrap();
    selection
        .point_set(Which::Focus, last.get_key(), 5, PointType::Text)
        .unwrap();
    selection
}

fn mark_across_figure() -> Result<(), LexicalError> {
    add_mark_id("c1", &select_across_figure()).map(|_| ())
}

const MARKED: &str = concat!(
    r##"paragraph("Revenue " "grew 4% in Q3."[c1]) / "##,
    r##"test-figure{paragraph("A caption"[c1])} / "##,
    r##"paragraph("After"[c1] " it.")"##
);

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > marking a range marks the content of the slots in it
#[test]
fn marking_a_range_marks_the_content_of_the_slots_in_it() {
    let editor = create_test_editor();
    update(&editor, init_figure);
    update(&editor, mark_across_figure);
    assert_eq!(tree(&editor), MARKED);
    assert_eq!(
        registry_text(&editor),
        registry(&[("c1", &["A caption", "After", "grew 4% in Q3."])])
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > a NodeSelection of a slot host marks the content of its slots
#[test]
fn a_node_selection_of_a_slot_host_marks_the_content_of_its_slots() {
    let editor = create_test_editor();
    update(&editor, init_figure);
    update(&editor, || {
        let selection = create_node_selection();
        selection.add(get_root().get_child_at_index(1).unwrap().get_key());
        add_mark_id("c1", &selection).map(|_| ())
    });
    assert_eq!(
        tree(&editor),
        concat!(
            r##"paragraph("Revenue grew 4% in Q3.") / "##,
            r##"test-figure{paragraph("A caption"[c1])} / paragraph("After it.")"##
        )
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > removing an id removes it from the slots too
#[test]
fn removing_an_id_removes_it_from_the_slots_too() {
    for with_extension in [true, false] {
        let editor = create_test_editor();
        update(&editor, init_figure);
        update(&editor, mark_across_figure);
        let json = json_of(&editor);
        // Without the extension, $removeMarkID walks the document
        let other;
        let target: &Editor = if with_extension {
            &editor
        } else {
            other = rich_text_editor(vec![NodeClass::of::<FigureNode>()]);
            &other
        };
        load(target, &json);
        update(target, || remove_mark_id("c1", None));
        assert_eq!(
            tree(target),
            concat!(
                r##"paragraph("Revenue grew 4% in Q3.") / "##,
                r##"test-figure{paragraph("A caption")} / paragraph("After it.")"##
            )
        );
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > the output is rebuilt from the slots of a loaded state
#[test]
fn the_output_is_rebuilt_from_the_slots_of_a_loaded_state() {
    let editor = create_test_editor();
    update(&editor, init_figure);
    update(&editor, mark_across_figure);
    let loaded = create_test_editor();
    load(&loaded, &json_of(&editor));
    assert_eq!(tree(&loaded), MARKED);
    assert_eq!(
        registry_text(&loaded),
        registry(&[("c1", &["A caption", "After", "grew 4% in Q3."])])
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > a MarkNode inside a slot migrates
#[test]
fn a_mark_node_inside_a_slot_migrates() {
    let editor = create_test_editor();
    update(&editor, || {
        init_figure()?;
        let caption = get_slot(&get_root().get_child_at_index(1).unwrap(), "caption").unwrap();
        let text = caption.get_first_child_or_throw()?;
        assert!(text.is_text());
        let parts = text.split_text(&[2])?;
        let rest = parts[1].clone();
        let mark_node = create_mark_node(&["c2".to_owned()]);
        rest.replace(&mark_node, false)?;
        mark_node.append(&[rest])
    });
    update(&editor, || {
        assert_eq!(migrate_mark_nodes_to_mark_state(None)?, 1);
        Ok(())
    });
    assert_eq!(
        tree(&editor),
        concat!(
            r##"paragraph("Revenue grew 4% in Q3.") / "##,
            r##"test-figure{paragraph("A " "caption"[c2])} / paragraph("After it.")"##
        )
    );
}

/// `$initMixed()`.
fn init_mixed() -> Result<(), LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[
        create_text_node("ab"),
        crate::create::<TestDecoratorTextNode>(),
        create_line_break_node(),
        create_chip_node(),
        create_text_node("cd"),
    ])?;
    let root = get_root();
    root.clear()?;
    root.append(&[paragraph])
}

/// `$selectAll()`: from the start of the first text to the end of the
/// last.
fn select_all_mixed() -> SelectionRef {
    let paragraph = paragraph();
    let selection = create_range_selection();
    selection
        .point_set(
            Which::Anchor,
            paragraph.get_first_child().unwrap().get_key(),
            0,
            PointType::Text,
        )
        .unwrap();
    selection
        .point_set(
            Which::Focus,
            paragraph.get_children().last().unwrap().get_key(),
            2,
            PointType::Text,
        )
        .unwrap();
    selection
}

fn mark_all_mixed() -> Result<(), LexicalError> {
    add_mark_id("c1", &select_all_mixed()).map(|_| ())
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > markable nodes > a class list
#[test]
fn a_class_list() {
    // `[TextNode]`: the class, as its guard (rust/PARITY.md)
    let editor = create_test_editor_with(Options {
        markable_nodes: Some(vec![MarkableNodeMatch::new(NodeRef::is_text)]),
        ..Options::default()
    });
    update(&editor, init_mixed);
    update(&editor, mark_all_mixed);
    assert_eq!(
        tree(&editor),
        r##"paragraph("ab"[c1] test-decorator-text br test-chip "cd"[c1])"##
    );
    assert_eq!(registry_text(&editor), registry(&[("c1", &["ab", "cd"])]));
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > markable nodes > a guard function
#[test]
fn a_guard_function() {
    let editor = create_test_editor_with(Options {
        markable_nodes: Some(vec![
            MarkableNodeMatch::new(|node: &NodeRef| {
                node.is_text() && !node.get_text_content().contains('c')
            }),
            MarkableNodeMatch::new(|node: &NodeRef| {
                node.get_latest().is::<TestDecoratorTextNode>()
            }),
        ]),
        ..Options::default()
    });
    update(&editor, init_mixed);
    update(&editor, mark_all_mixed);
    assert_eq!(
        tree(&editor),
        r##"paragraph("ab"[c1] test-decorator-text[c1] br test-chip "cd")"##
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > markable nodes > extending the default with a block decorator
#[test]
fn extending_the_default_with_a_block_decorator() {
    let mut markable = default_markable_nodes();
    markable.push(MarkableNodeMatch::new(|node: &NodeRef| {
        node.get_latest().is::<FigureNode>()
    }));
    let editor = create_test_editor_with(Options {
        markable_nodes: Some(markable),
        ..Options::default()
    });
    update(&editor, || {
        init()?;
        get_root().append(&[crate::create::<FigureNode>()])
    });
    update(&editor, || {
        let selection = create_node_selection();
        selection.add(get_root().get_children().last().unwrap().get_key());
        add_mark_id("c1", &selection).map(|_| ())
    });
    assert_eq!(
        tree(&editor),
        r##"paragraph("Revenue grew 4% in Q3.") / test-figure[c1]{}"##
    );
    assert_eq!(registry_text(&editor), registry(&[("c1", &[""])]));
}
