use super::*;
use lexical_playground_core::format_flag;
use rune::{Env, Mods, Runtime};

fn runtime() -> Runtime<State> {
    let mut rt = Runtime::new(
        State::new(),
        view::view,
        (1100.0, 720.0),
        Env::new(false, 2.0),
    );
    rt.frame(0.0);
    rt
}

fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(0.0);
}

fn typed(rt: &mut Runtime<State>, s: &str) {
    for c in s.chars() {
        press(rt, Key::Text(c.to_string()), Mods::default());
    }
}

#[test]
fn typing_return_and_backspace_edit_the_lexical_document() {
    let mut rt = runtime();
    // The caret starts at the document's start.
    let blocks = rt.state.playground.document().blocks.len();
    typed(&mut rt, "Hi ");
    press(&mut rt, Key::Enter, Mods::default());
    typed(&mut rt, "x");
    press(&mut rt, Key::Backspace, Mods::default());
    let doc = rt.state.playground.document();
    assert_eq!(rt.state.playground.last_error(), None);
    assert_eq!(doc.blocks.len(), blocks + 1);
    assert_eq!(doc.blocks[0].text, "Hi ");
    assert!(doc.blocks[1].text.starts_with("Hello from "));
    // The field mirrors the document, with the caret where Lexical put it.
    assert_eq!(rt.state.edit.text, doc.text());
    assert_eq!(rt.state.edit.range(), ("Hi \n".len(), "Hi \n".len()));
}

#[test]
fn cmd_b_bolds_what_is_typed_next_and_the_field_draws_it_bold() {
    let mut rt = runtime();
    let cmd = Mods {
        cmd: true,
        ..Mods::default()
    };
    press(&mut rt, Key::Command('b'), cmd);
    typed(&mut rt, "Bold");
    let doc = rt.state.playground.document();
    let run = &doc.blocks[0].runs[0];
    assert_eq!((run.text.as_str(), run.has_format("bold")), ("Bold", true));
    assert!(
        rt.state
            .edit
            .marks()
            .iter()
            .any(|m| m.mark == Mark::Bold && m.range() == (0..4))
    );
}

#[test]
fn the_fields_own_change_replays_in_lexical() {
    let mut rt = runtime();
    // As an input method's commit: the field's text changes by itself.
    let new = rt.state.edit.text.replacen("Hello", "家", 1);
    rt.state.edit.text = new.clone();
    rt.state.changed();
    assert_eq!(rt.state.playground.document().text(), new);
    assert_eq!(rt.state.playground.last_error(), None);
}

#[test]
fn a_toolbar_button_formats_the_selection_and_keeps_the_keyboard() {
    let mut rt = runtime();
    // Select "Hello" in the field, as a drag would.
    rt.update(|s, _| s.edit.set_selection(0, 5));
    rt.frame(0.0);
    let (x, y) = rt.find("format-italic").expect("the italic button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    let doc = rt.state.playground.document();
    assert_eq!(doc.blocks[0].runs[0].text, "Hello");
    assert!(doc.blocks[0].runs[0].has_format("italic"));
    assert!(rt.state.edit.focused);
    typed(&mut rt, "!");
    assert!(
        rt.state.playground.document().blocks[0]
            .text
            .starts_with("!")
    );
}

/// Presses the button keyed `key`. Rune ignores a press on a button that
/// appeared that instant (the check button appears with the caret in a
/// check item), so a moment passes first, as it does for a person.
fn click(rt: &mut Runtime<State>, key: &str) {
    rt.frame(1.0);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("the {key} button"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

#[test]
fn the_field_draws_the_examples_lists_quote_and_link() {
    let rt = runtime();
    let doc = rt.state.playground.document();
    let kinds: Vec<&str> = doc.blocks.iter().map(|b| b.block_type()).collect();
    assert_eq!(
        kinds,
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
    assert_eq!(
        rt.state.edit.blocks(),
        &[
            Block::Paragraph,
            Block::Paragraph,
            Block::Paragraph,
            Block::Bullet(0),
            Block::Bullet(0),
            Block::Bullet(0),
            Block::Bullet(0),
            Block::Quote,
        ]
    );
    let link = rt
        .state
        .edit
        .marks()
        .iter()
        .find(|m| matches!(&m.mark, Mark::Link(url) if url == "https://lexical.dev"))
        .expect("the link's mark");
    assert_eq!(&rt.state.edit.text[link.range()], "lexical.dev");
    // The h2 draws larger and heavier.
    let heading = doc.offset_of(Pos { block: 2, byte: 0 });
    assert!(
        decorations(doc, false)
            .iter()
            .any(|s| s.start as usize == heading && s.size == Some(21.0))
    );
}

#[test]
fn the_block_buttons_convert_the_caret_block_and_show_its_type() {
    let mut rt = runtime();
    rt.update(|s, _| s.edit.set_selection(2, 2));
    rt.frame(0.0);
    click(&mut rt, "block-quote");
    assert_eq!(rt.state.playground.block_type(), "quote");
    assert_eq!(rt.state.edit.blocks()[0], Block::Quote);
    assert!(rt.state.edit.focused);
    click(&mut rt, "block-number");
    assert_eq!(rt.state.edit.blocks()[0], Block::Numbered(0));
    // Tab at an item's start nests it; Shift-Tab brings it back.
    rt.update(|s, _| s.edit.set_selection(0, 0));
    press(&mut rt, Key::Tab, Mods::default());
    assert_eq!(rt.state.edit.blocks()[0], Block::Numbered(1));
    press(
        &mut rt,
        Key::Tab,
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    assert_eq!(rt.state.edit.blocks()[0], Block::Numbered(0));
    // The same list again makes it a paragraph.
    click(&mut rt, "block-number");
    assert_eq!(rt.state.playground.block_type(), "paragraph");
    assert_eq!(rt.state.edit.blocks()[0], Block::Paragraph);
    assert_eq!(rt.state.playground.last_error(), None);
}

#[test]
fn the_link_button_links_the_selection_and_unlinks_it() {
    let mut rt = runtime();
    rt.update(|s, _| s.edit.set_selection(0, 5));
    rt.frame(0.0);
    click(&mut rt, "link");
    assert_eq!(rt.state.playground.link(), Some("https://"));
    assert!(rt.state.edit.marks().iter().any(|m| {
        matches!(&m.mark, Mark::Link(url) if url == "https://") && m.range() == (0..5)
    }));
    click(&mut rt, "link");
    assert_eq!(rt.state.playground.link(), None);
    assert!(
        !rt.state
            .edit
            .marks()
            .iter()
            .any(|m| matches!(&m.mark, Mark::Link(url) if url == "https://"))
    );
}

#[test]
fn the_check_button_toggles_the_caret_item_and_strikes_it_when_checked() {
    let mut rt = runtime();
    let checked = |rt: &Runtime<State>| {
        rt.state.playground.document().blocks[6]
            .list
            .as_ref()
            .is_some_and(|l| l.checked)
    };
    let struck = |rt: &Runtime<State>| {
        let doc = rt.state.playground.document();
        let item = doc.offset_of(Pos { block: 6, byte: 0 });
        let len = doc.blocks[6].text.len();
        rt.state
            .edit
            .marks()
            .iter()
            .any(|m| m.mark == Mark::Strike && m.range() == (item..item + len))
    };
    // The example's second check item starts unchecked.
    assert!(!checked(&rt) && !struck(&rt));
    let item = rt
        .state
        .playground
        .document()
        .offset_of(Pos { block: 6, byte: 3 });
    rt.update(|s, _| s.edit.set_selection(item, item));
    rt.frame(0.0);
    click(&mut rt, "check-item");
    assert!(checked(&rt) && struck(&rt));
    click(&mut rt, "check-item");
    assert!(!checked(&rt) && !struck(&rt));
    assert_eq!(rt.state.playground.last_error(), None);
}

#[test]
fn the_toolbar_follows_a_click_before_lexical_hears_of_it() {
    let mut rt = runtime();
    let steps = rt.state.playground.step_count();
    // Into "Rust", which is bold, then into the h2: no edit yet.
    rt.update(|s, _| s.edit.set_selection(13, 13));
    rt.frame(0.0);
    let format = rt.state.playground.format_at(rt.state.shown_selection());
    assert_eq!(format, format_flag("bold"));
    let h2 = rt
        .state
        .playground
        .document()
        .offset_of(Pos { block: 2, byte: 1 });
    rt.update(|s, _| s.edit.set_selection(h2, h2));
    rt.frame(0.0);
    let sel = rt.state.shown_selection();
    assert_eq!(rt.state.playground.block_type_at(sel), "h2");
    assert_eq!(rt.state.playground.step_count(), steps);
}
