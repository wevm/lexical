//! Ports of `packages/lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts`.
//!
//! `vi.fn()` spies are counters and recorded arguments.

use crate::extension::{
    FormatKeyboardShortcutOptions, KeyboardShortcutsConfigOverrides, LexicalEditorWithDispose,
    NamedKeyboardShortcuts, build_editor_from_extensions, format_keyboard_shortcut,
    get_extension_dependency_from_editor, keyboard_shortcuts_extension, nested_editor_extension,
};
use crate::{
    AnyLexicalExtensionArgument, COMMAND_PRIORITY_BEFORE_EDITOR, COMMAND_PRIORITY_CRITICAL,
    COMMAND_PRIORITY_EDITOR, COMMAND_PRIORITY_HIGH, COMMAND_PRIORITY_LOW, COMMAND_PRIORITY_NORMAL,
    Editor, KEY_DOWN_COMMAND, KeyboardEvent, KeyboardEventModifierMask, KeyboardShortcut,
    KeyboardShortcutMatch, LexicalCommand, LexicalExtension, ModifierExpectation, Unregister,
    compile_keyboard_shortcuts, config_extension, control_or_alt, control_or_meta, create_command,
    define_extension, is_exact_shortcut_match, merge_register,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ModifierExpectation::{Any, Pressed};

/// A `{key, modifiers, name}` object literal.
#[derive(Clone, Debug, PartialEq)]
struct Match {
    key: &'static str,
    modifiers: KeyboardEventModifierMask,
    name: &'static str,
}

impl KeyboardShortcutMatch for Match {
    fn key(&self) -> &str {
        self.key
    }
    fn modifiers(&self) -> KeyboardEventModifierMask {
        self.modifiers
    }
}

fn mask(
    alt: ModifierExpectation,
    ctrl: ModifierExpectation,
    meta: ModifierExpectation,
    shift: ModifierExpectation,
) -> KeyboardEventModifierMask {
    KeyboardEventModifierMask {
        alt_key: alt,
        ctrl_key: ctrl,
        meta_key: meta,
        shift_key: shift,
        control_or_other: None,
    }
}

const NO: ModifierExpectation = ModifierExpectation::Released;

fn ctrl() -> KeyboardEventModifierMask {
    mask(NO, Pressed, NO, NO)
}

/// `makeEvent(key, code, bits)`.
fn make_event(key: &str, code: &str, bits: u8) -> KeyboardEvent {
    KeyboardEvent {
        key: key.to_owned(),
        code: code.to_owned(),
        alt_key: bits & 1 != 0,
        ctrl_key: bits & 2 != 0,
        meta_key: bits & 4 != 0,
        shift_key: bits & 8 != 0,
        ..KeyboardEvent::default()
    }
}

/// `keyboardEvent({key, ...modifiers})`, the modifiers given as the bits of
/// [`make_event`].
fn keyboard_event(key: &str, bits: u8) -> Rc<KeyboardEvent> {
    Rc::new(make_event(key, "", bits))
}

const ALT: u8 = 1;
const CTRL: u8 = 2;
const META: u8 = 4;
const SHIFT: u8 = 8;

fn ctrl_k() -> Rc<KeyboardEvent> {
    keyboard_event("k", CTRL)
}

type Command = LexicalCommand<Rc<KeyboardEvent>>;
type Listener = (Command, Rc<dyn Fn(&Rc<KeyboardEvent>) -> bool>);

fn listener(command: &Command, f: impl Fn(&Rc<KeyboardEvent>) -> bool + 'static) -> Listener {
    (command.clone(), Rc::new(f))
}

/// `vi.fn().mockReturnValue(result)`, recording the events it is called with.
#[derive(Clone, Default)]
struct Spy(Rc<RefCell<Vec<Rc<KeyboardEvent>>>>);

impl Spy {
    fn listener(&self, command: &Command, result: bool) -> Listener {
        let calls = self.0.clone();
        listener(command, move |event| {
            calls.borrow_mut().push(event.clone());
            result
        })
    }
    fn count(&self) -> usize {
        self.0.borrow().len()
    }
}

fn shortcut(
    command: &Command,
    key: &str,
    modifiers: KeyboardEventModifierMask,
) -> KeyboardShortcut {
    KeyboardShortcut {
        modifiers,
        ..KeyboardShortcut::new(key, command.clone())
    }
}

fn table(entries: Vec<(&str, Option<Vec<KeyboardShortcut>>)>) -> NamedKeyboardShortcuts {
    entries
        .into_iter()
        .fold(NamedKeyboardShortcuts::new(), |t, (name, s)| {
            t.with(name, s)
        })
}

fn shortcuts(table: NamedKeyboardShortcuts) -> KeyboardShortcutsConfigOverrides {
    KeyboardShortcutsConfigOverrides {
        shortcuts: Some(table),
        ..Default::default()
    }
}

fn build_with(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
    listeners: Vec<Listener>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        register: Some(Rc::new(move |editor: &Editor, _, _| {
            Ok(merge_register(
                listeners
                    .iter()
                    .map(|(command, listener)| {
                        let listener = listener.clone();
                        editor.register_command(
                            command,
                            move |event, _| Ok(listener(event)),
                            COMMAND_PRIORITY_EDITOR,
                        )
                    })
                    .collect(),
            ))
        })),
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `buildTestEditor(shortcuts, listeners)`.
fn build_test_editor(
    table: NamedKeyboardShortcuts,
    listeners: Vec<Listener>,
) -> LexicalEditorWithDispose {
    build_with(
        "keyboard-shortcuts-test",
        vec![config_extension(
            &keyboard_shortcuts_extension(),
            [shortcuts(table)],
        )],
        listeners,
    )
}

/// `buildLayeredEditor(layers, listeners)`.
fn build_layered_editor(
    layers: Vec<KeyboardShortcutsConfigOverrides>,
    listeners: Vec<Listener>,
) -> LexicalEditorWithDispose {
    build_with(
        "layered-test",
        layers
            .into_iter()
            .map(|layer| config_extension(&keyboard_shortcuts_extension(), [layer]))
            .collect(),
        listeners,
    )
}

fn output(editor: &Editor) -> Rc<crate::extension::KeyboardShortcutsOutput> {
    get_extension_dependency_from_editor(editor, &keyboard_shortcuts_extension())
        .unwrap()
        .output
        .unwrap()
}

fn key_down(editor: &Editor, event: Rc<KeyboardEvent>) -> bool {
    editor.dispatch_command(&KEY_DOWN_COMMAND, event).unwrap()
}

/// `commandRecorder()`.
#[derive(Default)]
struct Recorder {
    calls: Rc<RefCell<Vec<String>>>,
    listeners: Vec<Listener>,
}

impl Recorder {
    /// `ctrlKShortcut(name, handled)`.
    fn ctrl_k_shortcut(&mut self, name: &str, handled: bool) -> KeyboardShortcut {
        let command = create_command(&format!("recorder/{name}"));
        let calls = self.calls.clone();
        let name = name.to_owned();
        self.listeners.push(listener(&command, move |_| {
            calls.borrow_mut().push(name.clone());
            handled
        }));
        shortcut(&command, "k", ctrl())
    }
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    fn clear(&self) {
        self.calls.borrow_mut().clear();
    }
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::compileKeyboardShortcuts > matches exactly the events that isExactShortcutMatch matches
#[test]
fn compile_matches_exactly_what_is_exact_shortcut_match_matches() {
    let event_keys = [
        ("b", "KeyB"),
        ("B", "KeyB"),
        ("б", "KeyB"),
        ("1", "Digit1"),
        ("!", "Digit1"),
        ("١", "Digit1"),
        ("Enter", "Enter"),
        (",", "Comma"),
        ("[", "BracketLeft"),
        ("z", "KeyZ"),
    ];
    let shortcuts = [
        ("b", ctrl()),
        ("B", mask(NO, NO, Pressed, NO)),
        ("1", mask(Pressed, Pressed, NO, NO)),
        ("Enter", mask(NO, NO, NO, Any)),
        (",", ctrl()),
        ("[", KeyboardEventModifierMask::default()),
        ("z", mask(NO, Pressed, NO, Any)),
    ];
    for (key, modifiers) in shortcuts {
        let compiled = compile_keyboard_shortcuts([Match {
            key,
            modifiers,
            name: "",
        }])
        .unwrap();
        for (event_key, event_code) in event_keys {
            for bits in 0..16 {
                let event = make_event(event_key, event_code, bits);
                assert_eq!(
                    compiled.r#match(&event).is_some(),
                    is_exact_shortcut_match(&event, key, &modifiers),
                    "key={key} modifiers={modifiers:?} event={event_key}/{event_code}/{bits}"
                );
            }
        }
    }
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::compileKeyboardShortcuts > matches returns all matching shortcuts in insertion order
#[test]
fn compile_matches_returns_all_matches_in_insertion_order() {
    let first = Match {
        key: "k",
        modifiers: ctrl(),
        name: "first",
    };
    let second = Match {
        key: "K",
        modifiers: mask(NO, Pressed, NO, Any),
        name: "second",
    };
    let other = Match {
        key: "j",
        modifiers: ctrl(),
        name: "other",
    };
    let compiled = compile_keyboard_shortcuts([first.clone(), second.clone(), other]).unwrap();
    let names = |event: KeyboardEvent| -> Vec<&'static str> {
        compiled.matches(&event).iter().map(|m| m.name).collect()
    };
    assert_eq!(names(make_event("k", "KeyK", 2)), [first.name, second.name]);
    assert_eq!(names(make_event("K", "KeyK", 2 | 8)), [second.name]);
    assert!(names(make_event("k", "KeyK", 0)).is_empty());
}

fn format(shortcut: &Match, is_apple: bool) -> Vec<String> {
    format_keyboard_shortcut(
        shortcut,
        &FormatKeyboardShortcutOptions {
            is_apple: Some(is_apple),
            separator: None,
        },
    )
}

fn m(key: &'static str, modifiers: KeyboardEventModifierMask) -> Match {
    Match {
        key,
        modifiers,
        name: "",
    }
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > CONTROL_OR_META
#[test]
fn format_control_or_meta() {
    let shortcut = m(" ", control_or_meta());
    assert_eq!(format(&shortcut, true), ["\u{2318}", "Space"]);
    assert_eq!(format(&shortcut, false), ["Ctrl", "Space"]);
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > CONTROL_OR_ALT
#[test]
fn format_control_or_alt() {
    let shortcut = m(" ", control_or_alt());
    assert_eq!(format(&shortcut, true), ["\u{2325}", "Space"]);
    assert_eq!(format(&shortcut, false), ["Ctrl", "Space"]);
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'k', modifiers: [Object] }) -> apple: ⇧+⌘+K other: Shift+Meta+K
// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'q', modifiers: [Object] }) -> apple: ⌃+⇧+Q other: Ctrl+Shift+Q
// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'q', modifiers: [Object] }) -> apple: ⇧+⌘+Q other: Ctrl+Shift+Q
// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'Backspace', …(1) }) -> apple: ⌥+⌫ other: Ctrl+Backspace
// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: ' ' }) -> apple: Space other: Space
// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'ArrowLeft', …(1) }) -> apple: ← other: ArrowLeft
#[test]
fn format_table() {
    let cases = [
        (
            m("k", mask(NO, NO, Pressed, Pressed)),
            "⇧+⌘+K",
            "Shift+Meta+K",
        ),
        (
            m("q", mask(NO, Pressed, NO, Pressed)),
            "\u{2303}+⇧+Q",
            "Ctrl+Shift+Q",
        ),
        (
            m(
                "q",
                KeyboardEventModifierMask {
                    shift_key: Pressed,
                    ..control_or_meta()
                },
            ),
            "⇧+⌘+Q",
            "Ctrl+Shift+Q",
        ),
        (
            m("Backspace", control_or_alt()),
            "\u{2325}+\u{232B}",
            "Ctrl+Backspace",
        ),
        (
            m(" ", KeyboardEventModifierMask::default()),
            "Space",
            "Space",
        ),
        (
            m("ArrowLeft", mask(NO, NO, NO, Any)),
            "\u{2190}",
            "ArrowLeft",
        ),
    ];
    for (shortcut, apple, other) in cases {
        assert_eq!(
            [true, false].map(|is_apple| format(&shortcut, is_apple).join("+")),
            [apple, other],
            "{shortcut:?}"
        );
    }
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formats platform conventions
#[test]
fn format_platform_conventions() {
    let shortcut = m("k", mask(NO, NO, Pressed, Pressed));
    assert_eq!(format(&shortcut, true).join("+"), "⇧+⌘+K");
    assert_eq!(format(&shortcut, false).join("+"), "Shift+Meta+K");
    assert_eq!(
        format(&m("q", mask(NO, Pressed, NO, Pressed)), true).join("+"),
        "⌃+⇧+Q"
    );
    assert_eq!(
        format(&m("0", mask(Pressed, Pressed, NO, NO)), false).join("+"),
        "Ctrl+Alt+0"
    );
    assert_eq!(
        format(&m(" ", KeyboardEventModifierMask::default()), false).join("+"),
        "Space"
    );
    assert_eq!(
        format(&m("ArrowLeft", mask(NO, NO, NO, Any)), false).join("+"),
        "ArrowLeft"
    );
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > dispatches the matched shortcut command with the event as payload
#[test]
fn register_dispatches_the_matched_command_with_the_event() {
    let bold_command = create_command("test/BOLD");
    let italic_command = create_command("test/ITALIC");
    let (bold, italic) = (Spy::default(), Spy::default());
    let editor = build_test_editor(
        table(vec![
            ("BOLD", Some(vec![shortcut(&bold_command, "b", ctrl())])),
            ("ITALIC", Some(vec![shortcut(&italic_command, "i", ctrl())])),
        ]),
        vec![
            bold.listener(&bold_command, true),
            italic.listener(&italic_command, true),
        ],
    );
    let event = keyboard_event("b", CTRL);
    assert!(key_down(&editor, event.clone()));
    assert_eq!(bold.count(), 1);
    assert!(Rc::ptr_eq(&bold.0.borrow()[0], &event));
    assert_eq!(italic.count(), 0);
    // No modifier match -> no dispatch (the event may still be handled by
    // the core $handleKeyDown listener at COMMAND_PRIORITY_EDITOR)
    key_down(&editor, keyboard_event("b", 0));
    assert_eq!(bold.count(), 1);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > falls through to the next matching shortcut when a dispatch is unhandled
#[test]
fn register_falls_through_when_unhandled() {
    let skipped_command = create_command("test/SKIPPED");
    let handled_command = create_command("test/HANDLED");
    let (skipped, handled) = (Spy::default(), Spy::default());
    let editor = build_test_editor(
        table(vec![
            (
                "SKIPPED",
                Some(vec![shortcut(&skipped_command, "k", ctrl())]),
            ),
            (
                "HANDLED",
                Some(vec![shortcut(&handled_command, "k", ctrl())]),
            ),
        ]),
        vec![
            skipped.listener(&skipped_command, false),
            handled.listener(&handled_command, true),
        ],
    );
    assert!(key_down(&editor, ctrl_k()));
    assert_eq!(skipped.count(), 1);
    assert_eq!(handled.count(), 1);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > skips shortcuts whose $disabled predicate returns true
#[test]
fn register_skips_disabled_shortcuts() {
    let disabled_command = create_command("test/DISABLED");
    let enabled_command = create_command("test/ENABLED");
    let (disabled, enabled) = (Spy::default(), Spy::default());
    let predicate_calls = Rc::new(Cell::new(0));
    let calls = predicate_calls.clone();
    let editor = build_test_editor(
        table(vec![
            (
                "DISABLED",
                Some(vec![KeyboardShortcut {
                    disabled: Some(Rc::new(move |_, _| {
                        calls.set(calls.get() + 1);
                        Ok(true)
                    })),
                    ..shortcut(&disabled_command, "k", ctrl())
                }]),
            ),
            (
                "ENABLED",
                Some(vec![shortcut(&enabled_command, "k", ctrl())]),
            ),
        ]),
        vec![
            disabled.listener(&disabled_command, true),
            enabled.listener(&enabled_command, true),
        ],
    );
    assert!(key_down(&editor, ctrl_k()));
    assert_eq!(predicate_calls.get(), 1);
    assert_eq!(disabled.count(), 0);
    assert_eq!(enabled.count(), 1);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > falls through when $dispatch returns false
#[test]
fn register_falls_through_when_dispatch_returns_false() {
    let skipped_command = create_command("test/DISPATCH_SKIP");
    let handled_command = create_command("test/DISPATCH_HANDLED");
    let handled = Spy::default();
    let editor = build_test_editor(
        table(vec![
            (
                "SKIPPED",
                Some(vec![KeyboardShortcut {
                    dispatch: Some(Rc::new(|_, _, _, _| Ok(false))),
                    ..shortcut(&skipped_command, "k", ctrl())
                }]),
            ),
            (
                "HANDLED",
                Some(vec![shortcut(&handled_command, "k", ctrl())]),
            ),
        ]),
        vec![handled.listener(&handled_command, true)],
    );
    assert!(key_down(&editor, ctrl_k()));
    assert_eq!(handled.count(), 1);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > $dispatch middleware wraps the command dispatch
#[test]
fn register_dispatch_middleware_wraps_the_dispatch() {
    let wrapped_command: Command = create_command("test/WRAPPED");
    let order = Rc::new(RefCell::new(Vec::<&str>::new()));
    let seen_editor: Rc<RefCell<Option<Editor>>> = Rc::default();
    let (o, seen, expected) = (order.clone(), seen_editor.clone(), wrapped_command.clone());
    let o2 = order.clone();
    let editor = build_test_editor(
        table(vec![(
            "WRAPPED",
            Some(vec![KeyboardShortcut {
                dispatch: Some(Rc::new(move |command, _, next, editor2| {
                    assert_eq!(command.id(), expected.id());
                    *seen.borrow_mut() = Some(editor2.clone());
                    o.borrow_mut().push("before");
                    let handled = next()?;
                    o.borrow_mut().push("after");
                    Ok(handled)
                })),
                ..shortcut(&wrapped_command, "k", ctrl())
            }]),
        )]),
        vec![listener(&wrapped_command, move |_| {
            o2.borrow_mut().push("listener");
            true
        })],
    );
    assert!(key_down(&editor, ctrl_k()));
    assert_eq!(*order.borrow(), ["before", "listener", "after"]);
    assert!(seen_editor.borrow_mut().take().as_ref() == Some(&*editor));
    editor.dispose();
}

/// `buildExtensionEditor(shortcuts, listeners, overlay)`.
fn build_extension_editor(
    base: NamedKeyboardShortcuts,
    listeners: Vec<Listener>,
    overlay: Option<NamedKeyboardShortcuts>,
) -> LexicalEditorWithDispose {
    let mut layers = vec![shortcuts(base)];
    layers.extend(overlay.map(shortcuts));
    build_layered_editor(layers, listeners)
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension > dispatches configured shortcuts
#[test]
fn extension_dispatches_configured_shortcuts() {
    let bold_command = create_command("ext/BOLD");
    let bold = Spy::default();
    let editor = build_extension_editor(
        table(vec![(
            "bold",
            Some(vec![shortcut(&bold_command, "b", ctrl())]),
        )]),
        vec![bold.listener(&bold_command, true)],
        None,
    );
    key_down(&editor, keyboard_event("b", CTRL));
    assert_eq!(bold.count(), 1);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension > overlays merge by name: add, remap, and disable
#[test]
fn extension_overlays_merge_by_name() {
    let bold_command = create_command("overlay/BOLD");
    let italic_command = create_command("overlay/ITALIC");
    let custom_command = create_command("overlay/CUSTOM");
    let (bold, italic, custom) = (Spy::default(), Spy::default(), Spy::default());
    let editor = build_extension_editor(
        table(vec![
            ("bold", Some(vec![shortcut(&bold_command, "b", ctrl())])),
            ("italic", Some(vec![shortcut(&italic_command, "i", ctrl())])),
        ]),
        vec![
            bold.listener(&bold_command, true),
            italic.listener(&italic_command, true),
            custom.listener(&custom_command, true),
        ],
        Some(table(vec![
            // remap bold to a different key
            (
                "bold",
                Some(vec![shortcut(
                    &bold_command,
                    "b",
                    mask(NO, Pressed, NO, Pressed),
                )]),
            ),
            // add a new shortcut
            (
                "custom",
                Some(vec![shortcut(
                    &custom_command,
                    "m",
                    mask(Pressed, NO, NO, NO),
                )]),
            ),
            // disable italic
            ("italic", None),
        ])),
    );
    key_down(&editor, keyboard_event("b", CTRL));
    assert_eq!(bold.count(), 0);
    key_down(&editor, keyboard_event("b", CTRL | SHIFT));
    assert_eq!(bold.count(), 1);
    key_down(&editor, keyboard_event("i", CTRL));
    assert_eq!(italic.count(), 0);
    key_down(&editor, keyboard_event("m", ALT));
    assert_eq!(custom.count(), 1);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension > shortcuts can be remapped and disabled at runtime through the output signals
#[test]
fn extension_remaps_and_disables_at_runtime() {
    let bold_command = create_command("runtime/BOLD");
    let bold = Spy::default();
    let editor = build_extension_editor(
        table(vec![(
            "bold",
            Some(vec![shortcut(&bold_command, "b", ctrl())]),
        )]),
        vec![bold.listener(&bold_command, true)],
        None,
    );
    let output = output(&editor);
    output.shortcuts.set_value(output.shortcuts.value().with(
        "bold",
        Some(vec![shortcut(
            &bold_command,
            "b",
            mask(NO, NO, Pressed, NO),
        )]),
    ));
    key_down(&editor, keyboard_event("b", CTRL));
    assert_eq!(bold.count(), 0);
    key_down(&editor, keyboard_event("b", META));
    assert_eq!(bold.count(), 1);

    output.disabled.set_value(true);
    key_down(&editor, keyboard_event("b", META));
    assert_eq!(bold.count(), 1);
    editor.dispose();
}

/// One layered editor of `bold` mapped by each layer, dispatched Ctrl+K.
fn layered_calls(
    build: impl FnOnce(&mut Recorder) -> Vec<KeyboardShortcutsConfigOverrides>,
) -> Vec<String> {
    let mut rec = Recorder::default();
    let layers = build(&mut rec);
    let editor = build_layered_editor(layers, std::mem::take(&mut rec.listeners));
    key_down(&editor, ctrl_k());
    editor.dispose();
    rec.calls()
}

fn bold(shortcuts_: Option<Vec<KeyboardShortcut>>) -> KeyboardShortcutsConfigOverrides {
    shortcuts(table(vec![("bold", shortcuts_)]))
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > a later layer replaces the mapping it overrides
#[test]
fn merge_a_later_layer_replaces_the_mapping() {
    // The override left the event unhandled, and the mapping it replaced is
    // gone rather than being a fallback for it
    let calls = layered_calls(|rec| {
        vec![
            bold(Some(vec![rec.ctrl_k_shortcut("base", false)])),
            bold(Some(vec![rec.ctrl_k_shortcut("override", false)])),
        ]
    });
    assert_eq!(calls, ["override"]);
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > only the last layer to configure a name survives
#[test]
fn merge_only_the_last_layer_survives() {
    let calls = layered_calls(|rec| {
        vec![
            bold(Some(vec![rec.ctrl_k_shortcut("first", false)])),
            bold(Some(vec![rec.ctrl_k_shortcut("second", false)])),
            bold(Some(vec![rec.ctrl_k_shortcut("third", false)])),
        ]
    });
    assert_eq!(calls, ["third"]);
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > null disables the name
#[test]
fn merge_null_disables_the_name() {
    let calls = layered_calls(|rec| {
        vec![
            bold(Some(vec![rec.ctrl_k_shortcut("base", false)])),
            bold(None),
        ]
    });
    assert!(calls.is_empty());
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > an array replaces the mapping with several bindings
#[test]
fn merge_an_array_replaces_the_mapping() {
    let calls = layered_calls(|rec| {
        let replaced = rec.ctrl_k_shortcut("replaced", false);
        vec![
            bold(Some(vec![rec.ctrl_k_shortcut("base", false)])),
            bold(Some(vec![replaced])),
        ]
    });
    assert_eq!(calls, ["replaced"]);
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > an empty array disables the name like null does
#[test]
fn merge_an_empty_array_disables_the_name() {
    let calls = layered_calls(|rec| {
        vec![
            bold(Some(vec![rec.ctrl_k_shortcut("base", false)])),
            bold(Some(vec![])),
        ]
    });
    assert!(calls.is_empty());
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > a name mapped to an array registers every entry in order
#[test]
fn merge_an_array_registers_every_entry_in_order() {
    let calls = layered_calls(|rec| {
        vec![bold(Some(vec![
            rec.ctrl_k_shortcut("first", false),
            rec.ctrl_k_shortcut("second", false),
        ]))]
    });
    assert_eq!(calls, ["first", "second"]);
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > names added by a later layer are matched before existing names
#[test]
fn merge_later_names_are_matched_first() {
    // 'zzz' sorts after 'aaa', so only the layer order can put it first
    let calls = layered_calls(|rec| {
        vec![
            shortcuts(table(vec![(
                "aaa",
                Some(vec![rec.ctrl_k_shortcut("aaa", false)]),
            )])),
            shortcuts(table(vec![(
                "zzz",
                Some(vec![rec.ctrl_k_shortcut("zzz", false)]),
            )])),
        ]
    });
    assert_eq!(calls, ["zzz", "aaa"]);
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > a layer that configures no shortcuts leaves the table alone
#[test]
fn merge_a_layer_without_shortcuts_leaves_the_table() {
    let calls = layered_calls(|rec| {
        vec![
            bold(Some(vec![rec.ctrl_k_shortcut("base", true)])),
            KeyboardShortcutsConfigOverrides {
                priority: Some(COMMAND_PRIORITY_CRITICAL),
                ..Default::default()
            },
        ]
    });
    assert_eq!(calls, ["base"]);
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > arrays assigned to the runtime signal are flattened too
#[test]
fn merge_runtime_arrays_are_flattened() {
    let mut rec = Recorder::default();
    // All commands must exist before the editor is built so their listeners
    // are registered, even the ones only used after the signal is assigned
    let base = rec.ctrl_k_shortcut("base", false);
    let runtime_first = rec.ctrl_k_shortcut("runtimeFirst", false);
    let runtime_second = rec.ctrl_k_shortcut("runtimeSecond", false);
    let editor = build_layered_editor(
        vec![bold(Some(vec![base]))],
        std::mem::take(&mut rec.listeners),
    );
    output(&editor).shortcuts.set_value(table(vec![(
        "bold",
        Some(vec![runtime_first, runtime_second]),
    )]));
    key_down(&editor, ctrl_k());
    assert_eq!(rec.calls(), ["runtimeFirst", "runtimeSecond"]);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension priority > defaults to COMMAND_PRIORITY_NORMAL, ahead of $handleKeyDown
#[test]
fn priority_defaults_to_normal() {
    let mut rec = Recorder::default();
    let layer = bold(Some(vec![rec.ctrl_k_shortcut("bold", true)]));
    let editor = build_layered_editor(vec![layer], std::mem::take(&mut rec.listeners));
    let output = output(&editor);
    assert_eq!(output.priority.value(), COMMAND_PRIORITY_NORMAL);
    key_down(&editor, ctrl_k());
    assert_eq!(rec.calls(), ["bold"]);

    // Every editor registers the core $handleKeyDown at
    // COMMAND_PRIORITY_EDITOR and it always reports the event as handled, so
    // a shortcut listener at that priority is never reached.
    rec.clear();
    output.priority.set_value(COMMAND_PRIORITY_EDITOR);
    key_down(&editor, ctrl_k());
    assert!(rec.calls().is_empty());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension priority > a configured priority is used for the KEY_DOWN_COMMAND listener
#[test]
fn priority_configured_is_used() {
    let mut rec = Recorder::default();
    let layer = KeyboardShortcutsConfigOverrides {
        priority: Some(COMMAND_PRIORITY_LOW),
        ..bold(Some(vec![rec.ctrl_k_shortcut("bold", true)]))
    };
    let editor = build_layered_editor(vec![layer], std::mem::take(&mut rec.listeners));
    let calls = rec.calls.clone();
    let key_down_unregister: Unregister = editor.register_command(
        &KEY_DOWN_COMMAND,
        move |_, _| {
            calls.borrow_mut().push("high".into());
            Ok(true)
        },
        COMMAND_PRIORITY_HIGH,
    );
    key_down(&editor, ctrl_k());
    assert_eq!(rec.calls(), ["high"]);

    rec.clear();
    output(&editor)
        .priority
        .set_value(COMMAND_PRIORITY_CRITICAL);
    key_down(&editor, ctrl_k());
    assert_eq!(rec.calls(), ["bold"]);

    key_down_unregister();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension priority > the last layer to configure a priority wins
#[test]
fn priority_last_layer_wins() {
    let priority = |p| KeyboardShortcutsConfigOverrides {
        priority: Some(p),
        ..Default::default()
    };
    let editor = build_layered_editor(
        vec![
            priority(COMMAND_PRIORITY_LOW),
            priority(COMMAND_PRIORITY_CRITICAL),
        ],
        vec![],
    );
    assert_eq!(output(&editor).priority.value(), COMMAND_PRIORITY_CRITICAL);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension priority > disabled: true never registers the listener
#[test]
fn priority_disabled_never_registers() {
    let calls = layered_calls(|rec| {
        vec![KeyboardShortcutsConfigOverrides {
            disabled: Some(true),
            ..bold(Some(vec![rec.ctrl_k_shortcut("bold", true)]))
        }]
    });
    assert!(calls.is_empty());
}

/// `buildNestedEditors(shortcuts, priority)`.
struct Nested {
    calls: Rc<RefCell<Vec<String>>>,
    parent: LexicalEditorWithDispose,
    child: LexicalEditorWithDispose,
}

impl Nested {
    fn dispose(self) {
        self.child.dispose();
        self.parent.dispose();
    }
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

thread_local! {
    static BOLD_COMMAND: Command = create_command("nested/BOLD");
}

fn bold_command() -> Command {
    BOLD_COMMAND.with(Clone::clone)
}

fn build_nested_editors(table_: NamedKeyboardShortcuts, priority: Option<i8>) -> Nested {
    let calls: Rc<RefCell<Vec<String>>> = Rc::default();
    let recorder = |name: &'static str| {
        let calls = calls.clone();
        Rc::new(move |editor: &Editor, _: &(), _: &_| {
            let calls = calls.clone();
            Ok(editor.register_command(
                &bold_command(),
                move |_, _| {
                    calls.borrow_mut().push(format!("bold@{name}"));
                    Ok(true)
                },
                COMMAND_PRIORITY_EDITOR,
            ))
        })
    };
    let parent = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![config_extension(
            &keyboard_shortcuts_extension(),
            [KeyboardShortcutsConfigOverrides {
                priority,
                ..shortcuts(table_)
            }],
        )],
        register: Some(recorder("parent")),
        ..LexicalExtension::<(), (), ()>::new("parent", ())
    })
    .into()])
    .unwrap();
    let child = parent
        .read(|| {
            build_editor_from_extensions([define_extension(LexicalExtension {
                dependencies: vec![nested_editor_extension().into()],
                register: Some(recorder("child")),
                ..LexicalExtension::<(), (), ()>::new("child", ())
            })
            .into()])
        })
        .unwrap()
        .unwrap();
    Nested {
        calls,
        parent,
        child,
    }
}

/// `boldShortcut(overrides)`, with `bubbleFromNestedEditors`.
fn bold_shortcut(bubble: bool) -> KeyboardShortcut {
    KeyboardShortcut {
        bubble_from_nested_editors: bubble,
        ..shortcut(&bold_command(), "k", ctrl())
    }
}

fn bold_table(shortcut: KeyboardShortcut) -> NamedKeyboardShortcuts {
    table(vec![("bold", Some(vec![shortcut]))])
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > events bubbled up from a nested editor are ignored by default
#[test]
fn nested_bubbled_events_are_ignored_by_default() {
    let nested = build_nested_editors(bold_table(bold_shortcut(false)), None);
    key_down(&nested.child, ctrl_k());
    assert!(nested.calls().is_empty());
    nested.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > events from the registering editor are dispatched without the flag
#[test]
fn nested_own_events_are_dispatched() {
    let nested = build_nested_editors(bold_table(bold_shortcut(false)), None);
    key_down(&nested.parent, ctrl_k());
    assert_eq!(nested.calls(), ["bold@parent"]);
    nested.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > bubbleFromNestedEditors dispatches on the originating editor
#[test]
fn nested_bubbling_dispatches_on_the_originating_editor() {
    let nested = build_nested_editors(bold_table(bold_shortcut(true)), None);
    key_down(&nested.child, ctrl_k());
    // Dispatched on the child, not the editor the shortcut is registered on
    assert_eq!(nested.calls(), ["bold@child"]);
    nested.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > a non-bubbling shortcut does not shadow a bubbling one on the same key
#[test]
fn nested_non_bubbling_does_not_shadow_bubbling() {
    let ignored_command = create_command("nested/IGNORED");
    let ignored = Spy::default();
    let nested = build_nested_editors(
        table(vec![
            (
                "ignored",
                Some(vec![shortcut(&ignored_command, "k", ctrl())]),
            ),
            ("bold", Some(vec![bold_shortcut(true)])),
        ]),
        None,
    );
    let (_, f) = ignored.listener(&ignored_command, true);
    let cleanup = nested.parent.register_command(
        &ignored_command,
        move |event, _| Ok(f(event)),
        COMMAND_PRIORITY_EDITOR,
    );
    key_down(&nested.child, ctrl_k());
    assert_eq!(ignored.count(), 0);
    assert_eq!(nested.calls(), ["bold@child"]);
    cleanup();
    nested.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > $disabled and $dispatch receive the originating editor
#[test]
fn nested_disabled_and_dispatch_receive_the_originating_editor() {
    let disabled_editors: Rc<RefCell<Vec<Editor>>> = Rc::default();
    let dispatch_editors: Rc<RefCell<Vec<Editor>>> = Rc::default();
    let (d1, d2) = (disabled_editors.clone(), dispatch_editors.clone());
    let nested = build_nested_editors(
        bold_table(KeyboardShortcut {
            disabled: Some(Rc::new(move |_, editor| {
                d1.borrow_mut().push(editor.clone());
                Ok(false)
            })),
            dispatch: Some(Rc::new(move |_, _, next, editor| {
                d2.borrow_mut().push(editor.clone());
                next()
            })),
            ..bold_shortcut(true)
        }),
        None,
    );
    key_down(&nested.child, ctrl_k());
    assert_eq!(nested.calls(), ["bold@child"]);
    let disabled_editors = std::mem::take(&mut *disabled_editors.borrow_mut());
    let dispatch_editors = std::mem::take(&mut *dispatch_editors.borrow_mut());
    assert_eq!(disabled_editors.len(), 1);
    assert!(disabled_editors[0] == *nested.child);
    assert_eq!(dispatch_editors.len(), 1);
    assert!(dispatch_editors[0] == *nested.child);
    nested.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > $disabled is not consulted for events that will not bubble
#[test]
fn nested_disabled_is_not_consulted_for_non_bubbling_events() {
    let predicate_calls = Rc::new(Cell::new(0));
    let calls = predicate_calls.clone();
    let nested = build_nested_editors(
        bold_table(KeyboardShortcut {
            disabled: Some(Rc::new(move |_, _| {
                calls.set(calls.get() + 1);
                Ok(false)
            })),
            ..bold_shortcut(false)
        }),
        None,
    );
    key_down(&nested.child, ctrl_k());
    assert!(nested.calls().is_empty());
    assert_eq!(predicate_calls.get(), 0);
    nested.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > bubbling requires a priority above COMMAND_PRIORITY_EDITOR
#[test]
fn nested_bubbling_requires_a_priority_above_editor() {
    for priority in [
        COMMAND_PRIORITY_LOW,
        COMMAND_PRIORITY_NORMAL,
        COMMAND_PRIORITY_CRITICAL,
    ] {
        let nested = build_nested_editors(bold_table(bold_shortcut(true)), Some(priority));
        key_down(&nested.child, ctrl_k());
        assert_eq!(nested.calls(), ["bold@child"], "priority {priority}");
        nested.dispose();
    }
    for priority in [COMMAND_PRIORITY_BEFORE_EDITOR, COMMAND_PRIORITY_EDITOR] {
        let nested = build_nested_editors(bold_table(bold_shortcut(true)), Some(priority));
        key_down(&nested.child, ctrl_k());
        assert!(nested.calls().is_empty(), "priority {priority}");
        nested.dispose();
    }
}
