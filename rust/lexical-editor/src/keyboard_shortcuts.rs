//! `LexicalKeyboardShortcuts.ts`: shortcuts as data (a key, its modifiers
//! and a command), the platform's primary and secondary modifier masks, and
//! shortcut tables compiled for look-up by key and code.

use crate::environment::is_apple;
use crate::error::invariant;
use crate::events::KeyboardEvent;
use crate::lexical_selection::SelectionRef;
use crate::lexical_utils::{
    ControlOrOther, KeyboardEventModifierMask, KeyboardEventModifiers, ModifierExpectation,
};
use crate::{Editor, LexicalCommand, LexicalError, utf16};
use std::collections::HashMap;
use std::rc::Rc;

/// `KeyboardShortcutMatch` (`@experimental`): which keyboard events a
/// shortcut matches, an `event.key` value (case-insensitive) and a
/// [`KeyboardEventModifierMask`], with the semantics of
/// [`is_exact_shortcut_match`](crate::is_exact_shortcut_match), including
/// the `event.code` fallback for non-Latin layouts.
pub trait KeyboardShortcutMatch {
    /// `key`: the `KeyboardEvent.key` to match, case-insensitive.
    fn key(&self) -> &str;
    /// `modifiers`: the expected state of the modifier keys (none pressed
    /// by default).
    fn modifiers(&self) -> KeyboardEventModifierMask;
    /// `unshiftedKey`: the key without Shift, for display.
    fn unshifted_key(&self) -> Option<&str> {
        None
    }
}

/// A shortcut's `$disabled(selection, editor)`.
pub type ShortcutDisabledFn =
    Rc<dyn Fn(Option<&SelectionRef>, &Editor) -> Result<bool, LexicalError>>;

/// A shortcut's `$dispatch(command, event, $next, editor)`.
pub type ShortcutDispatchFn = Rc<
    dyn Fn(
        &LexicalCommand<Rc<KeyboardEvent>>,
        &Rc<KeyboardEvent>,
        &dyn Fn() -> Result<bool, LexicalError>,
        &Editor,
    ) -> Result<bool, LexicalError>,
>;

/// `KeyboardShortcut` (`@experimental`): a shortcut as data, the key and
/// modifiers to match and the command to dispatch with the matched event.
/// (`@lexical/extension`'s `KeyboardShortcutsExtension`, which reads them,
/// isn't ported.)
#[derive(Clone)]
pub struct KeyboardShortcut {
    pub key: String,
    pub modifiers: KeyboardEventModifierMask,
    pub unshifted_key: Option<String>,
    /// `command`: dispatched with the matched event as its payload.
    pub command: LexicalCommand<Rc<KeyboardEvent>>,
    /// `description`, for menus and help.
    pub description: Option<String>,
    /// `$disabled`: skip the shortcut when it returns `true`.
    pub disabled: Option<ShortcutDisabledFn>,
    /// `$dispatch`: replaces dispatching `command`; its `$next` dispatches
    /// it.
    pub dispatch: Option<ShortcutDispatchFn>,
    /// `bubbleFromNestedEditors`.
    pub bubble_from_nested_editors: bool,
}

impl KeyboardShortcut {
    /// A shortcut of `key` with no modifiers that dispatches `command`.
    pub fn new(key: &str, command: LexicalCommand<Rc<KeyboardEvent>>) -> Self {
        Self {
            key: key.to_owned(),
            modifiers: KeyboardEventModifierMask::default(),
            unshifted_key: None,
            command,
            description: None,
            disabled: None,
            dispatch: None,
            bubble_from_nested_editors: false,
        }
    }
}

impl KeyboardShortcutMatch for KeyboardShortcut {
    fn key(&self) -> &str {
        &self.key
    }
    fn modifiers(&self) -> KeyboardEventModifierMask {
        self.modifiers
    }
    fn unshifted_key(&self) -> Option<&str> {
        self.unshifted_key.as_deref()
    }
}

/// `CONTROL_OR_META`: the primary shortcut modifier, ⌘ (`metaKey`) on Apple
/// platforms and Ctrl elsewhere, for the platform [`is_apple`] names.
pub fn control_or_meta() -> KeyboardEventModifierMask {
    let apple = is_apple();
    KeyboardEventModifierMask {
        ctrl_key: (!apple).into(),
        meta_key: apple.into(),
        control_or_other: Some(ControlOrOther::MetaKey),
        ..KeyboardEventModifierMask::default()
    }
}

/// `CONTROL_OR_ALT`: the secondary shortcut modifier, ⌥ (`altKey`) on Apple
/// platforms and Ctrl elsewhere, for word-level editing and block formats.
pub fn control_or_alt() -> KeyboardEventModifierMask {
    let apple = is_apple();
    KeyboardEventModifierMask {
        alt_key: apple.into(),
        ctrl_key: (!apple).into(),
        control_or_other: Some(ControlOrOther::AltKey),
        ..KeyboardEventModifierMask::default()
    }
}

/// `MODIFIER_BITS`, in JS's order.
fn modifier_bits(mask: &KeyboardEventModifierMask) -> [(ModifierExpectation, u8); 4] {
    [
        (mask.alt_key, 1),
        (mask.ctrl_key, 2),
        (mask.meta_key, 4),
        (mask.shift_key, 8),
    ]
}

/// `getEventModifierBits(event)`.
fn get_event_modifier_bits(event: &impl KeyboardEventModifiers) -> u8 {
    u8::from(event.alt_key())
        | (u8::from(event.ctrl_key()) << 1)
        | (u8::from(event.meta_key()) << 2)
        | (u8::from(event.shift_key()) << 3)
}

/// `getMaskModifierBits(mask)`: the modifier bitmasks that satisfy the mask,
/// each `'any'` expanded into both states.
fn get_mask_modifier_bits(mask: &KeyboardEventModifierMask) -> Vec<u8> {
    let mut combos = vec![0];
    for (expected, bit) in modifier_bits(mask) {
        match expected {
            ModifierExpectation::Any => {
                let with: Vec<u8> = combos.iter().map(|bits| bits | bit).collect();
                combos.extend(with);
            }
            ModifierExpectation::Pressed => {
                for bits in &mut combos {
                    *bits |= bit;
                }
            }
            ModifierExpectation::Released => {}
        }
    }
    combos
}

/// Whether `key` is one ASCII character (one UTF-16 code unit up to 127).
fn is_ascii_key(key: &str) -> bool {
    utf16::len(key) == 1 && key.encode_utf16().next().is_some_and(|c| c <= 127)
}

/// `CompiledKeyboardShortcuts` (`@experimental @internal`): a shortcut
/// table indexed by the event's modifier bits and `key` (and, for
/// non-Latin layouts, its `code`), so a look-up costs the same however
/// many shortcuts it holds.
pub struct CompiledKeyboardShortcuts<S> {
    /// `${modifierBits}:${key.toLowerCase()}`: shortcuts in insertion order.
    by_key: HashMap<String, Vec<Rc<S>>>,
    /// `${modifierBits}:${code}` (`Digit1`, `KeyB`), used only when
    /// `event.key` is not a single ASCII character.
    by_code: HashMap<String, Vec<Rc<S>>>,
}

impl<S> Default for CompiledKeyboardShortcuts<S> {
    fn default() -> Self {
        Self {
            by_key: HashMap::new(),
            by_code: HashMap::new(),
        }
    }
}

impl<S: KeyboardShortcutMatch> CompiledKeyboardShortcuts<S> {
    pub fn new() -> Self {
        Self::default()
    }

    /// `add(shortcut)`.
    pub fn add(&mut self, shortcut: S) -> Result<&mut Self, LexicalError> {
        let key = shortcut.key().to_owned();
        invariant!(
            !key.is_empty(),
            "KeyboardShortcutMatch: key must be non-empty"
        );
        let modifiers = shortcut.modifiers();
        let shortcut = Rc::new(shortcut);
        let lower_key = key.to_lowercase();
        let single = utf16::len(&key) == 1;
        for bits in get_mask_modifier_bits(&modifiers) {
            self.by_key
                .entry(format!("{bits}:{lower_key}"))
                .or_default()
                .push(shortcut.clone());
            if single {
                if key.bytes().any(|b| b.is_ascii_digit()) {
                    self.by_code
                        .entry(format!("{bits}:Digit{key}"))
                        .or_default()
                        .push(shortcut.clone());
                } else if lower_key.bytes().any(|b| b.is_ascii_lowercase()) {
                    self.by_code
                        .entry(format!("{bits}:Key{}", lower_key.to_uppercase()))
                        .or_default()
                        .push(shortcut.clone());
                }
            }
        }
        Ok(self)
    }

    /// `matches(event)`: every shortcut matching the event, in insertion
    /// order, the matches by `key` before those by the `code` fallback.
    pub fn matches(&self, event: &impl KeyboardEventModifiers) -> Vec<Rc<S>> {
        let key = event.key();
        if key.is_empty() {
            return Vec::new();
        }
        let bits = get_event_modifier_bits(event);
        let mut matches = self
            .by_key
            .get(&format!("{bits}:{}", key.to_lowercase()))
            .cloned()
            .unwrap_or_default();
        // The code fallback only applies when event.key is not a single
        // ASCII character, otherwise it would break remapped layouts
        // (Dvorak, etc.).
        if !self.by_code.is_empty()
            && !is_ascii_key(key)
            && let Some(by_code) = self.by_code.get(&format!("{bits}:{}", event.code()))
        {
            matches.extend(by_code.iter().cloned());
        }
        matches
    }

    /// `match(event)`: the first shortcut matching the event.
    pub fn r#match(&self, event: &impl KeyboardEventModifiers) -> Option<Rc<S>> {
        self.matches(event).into_iter().next()
    }
}

/// `compileKeyboardShortcuts(shortcuts)` (`@experimental @internal`).
pub fn compile_keyboard_shortcuts<S: KeyboardShortcutMatch>(
    shortcuts: impl IntoIterator<Item = S>,
) -> Result<CompiledKeyboardShortcuts<S>, LexicalError> {
    let mut compiled = CompiledKeyboardShortcuts::new();
    for shortcut in shortcuts {
        compiled.add(shortcut)?;
    }
    Ok(compiled)
}
