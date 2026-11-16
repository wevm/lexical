//! `KeyboardShortcutsExtension.ts`: `formatKeyboardShortcut`,
//! `NamedKeyboardShortcuts` and `KeyboardShortcutsExtension`, which
//! dispatches a table of [`KeyboardShortcut`]s from one compiled
//! `KEY_DOWN_COMMAND` listener.

use super::signals::effect;
use crate::environment::is_apple;
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, ExtensionRegisterState, LexicalExtension,
    define_extension,
};
use crate::keyboard_shortcuts::{
    KeyboardShortcut, KeyboardShortcutMatch, compile_keyboard_shortcuts,
};
use crate::lexical_utils::{ModifierExpectation, keyboard_event_mask_for_platform};
use crate::{
    COMMAND_PRIORITY_NORMAL, Editor, KEY_DOWN_COMMAND, LexicalError, Unregister, get_selection,
    utf16,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `FormatKeyboardShortcutOptions`.
#[derive(Clone, Debug, Default)]
pub struct FormatKeyboardShortcutOptions {
    /// `isApple`: the platform convention; `None` for the runtime's
    /// ([`is_apple`]).
    pub is_apple: Option<bool>,
    /// `separator`, which JS declares and never reads.
    pub separator: Option<String>,
}

/// `UNIVERSAL_KEYS`.
fn universal_key(name: &str) -> Option<&'static str> {
    (name == " ").then_some("Space")
}

/// `APPLE_KEYS`.
fn apple_key(name: &str) -> Option<&'static str> {
    Some(match name {
        "Alt" => "\u{2325}",
        "ArrowDown" => "\u{2193}",
        "ArrowLeft" => "\u{2190}",
        "ArrowRight" => "\u{2192}",
        "ArrowUp" => "\u{2191}",
        "Backspace" => "\u{232B}",
        "CapsLock" => "\u{21EA}",
        "Ctrl" => "\u{2303}",
        "Delete" => "\u{2326}",
        "End" => "\u{2198}",
        "Enter" => "\u{21A9}",
        "Escape" => "\u{238B}",
        "Home" => "\u{2196}",
        "Meta" => "\u{2318}",
        "PageDown" => "\u{21DF}",
        "PageUp" => "\u{21DE}",
        "Shift" => "\u{21E7}",
        "Tab" => "\u{21E5}",
        _ => return universal_key(name),
    })
}

/// `SHIFT_APPLE_KEYS`.
fn shift_apple_key(name: &str) -> Option<&'static str> {
    if name == "Tab" {
        Some("\u{21E4}")
    } else {
        apple_key(name)
    }
}

/// `formatKeyboardShortcut(shortcut, options)`: the shortcut's binding as
/// the segments of a human readable string for menus, tooltips and help
/// (`['⌘', 'Shift', 'K']` joined with `+`): its pressed modifiers, with the
/// platform's symbols on Apple platforms, then its key. Modifiers with an
/// `'any'` mask aren't shown.
pub fn format_keyboard_shortcut(
    shortcut: &impl KeyboardShortcutMatch,
    options: &FormatKeyboardShortcutOptions,
) -> Vec<String> {
    let is_apple = options.is_apple.unwrap_or_else(is_apple);
    let key = shortcut.key();
    // `unshiftedKey`, which JS reads as a boolean.
    let unshifted_key = shortcut.unshifted_key().filter(|k| !k.is_empty());
    let modifiers = keyboard_event_mask_for_platform(shortcut.modifiers(), is_apple);
    let shift = modifiers.shift_key == ModifierExpectation::Pressed;
    let key_names: fn(&str) -> Option<&'static str> = if !is_apple {
        universal_key
    } else if shift {
        shift_apple_key
    } else {
        apple_key
    };
    let single = utf16::len(key) == 1;
    let mut segments = Vec::new();
    for (expected, is_shift, name) in [
        (modifiers.ctrl_key, false, "Ctrl"),
        (modifiers.alt_key, false, "Alt"),
        (modifiers.shift_key, true, "Shift"),
        (modifiers.meta_key, false, "Meta"),
    ] {
        if expected == ModifierExpectation::Pressed {
            // Apple omits the shift modifier in cases where unshifted key
            // differs from the key, e.g. 'shift+/', is displayed as '?'
            if is_apple && is_shift && unshifted_key.is_some() && single {
                continue;
            }
            segments.push(key_names(name).unwrap_or(name).to_owned());
        }
    }
    segments.push(if let Some(name) = key_names(key) {
        name.to_owned()
    } else if let Some(unshifted_key) = unshifted_key.filter(|_| !is_apple && shift) {
        unshifted_key.to_owned()
    } else if single {
        key.to_uppercase()
    } else {
        key.to_owned()
    });
    segments
}

/// `NamedKeyboardShortcuts` (`@experimental`): keyboard shortcuts by name,
/// in the order the names were added, so that other extensions and apps can
/// overlay the table: configuring an existing name remaps that shortcut,
/// configuring it to `None` (JS `null`) disables it, and new names add new
/// shortcuts. A name maps to a list, which is JS's single shortcut or array.
///
/// Tables compare by identity, as JS's objects do: a signal set to a table
/// notifies unless it is the table it holds.
#[derive(Clone, Default)]
pub struct NamedKeyboardShortcuts {
    entries: Rc<Vec<(String, Option<Vec<KeyboardShortcut>>)>>,
}

impl PartialEq for NamedKeyboardShortcuts {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.entries, &other.entries)
    }
}

impl NamedKeyboardShortcuts {
    /// An empty table (JS `{}`).
    pub fn new() -> Self {
        Self::default()
    }

    /// The names and their shortcuts, in order.
    pub fn entries(&self) -> &[(String, Option<Vec<KeyboardShortcut>>)] {
        &self.entries
    }

    /// Whether `name` is in the table, with shortcuts or disabled.
    pub fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|(n, _)| n == name)
    }

    /// The shortcuts of `name`; `None` when it is absent or disabled.
    pub fn get(&self, name: &str) -> Option<&[KeyboardShortcut]> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, s)| s.as_deref())
    }

    /// Sets `name` to `shortcuts`, or with `None` disables it, in its place
    /// when it is there and last otherwise.
    pub fn set(&mut self, name: &str, shortcuts: Option<Vec<KeyboardShortcut>>) {
        let entries = Rc::make_mut(&mut self.entries);
        match entries.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => entry.1 = shortcuts,
            None => entries.push((name.to_owned(), shortcuts)),
        }
    }

    /// The table with `name` set (see [`set`](Self::set)).
    pub fn with(mut self, name: &str, shortcuts: Option<Vec<KeyboardShortcut>>) -> Self {
        self.set(name, shortcuts);
        self
    }
}

/// `KeyboardShortcutsConfig` (`@experimental`).
#[derive(Clone)]
pub struct KeyboardShortcutsConfig {
    /// `disabled`: when `true`, the shortcut listener is not registered.
    pub disabled: bool,
    /// `priority`: the `KEY_DOWN_COMMAND` priority,
    /// [`COMMAND_PRIORITY_NORMAL`] by default. It must be above
    /// `COMMAND_PRIORITY_EDITOR`, where the core's keydown handler
    /// handles every event.
    pub priority: i8,
    /// `shortcuts`: the table, merged by name across the extension graph.
    pub shortcuts: NamedKeyboardShortcuts,
}

/// `Partial<KeyboardShortcutsConfig>`.
#[derive(Clone, Default)]
pub struct KeyboardShortcutsConfigOverrides {
    pub disabled: Option<bool>,
    pub priority: Option<i8>,
    pub shortcuts: Option<NamedKeyboardShortcuts>,
}

impl ExtensionConfigBase for KeyboardShortcutsConfig {
    type Partial = KeyboardShortcutsConfigOverrides;

    fn shallow_merge(&self, overrides: &KeyboardShortcutsConfigOverrides) -> Self {
        KeyboardShortcutsConfig {
            disabled: overrides.disabled.unwrap_or(self.disabled),
            priority: overrides.priority.unwrap_or(self.priority),
            shortcuts: overrides
                .shortcuts
                .clone()
                .unwrap_or_else(|| self.shortcuts.clone()),
        }
    }
}

/// `namedSignals(KeyboardShortcutsConfig)`: the extension's output, whose
/// signals remap and disable the shortcuts at runtime (the listener is
/// compiled again when they change).
#[derive(Clone)]
pub struct KeyboardShortcutsOutput {
    pub disabled: super::Signal<bool>,
    pub priority: super::Signal<i8>,
    pub shortcuts: super::Signal<NamedKeyboardShortcuts>,
}

impl KeyboardShortcutsOutput {
    /// `namedSignals(config)`.
    pub fn of(config: &KeyboardShortcutsConfig) -> Self {
        KeyboardShortcutsOutput {
            disabled: super::signal(config.disabled),
            priority: super::signal(config.priority),
            shortcuts: super::signal(config.shortcuts.clone()),
        }
    }
}

/// `registerKeyboardShortcuts(editor, shortcuts, priority)`
/// (`@experimental @internal`): one `KEY_DOWN_COMMAND` listener that
/// dispatches the command of each shortcut matching the event, with the
/// event as its payload, in order until a dispatch is handled. A shortcut
/// is skipped when its `$disabled` says so of the selection, or (unless it
/// `bubbleFromNestedEditors`) when the event comes from a nested editor.
fn register_keyboard_shortcuts(
    editor: &Editor,
    shortcuts: Vec<KeyboardShortcut>,
    priority: i8,
) -> Result<Unregister, LexicalError> {
    let compiled = compile_keyboard_shortcuts(shortcuts)?;
    let this = editor.weak();
    Ok(editor.register_command(
        &KEY_DOWN_COMMAND,
        move |event, from_editor| {
            let is_this = this.upgrade().is_some_and(|e| Editor(e) == *from_editor);
            // `undefined` until a shortcut's `$disabled` reads it.
            let mut selection = None;
            for shortcut in compiled.matches(&**event) {
                if !is_this && !shortcut.bubble_from_nested_editors {
                    continue;
                }
                if let Some(disabled) = &shortcut.disabled {
                    let selection = selection.get_or_insert_with(get_selection);
                    if disabled(selection.as_ref(), from_editor)? {
                        continue;
                    }
                }
                let next = || from_editor.dispatch_command(&shortcut.command, event.clone());
                let handled = match &shortcut.dispatch {
                    Some(dispatch) => dispatch(&shortcut.command, event, &next, from_editor)?,
                    None => next()?,
                };
                if handled {
                    return Ok(true);
                }
            }
            Ok(false)
        },
        priority,
    ))
}

/// `mergeNamedShortcuts(config, overrides)`: merged by name, as
/// `shallowMergeConfig` would, except that the overriding names come first,
/// so they are also the first offered a matching keypress.
fn merge_named_shortcuts(
    config: &NamedKeyboardShortcuts,
    overrides: Option<&NamedKeyboardShortcuts>,
) -> NamedKeyboardShortcuts {
    let Some(overrides) = overrides else {
        return config.clone();
    };
    let mut dest = overrides.clone();
    for (name, shortcuts) in config.entries() {
        if !dest.has(name) {
            dest.set(name, shortcuts.clone());
        }
    }
    dest
}

/// `KeyboardShortcutsExtension`.
pub type KeyboardShortcutsExtensionType =
    LexicalExtension<KeyboardShortcutsConfig, KeyboardShortcutsOutput, ()>;

fn make() -> Rc<KeyboardShortcutsExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &KeyboardShortcutsConfig, _: &ExtensionBuildState<()>| {
                Ok(KeyboardShortcutsOutput::of(config))
            },
        )),
        merge_config: Some(Rc::new(
            |config: &KeyboardShortcutsConfig, overrides: &KeyboardShortcutsConfigOverrides| {
                let mut merged = config.shallow_merge(overrides);
                merged.shortcuts =
                    merge_named_shortcuts(&config.shortcuts, overrides.shortcuts.as_ref());
                merged
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &KeyboardShortcutsConfig,
             state: &ExtensionRegisterState<(), KeyboardShortcutsOutput>| {
                let output = state
                    .get_output()
                    .expect("KeyboardShortcutsExtension has a build");
                let editor = editor.weak();
                // An effect's body can't fail: the first run's error is
                // `register`'s, as JS's effect throws from it, and a later
                // one's (a shortcut set with an empty key) panics.
                let first_error: Rc<RefCell<Option<LexicalError>>> = Rc::default();
                let first = Rc::new(std::cell::Cell::new(true));
                let error = first_error.clone();
                let dispose = effect(move || {
                    let is_first = first.replace(false);
                    if output.disabled.value() {
                        return None;
                    }
                    let editor = editor.upgrade().map(Editor)?;
                    let shortcuts: Vec<KeyboardShortcut> = output
                        .shortcuts
                        .value()
                        .entries()
                        .iter()
                        .flat_map(|(_, s)| s.iter().flatten().cloned())
                        .collect();
                    match register_keyboard_shortcuts(&editor, shortcuts, output.priority.value()) {
                        Ok(unregister) => Some(unregister),
                        Err(e) if is_first => {
                            *error.borrow_mut() = Some(e);
                            None
                        }
                        Err(e) => panic!("{e}"),
                    }
                });
                if let Some(e) = first_error.borrow_mut().take() {
                    dispose();
                    return Err(e);
                }
                Ok(dispose)
            },
        )),
        ..LexicalExtension::new(
            "@lexical/extension/KeyboardShortcuts",
            KeyboardShortcutsConfig {
                disabled: false,
                priority: COMMAND_PRIORITY_NORMAL,
                shortcuts: NamedKeyboardShortcuts::new(),
            },
        )
    })
}

thread_local! {
    static KEYBOARD_SHORTCUTS_EXTENSION: Rc<KeyboardShortcutsExtensionType> = make();
}

/// `KeyboardShortcutsExtension` (`@experimental`): dispatches a table of
/// keyboard shortcuts from one compiled `KEY_DOWN_COMMAND` listener, at the
/// config's `priority`, in O(1) per keypress. The table
/// ([`NamedKeyboardShortcuts`]) is merged across the extension graph by
/// name, the overriding names first, and its output's `shortcuts` signal
/// remaps it at runtime.
pub fn keyboard_shortcuts_extension() -> Rc<KeyboardShortcutsExtensionType> {
    KEYBOARD_SHORTCUTS_EXTENSION.with(Rc::clone)
}
