//! `LexicalEvents.ts`, the part that needs no DOM: the default handlers of
//! `KEY_DOWN_COMMAND` (`$handleKeyDown` and `buildKeyDownShortcuts`),
//! `BEFORE_INPUT_COMMAND` (`$handleBeforeInput`) and the composition
//! commands, which `registerDefaultCommandHandlers` registers on every
//! editor, and the DOM-shaped events those commands carry, as Swift has
//! them (`swift/Sources/Lexical/LexicalEvents.swift`).
//!
//! A headless editor has no DOM to listen to, so an app makes the events
//! from its own (a key press, an input method, the platform's clipboard)
//! with the DOM's `key` and `code` values, and passes them to
//! [`on_key_down`], [`on_before_input`], [`on_composition_start`] and
//! [`on_composition_end`], as JS's root element listeners do. Handlers
//! ported from JS read the same fields.
//!
//! Input is model-first, as in Swift: an app never edits its own text, so
//! every `beforeinput` is handled the way JS handles one it prevents, in
//! controlled mode. The app sets the model selection to the input's target
//! range before it dispatches, where JS applies `getTargetRanges()`, and
//! writes an input method's marked text through the model, where JS reads
//! the composed text back from the DOM.

use crate::commands::{BEFORE_INPUT_COMMAND, COMPOSITION_END_COMMAND, COMPOSITION_START_COMMAND};
use crate::commands::{
    CONTROLLED_TEXT_INSERTION_COMMAND, COPY_COMMAND, CUT_COMMAND, DELETE_CHARACTER_COMMAND,
    DELETE_LINE_COMMAND, DELETE_WORD_COMMAND, FORMAT_TEXT_COMMAND, INSERT_LINE_BREAK_COMMAND,
    INSERT_PARAGRAPH_COMMAND, KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND,
    KEY_ARROW_RIGHT_COMMAND, KEY_ARROW_UP_COMMAND, KEY_BACKSPACE_COMMAND, KEY_DELETE_COMMAND,
    KEY_DOWN_COMMAND, KEY_ENTER_COMMAND, KEY_ESCAPE_COMMAND, KEY_MODIFIER_COMMAND,
    KEY_SPACE_COMMAND, KEY_TAB_COMMAND, MOVE_TO_END, MOVE_TO_START, PASTE_COMMAND, REDO_COMMAND,
    REMOVE_TEXT_COMMAND, SELECT_ALL_COMMAND, UNDO_COMMAND,
};
use crate::constants::DOUBLE_LINE_BREAK;
use crate::editor::{
    COMMAND_PRIORITY_EDITOR, COMPOSITION_END_TAG, COMPOSITION_START_TAG, SKIP_SELECTION_FOCUS_TAG,
};
use crate::environment::is_apple;
use crate::keyboard_shortcuts::{
    CompiledKeyboardShortcuts, KeyboardShortcutMatch, compile_keyboard_shortcuts, control_or_alt,
    control_or_meta,
};
use crate::lexical_selection::{SelectionRef, Which, get_selection, set_selection};
use crate::lexical_utils::{
    KeyboardEventModifierMask, KeyboardEventModifiers, ModifierExpectation, get_node_by_key,
    is_modifier, is_token_or_segmented, set_composition_key,
};
use crate::node::{NodeKey, NodeRef};
use crate::nodes::text::create_text_node;
use crate::selection_insert_nodes::{find_matching_parent, get_previous_selection};
use crate::{Editor, LexicalCommand, LexicalError, Unregister, utf16};
use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::rc::Rc;

// The events.

/// The `Event` fields handlers use. Every event below holds one and
/// dereferences to it, as the DOM's event interfaces inherit from `Event`.
/// Events are shared (`Rc`), so a handler's `prevent_default()` is seen by
/// the code that dispatched the event.
#[derive(Debug)]
pub struct Event {
    /// `type` (`"keydown"`, `"beforeinput"`...).
    pub type_: String,
    pub cancelable: bool,
    /// `timeStamp`, in milliseconds.
    pub time_stamp: f64,
    default_prevented: Cell<bool>,
    propagation_stopped: Cell<bool>,
}

impl Event {
    /// An event of `type_` that can be cancelled, at time 0.
    pub fn new(type_: &str) -> Self {
        Self::with_cancelable(type_, true)
    }

    fn with_cancelable(type_: &str, cancelable: bool) -> Self {
        Self {
            type_: type_.to_owned(),
            cancelable,
            time_stamp: 0.0,
            default_prevented: Cell::new(false),
            propagation_stopped: Cell::new(false),
        }
    }

    /// `preventDefault()`: a cancelable event's default action is not to
    /// happen.
    pub fn prevent_default(&self) {
        if self.cancelable {
            self.default_prevented.set(true);
        }
    }

    /// `defaultPrevented`.
    pub fn default_prevented(&self) -> bool {
        self.default_prevented.get()
    }

    /// `stopPropagation()`.
    pub fn stop_propagation(&self) {
        self.propagation_stopped.set(true);
    }

    /// Whether `stopPropagation()` was called.
    pub fn propagation_stopped(&self) -> bool {
        self.propagation_stopped.get()
    }
}

macro_rules! extends {
    ($name:ty, $field:ident: $base:ty) => {
        impl Deref for $name {
            type Target = $base;
            fn deref(&self) -> &$base {
                &self.$field
            }
        }
    };
}

/// A DOM `KeyboardEvent`, with the DOM's `key` and `code` values
/// (`"Enter"`, `"b"`, `"KeyB"`, `"ArrowLeft"`...).
#[derive(Debug)]
pub struct KeyboardEvent {
    pub event: Event,
    pub key: String,
    pub code: String,
    pub alt_key: bool,
    pub ctrl_key: bool,
    pub meta_key: bool,
    pub shift_key: bool,
    pub repeat: bool,
    pub is_composing: bool,
}

extends!(KeyboardEvent, event: Event);

impl Default for KeyboardEvent {
    /// A `keydown` with no key and no modifiers.
    fn default() -> Self {
        Self {
            event: Event::new("keydown"),
            key: String::new(),
            code: String::new(),
            alt_key: false,
            ctrl_key: false,
            meta_key: false,
            shift_key: false,
            repeat: false,
            is_composing: false,
        }
    }
}

impl KeyboardEventModifiers for KeyboardEvent {
    fn key(&self) -> &str {
        &self.key
    }
    fn code(&self) -> &str {
        &self.code
    }
    fn alt_key(&self) -> bool {
        self.alt_key
    }
    fn ctrl_key(&self) -> bool {
        self.ctrl_key
    }
    fn meta_key(&self) -> bool {
        self.meta_key
    }
    fn shift_key(&self) -> bool {
        self.shift_key
    }
}

/// A DOM `MouseEvent` (and the `PointerEvent` fields Lexical reads), with
/// the node the app hit in place of the DOM target.
#[derive(Debug)]
pub struct MouseEvent {
    pub event: Event,
    pub client_x: f64,
    pub client_y: f64,
    pub button: i32,
    pub detail: i32,
    pub alt_key: bool,
    pub ctrl_key: bool,
    pub meta_key: bool,
    pub shift_key: bool,
    /// The key of the node under the pointer, as the app resolves it
    /// (Swift's `targetKey`).
    pub target_key: Option<NodeKey>,
}

extends!(MouseEvent, event: Event);

impl MouseEvent {
    /// A mouse event of `type_` at (0, 0), with the main button, one click
    /// and no modifiers.
    pub fn new(type_: &str) -> Self {
        Self {
            event: Event::new(type_),
            client_x: 0.0,
            client_y: 0.0,
            button: 0,
            detail: 1,
            alt_key: false,
            ctrl_key: false,
            meta_key: false,
            shift_key: false,
            target_key: None,
        }
    }
}

impl Default for MouseEvent {
    /// A `click`.
    fn default() -> Self {
        Self::new("click")
    }
}

impl KeyboardEventModifiers for MouseEvent {
    fn key(&self) -> &str {
        ""
    }
    fn code(&self) -> &str {
        ""
    }
    fn alt_key(&self) -> bool {
        self.alt_key
    }
    fn ctrl_key(&self) -> bool {
        self.ctrl_key
    }
    fn meta_key(&self) -> bool {
        self.meta_key
    }
    fn shift_key(&self) -> bool {
        self.shift_key
    }
}

/// A DOM `DragEvent`: a drag or a drop, with its data.
#[derive(Debug)]
pub struct DragEvent {
    pub mouse: MouseEvent,
    pub data_transfer: Option<Rc<DataTransfer>>,
}

extends!(DragEvent, mouse: MouseEvent);

impl DragEvent {
    pub fn new(type_: &str, data_transfer: Option<Rc<DataTransfer>>) -> Self {
        Self {
            mouse: MouseEvent::new(type_),
            data_transfer,
        }
    }
}

/// A DOM `FocusEvent` (`focus`, `blur`).
#[derive(Debug)]
pub struct FocusEvent {
    pub event: Event,
}

extends!(FocusEvent, event: Event);

impl FocusEvent {
    pub fn new(type_: &str) -> Self {
        Self {
            event: Event::with_cancelable(type_, false),
        }
    }
}

/// A DOM `InputEvent`: a `beforeinput` or `input`.
#[derive(Debug)]
pub struct InputEvent {
    pub event: Event,
    /// `inputType` (`"insertText"`, `"deleteContentBackward"`...).
    pub input_type: String,
    pub data: Option<String>,
    pub data_transfer: Option<Rc<DataTransfer>>,
    pub is_composing: bool,
}

extends!(InputEvent, event: Event);

impl InputEvent {
    /// A `beforeinput` of `input_type`, with no data.
    pub fn new(input_type: &str) -> Self {
        Self {
            event: Event::new("beforeinput"),
            input_type: input_type.to_owned(),
            data: None,
            data_transfer: None,
            is_composing: false,
        }
    }
}

/// A DOM `CompositionEvent`: an input method composition starts or ends.
#[derive(Debug)]
pub struct CompositionEvent {
    pub event: Event,
    pub data: String,
}

extends!(CompositionEvent, event: Event);

impl CompositionEvent {
    pub fn new(type_: &str, data: &str) -> Self {
        Self {
            event: Event::with_cancelable(type_, false),
            data: data.to_owned(),
        }
    }
}

/// A DOM `ClipboardEvent`: a copy, cut or paste, with its data.
#[derive(Debug)]
pub struct ClipboardEvent {
    pub event: Event,
    pub clipboard_data: Option<Rc<DataTransfer>>,
}

extends!(ClipboardEvent, event: Event);

impl ClipboardEvent {
    pub fn new(type_: &str, clipboard_data: Option<Rc<DataTransfer>>) -> Self {
        Self {
            event: Event::new(type_),
            clipboard_data,
        }
    }
}

/// `PasteCommandType`: `ClipboardEvent | InputEvent | KeyboardEvent`, the
/// payload of `PASTE_COMMAND`.
#[derive(Clone, Debug)]
pub enum PasteCommandType {
    Clipboard(Rc<ClipboardEvent>),
    Input(Rc<InputEvent>),
    Keyboard(Rc<KeyboardEvent>),
}

impl PasteCommandType {
    /// The event, whichever it is.
    pub fn event(&self) -> &Event {
        match self {
            PasteCommandType::Clipboard(e) => e,
            PasteCommandType::Input(e) => e,
            PasteCommandType::Keyboard(e) => e,
        }
    }
}

/// `ClipboardEvent | KeyboardEvent`, the payload of `COPY_COMMAND` and
/// `CUT_COMMAND` (or `None`, JS's `null`).
#[derive(Clone, Debug)]
pub enum ClipboardOrKeyboardEvent {
    Clipboard(Rc<ClipboardEvent>),
    Keyboard(Rc<KeyboardEvent>),
}

impl ClipboardOrKeyboardEvent {
    /// The event, whichever it is.
    pub fn event(&self) -> &Event {
        match self {
            ClipboardOrKeyboardEvent::Clipboard(e) => e,
            ClipboardOrKeyboardEvent::Keyboard(e) => e,
        }
    }

    /// The clipboard event, when it is one (JS's
    /// `objectKlassEquals(event, ClipboardEvent) ? event : null`).
    pub fn clipboard_event(event: Option<&Self>) -> Option<&Rc<ClipboardEvent>> {
        match event {
            Some(ClipboardOrKeyboardEvent::Clipboard(e)) => Some(e),
            _ => None,
        }
    }
}

/// One file of a [`DataTransfer`], as a web `File`: its name, type and
/// size are known up front, and its contents are read only when
/// [`bytes`](Self::bytes) asks for them, as `File.bytes()` reads a lazy
/// `Blob`.
#[derive(Clone)]
pub struct DataTransferFile {
    /// `File.name`.
    pub name: String,
    /// `File.type`, a MIME type.
    pub type_: String,
    /// `File.size` in bytes, or `None` when it isn't known without reading
    /// the file.
    pub size: Option<usize>,
    load: Rc<dyn Fn() -> std::io::Result<Vec<u8>>>,
}

impl DataTransferFile {
    /// A file whose contents are `data`, in memory.
    pub fn new(name: &str, type_: &str, data: Vec<u8>) -> Self {
        let size = data.len();
        Self::lazy(name, type_, Some(size), move || Ok(data.clone()))
    }

    /// A file that `load` reads each time [`bytes`](Self::bytes) is called.
    pub fn lazy(
        name: &str,
        type_: &str,
        size: Option<usize>,
        load: impl Fn() -> std::io::Result<Vec<u8>> + 'static,
    ) -> Self {
        Self {
            name: name.to_owned(),
            type_: type_.to_owned(),
            size,
            load: Rc::new(load),
        }
    }

    /// `File.bytes()`: the file's contents, or the error that it can no
    /// longer be read (a web `File` rejects with `NotReadableError`).
    pub fn bytes(&self) -> std::io::Result<Vec<u8>> {
        (self.load)()
    }
}

impl std::fmt::Debug for DataTransferFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataTransferFile")
            .field("name", &self.name)
            .field("type_", &self.type_)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// `DataTransfer`: the data of a clipboard or drag operation, by format (a
/// MIME type), in the order each format was first set, and its files.
///
/// It is shared as a DOM `DataTransfer` is, so its setters take `&self`: a
/// copy fills the one the platform's clipboard event carries, and the
/// caller reads it afterwards.
#[derive(Debug)]
pub struct DataTransfer {
    entries: RefCell<Vec<(String, String)>>,
    files: RefCell<Vec<DataTransferFile>>,
    drop_effect: RefCell<String>,
    effect_allowed: RefCell<String>,
}

impl Default for DataTransfer {
    fn default() -> Self {
        Self {
            entries: RefCell::new(Vec::new()),
            files: RefCell::new(Vec::new()),
            drop_effect: RefCell::new("none".to_owned()),
            effect_allowed: RefCell::new("uninitialized".to_owned()),
        }
    }
}

impl DataTransfer {
    /// `new DataTransfer()`: one with no data.
    pub fn new() -> Self {
        Self::default()
    }

    /// The format a type names: lowercased, with `text` read as
    /// `text/plain` and `url` as `text/uri-list`, as browsers do.
    fn format(format: &str) -> String {
        let format = format.to_lowercase();
        match format.as_str() {
            "text" => "text/plain".to_owned(),
            "url" => "text/uri-list".to_owned(),
            _ => format,
        }
    }

    /// `types`: the formats set so far, in order, then `"Files"` when there
    /// are files, as browsers report it.
    pub fn types(&self) -> Vec<String> {
        let mut types: Vec<String> = self
            .entries
            .borrow()
            .iter()
            .map(|(t, _)| t.clone())
            .collect();
        if !self.files.borrow().is_empty() {
            types.push("Files".to_owned());
        }
        types
    }

    /// `getData(format)`: the data for `format`, or `""`.
    pub fn get_data(&self, format: &str) -> String {
        let format = Self::format(format);
        self.entries
            .borrow()
            .iter()
            .find(|(t, _)| *t == format)
            .map(|(_, data)| data.clone())
            .unwrap_or_default()
    }

    /// `setData(format, data)`: sets the data for `format`, keeping its
    /// place if it was set before.
    pub fn set_data(&self, format: &str, data: &str) {
        let format = Self::format(format);
        let mut entries = self.entries.borrow_mut();
        match entries.iter_mut().find(|(t, _)| *t == format) {
            Some(entry) => entry.1 = data.to_owned(),
            None => entries.push((format, data.to_owned())),
        }
    }

    /// `clearData(format)`: removes the data for `format`, or with `None`
    /// all of it (not the files).
    pub fn clear_data(&self, format: Option<&str>) {
        let mut entries = self.entries.borrow_mut();
        match format {
            Some(format) => {
                let format = Self::format(format);
                entries.retain(|(t, _)| *t != format);
            }
            None => entries.clear(),
        }
    }

    /// `files`.
    pub fn files(&self) -> Vec<DataTransferFile> {
        self.files.borrow().clone()
    }

    /// Sets `files` (what `items.add(file)` builds up in a browser).
    pub fn set_files(&self, files: Vec<DataTransferFile>) {
        *self.files.borrow_mut() = files;
    }

    /// `dropEffect` (`"none"` at first).
    pub fn drop_effect(&self) -> String {
        self.drop_effect.borrow().clone()
    }

    pub fn set_drop_effect(&self, drop_effect: &str) {
        *self.drop_effect.borrow_mut() = drop_effect.to_owned();
    }

    /// `effectAllowed` (`"uninitialized"` at first).
    pub fn effect_allowed(&self) -> String {
        self.effect_allowed.borrow().clone()
    }

    pub fn set_effect_allowed(&self, effect_allowed: &str) {
        *self.effect_allowed.borrow_mut() = effect_allowed.to_owned();
    }
}

// The editor's input state.

/// The part of `editor._inputState` the model reads: whether the Enter
/// that comes before an `insertParagraph` asked for a line break, the
/// `key` of the last key pressed, and the segmented node a composition was
/// redirected into.
#[derive(Debug, Default)]
pub(crate) struct InputState {
    pub(crate) is_insert_line_break: bool,
    pub(crate) last_key_code: Option<String>,
    pub(crate) composed_segmented_key: Option<NodeKey>,
}

// The entry points.

/// `onKeyDown(event, editor)`: an app's key press. It records the key and,
/// unless an input method is composing, dispatches `KEY_DOWN_COMMAND`,
/// whose default handler dispatches the command the key stands for. Read
/// `event.default_prevented()` afterwards to know whether the key was
/// taken (the app then skips its own handling of it).
pub fn on_key_down(event: &Rc<KeyboardEvent>, editor: &Editor) -> Result<(), LexicalError> {
    editor.0.input_state.borrow_mut().last_key_code = Some(event.key.clone());
    if editor.is_composing() {
        return Ok(());
    }
    editor.dispatch_command(&KEY_DOWN_COMMAND, event.clone())?;
    Ok(())
}

/// `onBeforeInput(event, editor)`: an app's `beforeinput`. A composition's
/// own input (`insertCompositionText`, `deleteCompositionText`) is left to
/// the composition commands; anything else dispatches
/// `BEFORE_INPUT_COMMAND`.
pub fn on_before_input(event: &Rc<InputEvent>, editor: &Editor) -> Result<(), LexicalError> {
    if matches!(
        event.input_type.as_str(),
        "deleteCompositionText" | "insertCompositionText"
    ) {
        return Ok(());
    }
    editor.dispatch_command(&BEFORE_INPUT_COMMAND, event.clone())?;
    Ok(())
}

/// `onCompositionStart(event, editor)`: dispatches
/// `COMPOSITION_START_COMMAND`.
pub fn on_composition_start(
    event: &Rc<CompositionEvent>,
    editor: &Editor,
) -> Result<(), LexicalError> {
    editor.dispatch_command(&COMPOSITION_START_COMMAND, event.clone())?;
    Ok(())
}

/// `onCompositionEnd(event, editor)`: dispatches
/// `COMPOSITION_END_COMMAND` (where JS waits for Firefox's and Safari's
/// input event, the model has none to wait for).
pub fn on_composition_end(
    event: &Rc<CompositionEvent>,
    editor: &Editor,
) -> Result<(), LexicalError> {
    editor.dispatch_command(&COMPOSITION_END_COMMAND, event.clone())?;
    Ok(())
}

// The default KEY_DOWN_COMMAND handler.

type OnMatch = Rc<dyn Fn(&Rc<KeyboardEvent>, &Editor) -> Result<(), LexicalError>>;

/// `KeyDownShortcut`: a key-down binding of `$handleKeyDown`.
#[derive(Clone)]
pub(crate) struct KeyDownShortcut {
    key: &'static str,
    modifiers: KeyboardEventModifierMask,
    on_match: OnMatch,
}

impl KeyboardShortcutMatch for KeyDownShortcut {
    fn key(&self) -> &str {
        self.key
    }
    fn modifiers(&self) -> KeyboardEventModifierMask {
        self.modifiers
    }
}

const ANY: ModifierExpectation = ModifierExpectation::Any;
const PRESSED: ModifierExpectation = ModifierExpectation::Pressed;
const RELEASED: ModifierExpectation = ModifierExpectation::Released;

const ANY_MODIFIERS: KeyboardEventModifierMask = KeyboardEventModifierMask {
    alt_key: ANY,
    ctrl_key: ANY,
    meta_key: ANY,
    shift_key: ANY,
    control_or_other: None,
};
const NO_MODIFIERS: KeyboardEventModifierMask = KeyboardEventModifierMask {
    alt_key: RELEASED,
    ctrl_key: RELEASED,
    meta_key: RELEASED,
    shift_key: RELEASED,
    control_or_other: None,
};
const CTRL_KEY: KeyboardEventModifierMask = KeyboardEventModifierMask {
    ctrl_key: PRESSED,
    ..NO_MODIFIERS
};
const META_KEY: KeyboardEventModifierMask = KeyboardEventModifierMask {
    meta_key: PRESSED,
    ..NO_MODIFIERS
};
const SHIFT_KEY_ANY: KeyboardEventModifierMask = KeyboardEventModifierMask {
    shift_key: ANY,
    ..NO_MODIFIERS
};
const ALT_SHIFT_KEY_ANY: KeyboardEventModifierMask = KeyboardEventModifierMask {
    alt_key: ANY,
    shift_key: ANY,
    ..NO_MODIFIERS
};

/// `buildKeyDownShortcuts()`: the key-down bindings, for the platform
/// [`is_apple`] names. Each mask is exclusive of every other mask on the
/// same key, so at most one matches an event. JS's bindings for Chrome's
/// text replacement (`markHandledSelectionCommandInsertText`), iOS's
/// virtual Shift key and inputs embedded in a node's DOM
/// (`$isEventFromEmbeddedInput`) have no counterpart without a browser.
pub(crate) fn build_key_down_shortcuts() -> Vec<KeyDownShortcut> {
    fn shortcut(
        key: &'static str,
        modifiers: KeyboardEventModifierMask,
        on_match: impl Fn(&Rc<KeyboardEvent>, &Editor) -> Result<(), LexicalError> + 'static,
    ) -> KeyDownShortcut {
        KeyDownShortcut {
            key,
            modifiers,
            on_match: Rc::new(on_match),
        }
    }
    // Dispatches the command with the KeyboardEvent as its payload.
    fn dispatch(
        key: &'static str,
        modifiers: KeyboardEventModifierMask,
        command: LexicalCommand<Rc<KeyboardEvent>>,
    ) -> KeyDownShortcut {
        shortcut(key, modifiers, move |event, editor| {
            editor.dispatch_command(&command, event.clone())?;
            Ok(())
        })
    }
    // `preventDefault()`, then dispatches the command with a fixed payload.
    fn prevent<T: Clone + 'static>(
        key: &'static str,
        modifiers: KeyboardEventModifierMask,
        command: LexicalCommand<T>,
        payload: T,
    ) -> KeyDownShortcut {
        shortcut(key, modifiers, move |event, editor| {
            event.prevent_default();
            editor.dispatch_command(&command, payload.clone())?;
            Ok(())
        })
    }
    fn enter(modifiers: KeyboardEventModifierMask, is_insert_line_break: bool) -> KeyDownShortcut {
        shortcut("Enter", modifiers, move |event, editor| {
            editor.0.input_state.borrow_mut().is_insert_line_break = is_insert_line_break;
            editor.dispatch_command(&KEY_ENTER_COMMAND, Some(event.clone()))?;
            if event.default_prevented() {
                editor.0.input_state.borrow_mut().is_insert_line_break = false;
            }
            Ok(())
        })
    }
    // Only a RangeSelection can use the native cut and copy.
    fn copy_or_cut(
        key: &'static str,
        command: LexicalCommand<Option<ClipboardOrKeyboardEvent>>,
    ) -> KeyDownShortcut {
        shortcut(key, control_or_meta(), move |event, editor| {
            let prev_selection = editor.get_editor_state().selection();
            if prev_selection.is_some_and(|s| !s.is_range()) {
                event.prevent_default();
                editor.dispatch_command(
                    &command,
                    Some(ClipboardOrKeyboardEvent::Keyboard(event.clone())),
                )?;
            }
            Ok(())
        })
    }
    let mut shortcuts = vec![
        // moveForward / moveToEnd / moveBackward / moveToStart / moveUp / moveDown
        dispatch("ArrowRight", SHIFT_KEY_ANY, KEY_ARROW_RIGHT_COMMAND),
        dispatch("ArrowLeft", SHIFT_KEY_ANY, KEY_ARROW_LEFT_COMMAND),
        dispatch("ArrowUp", ALT_SHIFT_KEY_ANY, KEY_ARROW_UP_COMMAND),
        dispatch("ArrowDown", ALT_SHIFT_KEY_ANY, KEY_ARROW_DOWN_COMMAND),
        // lineBreak / paragraph
        enter(
            KeyboardEventModifierMask {
                shift_key: PRESSED,
                ..ANY_MODIFIERS
            },
            true,
        ),
        enter(
            KeyboardEventModifierMask {
                shift_key: RELEASED,
                ..ANY_MODIFIERS
            },
            false,
        ),
        dispatch(" ", ANY_MODIFIERS, KEY_SPACE_COMMAND),
        // deleteBackward
        dispatch("Backspace", SHIFT_KEY_ANY, KEY_BACKSPACE_COMMAND),
        dispatch("Escape", ANY_MODIFIERS, KEY_ESCAPE_COMMAND),
        // deleteForward
        dispatch("Delete", NO_MODIFIERS, KEY_DELETE_COMMAND),
        // deleteWordBackward / deleteWordForward
        prevent("Backspace", control_or_alt(), DELETE_WORD_COMMAND, true),
        prevent("Delete", control_or_alt(), DELETE_WORD_COMMAND, false),
        prevent(
            "b",
            control_or_meta(),
            FORMAT_TEXT_COMMAND,
            "bold".to_owned(),
        ),
        prevent(
            "u",
            control_or_meta(),
            FORMAT_TEXT_COMMAND,
            "underline".to_owned(),
        ),
        prevent(
            "i",
            control_or_meta(),
            FORMAT_TEXT_COMMAND,
            "italic".to_owned(),
        ),
        dispatch("Tab", SHIFT_KEY_ANY, KEY_TAB_COMMAND),
        // undo / redo
        prevent("z", control_or_meta(), UNDO_COMMAND, ()),
        prevent(
            "z",
            KeyboardEventModifierMask {
                shift_key: PRESSED,
                ..control_or_meta()
            },
            REDO_COMMAND,
            (),
        ),
    ];
    if is_apple() {
        shortcuts.extend([
            // openLineBreak
            prevent("o", CTRL_KEY, INSERT_LINE_BREAK_COMMAND, true),
            // moveToStart / moveToEnd mac
            dispatch(
                "ArrowLeft",
                KeyboardEventModifierMask {
                    meta_key: PRESSED,
                    ..SHIFT_KEY_ANY
                },
                MOVE_TO_START,
            ),
            dispatch(
                "ArrowRight",
                KeyboardEventModifierMask {
                    meta_key: PRESSED,
                    ..SHIFT_KEY_ANY
                },
                MOVE_TO_END,
            ),
            // deleteBackward / deleteForward
            prevent("h", CTRL_KEY, DELETE_CHARACTER_COMMAND, true),
            prevent("d", CTRL_KEY, DELETE_CHARACTER_COMMAND, false),
            // deleteLineBackward / deleteLineForward
            prevent("Backspace", META_KEY, DELETE_LINE_COMMAND, true),
            prevent("Delete", META_KEY, DELETE_LINE_COMMAND, false),
            prevent("k", CTRL_KEY, DELETE_LINE_COMMAND, false),
        ]);
    } else {
        shortcuts.extend([
            dispatch("Home", SHIFT_KEY_ANY, MOVE_TO_START),
            dispatch("End", SHIFT_KEY_ANY, MOVE_TO_END),
            prevent("y", CTRL_KEY, REDO_COMMAND, ()),
        ]);
    }
    shortcuts.extend([
        // selectAll
        shortcut("a", control_or_meta(), |event, editor| {
            event.prevent_default();
            editor.dispatch_command(&SELECT_ALL_COMMAND, event.clone())?;
            Ok(())
        }),
        copy_or_cut("c", COPY_COMMAND),
        copy_or_cut("x", CUT_COMMAND),
    ]);
    shortcuts
}

/// The editor's compiled key-down bindings (JS `editor._keyDownShortcuts`),
/// for the platform they were compiled for.
pub(crate) type KeyDownShortcuts = (bool, Rc<CompiledKeyboardShortcuts<KeyDownShortcut>>);

/// `$handleKeyDown(event)`: dispatches the command the key stands for,
/// then `KEY_MODIFIER_COMMAND` when a modifier is down. Always reports the
/// event as handled, as in JS.
pub(crate) fn handle_key_down(
    event: &Rc<KeyboardEvent>,
    editor: &Editor,
) -> Result<bool, LexicalError> {
    let is_apple = is_apple();
    let cached = editor
        .0
        .key_down_shortcuts
        .borrow()
        .as_ref()
        .filter(|(apple, _)| *apple == is_apple)
        .map(|(_, shortcuts)| shortcuts.clone());
    let compiled = match cached {
        Some(compiled) => compiled,
        None => {
            let compiled = Rc::new(compile_keyboard_shortcuts(build_key_down_shortcuts())?);
            *editor.0.key_down_shortcuts.borrow_mut() = Some((is_apple, compiled.clone()));
            compiled
        }
    };
    if let Some(shortcut) = compiled.r#match(&**event) {
        (shortcut.on_match)(event, editor)?;
    }
    if is_modifier(&**event) {
        editor.dispatch_command(&KEY_MODIFIER_COMMAND, event.clone())?;
    }
    Ok(true)
}

// The default BEFORE_INPUT_COMMAND handler.

/// `$isTokenOrTab(node)`.
fn is_token_or_tab(node: &NodeRef) -> bool {
    node.get_type() == "tab" || node.is_token()
}

/// `$maybeMoveSelectionPastTrailingAcceptanceBoundary(insertedText)`: after
/// a text replacement accepted by the key that ends it (a space, Enter,
/// Tab), puts the caret after that key's character, which the replacement
/// arrived in front of.
fn maybe_move_selection_past_trailing_acceptance_boundary(
    editor: &Editor,
    inserted_text: Option<&str>,
) -> Result<(), LexicalError> {
    let last_key_code = editor.0.input_state.borrow().last_key_code.clone();
    let (Some(inserted_text), Some(last_key_code)) = (inserted_text, last_key_code) else {
        return Ok(());
    };
    if utf16::len(inserted_text) <= 1 {
        return Ok(());
    }
    let character_to_search_for: u16 = if utf16::len(&last_key_code) == 1 {
        last_key_code.encode_utf16().next().unwrap_or(0)
    } else if last_key_code == "Enter" {
        u16::from(b'\n')
    } else if last_key_code == "Tab" {
        u16::from(b'\t')
    } else {
        return Ok(());
    };
    let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
        return Ok(());
    };
    let Some(range) = selection.range() else {
        return Ok(());
    };
    if !range.is_collapsed() {
        return Ok(());
    }
    let anchor_node = range.anchor.get_node()?;
    if !anchor_node.is_text() {
        return Ok(());
    }
    let offset = range.anchor.offset;
    let text: Vec<u16> = anchor_node.get_text_content().encode_utf16().collect();
    if text.len() == offset {
        let next_sibling = anchor_node.get_next_sibling();
        if character_to_search_for == u16::from(b'\n') {
            match next_sibling {
                Some(line_break) if line_break.get_type() == "linebreak" => {
                    line_break.select_end()?;
                }
                None => {
                    let block =
                        find_matching_parent(&anchor_node, |n| n.is_element() && !n.is_inline());
                    if let Some(next_block) = block.and_then(|b| b.get_next_sibling())
                        && next_block.is_element()
                    {
                        next_block.select_start()?;
                    }
                }
                Some(_) => {}
            }
        } else if character_to_search_for == u16::from(b'\t') {
            if let Some(tab) = next_sibling.filter(|n| n.get_type() == "tab") {
                tab.select_end()?;
            }
        } else if let Some(next) = next_sibling.filter(NodeRef::is_text)
            && next.get_text_content().encode_utf16().next() == Some(character_to_search_for)
        {
            next.select(Some(1), Some(1))?;
        }
    } else if text.get(offset) == Some(&character_to_search_for) {
        anchor_node.select(Some(offset + 1), Some(offset + 1))?;
    }
    Ok(())
}

/// `$canRemoveText(anchorNode, focusNode)`.
fn can_remove_text(anchor_node: &NodeRef, focus_node: &NodeRef) -> bool {
    anchor_node.get_key() != focus_node.get_key()
        || anchor_node.is_element()
        || focus_node.is_element()
        || !is_token_or_tab(anchor_node)
        || !is_token_or_tab(focus_node)
}

/// `$handleBeforeInput(event)`: dispatches the command an input stands
/// for. Text is always inserted in controlled mode, as a string
/// (`CONTROLLED_TEXT_INSERTION_COMMAND`), since there is no DOM edit to let
/// through and no DOM to read it back from; JS's workarounds for Android,
/// iOS, Chrome and Firefox, and its target ranges, have no counterpart.
pub(crate) fn handle_before_input(
    event: &Rc<InputEvent>,
    editor: &Editor,
) -> Result<bool, LexicalError> {
    let input_type = event.input_type.as_str();
    let selection = get_selection();

    if input_type == "deleteContentBackward" {
        match &selection {
            None => {
                // Use previous selection
                let Some(prev_selection) = get_previous_selection().filter(SelectionRef::is_range)
                else {
                    return Ok(true);
                };
                set_selection(Some(prev_selection.clone_selection()));
            }
            Some(selection) if selection.is_range() => {
                set_composition_key(None);
                event.prevent_default();
                editor.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
                return Ok(true);
            }
            Some(_) => {}
        }
    }

    let Some(selection) = selection.filter(SelectionRef::is_range) else {
        if input_type == "historyUndo" || input_type == "historyRedo" {
            event.prevent_default();
            let command = if input_type == "historyUndo" {
                UNDO_COMMAND
            } else {
                REDO_COMMAND
            };
            editor.dispatch_command(&command, ())?;
        }
        return Ok(true);
    };

    let data = event.data.as_deref();
    if input_type == "insertText" || input_type == "insertTranspose" {
        if data == Some("\n") {
            event.prevent_default();
            editor.dispatch_command(&INSERT_LINE_BREAK_COMMAND, false)?;
        } else if data == Some(DOUBLE_LINE_BREAK) {
            event.prevent_default();
            editor.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
        } else if data.is_none()
            && let Some(data_transfer) = &event.data_transfer
        {
            // Gets around a Safari text replacement bug.
            let text = data_transfer.get_data("text/plain");
            event.prevent_default();
            selection.insert_raw_text(&text)?;
        } else if let Some(data) = data {
            event.prevent_default();
            editor.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, data.to_owned())?;
            maybe_move_selection_past_trailing_acceptance_boundary(editor, Some(data))?;
        }
        return Ok(true);
    }

    // Prevent the default, so we control the output.
    event.prevent_default();
    let range = selection.range().expect("a range selection");
    let anchor_node = range.anchor.get_node()?;
    let focus_node = range.focus.get_node()?;

    match input_type {
        "insertFromYank" | "insertFromDrop" | "insertReplacementText" => {
            let text = event
                .data_transfer
                .as_ref()
                .map(|d| d.get_data("text/plain"));
            let inserted = text.as_deref().or(data);
            editor.dispatch_command(
                &CONTROLLED_TEXT_INSERTION_COMMAND,
                inserted.unwrap_or("").to_owned(),
            )?;
            maybe_move_selection_past_trailing_acceptance_boundary(editor, inserted)?;
        }
        "insertFromComposition" => {
            let prev_composition_key = editor.composition_key();
            set_composition_key(None);
            editor.dispatch_command(
                &CONTROLLED_TEXT_INSERTION_COMMAND,
                data.unwrap_or("").to_owned(),
            )?;
            cleanup_composed_subclass(editor, prev_composition_key.as_deref())?;
        }
        "insertLineBreak" => {
            // Used for Android
            set_composition_key(None);
            editor.0.input_state.borrow_mut().is_insert_line_break = false;
            editor.dispatch_command(&INSERT_LINE_BREAK_COMMAND, false)?;
        }
        "insertParagraph" => {
            // Used for Android
            set_composition_key(None);
            // Safari does not provide the type "insertLineBreak", so the
            // keydown handler saves the intent for this input.
            let is_insert_line_break =
                std::mem::take(&mut editor.0.input_state.borrow_mut().is_insert_line_break);
            if is_insert_line_break {
                editor.dispatch_command(&INSERT_LINE_BREAK_COMMAND, false)?;
            } else {
                editor.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
            }
        }
        "insertFromPaste" | "insertFromPasteAsQuotation" => {
            editor.dispatch_command(&PASTE_COMMAND, PasteCommandType::Input(event.clone()))?;
        }
        "deleteByComposition" => {
            if can_remove_text(&anchor_node, &focus_node) {
                editor.dispatch_command(&REMOVE_TEXT_COMMAND, ())?;
            }
        }
        "deleteByDrag" => {
            // The drop target is taking over focus and the selection.
            editor.add_tags(&[SKIP_SELECTION_FOCUS_TAG.to_owned()]);
            editor.dispatch_command(&REMOVE_TEXT_COMMAND, ())?;
        }
        "deleteByCut" => {
            editor.dispatch_command(&REMOVE_TEXT_COMMAND, ())?;
        }
        "deleteContent" => {
            editor.dispatch_command(&DELETE_CHARACTER_COMMAND, false)?;
        }
        "deleteWordBackward" => {
            editor.dispatch_command(&DELETE_WORD_COMMAND, true)?;
        }
        "deleteWordForward" => {
            editor.dispatch_command(&DELETE_WORD_COMMAND, false)?;
        }
        "deleteHardLineBackward" | "deleteSoftLineBackward" => {
            editor.dispatch_command(&DELETE_LINE_COMMAND, true)?;
        }
        "deleteContentForward" | "deleteHardLineForward" | "deleteSoftLineForward" => {
            // JS deletes one character instead when the target range lay
            // before a caret in a slot (#9363); there are no target ranges
            // here, and an app sets the selection to the range it deletes.
            editor.dispatch_command(&DELETE_LINE_COMMAND, false)?;
        }
        "formatStrikeThrough" => {
            editor.dispatch_command(&FORMAT_TEXT_COMMAND, "strikethrough".to_owned())?;
        }
        "formatBold" => {
            editor.dispatch_command(&FORMAT_TEXT_COMMAND, "bold".to_owned())?;
        }
        "formatItalic" => {
            editor.dispatch_command(&FORMAT_TEXT_COMMAND, "italic".to_owned())?;
        }
        "formatUnderline" => {
            editor.dispatch_command(&FORMAT_TEXT_COMMAND, "underline".to_owned())?;
        }
        "historyUndo" => {
            editor.dispatch_command(&UNDO_COMMAND, ())?;
        }
        "historyRedo" => {
            editor.dispatch_command(&REDO_COMMAND, ())?;
        }
        _ => {}
    }
    Ok(true)
}

// The default composition handlers.

/// `$handleCompositionStart(event)`: marks the anchor's node as the one
/// being composed, in an update tagged `composition-start`. JS also
/// prepares a text node for the browser to compose into (the empty space
/// heuristic, which inserts a placeholder character); an app writes the
/// marked text through the model, which makes one as typing does.
fn handle_composition_start(
    _event: &Rc<CompositionEvent>,
    editor: &Editor,
) -> Result<bool, LexicalError> {
    if let Some(range) = get_selection().and_then(|s| s.range())
        && !editor.is_composing()
    {
        set_composition_key(Some(&range.anchor.key));
        editor.add_tags(&[COMPOSITION_START_TAG.to_owned()]);
    }
    Ok(true)
}

/// `$cleanupComposedSubclass(compositionKey)`: a composition that ended in
/// a plain-text subclass node it was redirected into (a segmented node made
/// normal) turns that node back into a `TextNode`.
fn cleanup_composed_subclass(
    editor: &Editor,
    composition_key: Option<&str>,
) -> Result<(), LexicalError> {
    let composed_segmented_key = editor
        .0
        .input_state
        .borrow_mut()
        .composed_segmented_key
        .take();
    let Some(composition_key) = composition_key else {
        return Ok(());
    };
    if composed_segmented_key.as_deref() != Some(composition_key) {
        return Ok(());
    }
    let Some(composed_node) = get_node_by_key(composition_key) else {
        return Ok(());
    };
    if !composed_node.is_text()
        || composed_node.get_type() == "text"
        || is_token_or_segmented(&composed_node)
        || !composed_node.is_attached()
    {
        return Ok(());
    }
    let offset = get_selection()
        .and_then(|s| s.range())
        .filter(|r| r.anchor.key == composition_key)
        .map(|r| r.anchor.offset);
    let replacement = create_text_node(&composed_node.get_text_content());
    replacement.set_format(composed_node.text_data().get_format());
    replacement.set_style(composed_node.text_data().get_style());
    composed_node.replace(&replacement, false)?;
    if let Some(offset) = offset {
        let safe_offset = offset.min(replacement.get_text_content_size());
        replacement.select(Some(safe_offset), Some(safe_offset))?;
    }
    Ok(())
}

/// `$onCompositionEndImpl(editor, data)`. The composed text is already in
/// the model (the app writes the marked text through it), so where JS
/// reads it back from the DOM (`$updateTextNodeFromDOMContent`,
/// `$updateSelectedTextFromDOM`) there is nothing left to do.
fn on_composition_end_impl(editor: &Editor, data: Option<&str>) -> Result<bool, LexicalError> {
    let composition_key = editor.composition_key();
    set_composition_key(None);

    // Handle termination of composition.
    if let (Some(key), Some(data)) = (composition_key.as_deref(), data) {
        if data.is_empty() {
            cleanup_composed_subclass(editor, Some(key))?;
            return Ok(false);
        }
        if data.ends_with('\n')
            && let Some(selection) = get_selection()
        {
            // If the last character is a line break, we also need to insert
            // a line break.
            if let Some(range) = selection.range() {
                let focus = range.focus;
                selection.point_set(Which::Anchor, &focus.key, focus.offset, focus.point_type)?;
            }
            editor.dispatch_command(&KEY_ENTER_COMMAND, None)?;
            cleanup_composed_subclass(editor, Some(key))?;
            return Ok(false);
        }
        // When composition ends on a token node, the composed text goes
        // beside it, through insertText's token redirect.
        if let Some(node) = get_node_by_key(key).filter(NodeRef::is_text)
            && is_token_or_segmented(&node)
        {
            node.mark_dirty();
            let text_len = node.get_text_content_size();
            let offset = get_selection()
                .and_then(|s| s.range())
                .filter(|r| r.anchor.key == key)
                .map_or(text_len, |r| r.anchor.offset);
            node.select(Some(offset), Some(offset))?.insert_text(data)?;
            return Ok(true);
        }
    }
    cleanup_composed_subclass(editor, composition_key.as_deref())?;
    Ok(false)
}

/// `$handleCompositionEnd(event)`: ends the composition, in an update
/// tagged `composition-end`.
fn handle_composition_end(
    event: &Rc<CompositionEvent>,
    editor: &Editor,
) -> Result<bool, LexicalError> {
    on_composition_end_impl(editor, Some(&event.data))?;
    editor.add_tags(&[COMPOSITION_END_TAG.to_owned()]);
    Ok(true)
}

/// `registerDefaultCommandHandlers(editor)`: the handlers every editor has,
/// which `createEditor` registers. JS's `INPUT_COMMAND` handler
/// (`$handleInput`) reads what the browser wrote to the DOM, which an app
/// never does, so it isn't registered.
pub(crate) fn register_default_command_handlers(editor: &Editor) -> Unregister {
    let unregisters = [
        editor.register_command(
            &BEFORE_INPUT_COMMAND,
            handle_before_input,
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &COMPOSITION_START_COMMAND,
            handle_composition_start,
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &COMPOSITION_END_COMMAND,
            handle_composition_end,
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(&KEY_DOWN_COMMAND, handle_key_down, COMMAND_PRIORITY_EDITOR),
    ];
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_by_format_in_order() {
        let data_transfer = DataTransfer::new();
        data_transfer.set_data("text/html", "");
        data_transfer.set_data("Text", "plain");
        data_transfer.set_data("text/html", "<b>x</b>");
        assert_eq!(data_transfer.types(), ["text/html", "text/plain"]);
        assert_eq!(data_transfer.get_data("text/plain"), "plain");
        assert_eq!(data_transfer.get_data("TEXT/HTML"), "<b>x</b>");
        assert_eq!(data_transfer.get_data("URL"), "");
        data_transfer.set_files(vec![DataTransferFile::new(
            "a.txt",
            "text/plain",
            b"a".to_vec(),
        )]);
        assert_eq!(data_transfer.types(), ["text/html", "text/plain", "Files"]);
        assert_eq!(data_transfer.files()[0].bytes().unwrap(), b"a");
        data_transfer.clear_data(Some("text"));
        assert_eq!(data_transfer.types(), ["text/html", "Files"]);
        data_transfer.clear_data(None);
        assert_eq!(data_transfer.types(), ["Files"]);
        assert_eq!(data_transfer.drop_effect(), "none");
        assert_eq!(data_transfer.effect_allowed(), "uninitialized");
    }
}
