//! Scripts with `extensions`: the editor is built with
//! `buildEditorFromExtensions` from test extensions that `ops.ts`'s
//! `testExtension` makes from JSON (`ExtensionSpec`), and a few of
//! `@lexical/extension`'s own (`REAL_EXTENSIONS`). Each test extension logs
//! every phase it runs, so the log is the order the builder ran them in.
//!
//! The packages' own extensions (`RichTextExtension`, `HistoryExtension`...)
//! are real extensions too, whose config overrides are the JSON fields of
//! their configs and name their functions by `ops.ts`'s `CONFIG_PRESETS`
//! ([`real_configured`]).

use super::{Ctx, err};
use lexical::extension::{
    BlockIdConfigOverrides, GenerateId, IsIdentifiedBlock, block_id_extension, is_root_child_block,
};
use lexical::extension::{
    CanIndentPredicate, LexicalBuilder, NodeStructureConfigOverrides,
    NormalizeInlineElementsConfigOverrides, SelectBlockConfigOverrides,
    TabIndentationConfigOverrides, normalize_inline_elements_extension, select_block_extension,
    tab_indentation_extension,
};
use lexical::extension::{DecoratorTextNode, HorizontalRuleNode};
use lexical::extension::{
    LexicalEditorWithDispose, NamedSignalsOutput, batch, build_editor_from_extensions,
    clear_editor_extension, computed, decorator_text_extension, editor_state_extension, effect,
    get_extension_dependency_from_editor, get_known_types_and_nodes,
    get_peer_dependency_from_editor, named_signals, node_structure_extension, untracked,
};
use lexical::json::{Object, Value};
use lexical::{
    AnyLexicalExtensionArgument, COMMAND_PRIORITY_EDITOR, Editor, ExtensionBuildState,
    ExtensionInitState, ExtensionRegisterState, InitialEditorConfig, InitialEditorStateType,
    LexicalError, LexicalExtension, NodeClass, Unregister, config_extension, create_paragraph_node,
    create_text_node, declare_peer_dependency, define_extension, get_root, shallow_merge_config,
};
use lexical::{a11y, code, hashtag, history, link, list, overflow, plain_text, rich_text};
use std::cell::Cell;
use std::rc::Rc;

/// A test extension: its config is JSON, its output the `namedSignals` of
/// its config (empty unless it has `signals`), its init result
/// `{init: name}`.
pub(super) type TestExtension = LexicalExtension<Object, NamedSignalsOutput, Value>;

const PHASES: [&str; 4] = ["init", "build", "register", "afterRegistration"];

/// `REAL_EXTENSIONS`, without config overrides, but for `HistoryExtension`
/// (see [`real_extension_in`]).
fn real_extension(key: &str) -> Option<AnyLexicalExtensionArgument> {
    Some(match key {
        "ClearEditorExtension" => clear_editor_extension().into(),
        "DecoratorTextExtension" => decorator_text_extension().into(),
        "EditorStateExtension" => editor_state_extension().into(),
        "AutoLinkExtension" => link::auto_link_extension().into(),
        "BlockIdExtension" => block_id_extension().into(),
        "CheckListExtension" => list::check_list_extension().into(),
        "CodeExtension" => code::code_extension().into(),
        "CodeIndentExtension" => code::code_indent_extension().into(),
        "HashtagExtension" => hashtag::hashtag_extension().into(),
        "LinkExtension" => link::link_extension().into(),
        "ListExtension" => list::list_extension().into(),
        "NodeStructureExtension" => node_structure_extension().into(),
        "NormalizeInlineElementsExtension" => normalize_inline_elements_extension().into(),
        "OverflowExtension" => overflow::overflow_extension().into(),
        "PlainTextExtension" => plain_text::plain_text_extension().into(),
        "RichTextExtension" => rich_text::rich_text_extension().into(),
        "SelectBlockExtension" => select_block_extension().into(),
        "TabIndentationExtension" => tab_indentation_extension().into(),
        "AriaLiveRegionExtension" => a11y::aria_live_region_extension().into(),
        "AutoLinkAnnounceExtension" => link::auto_link_announce_extension().into(),
        "EditorModeAnnounceExtension" => a11y::editor_mode_announce_extension().into(),
        "HeadingAnnounceExtension" => rich_text::heading_announce_extension().into(),
        "HistoryAnnounceExtension" => a11y::history_announce_extension().into(),
        _ => return None,
    })
}

/// `REAL_EXTENSIONS` in a session: `HistoryExtension` reads the session's
/// clock, as JS's reads `Date.now()`, which the session sets while a step
/// runs.
fn real_extension_in(
    ctx: &Rc<Ctx>,
    key: &str,
) -> Option<Result<AnyLexicalExtensionArgument, LexicalError>> {
    if key == "HistoryExtension" {
        return Some(real_configured(ctx, key, &[]));
    }
    real_extension(key).map(Ok)
}

/// `CONFIG_PRESETS`' `validateUrl: 'https'`.
fn https_only(url: &str) -> bool {
    url.starts_with("https://")
}

/// `PREFILLED_HISTORY_STATE`.
const PREFILLED_HISTORY_STATE: &str = r#"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"prefilled","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;

fn bad_config(key: &str, field: &str, value: &Value) -> LexicalError {
    err(format!("bad config of {key}: {field} {value}"))
}

/// `configExtension(extension, ...configs.map(realConfig))` of a real
/// extension: each JSON config as its typed overrides, with the functions
/// `CONFIG_PRESETS` names.
fn real_configured(
    ctx: &Rc<Ctx>,
    key: &str,
    configs: &[Object],
) -> Result<AnyLexicalExtensionArgument, LexicalError> {
    let bool_of = |field: &str, v: &Value| v.as_bool().ok_or_else(|| bad_config(key, field, v));
    let number_of = |field: &str, v: &Value| v.as_f64().ok_or_else(|| bad_config(key, field, v));
    let string_of = |field: &str, v: &Value| {
        v.as_str()
            .map(str::to_owned)
            .ok_or_else(|| bad_config(key, field, v))
    };
    let usize_or_null = |field: &str, v: &Value| match v {
        Value::Null => Ok(None),
        v => number_of(field, v).map(|n| Some(n as usize)),
    };
    // Each config, field by field, into `$partial` with `$apply`.
    macro_rules! overrides {
        ($partial:ty, |$o:ident, $field:ident, $value:ident| $apply:expr) => {
            configs
                .iter()
                .map(|config| {
                    let mut $o = <$partial>::default();
                    for ($field, $value) in config.iter() {
                        $apply;
                    }
                    Ok($o)
                })
                .collect::<Result<Vec<$partial>, LexicalError>>()?
        };
    }
    let unknown =
        |field: &str, v: &Value| -> Result<(), LexicalError> { Err(bad_config(key, field, v)) };
    Ok(match key {
        "HistoryExtension" => {
            // The session's clock, first, so that a config could still set
            // its own (none can: `now` isn't JSON).
            let clock = ctx.clock.clone();
            let mut all = vec![history::HistoryConfigOverrides {
                now: Some(history::HistoryNow(Rc::new(move || clock.get()))),
                ..Default::default()
            }];
            all.extend(overrides!(
                history::HistoryConfigOverrides,
                |o, field, v| match field {
                    "delay" => o.delay = Some(number_of(field, v)?),
                    "disabled" => o.disabled = Some(bool_of(field, v)?),
                    "maxDepth" => o.max_depth = Some(usize_or_null(field, v)?),
                    "createInitialHistoryState" if v.as_str() == Some("prefilled") => {
                        o.create_initial_history_state = Some(history::CreateInitialHistoryState(
                            Rc::new(|editor: &Editor| {
                                let state = history::create_empty_history_state();
                                let editor_state = editor
                                    .parse_editor_state(PREFILLED_HISTORY_STATE)
                                    .expect("the prefilled history state parses");
                                state
                                    .borrow_mut()
                                    .undo_stack
                                    .push(history::HistoryStateEntry {
                                        editor: editor.clone(),
                                        editor_state,
                                    });
                                state
                            }),
                        ));
                    }
                    _ => unknown(field, v)?,
                }
            ));
            config_extension(&history::history_extension(), all)
        }
        "ListExtension" => config_extension(
            &list::list_extension(),
            overrides!(list::ListConfigOverrides, |o, field, v| match field {
                "hasStrictIndent" => o.has_strict_indent = Some(bool_of(field, v)?),
                "shouldPreserveNumbering" => o.should_preserve_numbering = Some(bool_of(field, v)?),
                _ => unknown(field, v)?,
            }),
        ),
        "CheckListExtension" => config_extension(
            &list::check_list_extension(),
            overrides!(list::CheckListConfigOverrides, |o, field, v| match field {
                "disableTakeFocusOnClick" => {
                    o.disable_take_focus_on_click = Some(bool_of(field, v)?)
                }
                _ => unknown(field, v)?,
            }),
        ),
        "LinkExtension" => config_extension(
            &link::link_extension(),
            overrides!(link::LinkConfigOverrides, |o, field, v| match field {
                "attributes" => {
                    o.attributes = Some(Some(
                        v.as_object()
                            .and_then(super::link_attributes_of)
                            .ok_or_else(|| bad_config(key, field, v))?,
                    ))
                }
                "validateUrl" if v.as_str() == Some("https") => {
                    o.validate_url = Some(Some(Rc::new(https_only)))
                }
                _ => unknown(field, v)?,
            }),
        ),
        "AutoLinkExtension" => config_extension(
            &link::auto_link_extension(),
            overrides!(link::AutoLinkConfigOverrides, |o, field, v| {
                let names = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|n| n.as_str().unwrap_or(""));
                match field {
                    "matchers" => {
                        o.matchers =
                            Some(
                                names
                                    .map(|name| match name {
                                        "url" => Ok(Rc::new(link::auto_link_url_matcher)
                                            as link::LinkMatcher),
                                        "email" => Ok(Rc::new(link::auto_link_email_matcher)
                                            as link::LinkMatcher),
                                        _ => Err(bad_config(key, field, v)),
                                    })
                                    .collect::<Result<_, _>>()?,
                            )
                    }
                    "changeHandlers" => {
                        o.change_handlers = Some(
                            names
                                .map(|name| match name {
                                    "log" => {
                                        let ctx = ctx.clone();
                                        Ok(Rc::new(
                                            move |url: Option<&str>, prev_url: Option<&str>| {
                                                let or_null = |u: Option<&str>| {
                                                    u.map_or(Value::Null, Value::from)
                                                };
                                                ctx.log(vec![
                                                    "autoLink".into(),
                                                    or_null(url),
                                                    or_null(prev_url),
                                                ]);
                                            },
                                        )
                                            as link::ChangeHandler)
                                    }
                                    _ => Err(bad_config(key, field, v)),
                                })
                                .collect::<Result<_, _>>()?,
                        )
                    }
                    _ => unknown(field, v)?,
                }
            }),
        ),
        "CodeIndentExtension" => config_extension(
            &code::code_indent_extension(),
            overrides!(code::CodeIndentConfigOverrides, |o, field, v| match field {
                "disabled" => o.disabled = Some(bool_of(field, v)?),
                "tabSize" => o.tab_size = Some(usize_or_null(field, v)?),
                "escapeWithArrows" => o.escape_with_arrows = Some(bool_of(field, v)?),
                _ => unknown(field, v)?,
            }),
        ),
        "NodeStructureExtension" => config_extension(
            &node_structure_extension(),
            overrides!(NodeStructureConfigOverrides, |o, field, v| match field {
                "disabled" => o.disabled = Some(bool_of(field, v)?),
                _ => unknown(field, v)?,
            }),
        ),
        "NormalizeInlineElementsExtension" => config_extension(
            &normalize_inline_elements_extension(),
            overrides!(
                NormalizeInlineElementsConfigOverrides,
                |o, field, v| match field {
                    "disabled" => o.disabled = Some(bool_of(field, v)?),
                    _ => unknown(field, v)?,
                }
            ),
        ),
        "TabIndentationExtension" => config_extension(
            &tab_indentation_extension(),
            overrides!(TabIndentationConfigOverrides, |o, field, v| match field {
                "disabled" => o.disabled = Some(bool_of(field, v)?),
                "maxIndent" => {
                    o.max_indent = Some(match v {
                        Value::Null => None,
                        v => Some(number_of(field, v)?),
                    })
                }
                "$canIndent" if v.as_str() == Some("never") => {
                    o.can_indent = Some(CanIndentPredicate(Rc::new(|_| false)))
                }
                _ => unknown(field, v)?,
            }),
        ),
        "BlockIdExtension" => config_extension(
            &block_id_extension(),
            overrides!(BlockIdConfigOverrides, |o, field, v| match field {
                "disabled" => o.disabled = Some(bool_of(field, v)?),
                "repairTransforms" => o.repair_transforms = Some(bool_of(field, v)?),
                "generateId" => {
                    o.generate_id = Some(match v.as_str() {
                        // `id1`, `id2`...
                        Some("counter") => {
                            let n = Rc::new(Cell::new(0u64));
                            GenerateId(Rc::new(move || {
                                n.set(n.get() + 1);
                                format!("id{}", n.get())
                            }))
                        }
                        // Each of `id1`, `id2`... twice in a row.
                        Some("stutter") => {
                            let n = Rc::new(Cell::new(0u64));
                            GenerateId(Rc::new(move || {
                                let id = format!("id{}", (n.get() >> 1) + 1);
                                n.set(n.get() + 1);
                                id
                            }))
                        }
                        Some("invalid") => GenerateId(Rc::new(|| "not valid".to_owned())),
                        _ => return Err(bad_config(key, field, v)),
                    })
                }
                "$isIdentifiedBlock" if v.as_str() == Some("rootChild") => {
                    o.is_identified_block = Some(IsIdentifiedBlock(Rc::new(is_root_child_block)))
                }
                _ => unknown(field, v)?,
            }),
        ),
        "SelectBlockExtension" => config_extension(
            &select_block_extension(),
            overrides!(SelectBlockConfigOverrides, |o, field, v| match field {
                "disabled" => o.disabled = Some(bool_of(field, v)?),
                "cascadeSelection" => o.cascade_selection = Some(bool_of(field, v)?),
                _ => unknown(field, v)?,
            }),
        ),
        "AriaLiveRegionExtension" => config_extension(
            &a11y::aria_live_region_extension(),
            overrides!(
                a11y::AriaLiveRegionExtensionConfigOverrides,
                |o, field, v| match field {
                    "politeness" => {
                        o.politeness = Some(
                            v.as_str()
                                .and_then(a11y::AriaPoliteness::parse)
                                .ok_or_else(|| bad_config(key, field, v))?,
                        )
                    }
                    _ => unknown(field, v)?,
                }
            ),
        ),
        "HistoryAnnounceExtension" => config_extension(
            &a11y::history_announce_extension(),
            overrides!(
                a11y::HistoryAnnounceExtensionConfigOverrides,
                |o, field, v| match field {
                    "undone" => o.undone = Some(string_of(field, v)?),
                    "redone" => o.redone = Some(string_of(field, v)?),
                    "disabled" => o.disabled = Some(bool_of(field, v)?),
                    _ => unknown(field, v)?,
                }
            ),
        ),
        "EditorModeAnnounceExtension" => config_extension(
            &a11y::editor_mode_announce_extension(),
            overrides!(
                a11y::EditorModeAnnounceExtensionConfigOverrides,
                |o, field, v| match field {
                    "editable" => o.editable = Some(string_of(field, v)?),
                    "readOnly" => o.read_only = Some(string_of(field, v)?),
                    "disabled" => o.disabled = Some(bool_of(field, v)?),
                    _ => unknown(field, v)?,
                }
            ),
        ),
        "HeadingAnnounceExtension" => config_extension(
            &rich_text::heading_announce_extension(),
            overrides!(
                rich_text::HeadingAnnounceExtensionConfigOverrides,
                |o, field, v| match field {
                    "created" => o.created = Some(string_of(field, v)?),
                    "destroyed" => o.destroyed = Some(string_of(field, v)?),
                    "disabled" => o.disabled = Some(bool_of(field, v)?),
                    _ => unknown(field, v)?,
                }
            ),
        ),
        "AutoLinkAnnounceExtension" => config_extension(
            &link::auto_link_announce_extension(),
            overrides!(
                link::AutoLinkAnnounceExtensionConfigOverrides,
                |o, field, v| match field {
                    "created" => o.created = Some(string_of(field, v)?),
                    "createdMany" => o.created_many = Some(string_of(field, v)?),
                    "destroyed" => o.destroyed = Some(string_of(field, v)?),
                    "destroyedMany" => o.destroyed_many = Some(string_of(field, v)?),
                    "disabled" => o.disabled = Some(bool_of(field, v)?),
                    _ => unknown(field, v)?,
                }
            ),
        ),
        _ => return Err(err(format!("{key} takes no config overrides"))),
    })
}

/// `output[name].value = value` for a signal of a real extension's output.
fn set_real_signal(
    editor: &Editor,
    key: &str,
    name: &str,
    value: &Value,
) -> Option<Result<(), LexicalError>> {
    let bad = || err(format!("bad value of {key}'s {name}: {value}"));
    let as_bool = || value.as_bool().ok_or_else(bad);
    let as_string = || value.as_str().map(str::to_owned).ok_or_else(bad);
    let as_usize_or_null = || match value {
        Value::Null => Ok(None),
        v => v.as_f64().map(|n| Some(n as usize)).ok_or_else(bad),
    };
    fn output<C: lexical::ExtensionConfigBase, O: Clone + 'static, I: 'static>(
        editor: &Editor,
        extension: &Rc<LexicalExtension<C, O, I>>,
    ) -> Result<O, LexicalError> {
        let dep = get_extension_dependency_from_editor(editor, extension)?;
        dep.output
            .map(|o| (*o).clone())
            .ok_or_else(|| err("no output"))
    }
    let set = || -> Result<(), LexicalError> {
        match (key, name) {
            ("HistoryExtension", "disabled") => output(editor, &history::history_extension())?
                .disabled
                .set_value(as_bool()?),
            ("HistoryExtension", "delay") => output(editor, &history::history_extension())?
                .delay
                .set_value(value.as_f64().ok_or_else(bad)?),
            ("HistoryExtension", "maxDepth") => output(editor, &history::history_extension())?
                .max_depth
                .set_value(as_usize_or_null()?),
            ("ListExtension", "hasStrictIndent") => output(editor, &list::list_extension())?
                .has_strict_indent
                .set_value(as_bool()?),
            ("ListExtension", "shouldPreserveNumbering") => {
                output(editor, &list::list_extension())?
                    .should_preserve_numbering
                    .set_value(as_bool()?)
            }
            ("NormalizeInlineElementsExtension", "disabled") => {
                output(editor, &normalize_inline_elements_extension())?
                    .disabled
                    .set_value(as_bool()?)
            }
            ("CodeIndentExtension", "disabled") => output(editor, &code::code_indent_extension())?
                .disabled
                .set_value(as_bool()?),
            ("CodeIndentExtension", "tabSize") => output(editor, &code::code_indent_extension())?
                .tab_size
                .set_value(as_usize_or_null()?),
            ("TabIndentationExtension", "disabled") => {
                output(editor, &tab_indentation_extension())?
                    .disabled
                    .set_value(as_bool()?)
            }
            ("TabIndentationExtension", "maxIndent") => {
                output(editor, &tab_indentation_extension())?
                    .max_indent
                    .set_value(match value {
                        Value::Null => None,
                        v => Some(v.as_f64().ok_or_else(bad)?),
                    })
            }
            ("BlockIdExtension", "disabled") => output(editor, &block_id_extension())?
                .disabled
                .set_value(as_bool()?),
            ("NodeStructureExtension", "disabled") => output(editor, &node_structure_extension())?
                .disabled
                .set_value(as_bool()?),
            ("SelectBlockExtension", "disabled") => output(editor, &select_block_extension())?
                .disabled
                .set_value(as_bool()?),
            ("AriaLiveRegionExtension", "politeness") => {
                output(editor, &a11y::aria_live_region_extension())?
                    .politeness
                    .set_value(
                        as_string()
                            .ok()
                            .and_then(|p| a11y::AriaPoliteness::parse(&p))
                            .ok_or_else(bad)?,
                    )
            }
            ("HistoryAnnounceExtension", field) => {
                let o = output(editor, &a11y::history_announce_extension())?;
                match field {
                    "undone" => o.undone.set_value(as_string()?),
                    "redone" => o.redone.set_value(as_string()?),
                    "disabled" => o.disabled.set_value(as_bool()?),
                    _ => return Err(err(format!("no signal {name} of {key}"))),
                }
            }
            ("EditorModeAnnounceExtension", field) => {
                let o = output(editor, &a11y::editor_mode_announce_extension())?;
                match field {
                    "editable" => o.editable.set_value(as_string()?),
                    "readOnly" => o.read_only.set_value(as_string()?),
                    "disabled" => o.disabled.set_value(as_bool()?),
                    _ => return Err(err(format!("no signal {name} of {key}"))),
                }
            }
            ("HeadingAnnounceExtension", field) => {
                let o = output(editor, &rich_text::heading_announce_extension())?;
                match field {
                    "created" => o.created.set_value(as_string()?),
                    "destroyed" => o.destroyed.set_value(as_string()?),
                    "disabled" => o.disabled.set_value(as_bool()?),
                    _ => return Err(err(format!("no signal {name} of {key}"))),
                }
            }
            ("AutoLinkAnnounceExtension", field) => {
                let o = output(editor, &link::auto_link_announce_extension())?;
                match field {
                    "created" => o.created.set_value(as_string()?),
                    "createdMany" => o.created_many.set_value(as_string()?),
                    "destroyed" => o.destroyed.set_value(as_string()?),
                    "destroyedMany" => o.destroyed_many.set_value(as_string()?),
                    "disabled" => o.disabled.set_value(as_bool()?),
                    _ => return Err(err(format!("no signal {name} of {key}"))),
                }
            }
            _ => return Err(err(format!("no signal {name} of {key}"))),
        }
        Ok(())
    };
    (key == "HistoryExtension" || real_extension(key).is_some()).then(set)
}

/// `extensionOf(ctx, key)` for a test extension: the one made for `key`,
/// made on first use.
pub(super) fn test_extension_of(
    ctx: &Rc<Ctx>,
    key: &str,
) -> Result<Rc<TestExtension>, LexicalError> {
    if let Some(made) = ctx.extensions.borrow().get(key) {
        return Ok(made.clone());
    }
    let spec = ctx
        .extension_defs
        .borrow()
        .get(key)
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| err(format!("unknown extension {key}")))?;
    let made = test_extension(ctx, &spec)?;
    ctx.extensions
        .borrow_mut()
        .insert(key.to_owned(), made.clone());
    Ok(made)
}

/// `extensionArg(ctx, arg)`.
pub(super) fn extension_arg(
    ctx: &Rc<Ctx>,
    arg: &Value,
) -> Result<AnyLexicalExtensionArgument, LexicalError> {
    if let Some(key) = arg.as_str() {
        if let Some(real) = real_extension_in(ctx, key) {
            return real;
        }
        return Ok(test_extension_of(ctx, key)?.into());
    }
    let parts = arg
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| err(format!("bad extension argument {arg}")))?;
    let key = parts[0].as_str().unwrap_or("");
    let configs = parts[1..]
        .iter()
        .map(|c| c.as_object().cloned().ok_or_else(|| err("bad config")))
        .collect::<Result<Vec<_>, _>>()?;
    if real_extension_in(ctx, key).is_some() {
        return real_configured(ctx, key, &configs);
    }
    Ok(config_extension(&test_extension_of(ctx, key)?, configs))
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_owned))
        .collect()
}

fn flag(spec: &Object, key: &str) -> bool {
    spec.get(key).and_then(Value::as_bool) == Some(true)
}

fn names(names: Vec<String>) -> Value {
    Value::Array(names.into_iter().map(Value::from).collect())
}

fn root_text(editor: &Editor) -> Value {
    editor
        .get_editor_state()
        .read(|| get_root().get_text_content())
        .into()
}

/// JS's truthiness of a JSON value.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Undefined | Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => *n != 0.0 && !n.is_nan(),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// `${value}` of a JSON string.
fn template(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

/// `testExtension(ctx, spec)`.
fn test_extension(ctx: &Rc<Ctx>, spec: &Object) -> Result<Rc<TestExtension>, LexicalError> {
    let name = spec
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| err("an extension needs a name"))?
        .to_owned();
    let phases = match spec.get("phases") {
        Some(_) => strings(spec.get("phases")),
        None => PHASES.iter().map(|p| (*p).to_owned()).collect(),
    };
    let has = |phase: &str| phases.iter().any(|p| p == phase);
    let config = spec
        .get("config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut extension = TestExtension::new(name.clone(), config);
    extension.conflicts_with = strings(spec.get("conflictsWith"));
    for dep in spec
        .get("dependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        extension.dependencies.push(extension_arg(ctx, dep)?);
    }
    for peer in spec
        .get("peerDependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let peer = peer.as_array().ok_or_else(|| err("bad peer dependency"))?;
        let peer_name = peer
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| err("bad peer dependency"))?;
        let config = peer.get(1).and_then(Value::as_object).cloned();
        extension
            .peer_dependencies
            .push(declare_peer_dependency::<Object>(peer_name, config));
    }
    for node_type in strings(spec.get("nodes")) {
        extension.nodes.push(match node_type.as_str() {
            "decorator-text" => NodeClass::of::<DecoratorTextNode>(),
            "horizontalrule" => NodeClass::of::<HorizontalRuleNode>(),
            "autolink" => NodeClass::of::<link::AutoLinkNode>(),
            "mark" => NodeClass::of::<lexical::mark::MarkNode>(),
            "overflow" => NodeClass::of::<overflow::OverflowNode>(),
            other => super::structure::test_node_class(other)
                .ok_or_else(|| err(format!("unknown node type {other}")))?,
        });
    }
    match spec.get("initialEditorState") {
        None => {}
        Some(Value::Null) => extension.initial_editor_state = Some(InitialEditorStateType::Null),
        Some(initial) => {
            if let Some(json) = initial.get("json") {
                extension.initial_editor_state =
                    Some(InitialEditorStateType::Json(json.to_string()));
            } else {
                let text = initial
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| err("bad initialEditorState"))?
                    .to_owned();
                extension.initial_editor_state =
                    Some(InitialEditorStateType::Update(Rc::new(move |_| {
                        let paragraph = create_paragraph_node();
                        paragraph.append(&[create_text_node(&text)])?;
                        get_root().append(&[paragraph])
                    })));
            }
        }
    }
    if spec.get("mergeConfig").and_then(Value::as_str) == Some("concat") {
        extension.merge_config = Some(Rc::new(|config: &Object, overrides: &Object| {
            let mut merged = shallow_merge_config(config, Some(overrides));
            for (k, b) in overrides.iter() {
                if let (Some(Value::Array(a)), Value::Array(b)) = (config.get(k), b) {
                    merged.insert(k, Value::Array(a.iter().chain(b).cloned().collect()));
                }
            }
            merged
        }));
    }
    let inspect = flag(spec, "inspect");
    let init_peers = strings(spec.get("initPeers"));
    let build_peers = strings(spec.get("buildPeers"));
    let lookups = strings(spec.get("lookups"));
    if has("init") {
        let (ctx, name, lookups) = (ctx.clone(), name.clone(), lookups.clone());
        extension.init = Some(Rc::new(
            move |editor_config: &mut InitialEditorConfig,
                  config: &Object,
                  state: &ExtensionInitState| {
                let log = |entry: Vec<Value>| ctx.log(entry);
                log(vec![
                    "init".into(),
                    name.as_str().into(),
                    config.clone().into(),
                ]);
                if inspect {
                    let types = get_known_types_and_nodes(editor_config).types;
                    log(vec![
                        "knownTypes".into(),
                        name.as_str().into(),
                        Value::Array(types.into_iter().map(Value::from).collect()),
                    ]);
                    log(vec![
                        "dependents".into(),
                        name.as_str().into(),
                        names(state.get_direct_dependent_names()),
                    ]);
                    log(vec![
                        "peerNames".into(),
                        name.as_str().into(),
                        names(state.get_peer_name_set()),
                    ]);
                }
                for peer in &init_peers {
                    let config = state.get_peer::<Object>(peer)?;
                    log(vec![
                        "initPeer".into(),
                        name.as_str().into(),
                        peer.as_str().into(),
                        config.map_or(Value::Null, |c| (*c).clone().into()),
                    ]);
                }
                for key in &lookups {
                    let config = state.get_dependency(&test_extension_of(&ctx, key)?)?;
                    log(vec![
                        "initDependency".into(),
                        name.as_str().into(),
                        key.as_str().into(),
                        (*config).clone().into(),
                    ]);
                }
                let mut result = Object::new();
                result.insert("init", name.as_str().into());
                Ok(Value::Object(result))
            },
        ));
    }
    let signals = flag(spec, "signals");
    if has("build") {
        let (ctx, name) = (ctx.clone(), name.clone());
        extension.build = Some(Rc::new(
            move |_editor: &Editor, config: &Object, state: &ExtensionBuildState<Value>| {
                let log = |entry: Vec<Value>| ctx.log(entry);
                log(vec![
                    "build".into(),
                    name.as_str().into(),
                    config.clone().into(),
                ]);
                for peer in &build_peers {
                    let dep = state.get_peer::<Object, NamedSignalsOutput, Value>(peer)?;
                    log(vec![
                        "buildPeer".into(),
                        name.as_str().into(),
                        peer.as_str().into(),
                        dep.map_or(Value::Null, |d| (*d.config).clone().into()),
                    ]);
                }
                for key in &lookups {
                    let dep = state.get_dependency(&test_extension_of(&ctx, key)?)?;
                    log(vec![
                        "buildDependency".into(),
                        name.as_str().into(),
                        key.as_str().into(),
                        (*dep.config).clone().into(),
                    ]);
                }
                Ok(if signals {
                    named_signals(config, None)
                } else {
                    NamedSignalsOutput::default()
                })
            },
        ));
    }
    let (init_result, abort, listen, editor_state) = (
        flag(spec, "initResult"),
        flag(spec, "abort"),
        flag(spec, "listen"),
        flag(spec, "editorState"),
    );
    if has("register") {
        let (ctx, name) = (ctx.clone(), name.clone());
        extension.register = Some(Rc::new(
            move |editor: &Editor,
                  config: &Object,
                  state: &ExtensionRegisterState<Value, NamedSignalsOutput>| {
                let log = {
                    let ctx = ctx.clone();
                    move |entry: Vec<Value>| ctx.log(entry)
                };
                log(vec![
                    "register".into(),
                    name.as_str().into(),
                    config.clone().into(),
                    root_text(editor),
                ]);
                if init_result {
                    let result = state.get_init_result()?;
                    log(vec![
                        "initResult".into(),
                        name.as_str().into(),
                        (*result).clone(),
                    ]);
                }
                let mut cleanups: Vec<Unregister> = Vec::new();
                if abort {
                    let (log, name) = (log.clone(), name.clone());
                    state
                        .get_signal()?
                        .add_abort_listener(move || log(vec!["aborted".into(), name.into()]));
                }
                if listen {
                    let (log, name) = (log.clone(), name.clone());
                    cleanups.push(editor.register_update_listener(move |payload| {
                        let mut tags = payload.tags.clone();
                        tags.sort();
                        log(vec![
                            "update".into(),
                            name.as_str().into(),
                            Value::Array(tags.into_iter().map(Value::from).collect()),
                        ]);
                    }));
                }
                let command = Rc::new(ctx.command("EXTENSION_COMMAND"));
                if signals {
                    let output = state.get_output().ok_or_else(|| err("no output"))?;
                    let disabled = output
                        .get("disabled")
                        .cloned()
                        .ok_or_else(|| err("no disabled signal"))?;
                    let label = output
                        .get("label")
                        .cloned()
                        .ok_or_else(|| err("no label signal"))?;
                    let summary = {
                        let (disabled, label) = (disabled.clone(), label.clone());
                        computed(move || {
                            if truthy(&disabled.value()) {
                                "off".to_owned()
                            } else {
                                format!("on:{}", template(&label.value()))
                            }
                        })
                    };
                    {
                        let (log, name) = (log.clone(), name.clone());
                        cleanups.push(effect(move || {
                            log(vec![
                                "summary".into(),
                                name.as_str().into(),
                                summary.value().into(),
                            ]);
                            None
                        }));
                    }
                    let (log, name, editor) = (log.clone(), name.clone(), editor.clone());
                    cleanups.push(effect(move || {
                        if truthy(&disabled.value()) {
                            return None;
                        }
                        let current = untracked(|| label.value());
                        log(vec!["enable".into(), name.as_str().into(), current.clone()]);
                        let unregister = {
                            let (log, name) = (log.clone(), name.clone());
                            command.register(
                                &editor,
                                move |payload, _| {
                                    log(vec![
                                        "handled".into(),
                                        name.as_str().into(),
                                        current.clone(),
                                        payload.clone(),
                                    ]);
                                    Ok(false)
                                },
                                COMMAND_PRIORITY_EDITOR,
                            )
                        };
                        let (log, name) = (log.clone(), name.clone());
                        Some(Box::new(move || {
                            log(vec!["disable".into(), name.into()]);
                            unregister();
                        }))
                    }));
                } else {
                    let (log, name) = (log.clone(), name.clone());
                    cleanups.push(command.register(
                        editor,
                        move |payload, _| {
                            log(vec![
                                "handled".into(),
                                name.as_str().into(),
                                payload.clone(),
                            ]);
                            Ok(false)
                        },
                        COMMAND_PRIORITY_EDITOR,
                    ));
                }
                if editor_state {
                    let dep = state.get_dependency(&editor_state_extension())?;
                    let signal = dep.output.ok_or_else(|| err("no editor state signal"))?;
                    let (log, name) = (log.clone(), name.clone());
                    cleanups.push(effect(move || {
                        let text = signal.value().read(|| get_root().get_text_content());
                        log(vec![
                            "editorState".into(),
                            name.as_str().into(),
                            text.into(),
                        ]);
                        None
                    }));
                }
                let name = name.clone();
                Ok(Box::new(move || {
                    log(vec!["unregister".into(), name.into()]);
                    for cleanup in cleanups.into_iter().rev() {
                        cleanup();
                    }
                }) as Unregister)
            },
        ));
    }
    if has("afterRegistration") {
        let (ctx, name) = (ctx.clone(), name.clone());
        extension.after_registration = Some(Rc::new(
            move |editor: &Editor,
                  config: &Object,
                  _state: &ExtensionRegisterState<Value, NamedSignalsOutput>| {
                ctx.log(vec![
                    "afterRegistration".into(),
                    name.as_str().into(),
                    config.clone().into(),
                    root_text(editor),
                ]);
                let (ctx, name) = (ctx.clone(), name.clone());
                Ok(
                    Box::new(move || ctx.log(vec!["afterUnregister".into(), name.into()]))
                        as Unregister,
                )
            },
        ));
    }
    Ok(define_extension(extension))
}

/// `buildEditorFromExtensions(...script.extensions)`.
pub(super) fn build(
    ctx: &Rc<Ctx>,
    extensions: &[Value],
) -> Result<LexicalEditorWithDispose, LexicalError> {
    let args = extensions
        .iter()
        .map(|arg| extension_arg(ctx, arg))
        .collect::<Result<Vec<_>, _>>()?;
    build_editor_from_extensions(args)
}

/// `runExtensionStep(step, ctx)`.
pub(super) fn run_step(
    step: &Value,
    ctx: &Rc<Ctx>,
    built: Option<&LexicalEditorWithDispose>,
) -> Result<(), LexicalError> {
    let built = built.ok_or_else(|| err("the editor was not built from extensions"))?;
    let editor: &Editor = built;
    if step.get("dispose").is_some() {
        built.dispose();
    } else if let Some(entries) = step.get("setSignals").and_then(Value::as_array) {
        batch(|| {
            for entry in entries {
                let entry = entry.as_array().map(Vec::as_slice);
                let Some([key, name, value]) = entry else {
                    return Err(err("bad setSignals"));
                };
                let key = key.as_str().unwrap_or("");
                let name = name.as_str().unwrap_or("");
                if let Some(result) = set_real_signal(editor, key, name, value) {
                    result?;
                    continue;
                }
                let extension = test_extension_of(ctx, key)?;
                let output = get_extension_dependency_from_editor(editor, &extension)?
                    .output
                    .ok_or_else(|| err("no output"))?;
                let signal = output
                    .get(name)
                    .ok_or_else(|| err(format!("no signal {name}")))?;
                signal.set_value(value.clone());
            }
            Ok(())
        })?;
    } else if let Some(names) = step.get("hasExtensions").and_then(Value::as_array) {
        let builder = LexicalBuilder::from_editor(editor)?;
        for name in names {
            let name = name.as_str().unwrap_or("");
            ctx.log(vec![
                "hasExtension".into(),
                name.into(),
                builder.has_extension_by_name(name).into(),
            ]);
        }
    } else if step.get("logHistory").is_some() {
        let output = get_extension_dependency_from_editor(editor, &history::history_extension())?
            .output
            .ok_or_else(|| err("no output"))?;
        let history_state = output.history_state.value();
        let state = history_state.0.borrow();
        ctx.log(vec![
            "history".into(),
            output.can_undo.value().into(),
            output.can_redo.value().into(),
            (state.undo_stack.len() as f64).into(),
            (state.redo_stack.len() as f64).into(),
        ]);
    } else if let Some(key) = step.get("logDependency").and_then(Value::as_str) {
        let extension = test_extension_of(ctx, key)?;
        let dep = get_extension_dependency_from_editor(editor, &extension)?;
        ctx.log(vec![
            "dependency".into(),
            key.into(),
            (*dep.config).clone().into(),
        ]);
    } else if let Some(name) = step.get("logPeer").and_then(Value::as_str) {
        let dep =
            get_peer_dependency_from_editor::<Object, NamedSignalsOutput, Value>(editor, name)?;
        ctx.log(vec![
            "peer".into(),
            name.into(),
            dep.map_or(Value::Null, |d| (*d.config).clone().into()),
        ]);
    } else {
        return Err(err(format!("unknown step {step}")));
    }
    Ok(())
}
