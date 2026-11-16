//! The parts of `vectors/scripts/input-extensions` (ops.ts and
//! `extensionScripts.ts`) beyond the core runner: the extensions that follow
//! keys, clicks, composition and focus as `REAL_EXTENSIONS` with their
//! config overrides, the `formatKeyboardShortcut` and
//! `fixFocusOverselection` actions, and the `domEvent`, `logIME` and
//! `logEditor` steps.
//!
//! Where JS dispatches a DOM event at the element the reconciler made for a
//! node (`domEvent`), this dispatches the command an app's view sends for
//! it: `CLICK_COMMAND` of the event's type with the node's key (`root` for
//! the root element) as `target_key`, or `COMPOSITION_END_COMMAND`.

use super::{Ctx, ScriptCommand, err, known_command, mouse_event_of};
use lexical::extension::{
    AutoFocusConfigOverrides, ClickAfterLastBlockConfigOverrides, FormatKeyboardShortcutOptions,
    KeyboardShortcutsConfigOverrides, NamedKeyboardShortcuts, ShouldInsertAfter,
    auto_focus_extension, click_after_last_block_extension, format_keyboard_shortcut,
    get_extension_dependency_from_editor, ime_extension, keyboard_shortcuts_extension,
    normalize_triple_click_selection_extension,
};
use lexical::json::{Object, Value};
use lexical::link::{
    ClickableLinkConfigOverrides, GetLinkWindow, LinkWindow, clickable_link_extension,
};
use lexical::{
    AnyLexicalExtensionArgument, CLICK_COMMAND, ControlOrOther, Editor, EditorFocusOptions,
    KEY_DOWN_COMMAND, KeyboardEventModifierMask, KeyboardShortcut, LexicalError,
    ModifierExpectation, ShortcutDispatchFn, config_extension,
};
use std::rc::Rc;

/// The extensions of `REAL_EXTENSIONS` this module adds, without config
/// overrides.
pub(super) fn real_extension(key: &str) -> Option<AnyLexicalExtensionArgument> {
    Some(match key {
        "KeyboardShortcutsExtension" => keyboard_shortcuts_extension().into(),
        "ClickAfterLastBlockExtension" => click_after_last_block_extension().into(),
        "NormalizeTripleClickSelectionExtension" => {
            normalize_triple_click_selection_extension().into()
        }
        "IMEExtension" => ime_extension().into(),
        "AutoFocusExtension" => auto_focus_extension().into(),
        "ClickableLinkExtension" => clickable_link_extension().into(),
        _ => return None,
    })
}

fn bad(key: &str, field: &str, value: &Value) -> LexicalError {
    err(format!("bad config of {key}: {field} {value}"))
}

/// `configExtension(extension, ...configs.map(realConfig))` of one of this
/// module's extensions, `None` for any other.
pub(super) fn real_configured(
    ctx: &Rc<Ctx>,
    key: &str,
    configs: &[Object],
) -> Option<Result<AnyLexicalExtensionArgument, LexicalError>> {
    real_extension(key)?;
    let bool_of = |field: &str, v: &Value| v.as_bool().ok_or_else(|| bad(key, field, v));
    Some((|| {
        Ok(match key {
            "KeyboardShortcutsExtension" => {
                let mut all = Vec::new();
                for config in configs {
                    let mut o = KeyboardShortcutsConfigOverrides::default();
                    for (field, v) in config.iter() {
                        match field {
                            "disabled" => o.disabled = Some(bool_of(field, v)?),
                            "priority" => {
                                o.priority =
                                    Some(v.as_f64().ok_or_else(|| bad(key, field, v))? as i8)
                            }
                            "shortcuts" => o.shortcuts = Some(shortcuts_of(ctx, v)?),
                            _ => return Err(bad(key, field, v)),
                        }
                    }
                    all.push(o);
                }
                config_extension(&keyboard_shortcuts_extension(), all)
            }
            "ClickAfterLastBlockExtension" => {
                let mut all = Vec::new();
                for config in configs {
                    let mut o = ClickAfterLastBlockConfigOverrides::default();
                    for (field, v) in config.iter() {
                        match field {
                            "disabled" => o.disabled = Some(bool_of(field, v)?),
                            // `$shouldInsertAfter: 'always'`.
                            "$shouldInsertAfter" if v.as_str() == Some("always") => {
                                o.should_insert_after = Some(ShouldInsertAfter(Rc::new(|_| true)))
                            }
                            _ => return Err(bad(key, field, v)),
                        }
                    }
                    all.push(o);
                }
                config_extension(&click_after_last_block_extension(), all)
            }
            "AutoFocusExtension" => {
                let mut all = Vec::new();
                for config in configs {
                    let mut o = AutoFocusConfigOverrides::default();
                    for (field, v) in config.iter() {
                        match (field, v.as_str()) {
                            ("disabled", _) => o.disabled = Some(bool_of(field, v)?),
                            ("defaultSelection", Some("rootStart")) => {
                                o.default_selection = Some(EditorFocusOptions::RootStart)
                            }
                            ("defaultSelection", Some("rootEnd")) => {
                                o.default_selection = Some(EditorFocusOptions::RootEnd)
                            }
                            _ => return Err(bad(key, field, v)),
                        }
                    }
                    all.push(o);
                }
                config_extension(&auto_focus_extension(), all)
            }
            "ClickableLinkExtension" => {
                let mut all = Vec::new();
                for config in configs {
                    let mut o = ClickableLinkConfigOverrides::default();
                    for (field, v) in config.iter() {
                        match (field, v.as_str()) {
                            ("disabled", _) => o.disabled = Some(bool_of(field, v)?),
                            ("newTab", _) => o.new_tab = Some(bool_of(field, v)?),
                            // `getWindow: 'log'`: a window whose `open` logs
                            // `['open', url, target, features]`.
                            ("getWindow", Some("log")) => {
                                let window: Rc<dyn LinkWindow> = Rc::new(LogWindow(ctx.clone()));
                                o.get_window =
                                    Some(GetLinkWindow(Rc::new(move |_| Some(window.clone()))))
                            }
                            ("getWindow", Some("none")) => {
                                o.get_window = Some(GetLinkWindow(Rc::new(|_| None)))
                            }
                            _ => return Err(bad(key, field, v)),
                        }
                    }
                    all.push(o);
                }
                config_extension(&clickable_link_extension(), all)
            }
            _ => return Err(err(format!("{key} takes no config overrides"))),
        })
    })())
}

/// `getWindow: 'log'`'s window.
struct LogWindow(Rc<Ctx>);

impl LinkWindow for LogWindow {
    fn open(&self, url: &str, target: &str, features: Option<&str>) {
        self.0.log(vec![
            "open".into(),
            url.into(),
            target.into(),
            features.map_or(Value::Null, Value::from),
        ]);
    }
}

/// A `ModifiersJson` as its mask.
fn modifiers_of(json: &Value) -> Result<KeyboardEventModifierMask, LexicalError> {
    let expectation = |name: &str| match json.get(name) {
        None | Some(Value::Bool(false)) => Ok(ModifierExpectation::Released),
        Some(Value::Bool(true)) => Ok(ModifierExpectation::Pressed),
        Some(Value::String(s)) if s == "any" => Ok(ModifierExpectation::Any),
        Some(other) => Err(err(format!("bad modifier {name}: {other}"))),
    };
    Ok(KeyboardEventModifierMask {
        alt_key: expectation("altKey")?,
        ctrl_key: expectation("ctrlKey")?,
        meta_key: expectation("metaKey")?,
        shift_key: expectation("shiftKey")?,
        control_or_other: match json.get("controlOrOther").and_then(Value::as_str) {
            None => None,
            Some("metaKey") => Some(ControlOrOther::MetaKey),
            Some("altKey") => Some(ControlOrOther::AltKey),
            Some(other) => return Err(err(format!("bad controlOrOther {other}"))),
        },
    })
}

/// `scriptShortcutMatch`: a `ShortcutJson` as a [`KeyboardShortcut`]
/// without functions, its command the keyboard command it names
/// (`KEY_DOWN_COMMAND` for a match that names none).
fn shortcut_match_of(json: &Value) -> Result<KeyboardShortcut, LexicalError> {
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| err(format!("a shortcut needs a key: {json}")))?;
    let command = match json.get("command").and_then(Value::as_str) {
        None => KEY_DOWN_COMMAND,
        Some(name) => match known_command(name) {
            Some(ScriptCommand::Keyboard(command)) => command,
            _ => return Err(err(format!("{name} is not a keyboard command"))),
        },
    };
    let mut shortcut = KeyboardShortcut::new(key, command);
    if let Some(modifiers) = json.get("modifiers") {
        shortcut.modifiers = modifiers_of(modifiers)?;
    }
    shortcut.unshifted_key = json
        .get("unshiftedKey")
        .and_then(Value::as_str)
        .map(str::to_owned);
    shortcut.description = json
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_owned);
    shortcut.bubble_from_nested_editors =
        json.get("bubbleFromNestedEditors").and_then(Value::as_bool) == Some(true);
    Ok(shortcut)
}

/// `scriptShortcuts`' shortcut: [`shortcut_match_of`] with its `$disabled`
/// and `$dispatch` by name.
fn shortcut_of(ctx: &Rc<Ctx>, json: &Value) -> Result<KeyboardShortcut, LexicalError> {
    let mut shortcut = shortcut_match_of(json)?;
    match json.get("$disabled").and_then(Value::as_str) {
        None => {}
        Some("always") => shortcut.disabled = Some(Rc::new(|_, _| Ok(true))),
        Some("notRange") => {
            shortcut.disabled = Some(Rc::new(|selection, _| {
                Ok(!selection.is_some_and(|s| s.is_range()))
            }))
        }
        Some(other) => return Err(err(format!("bad $disabled {other}"))),
    }
    if let Some(name) = json.get("$dispatch").and_then(Value::as_str) {
        let handled = match name {
            "log" => true,
            "unhandled" => false,
            other => return Err(err(format!("bad $dispatch {other}"))),
        };
        let ctx = ctx.clone();
        let dispatch: ShortcutDispatchFn = Rc::new(move |_, event, next, from_editor| {
            // `fromEditor === ctx.editor`: with no `parent` script every
            // editor is the session's, and with one the parent isn't.
            let is_editor = ctx
                .parent
                .borrow()
                .as_ref()
                .is_none_or(|parent| **parent != *from_editor);
            ctx.log(vec![
                "shortcutDispatch".into(),
                event.key.as_str().into(),
                is_editor.into(),
            ]);
            if handled { next() } else { Ok(false) }
        });
        shortcut.dispatch = Some(dispatch);
    }
    Ok(shortcut)
}

/// `scriptShortcuts(ctx, json)`: a `ShortcutsJson` as a table.
fn shortcuts_of(ctx: &Rc<Ctx>, json: &Value) -> Result<NamedKeyboardShortcuts, LexicalError> {
    let object = json
        .as_object()
        .ok_or_else(|| err(format!("bad shortcuts {json}")))?;
    let mut table = NamedKeyboardShortcuts::new();
    for (name, value) in object.iter() {
        let shortcuts = match value {
            Value::Null => None,
            Value::Array(list) => Some(
                list.iter()
                    .map(|s| shortcut_of(ctx, s))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            single => Some(vec![shortcut_of(ctx, single)?]),
        };
        table.set(name, shortcuts);
    }
    Ok(table)
}

/// The actions this module runs, `None` for any other.
pub(super) fn run_action(
    action: &Value,
    ctx: &Ctx,
    editor: &Editor,
) -> Option<Result<(), LexicalError>> {
    Some(match action.get("do").and_then(Value::as_str)? {
        "formatKeyboardShortcut" => (|| {
            let shortcut = shortcut_match_of(action.get("shortcut").unwrap_or(&Value::Null))?;
            let options = FormatKeyboardShortcutOptions {
                is_apple: action.get("isApple").and_then(Value::as_bool),
                ..Default::default()
            };
            let segments = format_keyboard_shortcut(&shortcut, &options);
            ctx.log(vec![
                "formatKeyboardShortcut".into(),
                Value::Array(segments.into_iter().map(Value::from).collect()),
            ]);
            Ok(())
        })(),
        "fixFocusOverselection" => get_extension_dependency_from_editor(
            editor,
            &normalize_triple_click_selection_extension(),
        )
        .and_then(|dep| {
            let output = dep.output.ok_or_else(|| err("no output"))?;
            (output.fix_focus_overselection.peek().0)()
        }),
        _ => return None,
    })
}

/// The steps this module runs, `None` for any other.
pub(super) fn run_step(
    step: &Value,
    ctx: &Rc<Ctx>,
    editor: &Editor,
) -> Option<Result<(), LexicalError>> {
    if let Some(event) = step.get("domEvent") {
        return Some(dom_event(event, ctx, editor));
    }
    if step.get("logEditor").is_some() {
        ctx.log(vec![
            "editor".into(),
            editor.get_namespace().into(),
            editor.is_editable().into(),
            editor.get_root_element().is_some().into(),
        ]);
        return Some(Ok(()));
    }
    if step.get("logIME").is_some() {
        return Some(
            get_extension_dependency_from_editor(editor, &ime_extension()).and_then(|dep| {
                let output = dep.output.ok_or_else(|| err("no output"))?;
                let node = output.composing_text_node.peek();
                ctx.log(vec![
                    "ime".into(),
                    output
                        .composition_key
                        .peek()
                        .map_or(Value::Null, Value::from),
                    node.map_or(Value::Null, |n| n.get_key().into()),
                ]);
                Ok(())
            }),
        );
    }
    None
}

/// The `domEvent` step: the command a view sends for the event, then
/// `['domEvent', type, defaultPrevented]`.
fn dom_event(json: &Value, ctx: &Rc<Ctx>, editor: &Editor) -> Result<(), LexicalError> {
    let type_ = json
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| err(format!("a domEvent needs a type: {json}")))?;
    let prevented = match type_ {
        "click" | "auxclick" => {
            let mut event =
                mouse_event_of(type_, json).ok_or_else(|| err(format!("bad event {json}")))?;
            // The root element when `targetKey` is left out.
            event.target_key.get_or_insert_with(|| "root".to_owned());
            let event = Rc::new(event);
            editor.dispatch_command(&CLICK_COMMAND, event.clone())?;
            event.default_prevented()
        }
        "compositionend" => {
            let mut payload = Object::new();
            payload.insert(
                "data",
                json.get("data").cloned().unwrap_or_else(|| "".into()),
            );
            let (_, prevented, _) = ctx
                .command("COMPOSITION_END_COMMAND")
                .dispatch(editor, Value::Object(payload))?;
            prevented.unwrap_or(false)
        }
        other => return Err(err(format!("unknown domEvent {other}"))),
    };
    ctx.log(vec!["domEvent".into(), type_.into(), prevented.into()]);
    Ok(())
}
