use super::*;
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
    typed(&mut rt, "Hi ");
    press(&mut rt, Key::Enter, Mods::default());
    typed(&mut rt, "x");
    press(&mut rt, Key::Backspace, Mods::default());
    let doc = rt.state.playground.document();
    assert_eq!(rt.state.playground.last_error(), None);
    assert_eq!(doc.blocks.len(), 3);
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
