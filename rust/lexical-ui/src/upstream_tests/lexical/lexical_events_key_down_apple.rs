//! Ports of `packages/lexical/src/__tests__/unit/LexicalEventsKeyDownApple.test.ts`,
//! with `keyDownDispatchParity.ts`, which it and `LexicalEventsKeyDown.test.ts`
//! run. JS mocks `IS_APPLE` for the module; Rust runs the test under
//! [`with_is_apple`](crate::with_is_apple).

#![cfg_attr(not(feature = "extension"), allow(dead_code, unused_imports))]

use crate::{
    COMMAND_PRIORITY_CRITICAL, COPY_COMMAND, CUT_COMMAND, DELETE_CHARACTER_COMMAND,
    DELETE_LINE_COMMAND, DELETE_WORD_COMMAND, Event, FORMAT_TEXT_COMMAND,
    INSERT_LINE_BREAK_COMMAND, KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND,
    KEY_ARROW_RIGHT_COMMAND, KEY_ARROW_UP_COMMAND, KEY_BACKSPACE_COMMAND, KEY_DELETE_COMMAND,
    KEY_DOWN_COMMAND, KEY_ENTER_COMMAND, KEY_ESCAPE_COMMAND, KEY_MODIFIER_COMMAND,
    KEY_SPACE_COMMAND, KEY_TAB_COMMAND, KeyboardEvent, KeyboardEventModifierMask, MOVE_TO_END,
    MOVE_TO_START, ModifierExpectation, REDO_COMMAND, SELECT_ALL_COMMAND, UNDO_COMMAND,
    control_or_alt, control_or_meta, is_exact_shortcut_match,
};
use std::cell::RefCell;
use std::rc::Rc;

/// A recorded payload: the dispatched event (by identity), or a value.
#[derive(Clone, Debug, PartialEq)]
enum Payload {
    Event(*const KeyboardEvent),
    NoEvent,
    Bool(bool),
    Str(String),
    Unit,
}

/// `{command, payload, preventsDefault}`; the command by name.
struct ExpectedDispatch {
    command: &'static str,
    payload: Payload,
    prevents_default: bool,
}

/// A mask: `None` is `'any'`.
fn mask(
    alt: Option<bool>,
    ctrl: Option<bool>,
    meta: Option<bool>,
    shift: Option<bool>,
) -> KeyboardEventModifierMask {
    let e = |v: Option<bool>| match v {
        None => ModifierExpectation::Any,
        Some(true) => ModifierExpectation::Pressed,
        Some(false) => ModifierExpectation::Released,
    };
    KeyboardEventModifierMask {
        alt_key: e(alt),
        ctrl_key: e(ctrl),
        meta_key: e(meta),
        shift_key: e(shift),
        control_or_other: None,
    }
}

const ANY: Option<bool> = None;
const NO: Option<bool> = Some(false);
const YES: Option<bool> = Some(true);

/// `referenceKeyDown(event, isApple)`.
fn reference_key_down(event: &Rc<KeyboardEvent>, is_apple: bool) -> Option<ExpectedDispatch> {
    let m =
        |key: &str, mask: KeyboardEventModifierMask| is_exact_shortcut_match(&**event, key, &mask);
    let ev = Payload::Event(Rc::as_ptr(event));
    let dispatched = |command: &'static str, payload: Payload, prevents_default: bool| {
        Some(ExpectedDispatch {
            command,
            payload,
            prevents_default,
        })
    };
    if m("ArrowRight", mask(NO, NO, NO, ANY)) {
        dispatched("KEY_ARROW_RIGHT_COMMAND", ev, false)
    } else if if is_apple {
        m("ArrowRight", mask(NO, NO, YES, ANY))
    } else {
        m("End", mask(NO, NO, NO, ANY))
    } {
        dispatched("MOVE_TO_END", ev, false)
    } else if m("ArrowLeft", mask(NO, NO, NO, ANY)) {
        dispatched("KEY_ARROW_LEFT_COMMAND", ev, false)
    } else if if is_apple {
        m("ArrowLeft", mask(NO, NO, YES, ANY))
    } else {
        m("Home", mask(NO, NO, NO, ANY))
    } {
        dispatched("MOVE_TO_START", ev, false)
    } else if m("ArrowUp", mask(ANY, NO, NO, ANY)) {
        dispatched("KEY_ARROW_UP_COMMAND", ev, false)
    } else if m("ArrowDown", mask(ANY, NO, NO, ANY)) {
        dispatched("KEY_ARROW_DOWN_COMMAND", ev, false)
    } else if m("Enter", mask(ANY, ANY, ANY, YES)) {
        dispatched("KEY_ENTER_COMMAND", ev, false)
    } else if event.key == " " {
        dispatched("KEY_SPACE_COMMAND", ev, false)
    } else if is_apple && m("o", mask(NO, YES, NO, NO)) {
        dispatched("INSERT_LINE_BREAK_COMMAND", Payload::Bool(true), true)
    } else if m("Enter", mask(ANY, ANY, ANY, NO)) {
        dispatched("KEY_ENTER_COMMAND", ev, false)
    } else if m("Backspace", mask(NO, NO, NO, ANY)) || (is_apple && m("h", mask(NO, YES, NO, NO))) {
        if event.key == "Backspace" {
            dispatched("KEY_BACKSPACE_COMMAND", ev, false)
        } else {
            dispatched("DELETE_CHARACTER_COMMAND", Payload::Bool(true), true)
        }
    } else if event.key == "Escape" {
        dispatched("KEY_ESCAPE_COMMAND", ev, false)
    } else if m("Delete", mask(NO, NO, NO, NO)) || (is_apple && m("d", mask(NO, YES, NO, NO))) {
        if event.key == "Delete" {
            dispatched("KEY_DELETE_COMMAND", ev, false)
        } else {
            dispatched("DELETE_CHARACTER_COMMAND", Payload::Bool(false), true)
        }
    } else if m("Backspace", control_or_alt()) {
        dispatched("DELETE_WORD_COMMAND", Payload::Bool(true), true)
    } else if m("Delete", control_or_alt()) {
        dispatched("DELETE_WORD_COMMAND", Payload::Bool(false), true)
    } else if is_apple && m("Backspace", mask(NO, NO, YES, NO)) {
        dispatched("DELETE_LINE_COMMAND", Payload::Bool(true), true)
    } else if is_apple && (m("Delete", mask(NO, NO, YES, NO)) || m("k", mask(NO, YES, NO, NO))) {
        dispatched("DELETE_LINE_COMMAND", Payload::Bool(false), true)
    } else if m("b", control_or_meta()) {
        dispatched("FORMAT_TEXT_COMMAND", Payload::Str("bold".into()), true)
    } else if m("u", control_or_meta()) {
        dispatched(
            "FORMAT_TEXT_COMMAND",
            Payload::Str("underline".into()),
            true,
        )
    } else if m("i", control_or_meta()) {
        dispatched("FORMAT_TEXT_COMMAND", Payload::Str("italic".into()), true)
    } else if m("Tab", mask(NO, NO, NO, ANY)) {
        dispatched("KEY_TAB_COMMAND", ev, false)
    } else if m("z", control_or_meta()) {
        dispatched("UNDO_COMMAND", Payload::Unit, true)
    } else if if is_apple {
        m("z", mask(NO, NO, YES, YES))
    } else {
        m("y", mask(NO, YES, NO, NO)) || m("z", mask(NO, YES, NO, YES))
    } {
        dispatched("REDO_COMMAND", Payload::Unit, true)
    } else if m("a", control_or_meta()) {
        dispatched("SELECT_ALL_COMMAND", ev, true)
    } else {
        // isCopy / isCut are gated on the previous selection being a
        // non-null, non-range selection; with no selection they dispatch
        // nothing.
        None
    }
}

const GRID_KEYS: [(&str, &str); 25] = [
    ("ArrowRight", "ArrowRight"),
    ("ArrowLeft", "ArrowLeft"),
    ("ArrowUp", "ArrowUp"),
    ("ArrowDown", "ArrowDown"),
    ("Enter", "Enter"),
    (" ", "Space"),
    ("Backspace", "Backspace"),
    ("Delete", "Delete"),
    ("Escape", "Escape"),
    ("Tab", "Tab"),
    ("a", "KeyA"),
    ("b", "KeyB"),
    ("c", "KeyC"),
    ("d", "KeyD"),
    ("h", "KeyH"),
    ("i", "KeyI"),
    ("k", "KeyK"),
    ("o", "KeyO"),
    ("q", "KeyQ"),
    ("u", "KeyU"),
    ("x", "KeyX"),
    ("y", "KeyY"),
    ("z", "KeyZ"),
    // Non-Latin layouts: event.key is non-ASCII, matching falls back to code
    ("и", "KeyB"),
    ("я", "KeyZ"),
];

type Recorded = Rc<RefCell<Vec<(&'static str, Payload)>>>;

/// `runKeyDownDispatchParityTests(isApple)`'s test.
#[cfg(feature = "extension")]
pub(super) fn run_key_down_dispatch_parity_test(is_apple: bool) {
    use crate::extension::build_editor_from_extensions;
    use crate::{
        Editor, LexicalCommand, LexicalExtension, define_extension, merge_register::merge_register,
    };

    crate::with_is_apple(is_apple, || {
        let recorded: Recorded = Rc::default();
        let log = recorded.clone();
        let editor = build_editor_from_extensions([define_extension(LexicalExtension {
            register: Some(Rc::new(move |editor2: &Editor, _, _| {
                fn observe<P: 'static>(
                    editor: &Editor,
                    log: &Recorded,
                    command: &LexicalCommand<P>,
                    name: &'static str,
                    payload: fn(&P) -> Payload,
                ) -> crate::Unregister {
                    let log = log.clone();
                    editor.register_command(
                        command,
                        move |p, _| {
                            log.borrow_mut().push((name, payload(p)));
                            Ok(true)
                        },
                        COMMAND_PRIORITY_CRITICAL,
                    )
                }
                let ev = |e: &Rc<KeyboardEvent>| Payload::Event(Rc::as_ptr(e));
                let l = &log;
                Ok(merge_register(vec![
                    observe(
                        editor2,
                        l,
                        &KEY_ARROW_RIGHT_COMMAND,
                        "KEY_ARROW_RIGHT_COMMAND",
                        ev,
                    ),
                    observe(
                        editor2,
                        l,
                        &KEY_ARROW_LEFT_COMMAND,
                        "KEY_ARROW_LEFT_COMMAND",
                        ev,
                    ),
                    observe(
                        editor2,
                        l,
                        &KEY_ARROW_UP_COMMAND,
                        "KEY_ARROW_UP_COMMAND",
                        ev,
                    ),
                    observe(
                        editor2,
                        l,
                        &KEY_ARROW_DOWN_COMMAND,
                        "KEY_ARROW_DOWN_COMMAND",
                        ev,
                    ),
                    observe(editor2, l, &MOVE_TO_END, "MOVE_TO_END", ev),
                    observe(editor2, l, &MOVE_TO_START, "MOVE_TO_START", ev),
                    observe(editor2, l, &KEY_ENTER_COMMAND, "KEY_ENTER_COMMAND", |e| {
                        e.as_ref()
                            .map_or(Payload::NoEvent, |e| Payload::Event(Rc::as_ptr(e)))
                    }),
                    observe(editor2, l, &KEY_SPACE_COMMAND, "KEY_SPACE_COMMAND", ev),
                    observe(
                        editor2,
                        l,
                        &INSERT_LINE_BREAK_COMMAND,
                        "INSERT_LINE_BREAK_COMMAND",
                        |b| Payload::Bool(*b),
                    ),
                    observe(
                        editor2,
                        l,
                        &KEY_BACKSPACE_COMMAND,
                        "KEY_BACKSPACE_COMMAND",
                        ev,
                    ),
                    observe(editor2, l, &KEY_DELETE_COMMAND, "KEY_DELETE_COMMAND", ev),
                    observe(editor2, l, &KEY_ESCAPE_COMMAND, "KEY_ESCAPE_COMMAND", ev),
                    observe(editor2, l, &KEY_TAB_COMMAND, "KEY_TAB_COMMAND", ev),
                    observe(
                        editor2,
                        l,
                        &DELETE_CHARACTER_COMMAND,
                        "DELETE_CHARACTER_COMMAND",
                        |b| Payload::Bool(*b),
                    ),
                    observe(
                        editor2,
                        l,
                        &DELETE_WORD_COMMAND,
                        "DELETE_WORD_COMMAND",
                        |b| Payload::Bool(*b),
                    ),
                    observe(
                        editor2,
                        l,
                        &DELETE_LINE_COMMAND,
                        "DELETE_LINE_COMMAND",
                        |b| Payload::Bool(*b),
                    ),
                    observe(
                        editor2,
                        l,
                        &FORMAT_TEXT_COMMAND,
                        "FORMAT_TEXT_COMMAND",
                        |s| Payload::Str(s.clone()),
                    ),
                    observe(editor2, l, &UNDO_COMMAND, "UNDO_COMMAND", |_| Payload::Unit),
                    observe(editor2, l, &REDO_COMMAND, "REDO_COMMAND", |_| Payload::Unit),
                    observe(editor2, l, &SELECT_ALL_COMMAND, "SELECT_ALL_COMMAND", ev),
                    observe(editor2, l, &COPY_COMMAND, "COPY_COMMAND", |_| Payload::Unit),
                    observe(editor2, l, &CUT_COMMAND, "CUT_COMMAND", |_| Payload::Unit),
                    observe(
                        editor2,
                        l,
                        &KEY_MODIFIER_COMMAND,
                        "KEY_MODIFIER_COMMAND",
                        ev,
                    ),
                ]))
            })),
            ..LexicalExtension::<(), (), ()>::new("keydown-parity-test", ())
        })
        .into()])
        .unwrap();

        for (key, code) in GRID_KEYS {
            for bits in 0..16 {
                let event = Rc::new(KeyboardEvent {
                    event: Event::new("keydown"),
                    key: key.to_owned(),
                    code: code.to_owned(),
                    alt_key: bits & 1 != 0,
                    ctrl_key: bits & 2 != 0,
                    meta_key: bits & 4 != 0,
                    shift_key: bits & 8 != 0,
                    ..KeyboardEvent::default()
                });
                recorded.borrow_mut().clear();
                editor
                    .dispatch_command(&KEY_DOWN_COMMAND, event.clone())
                    .unwrap();

                let label = format!(
                    "key={key} code={code} alt={} ctrl={} meta={} shift={}",
                    event.alt_key, event.ctrl_key, event.meta_key, event.shift_key
                );
                let expected = reference_key_down(&event, is_apple);
                let recorded = recorded.borrow();
                let modifier_dispatches = recorded
                    .iter()
                    .filter(|(c, _)| *c == "KEY_MODIFIER_COMMAND")
                    .count();
                let dispatches: Vec<_> = recorded
                    .iter()
                    .filter(|(c, _)| *c != "KEY_MODIFIER_COMMAND")
                    .collect();
                // KEY_MODIFIER_COMMAND is always dispatched (in addition to
                // any matched shortcut) when at least one modifier is pressed
                assert_eq!(modifier_dispatches, usize::from(bits != 0), "{label}");
                match expected {
                    None => {
                        assert!(dispatches.is_empty(), "{label}: {dispatches:?}");
                        assert!(!event.default_prevented(), "{label}");
                    }
                    Some(expected) => {
                        assert_eq!(dispatches.len(), 1, "{label}: {dispatches:?}");
                        assert_eq!(dispatches[0].0, expected.command, "{label}");
                        assert_eq!(dispatches[0].1, expected.payload, "{label}");
                        assert_eq!(
                            event.default_prevented(),
                            expected.prevents_default,
                            "{label}"
                        );
                    }
                }
            }
        }
        editor.dispose();
    });
}

// upstream: lexical/src/__tests__/unit/LexicalEventsKeyDownApple.test.ts::$handleKeyDown dispatch parity (IS_APPLE=true) > dispatches the same commands as the legacy predicate chain
#[cfg(feature = "extension")]
#[test]
fn dispatches_the_same_commands_as_the_legacy_predicate_chain() {
    run_key_down_dispatch_parity_test(true);
}
