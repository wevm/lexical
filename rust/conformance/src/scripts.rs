//! `scripts/*.json`: editor sessions in the script language of
//! `packages/lexical-conformance/src/ops.ts`, interpreted here as `ops.ts`
//! interprets them for JS. Each step is one discrete update or one command
//! dispatch; after each, the editor state, the selection, the text and a
//! log of what listeners, transforms and command handlers saw must match
//! what JS recorded.
//!
//! A script with `history: {delay}` registers `@lexical/history` on the
//! session's clock, which starts at 0 and which a step's `now` sets; JS
//! makes `Date.now()` return it while a step runs. `dispatch` and handlers
//! name Lexical's own commands (`UNDO_COMMAND`, `CAN_UNDO_COMMAND`...) as JS
//! exports them, and any other name is a command the script makes.
//!
//! A script with `richText: true` registers `@lexical/rich-text`'s nodes
//! (`HeadingNode`, `QuoteNode`) and `registerRichText`'s handlers, one
//! with `plainText: true` registers `registerPlainText`'s, and one with
//! `list: true` registers `@lexical/list`'s nodes (`ListNode`,
//! `ListItemNode`), `registerList` and `registerCheckList`. `dispatch` and
//! handlers name the list commands as `@lexical/list` exports them
//! (`PACKAGE_COMMANDS`). One with `link: true` registers `@lexical/link`'s
//! nodes (`LinkNode`, `AutoLinkNode`) and `registerLink` with its
//! defaults, and `TOGGLE_LINK_COMMAND` takes `null`, a URL or
//! `{url, target, rel, title}`.
//!
//! A script that uses an action the port can't run yet (selection editing)
//! is counted as waiting on that part of `lexical`, never as passing.

use crate::case::{Case, Outcome, same};
use lexical::history::{create_empty_history_state, register_history};
use lexical::json::{Object, Value};
use lexical::link;
use lexical::list;
use lexical::rich_text::{HeadingTagType, create_heading_node, create_quote_node};
use lexical::selection::set_blocks_type;
use lexical::{
    CAN_REDO_COMMAND, CAN_UNDO_COMMAND, CLEAR_EDITOR_COMMAND, CLEAR_HISTORY_COMMAND,
    COMMAND_PRIORITY_EDITOR, CONTROLLED_TEXT_INSERTION_COMMAND, CaretDirection, CreateEditorArgs,
    DELETE_CHARACTER_COMMAND, DELETE_LINE_COMMAND, DELETE_WORD_COMMAND, Direction, Editor,
    FORMAT_ELEMENT_COMMAND, FORMAT_TEXT_COMMAND, INDENT_CONTENT_COMMAND, INSERT_LINE_BREAK_COMMAND,
    INSERT_PARAGRAPH_COMMAND, INSERT_TAB_COMMAND, LexicalCommand, LexicalError, NodeRef,
    OUTDENT_CONTENT_COMMAND, PointCaret, PointType, REDO_COMMAND, REMOVE_TEXT_COMMAND,
    RangeSelection, SELECT_ALL_COMMAND, SET_TEXT_FORMAT_COMMAND, SelectionRef, SliceMode,
    TextOffset, TextPointCaretSlice, UNDO_COMMAND, Unregister, UpdateOptions, Which,
    caret_range_from_selection, create_command, create_editor, create_node_selection,
    create_paragraph_node, create_range_selection, get_caret_range, get_child_caret,
    get_node_by_key, get_root, get_selection, get_sibling_caret, get_text_point_caret,
    insert_nodes, normalize_caret, parse_serialized_node, remove_text_from_caret_range,
    reset_random_key, select_all, set_selection, set_selection_from_caret_range, utf16,
};
use std::cell::{Cell, RefCell};
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

    /// `command(ctx, name)`: one of Lexical's own commands
    /// (`CORE_COMMANDS`), a list or link command (`PACKAGE_COMMANDS`), or one
    /// command per other name.
    fn command(&self, name: &str) -> ScriptCommand {
        match name {
            "CAN_REDO_COMMAND" => ScriptCommand::Bool(CAN_REDO_COMMAND),
            "CAN_UNDO_COMMAND" => ScriptCommand::Bool(CAN_UNDO_COMMAND),
            "CLEAR_EDITOR_COMMAND" => ScriptCommand::Void(CLEAR_EDITOR_COMMAND),
            "CLEAR_HISTORY_COMMAND" => ScriptCommand::Void(CLEAR_HISTORY_COMMAND),
            "CONTROLLED_TEXT_INSERTION_COMMAND" => {
                ScriptCommand::Str(CONTROLLED_TEXT_INSERTION_COMMAND)
            }
            "DELETE_CHARACTER_COMMAND" => ScriptCommand::Bool(DELETE_CHARACTER_COMMAND),
            "DELETE_LINE_COMMAND" => ScriptCommand::Bool(DELETE_LINE_COMMAND),
            "DELETE_WORD_COMMAND" => ScriptCommand::Bool(DELETE_WORD_COMMAND),
            "FORMAT_ELEMENT_COMMAND" => ScriptCommand::Str(FORMAT_ELEMENT_COMMAND),
            "FORMAT_TEXT_COMMAND" => ScriptCommand::Str(FORMAT_TEXT_COMMAND),
            "INDENT_CONTENT_COMMAND" => ScriptCommand::Void(INDENT_CONTENT_COMMAND),
            "INSERT_LINE_BREAK_COMMAND" => ScriptCommand::Bool(INSERT_LINE_BREAK_COMMAND),
            "INSERT_PARAGRAPH_COMMAND" => ScriptCommand::Void(INSERT_PARAGRAPH_COMMAND),
            "INSERT_TAB_COMMAND" => ScriptCommand::Void(INSERT_TAB_COMMAND),
            "OUTDENT_CONTENT_COMMAND" => ScriptCommand::Void(OUTDENT_CONTENT_COMMAND),
            "REDO_COMMAND" => ScriptCommand::Void(REDO_COMMAND),
            "REMOVE_TEXT_COMMAND" => ScriptCommand::Void(REMOVE_TEXT_COMMAND),
            "SELECT_ALL_COMMAND" => ScriptCommand::Void(SELECT_ALL_COMMAND),
            "SET_TEXT_FORMAT_COMMAND" => ScriptCommand::Formats(SET_TEXT_FORMAT_COMMAND),
            "UNDO_COMMAND" => ScriptCommand::Void(UNDO_COMMAND),
            "INSERT_CHECK_LIST_COMMAND" => ScriptCommand::Void(list::INSERT_CHECK_LIST_COMMAND),
            "INSERT_ORDERED_LIST_COMMAND" => ScriptCommand::Void(list::INSERT_ORDERED_LIST_COMMAND),
            "INSERT_UNORDERED_LIST_COMMAND" => {
                ScriptCommand::Void(list::INSERT_UNORDERED_LIST_COMMAND)
            }
            "REMOVE_LIST_COMMAND" => ScriptCommand::Void(list::REMOVE_LIST_COMMAND),
            "UPDATE_LIST_START_COMMAND" => {
                ScriptCommand::ListStart(list::UPDATE_LIST_START_COMMAND)
            }
            "TOGGLE_LINK_COMMAND" => ScriptCommand::Link(link::TOGGLE_LINK_COMMAND),
            _ => ScriptCommand::Json(
                self.commands
                    .borrow_mut()
                    .entry(name.to_owned())
                    .or_insert_with(|| create_command(name))
                    .clone(),
            ),
        }
    }
}

/// A command a script names, by its payload type: Lexical's own take
/// nothing (`void`), a boolean, a string or a map of formats to booleans,
/// and those the script makes take any JSON.
enum ScriptCommand {
    Void(LexicalCommand<()>),
    Bool(LexicalCommand<bool>),
    Str(LexicalCommand<String>),
    Formats(LexicalCommand<Vec<(String, bool)>>),
    /// `UPDATE_LIST_START_COMMAND`: `{listNodeKey, newStart}`.
    ListStart(LexicalCommand<list::UpdateListStartPayload>),
    /// `TOGGLE_LINK_COMMAND`: `null`, a URL, or `{url, target, rel, title}`.
    Link(LexicalCommand<Option<link::ToggleLinkPayload>>),
    Json(LexicalCommand<Value>),
}

/// `TOGGLE_LINK_COMMAND`'s payload: `null`, a URL, or an object with a URL
/// and the attributes it has (`null` or a string each).
fn toggle_link_of(payload: &Value) -> Option<Option<link::ToggleLinkPayload>> {
    if payload.is_null() {
        return Some(None);
    }
    if let Some(url) = payload.as_str() {
        return Some(Some(link::ToggleLinkPayload::Url(url.to_owned())));
    }
    let object = payload.as_object()?;
    let attribute = |key: &str| -> Option<Option<Option<String>>> {
        Some(match object.get(key) {
            None => None,
            Some(Value::Null) => Some(None),
            Some(value) => Some(Some(value.as_str()?.to_owned())),
        })
    };
    Some(Some(link::ToggleLinkPayload::Attributes {
        url: object.get("url")?.as_str()?.to_owned(),
        attributes: link::LinkAttributes {
            rel: attribute("rel")?,
            target: attribute("target")?,
            title: attribute("title")?,
        },
    }))
}

/// `TOGGLE_LINK_COMMAND`'s payload as JSON: `url`, then the attributes it
/// has.
fn toggle_link_json(payload: &Option<link::ToggleLinkPayload>) -> Value {
    match payload {
        None => Value::Null,
        Some(link::ToggleLinkPayload::Url(url)) => url.as_str().into(),
        Some(link::ToggleLinkPayload::Attributes { url, attributes }) => {
            let mut json = Object::new();
            json.insert("url", url.as_str().into());
            for (key, value) in [
                ("target", &attributes.target),
                ("rel", &attributes.rel),
                ("title", &attributes.title),
            ] {
                if let Some(value) = value {
                    json.insert(key, value.as_deref().map_or(Value::Null, Value::from));
                }
            }
            json.into()
        }
    }
}

/// `UPDATE_LIST_START_COMMAND`'s payload, `{listNodeKey, newStart}`.
fn list_start_of(payload: &Value) -> Option<list::UpdateListStartPayload> {
    Some(list::UpdateListStartPayload {
        list_node_key: payload.get("listNodeKey")?.as_str()?.to_owned(),
        new_start: payload.get("newStart")?.as_f64()?,
    })
}

/// `SET_TEXT_FORMAT_COMMAND`'s payload, an object of booleans, in key order.
fn formats_of(payload: &Value) -> Option<Vec<(String, bool)>> {
    payload
        .as_object()?
        .iter()
        .map(|(name, value)| Some((name.to_owned(), value.as_bool()?)))
        .collect()
}

impl ScriptCommand {
    /// `editor.dispatchCommand(command, payload)`. A payload the command
    /// doesn't take is an error, which JS would pass on as it is.
    fn dispatch(&self, editor: &Editor, payload: Value) -> Result<bool, LexicalError> {
        match self {
            ScriptCommand::Void(command) if payload.is_null() => {
                editor.dispatch_command(command, ())
            }
            ScriptCommand::Bool(command) if payload.as_bool().is_some() => {
                editor.dispatch_command(command, payload.as_bool() == Some(true))
            }
            ScriptCommand::Str(command) if payload.as_str().is_some() => {
                editor.dispatch_command(command, payload.as_str().unwrap_or("").to_owned())
            }
            ScriptCommand::Formats(command) if formats_of(&payload).is_some() => {
                editor.dispatch_command(command, formats_of(&payload).unwrap_or_default())
            }
            ScriptCommand::ListStart(command) => match list_start_of(&payload) {
                Some(payload) => editor.dispatch_command(command, payload),
                None => Err(err(format!(
                    "{payload} is not a payload this command takes"
                ))),
            },
            ScriptCommand::Link(command) => match toggle_link_of(&payload) {
                Some(payload) => editor.dispatch_command(command, payload),
                None => Err(err(format!(
                    "{payload} is not a payload this command takes"
                ))),
            },
            ScriptCommand::Json(command) => editor.dispatch_command(command, payload),
            _ => Err(err(format!(
                "{payload} is not a payload this command takes"
            ))),
        }
    }

    /// `editor.registerCommand(command, listener, priority)`, the listener
    /// getting the payload as JSON (`null` for `void`).
    fn register(
        &self,
        editor: &Editor,
        listener: impl Fn(&Value, &Editor) -> Result<bool, LexicalError> + 'static,
        priority: u8,
    ) -> Unregister {
        match self {
            ScriptCommand::Void(command) => editor.register_command(
                command,
                move |_, editor| listener(&Value::Null, editor),
                priority,
            ),
            ScriptCommand::Bool(command) => editor.register_command(
                command,
                move |payload, editor| listener(&(*payload).into(), editor),
                priority,
            ),
            ScriptCommand::Str(command) => editor.register_command(
                command,
                move |payload, editor| listener(&payload.as_str().into(), editor),
                priority,
            ),
            ScriptCommand::Formats(command) => editor.register_command(
                command,
                move |payload, editor| {
                    let mut formats = Object::new();
                    for (name, value) in payload {
                        formats.insert(name.as_str(), (*value).into());
                    }
                    listener(&formats.into(), editor)
                },
                priority,
            ),
            ScriptCommand::ListStart(command) => editor.register_command(
                command,
                move |payload, editor| {
                    let mut json = Object::new();
                    json.insert("listNodeKey", payload.list_node_key.as_str().into());
                    json.insert("newStart", payload.new_start.into());
                    listener(&json.into(), editor)
                },
                priority,
            ),
            ScriptCommand::Link(command) => editor.register_command(
                command,
                move |payload, editor| listener(&toggle_link_json(payload), editor),
                priority,
            ),
            ScriptCommand::Json(command) => editor.register_command(command, listener, priority),
        }
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
            | "logNormalizeCaret"
            | "setBlocksType",
        ) => None,
        Some("selection" | "selectAll" | "selectNodes" | "insertNodes") => None,
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

fn list_item(node: NodeRef) -> Result<NodeRef, LexicalError> {
    if list::is_list_item_node(&node) {
        Ok(node)
    } else {
        Err(err(format!("{} is not a list item", node.get_key())))
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
/// `SELECTION_METHODS[method](args)` in ops.ts.
fn run_selection_method(method: &str, args: Option<&Value>) -> Result<(), LexicalError> {
    let a = |i| arg(args, i);
    let range = || {
        get_selection()
            .filter(SelectionRef::is_range)
            .ok_or_else(|| err("not a range selection"))
    };
    let any = || get_selection().ok_or_else(|| err("no selection"));
    match method {
        "deleteCharacter" => range()?.delete_character(flag(&a(0), false))?,
        "deleteLine" => range()?.delete_line(flag(&a(0), false))?,
        "deleteWord" => range()?.delete_word(flag(&a(0), false))?,
        "formatText" => range()?.format_text(
            &string(&a(0))?,
            match a(1) {
                Value::Null => None,
                v => Some(number(&v)?),
            },
        )?,
        "insertLineBreak" => range()?.insert_line_break(flag(&a(0), false))?,
        "insertNodes" => any()?.insert_nodes(parse(&a(0))?)?,
        "insertParagraph" => {
            range()?.insert_paragraph()?;
        }
        "insertRawText" => any()?.insert_raw_text(&string(&a(0))?)?,
        "insertText" => any()?.insert_text(&string(&a(0))?)?,
        "removeText" => range()?.remove_text()?,
        "setFormat" => range()?.set_format(number(&a(0))?),
        "setStyle" => range()?.set_style(&string(&a(0))?),
        "toggleFormat" => range()?.toggle_format(&string(&a(0))?),
        _ => return Err(err(format!("unknown selection method {method}"))),
    }
    Ok(())
}

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
        "setIndent" => element(n)?.set_indent(number(&a(0))?)?,
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
        "toggleChecked" => list_item(n)?.toggle_checked(),
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
        Some("selectNodes") => {
            let selection = create_node_selection();
            for key in action
                .get("keys")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                selection.add(&string(key)?);
            }
            set_selection(Some(selection));
            Ok(())
        }
        Some("selectAll") => select_all().map(|_| ()),
        Some("insertNodes") => insert_nodes(parse(action.get("nodes").unwrap_or(&Value::Null))?),
        Some("selection") => run_selection_method(
            action.get("method").and_then(Value::as_str).unwrap_or(""),
            action.get("args"),
        ),
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
        Some("setBlocksType") => {
            let block = action.get("block").and_then(Value::as_str).unwrap_or("");
            let create: Box<dyn Fn() -> NodeRef> = match block {
                "paragraph" => Box::new(create_paragraph_node),
                "quote" => Box::new(|| create_quote_node(false)),
                tag => {
                    let tag = HeadingTagType::parse(tag)
                        .ok_or_else(|| err(format!("unknown block {tag}")))?;
                    Box::new(move || create_heading_node(tag))
                }
            };
            set_blocks_type(get_selection().as_ref(), create, None)
        }
        Some("dispatch") => {
            let name = action.get("command").and_then(Value::as_str).unwrap_or("");
            let payload = action.get("payload").cloned().unwrap_or(Value::Null);
            let handled = ctx.command(name).dispatch(editor, payload)?;
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

/// A script's editor, set up with its transforms, command handlers and
/// listeners, that runs steps one at a time (`ops.ts`'s `Session`). The
/// conformance suite replays the vectors with it; the playground drives it
/// step by step alongside JS Lexical.
pub struct Session {
    editor: Editor,
    ctx: Rc<Ctx>,
    /// The session's clock, in milliseconds: what the history merges by.
    /// It starts at 0, and a step's `now` sets it.
    clock: Rc<Cell<f64>>,
    /// Unregisters the history, whose entries hold the editor.
    unregister_history: Option<Unregister>,
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(unregister) = self.unregister_history.take() {
            unregister();
        }
    }
}

/// One step's result.
pub struct StepResult {
    /// What `ops.ts`'s `session.run(step, snapshot)` returns, in its key
    /// order, so `json.to_string()` is JS's `JSON.stringify` of it.
    pub json: Value,
    /// Why the step failed, when `json` has `error: true`.
    pub error: Option<LexicalError>,
}

impl Session {
    /// `startSession(script)`: a new headless editor, with keys restarted.
    pub fn start(script: &Value) -> Result<Session, String> {
        reset_random_key();
        let rich_text = script.get("richText").and_then(Value::as_bool) == Some(true);
        let with_list = script.get("list").and_then(Value::as_bool) == Some(true);
        let with_link = script.get("link").and_then(Value::as_bool) == Some(true);
        let mut nodes = Vec::new();
        if rich_text {
            nodes.extend(lexical::rich_text::rich_text_nodes());
        }
        if with_list {
            nodes.extend(list::list_nodes());
        }
        if with_link {
            nodes.extend(link::link_nodes());
        }
        let editor = create_editor(CreateEditorArgs { nodes });
        let ctx = Rc::new(Ctx::default());
        if rich_text {
            let _unregister = lexical::rich_text::register_rich_text(&editor);
        }
        if script.get("plainText").and_then(Value::as_bool) == Some(true) {
            let _unregister = lexical::plain_text::register_plain_text(&editor);
        }
        if with_list {
            let _unregister = list::register_list(&editor, list::RegisterListOptions::default());
            let _unregister = list::register_check_list(&editor);
        }
        if with_link {
            let _unregister = link::register_link(&editor, link::LinkConfig::default());
        }
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
            let _unregister = command.register(
                &editor,
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
        // `registerHistory(editor, createEmptyHistoryState(), delay)` on the
        // session's clock.
        let clock = Rc::new(Cell::new(0.0));
        let unregister_history = match script.get("history") {
            None => None,
            Some(history) => {
                let delay = history
                    .get("delay")
                    .and_then(Value::as_f64)
                    .ok_or("history needs a delay")?;
                let clock = clock.clone();
                Some(register_history(
                    &editor,
                    &create_empty_history_state(),
                    delay,
                    Some(Rc::new(move || clock.get())),
                    None,
                    None,
                ))
            }
        };
        Ok(Session {
            editor,
            ctx,
            clock,
            unregister_history,
        })
    }

    /// The session's editor.
    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    /// `session.run(step, snapshot)`: runs one step and records the log,
    /// the selection and the text, and with `snapshot` the state in both
    /// forms.
    pub fn run(&self, step: &Value, snapshot: bool) -> Result<StepResult, String> {
        let (editor, ctx) = (&self.editor, &self.ctx);
        if let Some(now) = step.get("now").and_then(Value::as_f64) {
            self.clock.set(now);
        }
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
            let (actions, update_ctx, update_editor) =
                (actions.clone(), ctx.clone(), editor.clone());
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
            ctx.command(name)
                .dispatch(editor, payload)
                .map(|handled| ctx.log(vec!["dispatched".into(), name.into(), handled.into()]))
        };
        // A dispatch outside an update commits at the next read.
        let (selection, text) = editor
            .read(|| (selection_snapshot(), get_root().get_text_content()))
            .map_err(|e| format!("committing: {e}"))?;
        let mut json = Object::new();
        json.insert("log", Value::Array(ctx.log.borrow().clone()));
        json.insert("selection", selection);
        json.insert("text", text.into());
        if snapshot {
            let state = editor.get_editor_state();
            json.insert("state", state.to_json(false).to_string().into());
            json.insert("compact", state.to_json(true).to_string().into());
        }
        let error = outcome.err();
        if error.is_some() {
            json.insert("error", true.into());
        }
        Ok(StepResult {
            json: Value::Object(json),
            error,
        })
    }
}

/// `scripts/*.json`: each step's result matches what JS recorded.
pub fn check(case: &Value, _file: &Value) -> Result<Outcome, String> {
    if let Some(part) = unported(case) {
        return Ok(Outcome::NotPorted(part));
    }
    let session = Session::start(case)?;
    let steps = case.arr("steps")?;
    let results = case.arr("results")?;
    same("steps recorded", results.len(), steps.len())?;
    for (i, (step, expected)) in steps.iter().zip(results).enumerate() {
        let at = |what: &str| format!("step {i} {what}");
        let snapshot = expected.get("state").is_some();
        let StepResult { json, error } = session.run(step, snapshot).map_err(|e| at(&e))?;
        same(
            at("error"),
            error.is_some(),
            expected.get("error").is_some(),
        )
        .map_err(|e| match &error {
            Some(error) => format!("{e} ({error})"),
            None => e,
        })?;
        for field in ["log", "selection", "text", "state", "compact"] {
            same(at(field), json.get(field), expected.get(field))?;
        }
    }
    Ok(Outcome::Pass)
}
