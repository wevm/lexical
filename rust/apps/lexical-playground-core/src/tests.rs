use super::*;

fn playground() -> Playground {
    Playground::new(example()).expect("the example loads")
}

#[test]
fn reads_the_example_as_blocks_of_runs() {
    let p = playground();
    let doc = p.document();
    assert_eq!(doc.blocks.len(), 2);
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
    assert_eq!(p.document().blocks.len(), 3);
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
    assert_eq!(p.document().blocks.len(), 1);
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
