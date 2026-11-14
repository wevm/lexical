use super::*;

fn playground() -> Playground {
    Playground::new(example()).expect("the example loads")
}

#[test]
fn reads_the_example_as_blocks_of_runs() {
    let p = playground();
    let doc = p.document();
    assert_eq!(doc.blocks.len(), 8);
    let first = &doc.blocks[0];
    assert_eq!(first.kind, "paragraph");
    assert_eq!(
        first.text,
        "Hello from Rust. This document lives in a lexical-editor editor."
    );
    assert!(first.runs[1].has_format("bold"));
    assert_eq!(first.runs[1].start, "Hello from ".len());
}

#[test]
fn points_and_positions_round_trip_across_utf16() {
    let mut p = Playground::new(vec![
        json::parse(r#"{"type":"paragraph","children":[{"type":"text","text":"a😀b"},{"type":"linebreak"},{"type":"text","text":"c","format":1}]}"#).unwrap(),
        json::parse(r#"{"type":"paragraph","children":[]}"#).unwrap(),
    ])
    .unwrap();
    let doc = p.document().clone();
    // After the emoji: byte 5, UTF-16 offset 3.
    let point = doc.point_at(Pos { block: 0, byte: 5 });
    assert_eq!((point.offset, point.text), (3, true));
    assert_eq!(doc.pos_of(&point), Some(Pos { block: 0, byte: 5 }));
    // Right after the line break: the start of the next text run.
    let after_break = doc.point_at(Pos { block: 0, byte: 7 });
    assert_eq!((after_break.offset, after_break.text), (0, true));
    // The empty paragraph: an element point.
    let empty = doc.point_at(Pos { block: 1, byte: 0 });
    assert_eq!((empty.offset, empty.text), (0, false));
    p.set_selection(Some(Sel::caret(Pos { block: 1, byte: 0 })));
    assert_eq!(p.insert_text("x").error, None);
    assert_eq!(p.document().blocks[1].text, "x");
    assert_eq!(p.selection(), Some(Sel::caret(Pos { block: 1, byte: 1 })));
}

#[test]
fn typing_enter_and_formatting_edit_the_document() {
    let mut p = playground();
    let end = Pos {
        block: 1,
        byte: p.document().blocks[1].text.len(),
    };
    p.set_selection(Some(Sel::caret(end)));
    assert_eq!(p.insert_paragraph().error, None);
    assert_eq!(p.document().blocks.len(), 9);
    assert_eq!(p.format_text("bold").error, None);
    assert!(p.has_format("bold"));
    assert_eq!(p.insert_text("Bold").error, None);
    let block = &p.document().blocks[2];
    assert_eq!(block.text, "Bold");
    assert!(block.runs[0].has_format("bold"));
    // Select "Hello" and italicize it.
    p.set_selection(Some(Sel {
        anchor: Pos { block: 0, byte: 0 },
        focus: Pos { block: 0, byte: 5 },
    }));
    assert_eq!(p.selected_text(), "Hello");
    assert_eq!(p.format_text("italic").error, None);
    let first = &p.document().blocks[0];
    assert_eq!(first.runs[0].text, "Hello");
    assert!(first.runs[0].has_format("italic"));
    assert_eq!(
        p.export_case("session")
            .get("steps")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(p.step_count())
    );
}

#[test]
fn undo_takes_back_typing_merged_by_time_and_redo_brings_it_back() {
    use std::cell::Cell;
    use std::rc::Rc;
    let now = Rc::new(Cell::new(0.0));
    let clock = now.clone();
    let mut p = Playground::with_clock(example(), Rc::new(move || clock.get())).unwrap();
    let second = |p: &Playground| p.document().blocks[1].text.clone();
    let original = second(&p);
    assert!(!p.can_undo());
    p.set_selection(Some(Sel::caret(Pos { block: 1, byte: 0 })));
    // "ab" within the delay is one undo step; "c" after it, another.
    for (t, ch) in [(1000.0, "a"), (1100.0, "b"), (2000.0, "c")] {
        now.set(t);
        assert_eq!(p.insert_text(ch).error, None);
    }
    assert_eq!(second(&p), format!("abc{original}"));
    assert!(p.can_undo());
    now.set(2100.0);
    assert_eq!(p.undo().error, None);
    assert_eq!(second(&p), format!("ab{original}"));
    assert_eq!(p.selection(), Some(Sel::caret(Pos { block: 1, byte: 2 })));
    assert_eq!(p.undo().error, None);
    assert_eq!(second(&p), original);
    assert!(!p.can_undo());
    assert!(p.can_redo());
    assert_eq!(p.redo().error, None);
    assert_eq!(second(&p), format!("ab{original}"));
    // A new edit drops the redo.
    now.set(3000.0);
    assert_eq!(p.insert_text("x").error, None);
    assert!(!p.can_redo());
    let case = p.export_case("undo");
    assert!(case.get("history").is_some());
    assert!(
        case.get("steps").and_then(Value::as_array).unwrap()[1]
            .get("now")
            .is_some()
    );
}

#[test]
fn a_change_the_field_made_replays_as_a_lexical_edit() {
    let mut p = playground();
    let old = p.document().text();
    // An input method commits "家" over "Hello" in the first block.
    let new = old.replacen("Hello", "家", 1);
    assert_eq!(p.apply_text(&new).map(|o| o.error), Some(None));
    assert_eq!(p.document().text(), new);
    // A cut across the two blocks.
    let cut = format!("家{}", &new[new.find("Type").unwrap()..]);
    assert_eq!(p.apply_text(&cut).map(|o| o.error), Some(None));
    // The two paragraphs are one; the heading, lists and quote follow.
    assert_eq!(p.document().blocks.len(), 7);
    assert_eq!(p.document().text(), cut);
    assert_eq!(p.apply_text(&cut), None);
    // Offsets round-trip through the joined text.
    let doc = p.document();
    let pos = doc.pos_at(doc.text().len());
    assert_eq!(doc.offset_of(pos), doc.text().len());
}

#[test]
fn deleting_at_a_caret_takes_a_grapheme_a_word_or_a_line() {
    let mut p = Playground::new(vec![
        json::parse(r#"{"type":"paragraph","children":[{"type":"text","text":"one two👍🏽"},{"type":"linebreak"},{"type":"text","text":"three"}]}"#).unwrap(),
        json::parse(r#"{"type":"paragraph","children":[{"type":"text","text":"x"}]}"#).unwrap(),
    ])
    .unwrap();
    let end_of_first_line = "one two👍🏽".len();
    let at = |byte| Some(Sel::caret(Pos { block: 0, byte }));
    // Backspace takes the emoji with its skin tone whole.
    p.set_selection(at(end_of_first_line));
    assert_eq!(p.delete_character(true).error, None);
    assert_eq!(p.document().blocks[0].text, "one two\nthree");
    // Option-Backspace takes "two".
    p.set_selection(at("one two".len()));
    assert_eq!(p.delete_word(true).error, None);
    assert_eq!(p.document().blocks[0].text, "one \nthree");
    // Command-Delete from the start of "three" to the block's end.
    p.set_selection(at("one \n".len()));
    assert_eq!(p.delete_line(false).error, None);
    assert_eq!(p.document().blocks[0].text, "one \n");
    // Backspace at a block's start merges it into the one before.
    p.set_selection(Some(Sel::caret(Pos { block: 1, byte: 0 })));
    assert_eq!(p.delete_character(true).error, None);
    assert_eq!(p.document().blocks.len(), 1);
    assert_eq!(p.document().blocks[0].text, "one \nx");
}

#[test]
fn deleting_past_the_documents_ends_does_nothing() {
    let mut p = playground();
    p.set_selection(Some(Sel::caret(Pos::default())));
    assert_eq!(p.delete_character(true), Outcome::default());
    let end = p.document().end();
    p.set_selection(Some(Sel::caret(end)));
    assert_eq!(p.delete_word(false), Outcome::default());
    assert_eq!(p.step_count(), 1, "no step ran");
}

fn surface() -> Surface {
    Surface::new(
        Playground::new(vec![
            json::parse(r#"{"type":"paragraph","children":[{"type":"text","text":"a😀b"}]}"#)
                .unwrap(),
            json::parse(
                r#"{"type":"paragraph","children":[{"type":"text","text":"bold","format":1}]}"#,
            )
            .unwrap(),
        ])
        .unwrap(),
    )
}

#[test]
fn the_surface_takes_typing_and_ranges_in_utf16() {
    let mut s = surface();
    assert_eq!(s.display_text(), "a😀b\nbold");
    assert_eq!(s.text_length_utf16(), 9);
    // After the emoji: UTF-16 offset 3, byte 5.
    s.set_selected_range_utf16(3..3);
    assert_eq!(s.selection(), (5..5, false));
    assert_eq!(s.replace_text_utf16(None, "x").error, None);
    assert_eq!(s.display_text(), "a😀xb\nbold");
    assert_eq!(s.caret(), 6);
    // Replacing a given range: the emoji.
    assert_eq!(s.replace_text_utf16(Some(1..3), "é").error, None);
    assert_eq!(s.display_text(), "aéxb\nbold");
    assert_eq!(s.text_for_range_utf16(0..2), ("aé".to_owned(), 0..2));
    // A backward selection.
    s.move_to(4, false);
    s.move_to(1, true);
    assert_eq!(s.selection(), (1..4, true));
    assert_eq!(s.selected_text(), "éx");
}

#[test]
fn a_composition_shows_without_reaching_lexical_until_committed() {
    let mut s = surface();
    s.move_to(1, false);
    s.replace_and_mark_utf16(None, "か", Some(1..1));
    assert_eq!(s.display_text(), "aか😀b\nbold");
    assert_eq!(s.marked_range(), Some(1..4));
    assert_eq!(s.selection(), (4..4, false));
    assert_eq!(
        s.playground.document().text(),
        "a😀b\nbold",
        "Lexical hasn't seen it"
    );
    let runs = s.runs();
    assert!(runs.iter().any(|r| r.composing && r.range == (1..4)));
    let covered: usize = runs.iter().map(|r| r.range.len()).sum();
    assert_eq!(covered, s.display_text().len(), "the runs cover the text");
    // The input method replaces its composition, then commits.
    s.replace_and_mark_utf16(None, "かな", None);
    assert_eq!(s.display_text(), "aかな😀b\nbold");
    assert_eq!(s.replace_text_utf16(None, "仮名").error, None);
    assert_eq!(s.display_text(), "a仮名😀b\nbold");
    assert_eq!(s.marked_range(), None);
    assert_eq!(s.caret(), "a仮名".len());
    // An empty composition ends it without an edit.
    let steps = s.playground.step_count();
    s.replace_and_mark_utf16(None, "x", None);
    s.replace_and_mark_utf16(None, "", None);
    assert_eq!(s.display_text(), "a仮名😀b\nbold");
    assert_eq!(s.playground.step_count(), steps);
}

#[test]
fn the_surface_draws_formats_and_edits_through_lexical() {
    let mut s = surface();
    let bold = s
        .runs()
        .into_iter()
        .find(|r| r.format == format_flag("bold"));
    assert_eq!(bold.map(|r| r.range), Some(7..11));
    s.move_to(s.display_text().len(), false);
    assert_eq!(s.edit(Playground::insert_paragraph).error, None);
    assert_eq!(s.replace_text_utf16(None, "new").error, None);
    assert_eq!(s.display_text(), "a😀b\nbold\nnew");
    assert_eq!(s.delete(Unit::Character, true).error, None);
    assert_eq!(s.display_text(), "a😀b\nbold\nne");
    s.select_all();
    assert_eq!(s.edit(|p| p.format_text("italic")).error, None);
    assert!(s.playground.has_format("italic"));
}

#[test]
fn format_toggles_on_and_off_with_one_press_each() {
    // ⌘B over "dogs" splits it into its own bold node and selects that
    // node. The selection's start is also the end of "lazy ", and the next
    // ⌘B must keep Lexical's point, not send that one, or it bolds again.
    let mut p = Playground::new(vec![
        json::parse(r#"{"type":"paragraph","children":[{"type":"text","text":"lazy dogs."}]}"#)
            .unwrap(),
    ])
    .unwrap();
    let dogs = Sel {
        anchor: Pos { block: 0, byte: 5 },
        focus: Pos { block: 0, byte: 9 },
    };
    p.set_selection(Some(dogs));
    let bold_runs = |p: &Playground| -> Vec<String> {
        p.document().blocks[0]
            .runs
            .iter()
            .filter(|r| r.has_format("bold"))
            .map(|r| r.text.clone())
            .collect()
    };
    for (press, bold) in [true, false, true, false].into_iter().enumerate() {
        let steps = p.step_count();
        assert_eq!(p.format_text("bold").error, None);
        let want: Vec<String> = if bold { vec!["dogs".into()] } else { vec![] };
        assert_eq!(bold_runs(&p), want, "press {press}");
        assert_eq!(p.has_format("bold"), bold, "press {press}");
        assert_eq!(p.selection(), Some(dogs), "press {press}");
        // The first press also gives the editor the selection.
        assert_eq!(p.step_count() - steps, if press == 0 { 2 } else { 1 });
    }
}

#[test]
fn a_click_gives_the_selection_the_format_there_as_selectionchange_does() {
    // "Hello from **Rust**": a caret in "Rust" takes its bold, so the
    // toolbar shows bold before any edit and typing there stays bold, as in
    // a browser; a range shows only the format all of its text has.
    let mut p = playground();
    let in_rust = Sel::caret(Pos { block: 0, byte: 13 });
    assert_eq!(p.format_at(in_rust), format_flag("bold"));
    let across = Sel {
        anchor: Pos { block: 0, byte: 6 },
        focus: Pos { block: 0, byte: 15 },
    };
    assert_eq!(p.format_at(across), 0);
    // A range that ends at the start of "Rust" covers none of it.
    let before = Sel {
        anchor: Pos { block: 0, byte: 6 },
        focus: Pos { block: 0, byte: 11 },
    };
    assert_eq!(p.format_at(before), 0);
    let rust = Sel {
        anchor: Pos { block: 0, byte: 11 },
        focus: Pos { block: 0, byte: 15 },
    };
    assert_eq!(p.format_at(rust), format_flag("bold"));
    p.set_selection(Some(in_rust));
    assert!(p.has_format("bold"));
    let steps = p.step_count();
    assert_eq!(p.insert_text("y").error, None);
    let runs: Vec<(&str, bool)> = p.document().blocks[0]
        .runs
        .iter()
        .map(|r| (r.text.as_str(), r.has_format("bold")))
        .collect();
    assert_eq!(runs[1], ("Ruyst", true));
    // The select step, then the typing.
    assert_eq!(p.step_count() - steps, 2);
}

fn caret_in(p: &mut Playground, block: usize, byte: usize) {
    p.set_selection(Some(Sel::caret(Pos { block, byte })));
}

#[test]
fn reads_headings_lists_links_and_quotes() {
    let p = playground();
    let doc = p.document();
    let types: Vec<&str> = doc.blocks.iter().map(Block::block_type).collect();
    assert_eq!(
        types,
        [
            "paragraph",
            "paragraph",
            "h2",
            "bullet",
            "bullet",
            "check",
            "check",
            "quote"
        ]
    );
    let link = doc.blocks[4]
        .runs
        .iter()
        .find(|r| r.link.is_some())
        .unwrap();
    assert_eq!(
        (link.text.as_str(), link.link.as_deref()),
        ("lexical.dev", Some("https://lexical.dev"))
    );
    // The link's runs are its parent's second child.
    assert_eq!(link.child, 1);
    assert!(doc.blocks[5].list.as_ref().unwrap().checked);
    assert!(!doc.blocks[6].list.as_ref().unwrap().checked);
    // An element point in the root is the start of the block it names.
    let root = Point {
        key: "root".into(),
        offset: 3,
        text: false,
    };
    assert_eq!(doc.pos_of(&root), Some(Pos { block: 3, byte: 0 }));
    // After the link: an element point after the block's second child.
    let after_link = doc.blocks[4].runs.iter().find(|r| r.text == ".").unwrap();
    let point = doc.point_at(Pos {
        block: 4,
        byte: after_link.start + 1,
    });
    assert!(point.text);
}

#[test]
fn the_block_menu_converts_blocks_and_toggles_lists() {
    let mut p = playground();
    caret_in(&mut p, 1, 3);
    assert_eq!(p.set_block_type("h1").error, None);
    assert_eq!(p.document().blocks[1].block_type(), "h1");
    assert_eq!(p.block_type(), "h1");
    assert_eq!(p.set_block_type("quote").error, None);
    assert_eq!(p.block_type(), "quote");
    assert_eq!(p.set_block_type("number").error, None);
    assert_eq!(p.block_type(), "number");
    // The same list again makes it a paragraph.
    assert_eq!(p.set_block_type("number").error, None);
    assert_eq!(p.block_type(), "paragraph");
    assert_eq!(
        p.document().blocks[1].text,
        "Type, delete, press Enter, or select text and make it bold."
    );
}

#[test]
fn enter_in_a_list_item_adds_an_item_and_an_empty_one_ends_the_list() {
    let mut p = playground();
    let end = p.document().blocks[3].text.len();
    caret_in(&mut p, 3, end);
    assert_eq!(p.insert_paragraph().error, None);
    let doc = p.document();
    assert_eq!(doc.blocks[4].block_type(), "bullet");
    assert_eq!(doc.blocks[4].text, "");
    assert_eq!(p.selection(), Some(Sel::caret(Pos { block: 4, byte: 0 })));
    // Enter in the empty item: a paragraph after the list.
    assert_eq!(p.insert_paragraph().error, None);
    assert_eq!(p.block_type(), "paragraph");
    assert_eq!(p.document().blocks.len(), 9);
}

#[test]
fn indenting_a_list_item_nests_it() {
    let mut p = playground();
    caret_in(&mut p, 4, 2);
    assert_eq!(p.indent(false).error, None);
    let item = &p.document().blocks[4];
    assert_eq!(item.list.as_ref().map(|l| l.depth), Some(1));
    assert_eq!(p.indent(true).error, None);
    assert_eq!(
        p.document().blocks[4].list.as_ref().map(|l| l.depth),
        Some(0)
    );
}

#[test]
fn links_toggle_on_a_selection_and_off_at_a_caret() {
    let mut p = playground();
    p.set_selection(Some(Sel {
        anchor: Pos { block: 0, byte: 0 },
        focus: Pos { block: 0, byte: 5 },
    }));
    assert_eq!(p.toggle_link(Some("https://example.com")).error, None);
    let run = &p.document().blocks[0].runs[0];
    assert_eq!(
        (run.text.as_str(), run.link.as_deref()),
        ("Hello", Some("https://example.com"))
    );
    caret_in(&mut p, 0, 2);
    assert_eq!(p.link(), Some("https://example.com"));
    assert_eq!(p.toggle_link(None).error, None);
    assert!(p.document().blocks[0].runs.iter().all(|r| r.link.is_none()));
    assert_eq!(
        p.document().blocks[0].text,
        "Hello from Rust. This document lives in a lexical-editor editor."
    );
}

#[test]
fn clicking_a_check_box_toggles_the_item() {
    let mut p = playground();
    assert_eq!(p.toggle_checked(6).error, None);
    assert!(p.document().blocks[6].list.as_ref().unwrap().checked);
    assert_eq!(p.undo().error, None);
    assert!(!p.document().blocks[6].list.as_ref().unwrap().checked);
}

#[test]
fn the_tree_shows_lists_above_their_items() {
    let tree = playground().document().tree();
    assert!(
        tree.contains("  (11) list bullet\n    (12) listitem bullet\n"),
        "{tree}"
    );
    assert!(tree.contains("  (19) list check\n    (20) listitem check ☑\n"));
}

/// Types `text` a character at a time, as keys do.
fn type_keys(p: &mut Playground, text: &str) {
    for c in text.chars() {
        assert_eq!(p.insert_text(&c.to_string()).error, None, "typing {c:?}");
    }
}

#[test]
fn markdown_shortcuts_turn_typed_markdown_into_rich_text() {
    let mut p = Playground::new(vec![]).expect("an empty document");
    caret_in(&mut p, 0, 0);
    type_keys(&mut p, "# Title");
    assert_eq!(p.document().blocks[0].block_type(), "h1");
    assert_eq!(p.document().blocks[0].text, "Title");
    p.insert_paragraph();
    type_keys(&mut p, "- one **bold** and `code`");
    let item = &p.document().blocks[1];
    assert_eq!(item.block_type(), "bullet");
    assert_eq!(item.text, "one bold and code");
    let run = |text: &str| item.runs.iter().find(|r| r.text == text).unwrap();
    assert!(run("bold").has_format("bold"));
    assert!(run("code").has_format("code"));
    // An empty item ends the list; then a code fence on Enter.
    p.insert_paragraph();
    p.insert_paragraph();
    type_keys(&mut p, "```rust");
    p.insert_paragraph();
    type_keys(&mut p, "let x = 1;");
    let code = &p.document().blocks[2];
    assert_eq!(code.kind, "code");
    assert_eq!(code.text, "let x = 1;");
    // Undo takes the code back to the fence's text.
    let steps = p.step_count();
    p.undo();
    p.undo();
    assert_eq!(p.document().blocks[2].text, "```rust");
    assert_eq!(p.step_count(), steps + 2);
}
