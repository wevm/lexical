//! Scripts with `extensions`: the editor is built with
//! `buildEditorFromExtensions` from test extensions that `ops.ts`'s
//! `testExtension` makes from JSON (`ExtensionSpec`), and a few of
//! `@lexical/extension`'s own (`REAL_EXTENSIONS`). Each test extension logs
//! every phase it runs, so the log is the order the builder ran them in.

use super::{Ctx, err};
use lexical::extension::{DecoratorTextNode, HorizontalRuleNode};
use lexical::extension::{
    LexicalEditorWithDispose, NamedSignalsOutput, batch, build_editor_from_extensions,
    clear_editor_extension, computed, decorator_text_extension, editor_state_extension, effect,
    get_extension_dependency_from_editor, get_known_types_and_nodes,
    get_peer_dependency_from_editor, named_signals, untracked,
};
use lexical::json::{Object, Value};
use lexical::{
    AnyLexicalExtensionArgument, COMMAND_PRIORITY_EDITOR, Editor, ExtensionBuildState,
    ExtensionInitState, ExtensionRegisterState, InitialEditorConfig, InitialEditorStateType,
    LexicalError, LexicalExtension, NodeClass, Unregister, config_extension, create_paragraph_node,
    create_text_node, declare_peer_dependency, define_extension, get_root, shallow_merge_config,
};
use std::rc::Rc;

/// A test extension: its config is JSON, its output the `namedSignals` of
/// its config (empty unless it has `signals`), its init result
/// `{init: name}`.
pub(super) type TestExtension = LexicalExtension<Object, NamedSignalsOutput, Value>;

const PHASES: [&str; 4] = ["init", "build", "register", "afterRegistration"];

/// `REAL_EXTENSIONS`.
fn real_extension(key: &str) -> Option<AnyLexicalExtensionArgument> {
    Some(match key {
        "ClearEditorExtension" => clear_editor_extension().into(),
        "DecoratorTextExtension" => decorator_text_extension().into(),
        "EditorStateExtension" => editor_state_extension().into(),
        _ => return None,
    })
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
        if let Some(real) = real_extension(key) {
            return Ok(real);
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
            other => return Err(err(format!("unknown node type {other}"))),
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
                let extension = test_extension_of(ctx, key.as_str().unwrap_or(""))?;
                let output = get_extension_dependency_from_editor(editor, &extension)?
                    .output
                    .ok_or_else(|| err("no output"))?;
                let signal = output
                    .get(name.as_str().unwrap_or(""))
                    .ok_or_else(|| err(format!("no signal {name}")))?;
                signal.set_value(value.clone());
            }
            Ok(())
        })?;
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
