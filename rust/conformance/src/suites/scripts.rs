//! `scripts/*.json`: editor sessions in the script language of
//! `packages/lexical-conformance/src/ops.ts`, interpreted here as `ops.ts`
//! interprets them for JS. Each step is one discrete update or one command
//! dispatch; after each, the editor state, the selection, the text and a
//! log of what listeners, transforms and command handlers saw must match
//! what JS recorded.
//!
//! A script that uses an action the port can't run yet (selection editing)
//! is counted as waiting on that part of `lexical`, never as passing.

use crate::case::{Case, Outcome, same};
use lexical::json::Value;
use lexical::{
    COMMAND_PRIORITY_EDITOR, CaretDirection, CreateEditorArgs, Direction, Editor, LexicalCommand,
    LexicalError, NodeRef, PointCaret, PointType, RangeSelection, SliceMode, TextOffset,
    TextPointCaretSlice, UpdateOptions, Which, caret_range_from_selection, create_command,
    create_editor, create_range_selection, get_caret_range, get_child_caret, get_node_by_key,
    get_root, get_selection, get_sibling_caret, get_text_point_caret, normalize_caret,
    parse_serialized_node, remove_text_from_caret_range, reset_random_key, set_selection,
    set_selection_from_caret_range, utf16,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// What `ops.ts`'s `Context` holds besides the editor.
#[derive(Default)]
struct Ctx {
    log: RefCell<Vec<Value>>,
    commands: RefCell<HashMap<String, LexicalCommand<Value>>>,
}

impl Ctx {
    fn log(&self, entry: Vec<Value>) {
        self.log.borrow_mut().push(Value::Array(entry));
    }

    /// `command(ctx, name)`: one command per name.
    fn command(&self, name: &str) -> LexicalCommand<Value> {
        self.commands
            .borrow_mut()
            .entry(name.to_owned())
            .or_insert_with(|| create_command(name))
            .clone()
    }
}

/// The part of `lexical` an action needs that isn't ported yet, if any.
fn unported_action(action: &Value) -> Option<&'static str> {
    match action.get("do").and_then(Value::as_str) {
        Some(
            "append"
            | "select"
            | "clearSelection"
            | "node"
            | "dispatch"
            | "logCaretRange"
            | "removeTextFromCaretRange"
            | "logNormalizeCaret",
        ) => None,
        Some("selection" | "selectAll" | "selectNodes" | "insertNodes") => {
            Some("lexical (selection editing)")
        }
        _ => Some("unknown"),
    }
}

fn unported(script: &Value) -> Option<&'static str> {
    let actions = |v: Option<&Value>| -> Vec<Value> {
        v.and_then(Value::as_array).cloned().unwrap_or_default()
    };
    let steps = actions(script.get("steps"));
    let handlers = actions(script.get("handlers"));
    steps
        .iter()
        .flat_map(|s| actions(s.get("update")))
        .chain(handlers.iter().flat_map(|h| actions(h.get("actions"))))
        .find_map(|a| unported_action(&a))
}

fn err(message: impl Into<String>) -> LexicalError {
    LexicalError::new(message)
}

/// `$node(key)`.
fn node(key: &str) -> Result<NodeRef, LexicalError> {
    get_node_by_key(key).ok_or_else(|| err(format!("no node {key}")))
}

fn text(node: NodeRef) -> Result<NodeRef, LexicalError> {
    if node.is_text() {
        Ok(node)
    } else {
        Err(err(format!("{} is not a text node", node.get_key())))
    }
}

fn element(node: NodeRef) -> Result<NodeRef, LexicalError> {
    if node.is_element() {
        Ok(node)
    } else {
        Err(err(format!("{} is not an element", node.get_key())))
    }
}

/// `$parse(nodes)`.
fn parse(nodes: &Value) -> Result<Vec<NodeRef>, LexicalError> {
    nodes
        .as_array()
        .ok_or_else(|| err("expected an array of nodes"))?
        .iter()
        .map(parse_serialized_node)
        .collect()
}

/// `arg(args, i)`: the argument, or `null`.
fn arg(args: Option<&Value>, i: usize) -> Value {
    args.and_then(Value::as_array)
        .and_then(|a| a.get(i))
        .cloned()
        .unwrap_or(Value::Null)
}

fn number(v: &Value) -> Result<f64, LexicalError> {
    v.as_f64()
        .ok_or_else(|| err(format!("expected a number, got {v}")))
}

fn string(v: &Value) -> Result<String, LexicalError> {
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| err(format!("expected a string, got {v}")))
}

/// An optional offset (`undefined` for `null`).
fn offset(v: &Value) -> Result<Option<usize>, LexicalError> {
    if v.is_null() {
        Ok(None)
    } else {
        Ok(Some(number(v)? as usize))
    }
}

fn flag(v: &Value, default: bool) -> bool {
    v.as_bool().unwrap_or(default)
}

fn point_type(v: &Value) -> Result<PointType, LexicalError> {
    match v.as_str() {
        Some("text") => Ok(PointType::Text),
        Some("element") => Ok(PointType::Element),
        _ => Err(err(format!("bad point type {v}"))),
    }
}

/// `$sel()`: the range selection.
fn sel() -> Result<RangeSelection, LexicalError> {
    get_selection()
        .and_then(|s| s.range())
        .ok_or_else(|| err("not a range selection"))
}

fn direction(v: &Value) -> Result<CaretDirection, LexicalError> {
    match v.as_str() {
        Some("next") => Ok(CaretDirection::Next),
        Some("previous") => Ok(CaretDirection::Previous),
        _ => Err(err(format!("bad direction {v}"))),
    }
}

/// `$caret(spec)`.
fn caret(spec: &Value) -> Result<PointCaret, LexicalError> {
    let key = string(&arg(Some(spec), 0))?;
    let dir = direction(&arg(Some(spec), 1))?;
    let n = node(&key)?;
    match arg(Some(spec), 2).as_str() {
        Some("text") => {
            let n = text(n)?;
            let offset = match arg(Some(spec), 3) {
                v @ Value::String(_) => TextOffset::End(direction(&v)?),
                v => TextOffset::Offset(number(&v)? as usize),
            };
            get_text_point_caret(&n, dir, offset)
        }
        Some("child") => {
            let n = element(n)?;
            Ok(get_child_caret(&n, dir).expect("an element"))
        }
        _ => Ok(get_sibling_caret(&n, dir)),
    }
}

/// `caretSnapshot(caret)`.
fn caret_snapshot(caret: &PointCaret) -> Value {
    let key = caret.origin().get_key().into();
    let dir = caret.direction().as_str().into();
    match caret.offset() {
        Some(offset) => Value::Array(vec!["text".into(), key, dir, (offset as f64).into()]),
        None => Value::Array(vec![caret.caret_type().into(), key, dir]),
    }
}

/// A text slice in the log: `[key, offset, distance]`.
fn slice_snapshot(slice: &Option<TextPointCaretSlice>) -> Value {
    match slice {
        None => Value::Null,
        Some(s) => Value::Array(vec![
            s.caret.origin().get_key().into(),
            (s.caret.offset().unwrap_or(0) as f64).into(),
            (s.distance as f64).into(),
        ]),
    }
}

/// `NODE_METHODS[method](node, args, ctx)`.
fn run_node_method(
    n: NodeRef,
    method: &str,
    args: Option<&Value>,
    ctx: &Ctx,
) -> Result<(), LexicalError> {
    let a = |i| arg(args, i);
    match method {
        "append" => element(n)?.append(&parse(&a(0))?)?,
        "clear" => element(n)?.clear()?,
        "insertAfter" => {
            n.insert_after(&parse_serialized_node(&a(0))?, flag(&a(1), true))?;
        }
        "insertBefore" => {
            n.insert_before(&parse_serialized_node(&a(0))?, flag(&a(1), true))?;
        }
        "markDirty" => n.mark_dirty(),
        "mergeWithSibling" => {
            let target = text(node(&string(&a(0))?)?)?;
            text(n)?.merge_with_sibling(&target)?;
        }
        "remove" => n.remove(flag(&a(0), false))?,
        "replace" => {
            n.replace(&parse_serialized_node(&a(0))?, flag(&a(1), false))?;
        }
        "select" => {
            n.select(offset(&a(0))?, offset(&a(1))?)?;
        }
        "selectEnd" => {
            n.select_end()?;
        }
        "selectNext" => {
            n.select_next(offset(&a(0))?, offset(&a(1))?)?;
        }
        "selectPrevious" => {
            n.select_previous(offset(&a(0))?, offset(&a(1))?)?;
        }
        "selectStart" => {
            n.select_start()?;
        }
        "setDetail" => {
            let n = text(n)?;
            match a(0) {
                Value::String(name) => n.set_detail_type(&name)?,
                v => n.set_detail(number(&v)?)?,
            }
        }
        "setDirection" => element(n)?.set_direction(match a(0).as_str() {
            Some("ltr") => Some(Direction::Ltr),
            Some("rtl") => Some(Direction::Rtl),
            _ => None,
        }),
        "setFormat" if n.is_text() => match a(0) {
            Value::String(name) => n.set_format_type(&name),
            v => n.set_format(number(&v)?),
        },
        "setFormat" => element(n)?.set_element_format(a(0).as_str().unwrap_or("")),
        "setIndent" => element(n)?.set_indent(number(&a(0))?),
        "setMode" => text(n)?.set_mode(&string(&a(0))?)?,
        "setStyle" if n.is_text() => n.set_style(&string(&a(0))?),
        "setStyle" => element(n)?.set_element_style(a(0).as_str().unwrap_or("")),
        "setTextContent" => text(n)?.set_text_content(&string(&a(0))?),
        "setTextFormat" => element(n)?.set_text_format(number(&a(0))?),
        "setTextStyle" => element(n)?.set_text_style(&string(&a(0))?),
        "splice" => element(n)?.splice(
            number(&a(0))? as usize,
            number(&a(1))? as usize,
            &parse(&a(2))?,
        )?,
        "spliceText" => {
            text(n)?.splice_text(
                number(&a(0))? as i64,
                number(&a(1))? as i64,
                &string(&a(2))?,
                flag(&a(3), false),
            )?;
        }
        "splitText" => {
            let offsets: Vec<i64> = args
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|v| number(v).map(|n| n as i64)).collect())
                .transpose()?
                .unwrap_or_default();
            let parts = text(n)?.split_text(&offsets)?;
            ctx.log(vec![
                "splitText".into(),
                Value::Array(parts.iter().map(|p| p.get_key().into()).collect()),
            ]);
        }
        "toggleDirectionless" => text(n)?.toggle_directionless(),
        "toggleFormat" => text(n)?.toggle_format(&string(&a(0))?),
        "toggleUnmergeable" => text(n)?.toggle_unmergeable(),
        _ => return Err(err(format!("unknown node method {method}"))),
    }
    Ok(())
}

/// `$runAction(action, ctx)`.
fn run_action(action: &Value, ctx: &Ctx, editor: &Editor) -> Result<(), LexicalError> {
    match action.get("do").and_then(Value::as_str) {
        Some("append") => {
            let key = action.get("key").and_then(Value::as_str).unwrap_or("");
            element(node(key)?)?.append(&parse(action.get("nodes").unwrap_or(&Value::Null))?)
        }
        Some("select") => {
            let selection = create_range_selection();
            let anchor = action.get("anchor").cloned().unwrap_or(Value::Null);
            let focus = action
                .get("focus")
                .cloned()
                .unwrap_or_else(|| anchor.clone());
            for (which, spec) in [(Which::Anchor, &anchor), (Which::Focus, &focus)] {
                selection.point_set(
                    which,
                    &string(&arg(Some(spec), 0))?,
                    number(&arg(Some(spec), 1))? as usize,
                    point_type(&arg(Some(spec), 2))?,
                )?;
            }
            set_selection(Some(selection));
            Ok(())
        }
        Some("clearSelection") => {
            set_selection(None);
            Ok(())
        }
        Some("node") => {
            let key = action.get("key").and_then(Value::as_str).unwrap_or("");
            let method = action.get("method").and_then(Value::as_str).unwrap_or("");
            run_node_method(node(key)?, method, action.get("args"), ctx)
        }
        Some("logCaretRange") => {
            let range = match (action.get("anchor"), action.get("focus")) {
                (Some(anchor), Some(focus)) => get_caret_range(caret(anchor)?, caret(focus)?)?,
                _ => caret_range_from_selection(&sel()?)?,
            };
            let [anchor_slice, focus_slice] = range.get_text_slices();
            ctx.log(vec![
                "caretRange".into(),
                caret_snapshot(&range.anchor),
                caret_snapshot(&range.focus),
                Value::Array(range.iter().map(|c| caret_snapshot(&c)).collect()),
                Value::Array(vec![
                    slice_snapshot(&anchor_slice),
                    slice_snapshot(&focus_slice),
                ]),
            ]);
            Ok(())
        }
        Some("removeTextFromCaretRange") => {
            let range = remove_text_from_caret_range(
                &caret_range_from_selection(&sel()?)?,
                SliceMode::RemoveEmptySlices,
            )?;
            ctx.log(vec![
                "removeTextFromCaretRange".into(),
                caret_snapshot(&range.anchor),
                caret_snapshot(&range.focus),
            ]);
            set_selection_from_caret_range(&range)?;
            Ok(())
        }
        Some("logNormalizeCaret") => {
            let spec = action.get("caret").unwrap_or(&Value::Null);
            ctx.log(vec![
                "normalizeCaret".into(),
                caret_snapshot(&normalize_caret(&caret(spec)?)),
            ]);
            Ok(())
        }
        Some("dispatch") => {
            let name = action.get("command").and_then(Value::as_str).unwrap_or("");
            let payload = action.get("payload").cloned().unwrap_or(Value::Null);
            let handled = editor.dispatch_command(&ctx.command(name), payload)?;
            ctx.log(vec!["dispatched".into(), name.into(), handled.into()]);
            Ok(())
        }
        other => Err(err(format!("unsupported action {other:?}"))),
    }
}

/// `$selectionSnapshot()`.
fn selection_snapshot() -> Value {
    let Some(selection) = get_selection() else {
        return Value::Null;
    };
    let point = |which| {
        let p = selection.point(which).expect("a range selection");
        Value::Array(vec![
            p.key.as_str().into(),
            (p.offset as f64).into(),
            p.point_type.as_str().into(),
        ])
    };
    match selection.range() {
        Some(range) => Value::Array(vec![
            "range".into(),
            point(Which::Anchor),
            point(Which::Focus),
            range.format.into(),
            range.style.as_str().into(),
        ]),
        None => {
            let keys = selection.node_keys();
            Value::Array(vec![
                "node".into(),
                Value::Array(keys.into_iter().map(Value::from).collect()),
            ])
        }
    }
}

/// `TRANSFORMS[name](ctx)`.
fn register_transform(editor: &Editor, name: &'static str, ctx: &Rc<Ctx>) -> Result<(), String> {
    let ctx = ctx.clone();
    let log = move |node: &NodeRef| {
        ctx.log(vec!["transform".into(), name.into(), node.get_key().into()]);
    };
    // Registered for the whole session; dropping the unregister function
    // doesn't call it.
    let _unregister = match name {
        "paragraph:log" => editor.register_node_transform("paragraph", move |node| {
            log(node);
            Ok(())
        }),
        "paragraph:remove-empty" => editor.register_node_transform("paragraph", move |node| {
            log(node);
            if node.is_empty() && node.get_next_sibling().is_some() {
                node.remove(false)?;
            }
            Ok(())
        }),
        "root:log" => editor.register_node_transform("root", move |node| {
            log(node);
            Ok(())
        }),
        "text:bold-hash" => editor.register_node_transform("text", move |node| {
            log(node);
            if node.is_simple_text()
                && node.get_text_content().starts_with('#')
                && !node.has_format("bold")
            {
                node.toggle_format("bold");
            }
            Ok(())
        }),
        "text:split-hash" => editor.register_node_transform("text", move |node| {
            log(node);
            let content = node.get_text_content();
            // `indexOf`, in UTF-16 code units.
            let index = content.find('#').map(|i| utf16::len(&content[..i]));
            if node.is_simple_text()
                && let Some(index) = index.filter(|&i| i > 0)
            {
                let parts = node.split_text(&[index as i64])?;
                parts[1].toggle_unmergeable();
            }
            Ok(())
        }),
        "text:upper-a" => editor.register_node_transform("text", move |node| {
            log(node);
            let content = node.get_text_content();
            if content.contains('a') {
                node.set_text_content(&content.replace('a', "A"));
            }
            Ok(())
        }),
        _ => return Err(format!("unknown transform {name}")),
    };
    Ok(())
}

/// The transform names `ops.ts` defines, as `'static` strings.
fn transform_name(name: &str) -> Option<&'static str> {
    [
        "paragraph:log",
        "paragraph:remove-empty",
        "root:log",
        "text:bold-hash",
        "text:split-hash",
        "text:upper-a",
    ]
    .into_iter()
    .find(|n| *n == name)
}

/// `startSession(script)`.
fn start_session(script: &Value) -> Result<(Editor, Rc<Ctx>), String> {
    reset_random_key();
    let editor = create_editor(CreateEditorArgs::default());
    let ctx = Rc::new(Ctx::default());
    for name in script
        .get("transforms")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = name.as_str().unwrap_or("");
        let name = transform_name(name).ok_or_else(|| format!("unknown transform {name}"))?;
        register_transform(&editor, name, &ctx)?;
    }
    for handler in script
        .get("handlers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = handler.str("id")?.to_owned();
        let command = ctx.command(handler.str("command")?);
        let priority = handler
            .get("priority")
            .and_then(Value::as_f64)
            .map_or(COMMAND_PRIORITY_EDITOR, |p| p as u8);
        let result = handler
            .get("result")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let actions = handler
            .get("actions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let ctx = ctx.clone();
        let _unregister = editor.register_command(
            &command,
            move |payload: &Value, editor: &Editor| {
                ctx.log(vec!["handler".into(), id.as_str().into(), payload.clone()]);
                for action in &actions {
                    run_action(action, &ctx, editor)?;
                }
                Ok(result)
            },
            priority,
        );
    }
    if script.get("listen").and_then(Value::as_bool) == Some(true) {
        let log = ctx.clone();
        let _unregister = editor.register_update_listener(move |payload| {
            let elements = payload
                .dirty_elements
                .iter()
                .map(|(k, v)| Value::Array(vec![k.as_str().into(), (*v).into()]))
                .collect();
            let leaves = payload
                .dirty_leaves
                .iter()
                .map(|k| k.as_str().into())
                .collect();
            let mut tags = payload.tags.clone();
            tags.sort();
            log.log(vec![
                "update".into(),
                Value::Array(elements),
                Value::Array(leaves),
                Value::Array(tags.into_iter().map(Value::from).collect()),
            ]);
        });
        // Mutation listeners only hear from the DOM reconciler, which a
        // headless editor doesn't run, so JS logs no `mutation` entries.
        let log = ctx.clone();
        let _unregister = editor.register_text_content_listener(move |text| {
            log.log(vec!["textContent".into(), text.into()]);
        });
    }
    Ok((editor, ctx))
}

/// `session.run(step, snapshot)`, compared with what JS recorded.
fn run_step(
    editor: &Editor,
    ctx: &Rc<Ctx>,
    step: &Value,
    expected: &Value,
    i: usize,
) -> Result<(), String> {
    ctx.log.borrow_mut().clear();
    let outcome = if let Some(actions) = step.get("update").and_then(Value::as_array) {
        let tags = step
            .get("tag")
            .and_then(Value::as_array)
            .map(|t| {
                t.iter()
                    .filter_map(|t| t.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let (actions, update_ctx, update_editor) = (actions.clone(), ctx.clone(), editor.clone());
        editor.update(
            move || {
                for action in &actions {
                    run_action(action, &update_ctx, &update_editor)?;
                }
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                tags,
                ..UpdateOptions::default()
            },
        )
    } else {
        let name = step.str("dispatch")?;
        let payload = step.get("payload").cloned().unwrap_or(Value::Null);
        editor
            .dispatch_command(&ctx.command(name), payload)
            .map(|handled| ctx.log(vec!["dispatched".into(), name.into(), handled.into()]))
    };
    // A dispatch outside an update commits at the next read.
    let (selection, text) = editor
        .read(|| (selection_snapshot(), get_root().get_text_content()))
        .map_err(|e| format!("step {i}: committing: {e}"))?;
    let log = Value::Array(ctx.log.borrow().clone());

    let at = |what: &str| format!("step {i} {what}");
    same(
        at("error"),
        outcome.as_ref().err().map(|e| e.to_string()).is_some(),
        expected.get("error").is_some(),
    )
    .map_err(|e| match &outcome {
        Err(error) => format!("{e} ({error})"),
        Ok(()) => e,
    })?;
    same(at("log"), &log, expected.field("log")?)?;
    same(at("selection"), &selection, expected.field("selection")?)?;
    same(at("text"), text.as_str(), expected.str("text")?)?;
    if let Some(state) = expected.get("state").and_then(Value::as_str) {
        let current = editor.get_editor_state();
        same(
            at("state"),
            current.to_json(false).to_string().as_str(),
            state,
        )?;
        same(
            at("compact"),
            current.to_json(true).to_string().as_str(),
            expected.str("compact")?,
        )?;
    }
    Ok(())
}

pub fn check(case: &Value, _file: &Value) -> Result<Outcome, String> {
    if let Some(part) = unported(case) {
        return Ok(Outcome::NotPorted(part));
    }
    let (editor, ctx) = start_session(case)?;
    let steps = case.arr("steps")?;
    let results = case.arr("results")?;
    same("steps recorded", results.len(), steps.len())?;
    for (i, (step, expected)) in steps.iter().zip(results).enumerate() {
        run_step(&editor, &ctx, step, expected, i)?;
    }
    Ok(Outcome::Pass)
}
