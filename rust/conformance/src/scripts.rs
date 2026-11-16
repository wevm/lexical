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
//! `{url, target, rel, title}`. With `autoLink: true` as well, it registers
//! `registerAutoLink` with `autoLinkUrlMatcher` and `autoLinkEmailMatcher`,
//! and a change handler that logs `['autoLink', url, prevUrl]`. One with
//! `code: true` registers `@lexical/code-core`'s nodes (`CodeNode`,
//! `CodeHighlightNode`), `CodeExtension`'s transform and
//! `registerCodeIndentation`, one with `overflow: true`
//! `@lexical/overflow`'s `OverflowNode`, and one with `hashtag: true`
//! `@lexical/hashtag`'s `HashtagNode` and `registerLexicalHashtag`, one
//! with `extension: true` `@lexical/extension`'s decorators
//! (`HorizontalRuleNode`, `DecoratorTextNode`), and one with `mark: true`
//! `@lexical/mark`'s `MarkNode`; the MarkState actions (`addMarkID`...) run
//! without `MarkStateExtension` unless the script is built with it, and take
//! a mark flavor of `MARK_FLAVORS` (`flavor`), as in `ops.ts`. A script with `code: true`
//! and `codeHighlight` registers `registerCodeHighlighting` in place of
//! `registerCodeIndentation`, over `ops.ts`'s conformance tokenizer
//! (`code_highlight`) or tokens it recorded from Prism, and the step
//! `unregisterCodeHighlighting` unregisters it. One with
//! `markdown: true` registers `registerMarkdownShortcuts` with
//! `TRANSFORMERS`, after the hashtag; `KEY_ENTER_COMMAND` takes `null`.
//!
//! `setState`, `updateState`, `logState` and `logStateChange` name the
//! NodeState configs of `ops.ts`'s `STATES` and its `UPDATERS`, made here
//! with the same parse, unparse and equality (`States`).
//!
//! A script that uses an action the port can't run yet (selection editing)
//! is counted as waiting on that part of `lexical`, never as passing.

use crate::case::{Case, Outcome, same};
use lexical::a11y::{LiveRegion, LiveRegionHost, aria_live_region_extension};
use lexical::auto_embed::{INSERT_EMBED_COMMAND, is_single_token, url_matcher};
use lexical::clipboard;
use lexical::decorator_block::{
    DecoratorBlockData, export_decorator_block, is_decorator_block_node,
    update_decorator_block_from_json,
};
use lexical::extension as ext;
use lexical::extension::get_extension_dependency_from_editor;
use lexical::history::{create_empty_history_state, register_history};
use lexical::json::{Object, Value};
use lexical::link;
use lexical::list;
use lexical::mark;
use lexical::menu::{
    MenuTextMatch, TriggerFn, TypeaheadTriggerOptions, basic_typeahead_trigger_match,
    get_full_match_offset, get_text_up_to_anchor, is_selection_on_entity_boundary,
    split_node_containing_query,
};
use lexical::rich_text::{HeadingTagType, create_heading_node, create_quote_node, is_heading_node};
use lexical::selection::{self, set_blocks_type};
use lexical::utils;
use lexical::{
    BEFORE_INPUT_COMMAND, CLICK_COMMAND, COMPOSITION_END_COMMAND, COMPOSITION_START_COMMAND,
    COPY_COMMAND, CUT_COMMAND, ClipboardEvent, ClipboardOrKeyboardEvent, CompositionEvent,
    DRAGEND_COMMAND, DRAGOVER_COMMAND, DRAGSTART_COMMAND, DROP_COMMAND, DataTransferFile,
    DragEvent, Event, InputEvent, KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND,
    KEY_ARROW_RIGHT_COMMAND, KEY_ARROW_UP_COMMAND, KEY_BACKSPACE_COMMAND, KEY_DELETE_COMMAND,
    KEY_DOWN_COMMAND, KEY_ESCAPE_COMMAND, KEY_MODIFIER_COMMAND, KEY_SPACE_COMMAND, KEY_TAB_COMMAND,
    MOVE_TO_END, MOVE_TO_START, MouseEvent, PASTE_COMMAND, PasteCommandType, with_is_apple,
};
use lexical::{
    CAN_REDO_COMMAND, CAN_UNDO_COMMAND, CLEAR_EDITOR_COMMAND, CLEAR_HISTORY_COMMAND,
    COMMAND_PRIORITY_EDITOR, CONTROLLED_TEXT_INSERTION_COMMAND, CaretDirection, CreateEditorArgs,
    DELETE_CHARACTER_COMMAND, DELETE_LINE_COMMAND, DELETE_WORD_COMMAND, DataTransfer, Direction,
    Editor, EditorState, FORMAT_ELEMENT_COMMAND, FORMAT_TEXT_COMMAND, INDENT_CONTENT_COMMAND,
    INSERT_LINE_BREAK_COMMAND, INSERT_PARAGRAPH_COMMAND, INSERT_TAB_COMMAND, KEY_ENTER_COMMAND,
    KeyboardEvent, LexicalCommand, LexicalError, NodeRef, OUTDENT_CONTENT_COMMAND, PointCaret,
    PointType, REDO_COMMAND, REMOVE_TEXT_COMMAND, RangeSelection, SELECT_ALL_COMMAND,
    SET_TEXT_FORMAT_COMMAND, SelectionAlter, SelectionGranularity, SelectionRef, SliceMode,
    StateConfig, StateValueConfig, TextOffset, TextPointCaretSlice, UNDO_COMMAND, Unregister,
    UpdateOptions, Which, caret_range_from_selection, clone_with_properties, create_command,
    create_editor, create_node_selection, create_paragraph_node, create_range_selection,
    create_state, create_state_with, export_node_json, get_caret_range, get_child_caret,
    get_node_by_key, get_root, get_selection, get_sibling_caret, get_state, get_state_change,
    get_style_object_from_css, get_text_point_caret, insert_nodes, is_block_fully_selected,
    normalize_caret, parse_serialized_node, remove_text_from_caret_range, reset_random_key,
    select_all, set_selection, set_selection_from_caret_range, set_state, set_state_with,
    split_node, utf16,
};
use lexical::{LexicalNode, MutationListenerOptions, NodeBase, RootElement, RootElementRef};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

mod code_highlight;
mod extensions;
mod schema_node;
mod structure;

/// What `ops.ts`'s `Context` holds besides the editor.
#[derive(Default)]
struct Ctx {
    log: RefCell<Vec<Value>>,
    commands: RefCell<HashMap<String, LexicalCommand<Value>>>,
    states: States,
    /// The test extensions made so far, by key.
    extensions: RefCell<HashMap<String, Rc<extensions::TestExtension>>>,
    /// `script.extensionDefs`.
    extension_defs: RefCell<Object>,
    /// The `DataTransfer` of the last `copy`.
    clipboard: RefCell<Option<Rc<DataTransfer>>>,
    /// The session's clock, in milliseconds: what `Date.now()` returns
    /// while JS runs a step, so what the history merges by. It starts at 0,
    /// and a step's `now` sets it.
    clock: Rc<Cell<f64>>,
    /// The editor states `saveEditorState` kept, by name.
    saved: RefCell<HashMap<String, EditorState>>,
    /// The block handles `logBlockHandles` kept, by name.
    handles: RefCell<HashMap<String, Vec<(String, String)>>>,
    /// The session's root element and its document's body (`ctx.dom`),
    /// made on first use (`setRootElement`).
    dom: RefCell<Option<Rc<SessionRoot>>>,
    /// What `characterLimit` and `tableOfContents` mounted, to unmount.
    mounted: RefCell<HashMap<String, Unregister>>,
    /// The disposers `registerNodeStructure` returned, by name.
    structures: RefCell<HashMap<String, Unregister>>,
    /// The script's `parent` editor.
    parent: RefCell<Option<lexical::extension::LexicalEditorWithDispose>>,
    /// The effects `watchNodeKey` made, by key.
    watchers: RefCell<HashMap<String, Unregister>>,
    /// The disposers `registerMarkIDsListener` returned, by name.
    mark_ids_listeners: RefCell<HashMap<String, Unregister>>,
}

/// The session's root element (`sessionDOM(ctx).root`): a `div` whose
/// document's body hosts `AriaLiveRegionExtension`'s live region.
pub(crate) struct SessionRoot {
    body: Rc<SessionBody>,
    /// The computed styles the `computedStyles` step stubs, by node key.
    computed_styles: RefCell<HashMap<String, selection::ComputedStyle>>,
}

impl RootElement for SessionRoot {
    fn live_region_host(self: Rc<Self>) -> Option<Rc<dyn LiveRegionHost>> {
        Some(self.body.clone())
    }

    fn computed_style(&self, key: &str) -> Option<selection::ComputedStyle> {
        self.computed_styles.borrow().get(key).cloned()
    }
}

/// The body of the session's document: the live regions mounted in it, in
/// document order.
#[derive(Default)]
struct SessionBody {
    regions: RefCell<Vec<Rc<LiveRegion>>>,
}

impl LiveRegionHost for SessionBody {
    fn mount_live_region(&self, region: &Rc<LiveRegion>) {
        self.regions.borrow_mut().push(region.clone());
    }
    fn unmount_live_region(&self, region: &Rc<LiveRegion>) {
        self.regions.borrow_mut().retain(|r| !Rc::ptr_eq(r, region));
    }
}

impl Ctx {
    /// `sessionDOM(ctx)`.
    fn session_dom(&self) -> Rc<SessionRoot> {
        self.dom
            .borrow_mut()
            .get_or_insert_with(|| {
                Rc::new(SessionRoot {
                    body: Rc::new(SessionBody::default()),
                    computed_styles: RefCell::default(),
                })
            })
            .clone()
    }

    /// `liveRegionEntry(ctx)`: `['liveRegion', [[aria-live, textContent]...]]`.
    fn live_region_entry(&self) -> Value {
        let regions = match &*self.dom.borrow() {
            None => Vec::new(),
            Some(root) => root
                .body
                .regions
                .borrow()
                .iter()
                .map(|region| {
                    Value::Array(vec![
                        region
                            .politeness()
                            .map_or(Value::Null, |p| p.as_str().into()),
                        region.text_content().into(),
                    ])
                })
                .collect(),
        };
        Value::Array(vec!["liveRegion".into(), Value::Array(regions)])
    }
}

/// `TestDecoratorBlockNode` (ops.ts): a `DecoratorBlockNode` that adds
/// nothing, as an app's block embed would extend it.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestDecoratorBlockNode {
    base: NodeBase,
    block: DecoratorBlockData,
}

impl LexicalNode for TestDecoratorBlockNode {
    fn get_type(&self) -> &'static str {
        "test-decorator-block"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn decorator_block(&self) -> Option<&DecoratorBlockData> {
        Some(&self.block)
    }
    fn decorator_block_mut(&mut self) -> Option<&mut DecoratorBlockData> {
        Some(&mut self.block)
    }
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(export_decorator_block(self, compact))
    }
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_decorator_block_from_json(&mut self.block, json);
        true
    }
}

pub(crate) use extensions::mark_flavor;
pub(crate) use schema_node::test_schema_nodes;

/// `TestDecoratorBlockNode`'s class.
pub(crate) fn test_decorator_block_nodes() -> Vec<lexical::NodeClass> {
    vec![lexical::NodeClass::of::<TestDecoratorBlockNode>()]
}

/// `TypeaheadSpec`: `useBasicTypeaheadTriggerMatch(trigger, options)`'s
/// arguments, the options left out taking its defaults.
fn typeahead_trigger(spec: &Value) -> Result<TriggerFn, LexicalError> {
    let trigger = spec
        .get("trigger")
        .and_then(Value::as_str)
        .ok_or_else(|| err("no trigger"))?;
    let mut options = TypeaheadTriggerOptions::default();
    if let Some(n) = spec.get("minLength").and_then(Value::as_f64) {
        options.min_length = n as usize;
    }
    if let Some(n) = spec.get("maxLength").and_then(Value::as_f64) {
        options.max_length = n as usize;
    }
    if let Some(p) = spec.get("punctuation").and_then(Value::as_str) {
        options.punctuation = p.to_owned();
    }
    if let Some(w) = spec.get("allowWhitespace").and_then(Value::as_bool) {
        options.allow_whitespace = w;
    }
    Ok(basic_typeahead_trigger_match(trigger, options))
}

/// `$typeaheadMatch(spec, editor)`: the text before the caret and its match
/// (`None`s past a `None`).
fn typeahead_match(
    spec: &Value,
    editor: &Editor,
) -> Result<(Option<String>, Option<MenuTextMatch>), LexicalError> {
    let text = get_selection()
        .and_then(|s| s.range())
        .and_then(|range| get_text_up_to_anchor(&range));
    let text_match = match &text {
        None => None,
        Some(text) => typeahead_trigger(spec)?(text, editor),
    };
    Ok((text, text_match))
}

fn text_match_json(text_match: &MenuTextMatch) -> Value {
    let mut json = Object::new();
    json.insert("leadOffset", (text_match.lead_offset as f64).into());
    json.insert("matchingString", text_match.matching_string.as_str().into());
    json.insert(
        "replaceableString",
        text_match.replaceable_string.as_str().into(),
    );
    json.into()
}

/// `STRLEN[name]` of `ops.ts`: UTF-16 code units, or with `"utf8"` UTF-8
/// bytes (what `TextEncoder` encodes).
fn strlen_of(name: Option<&str>) -> Result<lexical::overflow::Strlen, LexicalError> {
    match name {
        None | Some("utf16") => Ok(Rc::new(|s: &str| utf16::len(s) as i64)),
        Some("utf8") => Ok(Rc::new(|s: &str| s.len() as i64)),
        Some(other) => Err(err(format!("unknown strlen {other}"))),
    }
}

/// `mountStep(step, ctx)` of `ops.ts`: `useCharacterLimit` or
/// `TableOfContentsPlugin`, as their registrations.
fn mount_step(step: &Value, ctx: &Rc<Ctx>, editor: &Editor) -> Result<(), LexicalError> {
    if let Some(spec) = step.get("characterLimit") {
        let max_characters = number(spec.get("maxCharacters").unwrap_or(&Value::Null))? as i64;
        let strlen = match spec.get("strlen").and_then(Value::as_str) {
            None => None,
            name => Some(strlen_of(name)?),
        };
        let log = ctx.clone();
        let unregister = lexical::overflow::register_character_limit(
            editor,
            max_characters,
            strlen,
            Some(Rc::new(move |characters| {
                log.log(vec![
                    "remainingCharacters".into(),
                    (characters as f64).into(),
                ]);
            })),
        )?;
        ctx.mounted
            .borrow_mut()
            .insert("characterLimit".to_owned(), unregister);
    } else {
        let log = ctx.clone();
        let unregister = lexical::rich_text::register_table_of_contents(editor, move |entries| {
            let entries = entries
                .iter()
                .map(|e| {
                    Value::Array(vec![
                        e.key.as_str().into(),
                        e.text.as_str().into(),
                        e.tag.as_str().into(),
                    ])
                })
                .collect();
            log.log(vec!["tableOfContents".into(), Value::Array(entries)]);
        })?;
        ctx.mounted
            .borrow_mut()
            .insert("tableOfContents".to_owned(), unregister);
    }
    Ok(())
}

/// `ops.ts`'s `STATES`, one set per session (JS's are shared by every
/// session, which no session can tell: a node type learns a config in one
/// editor only).
struct States {
    /// A number, 1 by default.
    count: StateConfig<f64>,
    /// A flag that a copy of its node does not keep.
    fresh: StateConfig<bool>,
    /// A string, '' by default.
    label: StateConfig<String>,
    /// Another config with the key of `label`.
    other_label: StateConfig<String>,
    /// An array of words, written as one string and compared by its words.
    words: StateConfig<Vec<String>>,
}

impl Default for States {
    fn default() -> Self {
        let string = |v: &Value| v.as_str().unwrap_or("").to_owned();
        States {
            count: create_state("count", |v| v.as_f64().unwrap_or(1.0)),
            fresh: create_state_with(
                "fresh",
                StateValueConfig {
                    reset_on_copy_node: true,
                    ..StateValueConfig::new(|v: &Value| v.as_bool() == Some(true))
                },
            ),
            label: create_state("label", string),
            other_label: create_state("label", string),
            words: create_state_with(
                "words",
                StateValueConfig {
                    parse: Rc::new(|v| match v.as_str() {
                        Some(s) if !s.is_empty() => s.split(' ').map(str::to_owned).collect(),
                        _ => Vec::new(),
                    }),
                    unparse: Rc::new(|v| Value::from(v.join(" "))),
                    is_equal: Rc::new(|a, b| a == b),
                    reset_on_copy_node: false,
                    derived: false,
                    schema: None,
                },
            ),
        }
    }
}

/// Runs `$body` with `$config` the state `$name` names, `$to_json` writing
/// its values as JSON and `$from_json` reading them.
macro_rules! with_state_config {
    ($states:expr, $name:expr, |$config:ident, $to_json:ident, $from_json:ident| $body:expr) => {
        match $name {
            "count" => {
                let $config = &$states.count;
                let $to_json = |v: &f64| Value::from(*v);
                let $from_json = |v: &Value| v.as_f64();
                $body
            }
            "fresh" => {
                let $config = &$states.fresh;
                let $to_json = |v: &bool| Value::from(*v);
                let $from_json = |v: &Value| v.as_bool();
                $body
            }
            "label" | "otherLabel" => {
                let $config = if $name == "label" {
                    &$states.label
                } else {
                    &$states.other_label
                };
                let $to_json = |v: &String| Value::from(v.as_str());
                let $from_json = |v: &Value| v.as_str().map(str::to_owned);
                $body
            }
            "words" => {
                let $config = &$states.words;
                let $to_json =
                    |v: &Vec<String>| Value::Array(v.iter().map(|w| w.as_str().into()).collect());
                let $from_json = |v: &Value| {
                    v.as_array()?
                        .iter()
                        .map(|w| w.as_str().map(str::to_owned))
                        .collect::<Option<Vec<String>>>()
                };
                $body
            }
            other => Err(err(format!("unknown state {other}"))),
        }
    };
}

impl Ctx {
    fn log(&self, entry: Vec<Value>) {
        self.log.borrow_mut().push(Value::Array(entry));
    }

    /// `command(ctx, name)`: one of Lexical's own commands
    /// (`CORE_COMMANDS`), a list or link command (`PACKAGE_COMMANDS`), or one
    /// command per other name.
    fn command(&self, name: &str) -> ScriptCommand {
        known_command(name).unwrap_or_else(|| {
            ScriptCommand::Json(
                self.commands
                    .borrow_mut()
                    .entry(name.to_owned())
                    .or_insert_with(|| create_command(name))
                    .clone(),
            )
        })
    }
}

/// One of Lexical's own commands (`CORE_COMMANDS`) or a list or link command
/// (`PACKAGE_COMMANDS`), by the name JS exports it as.
pub(crate) fn known_command(name: &str) -> Option<ScriptCommand> {
    Some(match name {
        "CAN_REDO_COMMAND" => ScriptCommand::Bool(CAN_REDO_COMMAND),
        "CAN_UNDO_COMMAND" => ScriptCommand::Bool(CAN_UNDO_COMMAND),
        "CLEAR_EDITOR_COMMAND" => ScriptCommand::Void(CLEAR_EDITOR_COMMAND),
        "CLEAR_HISTORY_COMMAND" => ScriptCommand::Void(CLEAR_HISTORY_COMMAND),
        "CLICK_COMMAND" => ScriptCommand::Mouse(CLICK_COMMAND),
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
        "BEFORE_INPUT_COMMAND" => ScriptCommand::Input(BEFORE_INPUT_COMMAND),
        "DRAGEND_COMMAND" => ScriptCommand::Drag(DRAGEND_COMMAND, "dragend"),
        "DRAGOVER_COMMAND" => ScriptCommand::Drag(DRAGOVER_COMMAND, "dragover"),
        "DRAGSTART_COMMAND" => ScriptCommand::Drag(DRAGSTART_COMMAND, "dragstart"),
        "DROP_COMMAND" => ScriptCommand::Drag(DROP_COMMAND, "drop"),
        "COMPOSITION_END_COMMAND" => {
            ScriptCommand::Composition(COMPOSITION_END_COMMAND, "compositionend")
        }
        "COMPOSITION_START_COMMAND" => {
            ScriptCommand::Composition(COMPOSITION_START_COMMAND, "compositionstart")
        }
        "COPY_COMMAND" => ScriptCommand::ClipboardOrKeyboard(COPY_COMMAND),
        "CUT_COMMAND" => ScriptCommand::ClipboardOrKeyboard(CUT_COMMAND),
        "KEY_ARROW_DOWN_COMMAND" => ScriptCommand::Keyboard(KEY_ARROW_DOWN_COMMAND),
        "KEY_ARROW_LEFT_COMMAND" => ScriptCommand::Keyboard(KEY_ARROW_LEFT_COMMAND),
        "KEY_ARROW_RIGHT_COMMAND" => ScriptCommand::Keyboard(KEY_ARROW_RIGHT_COMMAND),
        "KEY_ARROW_UP_COMMAND" => ScriptCommand::Keyboard(KEY_ARROW_UP_COMMAND),
        "KEY_BACKSPACE_COMMAND" => ScriptCommand::Keyboard(KEY_BACKSPACE_COMMAND),
        "KEY_DELETE_COMMAND" => ScriptCommand::Keyboard(KEY_DELETE_COMMAND),
        "KEY_DOWN_COMMAND" => ScriptCommand::Keyboard(KEY_DOWN_COMMAND),
        "KEY_ENTER_COMMAND" => ScriptCommand::Key(KEY_ENTER_COMMAND),
        "KEY_ESCAPE_COMMAND" => ScriptCommand::Keyboard(KEY_ESCAPE_COMMAND),
        "KEY_MODIFIER_COMMAND" => ScriptCommand::Keyboard(KEY_MODIFIER_COMMAND),
        "KEY_SPACE_COMMAND" => ScriptCommand::Keyboard(KEY_SPACE_COMMAND),
        "KEY_TAB_COMMAND" => ScriptCommand::Keyboard(KEY_TAB_COMMAND),
        "MOVE_TO_END" => ScriptCommand::Keyboard(MOVE_TO_END),
        "MOVE_TO_START" => ScriptCommand::Keyboard(MOVE_TO_START),
        "OUTDENT_CONTENT_COMMAND" => ScriptCommand::Void(OUTDENT_CONTENT_COMMAND),
        "PASTE_COMMAND" => ScriptCommand::Paste(PASTE_COMMAND),
        "REDO_COMMAND" => ScriptCommand::Void(REDO_COMMAND),
        "REMOVE_TEXT_COMMAND" => ScriptCommand::Void(REMOVE_TEXT_COMMAND),
        "SELECT_ALL_COMMAND" => ScriptCommand::Keyboard(SELECT_ALL_COMMAND),
        "SET_TEXT_FORMAT_COMMAND" => ScriptCommand::Formats(SET_TEXT_FORMAT_COMMAND),
        "UNDO_COMMAND" => ScriptCommand::Void(UNDO_COMMAND),
        "INSERT_CHECK_LIST_COMMAND" => ScriptCommand::Void(list::INSERT_CHECK_LIST_COMMAND),
        "INSERT_ORDERED_LIST_COMMAND" => ScriptCommand::Void(list::INSERT_ORDERED_LIST_COMMAND),
        "INSERT_UNORDERED_LIST_COMMAND" => ScriptCommand::Void(list::INSERT_UNORDERED_LIST_COMMAND),
        "REMOVE_LIST_COMMAND" => ScriptCommand::Void(list::REMOVE_LIST_COMMAND),
        "UPDATE_LIST_START_COMMAND" => ScriptCommand::ListStart(list::UPDATE_LIST_START_COMMAND),
        "TOGGLE_LINK_COMMAND" => ScriptCommand::Link(link::TOGGLE_LINK_COMMAND),
        "INSERT_EMBED_COMMAND" => ScriptCommand::Str(INSERT_EMBED_COMMAND),
        "INSERT_HORIZONTAL_RULE_COMMAND" => {
            ScriptCommand::Void(ext::INSERT_HORIZONTAL_RULE_COMMAND)
        }
        "DRAG_DROP_PASTE" => ScriptCommand::Files(lexical::rich_text::DRAG_DROP_PASTE),
        _ => return None,
    })
}

/// A command a script names, by its payload type: Lexical's own take
/// nothing (`void`), a boolean, a string, a map of formats to booleans or
/// a DOM event, and those the script makes take any JSON.
///
/// A JSON object is the event it describes for the commands of ops.ts's
/// `EVENT_COMMANDS` (`eventPayload`), and a handler logs an event as the
/// fields Lexical reads of it (`payloadJson`).
pub(crate) enum ScriptCommand {
    Void(LexicalCommand<()>),
    Bool(LexicalCommand<bool>),
    Str(LexicalCommand<String>),
    Formats(LexicalCommand<Vec<(String, bool)>>),
    /// A keyboard command: `{key, code, altKey, ctrlKey, metaKey,
    /// shiftKey}`, or `null`, which JS passes on as it is: here a keyboard
    /// event of no type, which handlers log as `null`.
    Keyboard(LexicalCommand<Rc<KeyboardEvent>>),
    /// `KEY_ENTER_COMMAND`: a keyboard event or `null`.
    Key(LexicalCommand<Option<Rc<KeyboardEvent>>>),
    /// `BEFORE_INPUT_COMMAND`: `{inputType, data}`.
    Input(LexicalCommand<Rc<InputEvent>>),
    /// A composition command and its event's type: `{data}`.
    Composition(LexicalCommand<Rc<CompositionEvent>>, &'static str),
    /// `CLICK_COMMAND`: `{targetKey, clientX, clientY, button, detail}` and
    /// the modifier flags,
    /// where JS's target is the DOM of the node `targetKey` and Rust's
    /// `target_key` is the key.
    Mouse(LexicalCommand<Rc<MouseEvent>>),
    /// A drag command and its event's type: a mouse event's fields and
    /// `dataTransfer`, `{data: {type: data...}, files: [{name, type,
    /// text}...]}` or `null`.
    Drag(LexicalCommand<Rc<DragEvent>>, &'static str),
    /// `PASTE_COMMAND`: a `paste` `ClipboardEvent` of `clipboardData`, a
    /// data transfer as a drag event's. The keydown and beforeinput
    /// handlers dispatch it with their event.
    Paste(LexicalCommand<PasteCommandType>),
    /// `DRAG_DROP_PASTE`, which scripts only listen to: its files are
    /// logged as `[{name, type}...]`.
    Files(LexicalCommand<Vec<DataTransferFile>>),
    /// `COPY_COMMAND` and `CUT_COMMAND`, which scripts only listen to.
    ClipboardOrKeyboard(LexicalCommand<Option<ClipboardOrKeyboardEvent>>),
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
    Some(Some(link::ToggleLinkPayload::Attributes {
        url: object.get("url")?.as_str()?.to_owned(),
        attributes: link_attributes_of(object)?,
    }))
}

/// `LinkAttributes` from JSON: `rel`, `target` and `title`, each absent,
/// `null` or a string.
pub(crate) fn link_attributes_of(object: &Object) -> Option<link::LinkAttributes> {
    let attribute = |key: &str| -> Option<Option<Option<String>>> {
        Some(match object.get(key) {
            None => None,
            Some(Value::Null) => Some(None),
            Some(value) => Some(Some(value.as_str()?.to_owned())),
        })
    };
    Some(link::LinkAttributes {
        rel: attribute("rel")?,
        target: attribute("target")?,
        title: attribute("title")?,
    })
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

/// `eventPayload`'s `KeyboardEvent`: `key`, `code` and the modifier flags
/// of a JSON object, cancelable.
fn keyboard_event_of(payload: &Value) -> Option<Rc<KeyboardEvent>> {
    let object = payload.as_object()?;
    let string = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let flag = |key: &str| object.get(key).and_then(Value::as_bool) == Some(true);
    Some(Rc::new(KeyboardEvent {
        key: string("key"),
        code: string("code"),
        alt_key: flag("altKey"),
        ctrl_key: flag("ctrlKey"),
        meta_key: flag("metaKey"),
        shift_key: flag("shiftKey"),
        ..KeyboardEvent::default()
    }))
}

/// `eventPayload`'s `InputEvent`: a cancelable `beforeinput` of
/// `inputType`, with `data` when it is a string.
fn input_event_of(payload: &Value) -> Option<Rc<InputEvent>> {
    let object = payload.as_object()?;
    let input_type = match object.get("inputType") {
        Some(Value::String(input_type)) => input_type.to_owned(),
        _ => "undefined".to_owned(),
    };
    Some(Rc::new(InputEvent {
        data: object
            .get("data")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ..InputEvent::new(&input_type)
    }))
}

/// `eventPayload`'s `MouseEvent` of `type_`: `clientX`, `clientY`,
/// `button`, `detail` and the modifier flags of a JSON object, cancelable,
/// whose target is the node `targetKey` (none when it is absent or `null`).
/// JS's target is `editor.getElementByKey`, which the click scripts reach
/// through a root element.
fn mouse_event_of(type_: &str, payload: &Value) -> Option<MouseEvent> {
    let object = payload.as_object()?;
    let number = |key: &str| object.get(key).and_then(Value::as_f64);
    let flag = |key: &str| object.get(key).and_then(Value::as_bool) == Some(true);
    Some(MouseEvent {
        client_x: number("clientX").unwrap_or(0.0),
        client_y: number("clientY").unwrap_or(0.0),
        button: number("button").unwrap_or(0.0) as i32,
        detail: number("detail").unwrap_or(1.0) as i32,
        alt_key: flag("altKey"),
        ctrl_key: flag("ctrlKey"),
        meta_key: flag("metaKey"),
        shift_key: flag("shiftKey"),
        target_key: object
            .get("targetKey")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ..MouseEvent::new(type_)
    })
}

/// `scriptDataTransfer`: a data transfer's JSON, `{data: {type: data...},
/// files: [{name, type, text}...]}`, or `None` for anything else (`null`).
fn script_data_transfer(json: Option<&Value>) -> Result<Option<Rc<DataTransfer>>, LexicalError> {
    let Some(object) = json.and_then(Value::as_object) else {
        return Ok(None);
    };
    let data_transfer = match object.get("data") {
        Some(data) => data_transfer_of(data)?,
        None => DataTransfer::new(),
    };
    let files = match object.get("files") {
        None => Vec::new(),
        Some(files) => files
            .as_array()
            .ok_or_else(|| err(format!("expected files, got {files}")))?
            .iter()
            .map(|file| {
                let field = |key: &str| file.get(key).and_then(Value::as_str);
                let name = field("name").ok_or_else(|| err(format!("no name in {file}")))?;
                let type_ = field("type").ok_or_else(|| err(format!("no type in {file}")))?;
                let text = field("text").unwrap_or("");
                Ok(DataTransferFile::new(name, type_, text.as_bytes().to_vec()))
            })
            .collect::<Result<Vec<_>, LexicalError>>()?,
    };
    data_transfer.set_files(files);
    Ok(Some(Rc::new(data_transfer)))
}

/// `['dataTransfer', [[type, data]...]]` of what a drag event's data
/// transfer carries: the drag marker of this editor
/// (`application/x-lexical-drag` of its `editorKey`), whose key is random,
/// as `'<this editor>'`.
fn data_transfer_entry(data_transfer: &DataTransfer, editor: &Editor) -> Vec<Value> {
    let mut types = data_transfer.types();
    if !data_transfer.files().is_empty() {
        types.pop();
    }
    let entries = types
        .into_iter()
        .map(|type_| {
            let data = data_transfer.get_data(&type_);
            let this_editor = type_ == "application/x-lexical-drag"
                && lexical::json::parse(&data).ok().and_then(|marker| {
                    marker
                        .get("editorKey")
                        .and_then(Value::as_str)
                        .map(|key| key == editor.get_key())
                }) == Some(true);
            Value::Array(vec![
                type_.as_str().into(),
                if this_editor {
                    "<this editor>".into()
                } else {
                    data.into()
                },
            ])
        })
        .collect();
    vec!["dataTransfer".into(), Value::Array(entries)]
}

/// `payloadJson` of files: `[{name, type}...]`.
fn files_json(files: &[DataTransferFile]) -> Value {
    Value::Array(
        files
            .iter()
            .map(|file| {
                let mut json = Object::new();
                json.insert("name", file.name.as_str().into());
                json.insert("type", file.type_.as_str().into());
                json.into()
            })
            .collect(),
    )
}

/// `payloadJson` of a `KeyboardEvent`, its keys in order; a keyboard event
/// of no type stands for JS's `null`.
fn keyboard_json(event: &KeyboardEvent) -> Value {
    if event.type_.is_empty() {
        return Value::Null;
    }
    let mut json = Object::new();
    json.insert("altKey", event.alt_key.into());
    json.insert("code", event.code.as_str().into());
    json.insert("ctrlKey", event.ctrl_key.into());
    json.insert("key", event.key.as_str().into());
    json.insert("metaKey", event.meta_key.into());
    json.insert("shiftKey", event.shift_key.into());
    json.insert("type", event.type_.as_str().into());
    json.into()
}

/// `payloadJson` of an `InputEvent`.
fn input_json(event: &InputEvent) -> Value {
    let mut json = Object::new();
    json.insert(
        "data",
        event.data.as_deref().map_or(Value::Null, Value::from),
    );
    json.insert("inputType", event.input_type.as_str().into());
    json.insert("type", event.type_.as_str().into());
    json.into()
}

/// `payloadJson` of a `CompositionEvent`.
fn composition_json(event: &CompositionEvent) -> Value {
    let mut json = Object::new();
    json.insert("data", event.data.as_str().into());
    json.insert("type", event.type_.as_str().into());
    json.into()
}

/// `payloadJson` of an event it has no JSON for, which throws in JS.
fn no_json(event: &Event) -> LexicalError {
    err(format!("no JSON for a {} event", event.type_))
}

/// What a dispatch did: whether a handler handled it, whether the event
/// the payload became had its default prevented, and for a drag event with
/// a data transfer, its `['dataTransfer', ...]` entry (`data_transfer_entry`).
pub(crate) type Dispatched = (bool, Option<bool>, Option<Vec<Value>>);

/// What `dispatchLogged` logs of a dispatch: `['dispatched', name,
/// handled]`, then `['defaultPrevented', prevented]` when the payload
/// became an event, then a drag event's `['dataTransfer', ...]`.
pub(crate) fn log_dispatched(log: impl Fn(Vec<Value>), name: &str, dispatched: Dispatched) {
    let (handled, default_prevented, data_transfer) = dispatched;
    log(vec!["dispatched".into(), name.into(), handled.into()]);
    if let Some(prevented) = default_prevented {
        log(vec!["defaultPrevented".into(), prevented.into()]);
    }
    if let Some(entry) = data_transfer {
        log(entry);
    }
}

impl ScriptCommand {
    /// `editor.dispatchCommand(command, payload)`, and whether the event
    /// the payload became had its default prevented. A payload the command
    /// doesn't take is an error, which JS would pass on as it is.
    pub(crate) fn dispatch(
        &self,
        editor: &Editor,
        payload: Value,
    ) -> Result<Dispatched, LexicalError> {
        let not_taken = || err(format!("{payload} is not a payload this command takes"));
        match self {
            ScriptCommand::Keyboard(command) if payload.is_null() => {
                let event = Rc::new(KeyboardEvent {
                    event: Event::new(""),
                    ..KeyboardEvent::default()
                });
                Ok((editor.dispatch_command(command, event)?, None, None))
            }
            ScriptCommand::Keyboard(command) => {
                let event = keyboard_event_of(&payload).ok_or_else(not_taken)?;
                let handled = editor.dispatch_command(command, event.clone())?;
                Ok((handled, Some(event.default_prevented()), None))
            }
            ScriptCommand::Key(command) if !payload.is_null() => {
                let event = keyboard_event_of(&payload).ok_or_else(not_taken)?;
                let handled = editor.dispatch_command(command, Some(event.clone()))?;
                Ok((handled, Some(event.default_prevented()), None))
            }
            ScriptCommand::Input(command) => {
                let event = input_event_of(&payload).ok_or_else(not_taken)?;
                let handled = editor.dispatch_command(command, event.clone())?;
                Ok((handled, Some(event.default_prevented()), None))
            }
            ScriptCommand::Composition(command, type_) => {
                let object = payload.as_object().ok_or_else(not_taken)?;
                let data = object.get("data").and_then(Value::as_str).unwrap_or("");
                let event = Rc::new(CompositionEvent::new(type_, data));
                let handled = editor.dispatch_command(command, event.clone())?;
                Ok((handled, Some(event.default_prevented()), None))
            }
            ScriptCommand::Mouse(command) => {
                let event = Rc::new(mouse_event_of("click", &payload).ok_or_else(not_taken)?);
                let handled = editor.dispatch_command(command, event.clone())?;
                Ok((handled, Some(event.default_prevented()), None))
            }
            ScriptCommand::Drag(command, type_) => {
                let mouse = mouse_event_of(type_, &payload).ok_or_else(not_taken)?;
                let data_transfer = script_data_transfer(payload.get("dataTransfer"))?;
                let event = Rc::new(DragEvent {
                    mouse,
                    data_transfer: data_transfer.clone(),
                });
                let handled = editor.dispatch_command(command, event.clone())?;
                Ok((
                    handled,
                    Some(event.default_prevented()),
                    data_transfer.map(|data_transfer| data_transfer_entry(&data_transfer, editor)),
                ))
            }
            ScriptCommand::Paste(command) => {
                let object = payload.as_object().ok_or_else(not_taken)?;
                let clipboard_data = script_data_transfer(object.get("clipboardData"))?;
                let event = Rc::new(ClipboardEvent::new("paste", clipboard_data));
                let handled =
                    editor.dispatch_command(command, PasteCommandType::Clipboard(event.clone()))?;
                Ok((handled, Some(event.default_prevented()), None))
            }
            _ => self
                .dispatch_json(editor, payload)
                .map(|handled| (handled, None, None)),
        }
    }

    /// `editor.dispatchCommand(command, payload)` of a payload that is no
    /// event.
    fn dispatch_json(&self, editor: &Editor, payload: Value) -> Result<bool, LexicalError> {
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
            ScriptCommand::Key(command) if payload.is_null() => {
                editor.dispatch_command(command, None)
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
            ScriptCommand::Keyboard(command) => editor.register_command(
                command,
                move |payload, editor| listener(&keyboard_json(payload), editor),
                priority,
            ),
            ScriptCommand::Key(command) => editor.register_command(
                command,
                move |payload, editor| {
                    listener(
                        &payload.as_deref().map_or(Value::Null, keyboard_json),
                        editor,
                    )
                },
                priority,
            ),
            ScriptCommand::Input(command) => editor.register_command(
                command,
                move |payload, editor| listener(&input_json(payload), editor),
                priority,
            ),
            ScriptCommand::Composition(command, _) => editor.register_command(
                command,
                move |payload, editor| listener(&composition_json(payload), editor),
                priority,
            ),
            // `payloadJson` throws for a `MouseEvent`.
            ScriptCommand::Mouse(command) => editor.register_command(
                command,
                move |payload, _| Err(no_json(&payload.event)),
                priority,
            ),
            ScriptCommand::Paste(command) => editor.register_command(
                command,
                move |payload, editor| {
                    let json = match payload {
                        PasteCommandType::Input(event) => input_json(event),
                        PasteCommandType::Keyboard(event) => keyboard_json(event),
                        PasteCommandType::Clipboard(event) => return Err(no_json(event)),
                    };
                    listener(&json, editor)
                },
                priority,
            ),
            ScriptCommand::ClipboardOrKeyboard(command) => editor.register_command(
                command,
                move |payload, editor| {
                    let json = match payload {
                        None => Value::Null,
                        Some(ClipboardOrKeyboardEvent::Keyboard(event)) => keyboard_json(event),
                        Some(ClipboardOrKeyboardEvent::Clipboard(event)) => {
                            return Err(no_json(event));
                        }
                    };
                    listener(&json, editor)
                },
                priority,
            ),
            ScriptCommand::Drag(command, _) => editor.register_command(
                command,
                move |payload, _| Err(no_json(&payload.mouse.event)),
                priority,
            ),
            ScriptCommand::Files(command) => editor.register_command(
                command,
                move |payload, editor| listener(&files_json(payload), editor),
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
            | "setState"
            | "updateState"
            | "logState"
            | "logStateChange"
            | "setBlocksType"
            | "logSelectionText"
            | "logRootTextContent"
            | "logRootTextContentEmpty"
            | "logCanShowPlaceholder"
            | "logTextIntersection"
            | "wrapSelectionInMarkNode"
            | "unwrapMarkNode"
            | "logMarkIDs"
            | "addMarkID"
            | "removeMarkID"
            | "logNodeMarkIDs"
            | "logMarkIDsAtSelection"
            | "migrateMarkNodesToMarkState"
            | "logMarkIDsByFlavor"
            | "copy"
            | "pasteClipboard"
            | "pasteLexical"
            | "pastePlainText"
            | "pasteData"
            | "generateJSON"
            | "insertGeneratedNodes"
            | "patchStyleText"
            | "logSelectionStyleValue"
            | "logStyleObject"
            | "logAtNodeEnd"
            | "trimTextContentFromAnchor"
            | "forEachSelectedTextNode"
            | "ensureForwardRangeSelection"
            | "wrapNodes"
            | "logShouldOverride"
            | "moveCharacter"
            | "logSlice"
            | "copyBlockFormatIndent"
            | "logCloneWithProperties"
            | "logDfs"
            | "logDepth"
            | "logNextSiblingOrParentSibling"
            | "logNextRightPreorderNode"
            | "logNearestBlockElementAncestor"
            | "logChildren"
            | "unwrapAndFilterDescendants"
            | "logDescendantsMatching"
            | "insertFirst"
            | "insertNodeToNearestRoot"
            | "insertNodeIntoLeaf"
            | "splitNode"
            | "wrapNodeInElement"
            | "unwrapNode"
            | "saveEditorState"
            | "restoreEditorState"
            | "escape"
            | "logBlockFullySelected"
            | "setSlot"
            | "removeSlot"
            | "logSlots"
            | "logSelectionSlotFrame"
            | "logDfsWithSlots"
            | "logNodesExist"
            | "setBlockId"
            | "logBlockIds"
            | "logNodeStructure"
            | "logBlockContainer"
            | "logStructureEnforced"
            | "normalizeNodeStructure"
            | "appendNew"
            | "logBlockById"
            | "logIdentified"
            | "logBlockHandles"
            | "logBlockHandle"
            | "resolveBlockHandle"
            | "selectTypeaheadOption"
            | "logFullMatchOffset"
            | "logDecoratorBlock"
            | "logUrlMatcher"
            | "logSingleTokenPaste"
            | "findOffset"
            | "wrapOverflowedNodes"
            | "mergePrevious",
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

fn code_node(node: NodeRef) -> Result<NodeRef, LexicalError> {
    if lexical::code::is_code_node(&node) {
        Ok(node)
    } else {
        Err(err(format!("{} is not a code node", node.get_key())))
    }
}

fn mark_node(node: NodeRef) -> Result<NodeRef, LexicalError> {
    if mark::is_mark_node(&node) {
        Ok(node)
    } else {
        Err(err(format!("{} is not a mark node", node.get_key())))
    }
}

/// An array of strings as JSON.
fn strings(values: &[String]) -> Value {
    Value::Array(values.iter().map(|v| v.as_str().into()).collect())
}

fn list_item(node: NodeRef) -> Result<NodeRef, LexicalError> {
    if list::is_list_item_node(&node) {
        Ok(node)
    } else {
        Err(err(format!("{} is not a list item", node.get_key())))
    }
}

/// `$parse(nodes)`.
/// `$nodeArg(value)`: the node `key` when `value` is a string, else
/// `value` parsed.
fn node_arg(value: &Value) -> Result<NodeRef, LexicalError> {
    match value.as_str() {
        Some(key) => node(key),
        None => parse_serialized_node(value),
    }
}

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
        "modify" => range()?.modify(
            match a(0).as_str() {
                Some("move") => SelectionAlter::Move,
                Some("extend") => SelectionAlter::Extend,
                _ => return Err(err(format!("bad alter {}", a(0)))),
            },
            flag(&a(1), false),
            match a(2).as_str() {
                Some("character") => SelectionGranularity::Character,
                Some("word") => SelectionGranularity::Word,
                Some("line") => SelectionGranularity::Line,
                Some("lineboundary") => SelectionGranularity::LineBoundary,
                _ => return Err(err(format!("bad granularity {}", a(2)))),
            },
        )?,
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

/// `$sel()` as the selection itself.
fn range_sel() -> Result<SelectionRef, LexicalError> {
    get_selection()
        .filter(SelectionRef::is_range)
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
        "addID" => mark_node(n)?.add_id(&string(&a(0))?),
        "append" => element(n)?.append(&parse(&a(0))?)?,
        "clear" => element(n)?.clear()?,
        "deleteID" => mark_node(n)?.delete_id(&string(&a(0))?),
        "excludeFromCopy" => {
            let destination = string(&a(0))?;
            let excluded = element(n.clone())?.exclude_from_copy(&destination);
            ctx.log(vec![
                "excludeFromCopy".into(),
                n.get_key().into(),
                destination.into(),
                excluded.into(),
            ]);
        }
        "extractWithChild" => {
            let child = string(&a(0))?;
            let destination = string(&a(1))?;
            let selection = get_selection().ok_or_else(|| err("no selection"))?;
            let extracted = element(n.clone())?.extract_with_child(
                &node(&child)?,
                Some(&selection),
                &destination,
            )?;
            ctx.log(vec![
                "extractWithChild".into(),
                n.get_key().into(),
                child.into(),
                destination.into(),
                extracted.into(),
            ]);
        }
        "getIDs" => {
            let ids = mark_node(n.clone())?.get_ids().unwrap_or_default();
            ctx.log(vec!["getIDs".into(), n.get_key().into(), strings(&ids)]);
        }
        "hasID" => {
            let id = string(&a(0))?;
            let has = mark_node(n.clone())?.has_id(&id);
            ctx.log(vec![
                "hasID".into(),
                n.get_key().into(),
                id.into(),
                has.into(),
            ]);
        }
        // A string names an existing node, which moves.
        "insertAfter" => {
            n.insert_after(&node_arg(&a(0))?, flag(&a(1), true))?;
        }
        "insertBefore" => {
            n.insert_before(&node_arg(&a(0))?, flag(&a(1), true))?;
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
        "setFormat" if is_decorator_block_node(&n) => {
            n.set_decorator_block_format(a(0).as_str().unwrap_or(""))
        }
        "setFormat" => element(n)?.set_element_format(a(0).as_str().unwrap_or("")),
        "setIDs" => {
            let ids = a(0)
                .as_array()
                .ok_or_else(|| err("expected an array of ids"))?
                .iter()
                .map(string)
                .collect::<Result<Vec<_>, _>>()?;
            mark_node(n)?.set_ids(&ids);
        }
        "setIndent" => element(n)?.set_indent(number(&a(0))?)?,
        "setIsSyntaxHighlightSupported" => code_node(n)?
            .set_is_syntax_highlight_supported(a(0).as_bool().ok_or_else(|| err("a boolean"))?),
        // `null` is `undefined`.
        "setLanguage" => code_node(n)?.set_language(a(0).as_str()),
        "setTag" => {
            if !is_heading_node(&n) {
                return Err(err(format!("{} is not a heading", n.get_key())));
            }
            let tag = HeadingTagType::parse(&string(&a(0))?).ok_or_else(|| err("bad tag"))?;
            n.set_heading_tag(tag);
        }
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
        Some("selectAll") => {
            if action.get("scoped").and_then(Value::as_bool) == Some(true) {
                selection::select_all_in(&range_sel()?).map(drop)
            } else {
                select_all().map(drop)
            }
        }
        Some("setSlot") => {
            let host = node(action.str("key").map_err(err)?)?;
            let value = match action.get("valueKey").and_then(Value::as_str) {
                Some(key) => node(key)?,
                None => parse(&Value::Array(vec![
                    action.get("node").cloned().unwrap_or(Value::Undefined),
                ]))?
                .remove(0),
            };
            lexical::set_slot(&host, action.str("name").map_err(err)?, &value)?;
            Ok(())
        }
        Some("removeSlot") => {
            let host = node(action.str("key").map_err(err)?)?;
            lexical::remove_slot(&host, action.str("name").map_err(err)?)?;
            Ok(())
        }
        Some("logSlots") => {
            let key = action.str("key").map_err(err)?;
            let n = node(key)?;
            let key_of = |n: Option<NodeRef>| n.map_or(Value::Null, |n| n.get_key().into());
            ctx.log(vec![
                "slots".into(),
                key.into(),
                strings(&lexical::get_slot_names(&n)),
                key_of(lexical::get_slot_host(&n)),
                lexical::get_slot_name_within_host(&n).map_or(Value::Null, Value::from),
                key_of(lexical::get_slot_frame(&n)),
            ]);
            Ok(())
        }
        Some("logSelectionSlotFrame") => {
            let frame = lexical::get_selection_slot_frame(get_selection().as_ref())?;
            ctx.log(vec![
                "selectionSlotFrame".into(),
                frame.map_or(Value::Null, |f| f.get_key().into()),
            ]);
            Ok(())
        }
        Some("logDfsWithSlots") => {
            let at = |name: &str| {
                action
                    .get(name)
                    .and_then(Value::as_str)
                    .map(node)
                    .transpose()
            };
            let (start, end) = (at("start")?, at("end")?);
            let reverse = flag(action.get("reverse").unwrap_or(&Value::Null), false);
            let nodes = if reverse {
                lexical::utils::reverse_dfs_with_slots(start.as_ref(), end.as_ref())
            } else {
                lexical::utils::dfs_with_slots(start.as_ref(), end.as_ref())
            };
            ctx.log(vec![
                "dfsWithSlots".into(),
                reverse.into(),
                Value::Array(
                    nodes
                        .into_iter()
                        .map(|d| {
                            Value::Array(vec![d.node.get_key().into(), (d.depth as f64).into()])
                        })
                        .collect(),
                ),
            ]);
            Ok(())
        }
        Some("logNodesExist") => {
            let keys = action
                .get("keys")
                .and_then(Value::as_array)
                .ok_or_else(|| err("keys"))?;
            let mut exist = Vec::new();
            for key in keys {
                let key = key.as_str().ok_or_else(|| err("a key"))?;
                exist.push(Value::Array(vec![
                    key.into(),
                    lexical::get_node_by_key(key).is_some().into(),
                ]));
            }
            ctx.log(vec!["nodesExist".into(), Value::Array(exist)]);
            Ok(())
        }
        Some("setBlockId") => {
            let key = action.get("key").and_then(Value::as_str).unwrap_or("");
            let id = action.get("id").and_then(Value::as_str).map(str::to_owned);
            set_state(&node(key)?, &ext::block_id_state(), id).map(|_| ())
        }
        Some("logNodeStructure") => {
            let types = action
                .get("types")
                .and_then(Value::as_array)
                .ok_or_else(|| err("types"))?;
            for node_type in types {
                let node_type = node_type.as_str().ok_or_else(|| err("a type"))?;
                let json = structure::structure_json(editor, node_type)?;
                ctx.log(vec!["nodeStructure".into(), node_type.into(), json]);
            }
            Ok(())
        }
        Some("logBlockContainer") => {
            let keys = action
                .get("keys")
                .and_then(Value::as_array)
                .ok_or_else(|| err("keys"))?;
            for key in keys {
                let key = key.as_str().ok_or_else(|| err("a key"))?;
                let container = lexical::is_block_container(&node(key)?);
                ctx.log(vec!["blockContainer".into(), key.into(), container.into()]);
            }
            Ok(())
        }
        Some("logStructureEnforced") => {
            let enforced = lexical::internal_is_node_structure_enforced(editor);
            ctx.log(vec!["structureEnforced".into(), enforced.into()]);
            Ok(())
        }
        Some("normalizeNodeStructure") => {
            lexical::internal_normalize_node_structure(&node(action.str("key").map_err(err)?)?)
        }
        Some("appendNew") => {
            let parent = node(action.str("key").map_err(err)?)?;
            let child = structure::create_test_node(action.str("type").map_err(err)?)?;
            parent.append(&[child])
        }
        Some("logBlockIds") => {
            let keys = action
                .get("keys")
                .and_then(Value::as_array)
                .ok_or_else(|| err("keys"))?;
            let mut ids = Vec::new();
            for key in keys {
                let key = key.as_str().ok_or_else(|| err("a key"))?;
                let id = ext::get_block_id(&node(key)?)?;
                ids.push(Value::Array(vec![
                    key.into(),
                    id.map_or(Value::Null, Value::from),
                ]));
            }
            ctx.log(vec!["blockIds".into(), Value::Array(ids)]);
            Ok(())
        }
        Some("logBlockById") => {
            let id = action.get("id").and_then(Value::as_str).unwrap_or("");
            let found = ext::get_block_by_id(id)?;
            ctx.log(vec![
                "blockById".into(),
                id.into(),
                found.map_or(Value::Null, |n| n.get_key().into()),
            ]);
            Ok(())
        }
        Some("logIdentified") => {
            let key = action.get("key").and_then(Value::as_str).unwrap_or("");
            let target = node(key)?;
            let output = ext::get_extension_output(&ext::block_id_extension())?
                .ok_or_else(|| err("BlockIdExtension has no output"))?;
            ctx.log(vec![
                "identified".into(),
                key.into(),
                (output.is_identified_block.0)(&target).into(),
                ext::default_is_identified_block(&target).into(),
                ext::is_root_child_block(&target).into(),
            ]);
            Ok(())
        }
        Some("logBlockHandles") => {
            let handles = ext::get_block_handles()?;
            if let Some(name) = action.get("name").and_then(Value::as_str) {
                ctx.handles
                    .borrow_mut()
                    .insert(name.to_owned(), handles.clone());
            }
            let entries = handles
                .into_iter()
                .map(|(id, handle)| Value::Array(vec![id.into(), handle.into()]))
                .collect();
            ctx.log(vec!["blockHandles".into(), Value::Array(entries)]);
            Ok(())
        }
        Some("logBlockHandle") => {
            let key = action.get("key").and_then(Value::as_str).unwrap_or("");
            let handle = ext::get_block_handle(&node(key)?)?;
            ctx.log(vec![
                "blockHandle".into(),
                key.into(),
                handle.map_or(Value::Null, Value::from),
            ]);
            Ok(())
        }
        Some("resolveBlockHandle") => {
            let handle = action.get("handle").and_then(Value::as_str).unwrap_or("");
            let options = match action.get("read").and_then(Value::as_str) {
                Some(name) => Some(ext::BlockHandleOptions {
                    read: Some(
                        ctx.handles
                            .borrow()
                            .get(name)
                            .cloned()
                            .ok_or_else(|| err(format!("no handles kept as {name}")))?,
                    ),
                }),
                None => None,
            };
            let result = ext::resolve_block_handle(handle, options.as_ref())?;
            let detail = match &result {
                ext::BlockHandleResult::Found { node } => node.get_key().into(),
                ext::BlockHandleResult::Ambiguous { candidates, .. } => {
                    Value::Array(candidates.iter().map(|c| c.as_str().into()).collect())
                }
                ext::BlockHandleResult::Invalid { reason, .. } => reason.as_str().into(),
                ext::BlockHandleResult::NotFound { .. } => Value::Null,
            };
            ctx.log(vec![
                "resolved".into(),
                handle.into(),
                result.status().into(),
                detail,
            ]);
            Ok(())
        }
        Some("logSelectionText") => {
            let selection = get_selection().ok_or_else(|| err("no selection"))?;
            ctx.log(vec![
                "selectionText".into(),
                selection.get_text_content()?.into(),
            ]);
            Ok(())
        }
        Some("logRootTextContent") => {
            ctx.log(vec![
                "rootTextContent".into(),
                lexical::text::root_text_content().into(),
            ]);
            Ok(())
        }
        Some("logRootTextContentEmpty") => {
            let is_composing = action.get("isComposing").and_then(Value::as_bool) == Some(true);
            // `trim` absent is JS's `undefined`, so the default (`true`).
            let trim = action.get("trim").and_then(Value::as_bool);
            let curried =
                lexical::text::is_root_text_content_empty_curry(is_composing, trim.unwrap_or(true));
            ctx.log(vec![
                "rootTextContentEmpty".into(),
                is_composing.into(),
                trim.map_or(Value::Null, Value::from),
                lexical::text::is_root_text_content_empty(is_composing, trim.unwrap_or(true))
                    .into(),
                curried().into(),
            ]);
            Ok(())
        }
        Some("logCanShowPlaceholder") => {
            let is_composing = action.get("isComposing").and_then(Value::as_bool) == Some(true);
            let curried = lexical::text::can_show_placeholder_curry(is_composing);
            ctx.log(vec![
                "canShowPlaceholder".into(),
                is_composing.into(),
                lexical::text::can_show_placeholder(is_composing).into(),
                curried().into(),
            ]);
            Ok(())
        }
        Some("logTextIntersection") => {
            let characters = number(action.get("characters").unwrap_or(&Value::Null))?;
            let found = lexical::text::find_text_intersection_from_characters(
                &get_root(),
                characters as usize,
            );
            ctx.log(vec![
                "textIntersection".into(),
                characters.into(),
                found.map_or(Value::Null, |found| {
                    Value::Array(vec![
                        found.node.get_key().into(),
                        (found.offset as f64).into(),
                    ])
                }),
            ]);
            Ok(())
        }
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
        Some("setState") => {
            let key = action.get("key").and_then(Value::as_str).unwrap_or("");
            let name = action.get("state").and_then(Value::as_str).unwrap_or("");
            let value = action.get("value").unwrap_or(&Value::Null);
            with_state_config!(ctx.states, name, |config, _to_json, from_json| {
                let value = from_json(value)
                    .ok_or_else(|| err(format!("{value} is no value of {name}")))?;
                set_state(&node(key)?, config, value).map(|_| ())
            })
        }
        Some("updateState") => {
            let key = action.get("key").and_then(Value::as_str).unwrap_or("");
            let name = action.get("state").and_then(Value::as_str).unwrap_or("");
            let node = node(key)?;
            let states = &ctx.states;
            match (name, action.get("updater").and_then(Value::as_str)) {
                ("count", Some("count:increment")) => {
                    set_state_with(&node, &states.count, |v| v + 1.0)
                }
                ("count", Some("count:same")) => set_state_with(&node, &states.count, |v| *v),
                ("label", Some("label:exclaim")) => {
                    set_state_with(&node, &states.label, |v| format!("{v}!"))
                }
                ("words", Some("words:copy")) => set_state_with(&node, &states.words, Vec::clone),
                ("words", Some("words:push-x")) => set_state_with(&node, &states.words, |v| {
                    let mut words = v.clone();
                    words.push("x".to_owned());
                    words
                }),
                (name, updater) => Err(err(format!("unknown updater {updater:?} of {name}"))),
            }
            .map(|_| ())
        }
        Some("logState") => {
            let key = action.get("key").and_then(Value::as_str).unwrap_or("");
            let name = action.get("state").and_then(Value::as_str).unwrap_or("");
            with_state_config!(ctx.states, name, |config, to_json, _from_json| {
                let value = get_state(&node(key)?, config)?;
                ctx.log(vec![
                    "state".into(),
                    key.into(),
                    name.into(),
                    to_json(&value),
                ]);
                Ok(())
            })
        }
        Some("logStateChange") => {
            let key = action.get("key").and_then(Value::as_str).unwrap_or("");
            let name = action.get("state").and_then(Value::as_str).unwrap_or("");
            // The node's version in the committed editor state.
            let prev_node = editor
                .get_editor_state()
                .read(|| get_node_by_key(key).map(|n| n.get_latest()));
            with_state_config!(ctx.states, name, |config, to_json, _from_json| {
                let change = match &prev_node {
                    None => Value::from("new"),
                    Some(prev_node) => {
                        match get_state_change(&*node(key)?.get_latest(), &**prev_node, config)? {
                            None => Value::Null,
                            Some((value, prev_value)) => {
                                Value::Array(vec![to_json(&value), to_json(&prev_value)])
                            }
                        }
                    }
                };
                ctx.log(vec!["stateChange".into(), key.into(), name.into(), change]);
                Ok(())
            })
        }
        Some("setBlocksType") => {
            let create = create_block(action.get("block").unwrap_or(&Value::Null))?;
            set_blocks_type(get_selection().as_ref(), create, None)
        }
        Some(_) if run_selection_utils_action(action, ctx, editor)? => Ok(()),
        Some("wrapSelectionInMarkNode") => {
            let is_backward = flag(action.get("isBackward").unwrap_or(&Value::Null), false);
            let id = action.str("id").map_err(err)?;
            mark::wrap_selection_in_mark_node(&range_sel()?, is_backward, id, None)
        }
        Some("unwrapMarkNode") => {
            mark::unwrap_mark_node(&mark_node(node(action.str("key").map_err(err)?)?)?)
        }
        Some("logMarkIDs") => {
            let key = action.str("key").map_err(err)?;
            let offset = number(action.get("offset").unwrap_or(&Value::Null))?;
            let ids = mark::get_mark_ids(&text(node(key)?)?, offset as usize);
            ctx.log(vec![
                "markIDs".into(),
                key.into(),
                offset.into(),
                ids.as_deref().map_or(Value::Null, strings),
            ]);
            Ok(())
        }
        Some("addMarkID") => {
            let id = action.str("id").map_err(err)?;
            let selection = get_selection().ok_or_else(|| err("no selection"))?;
            let nodes = match extensions::mark_flavor_of(action)? {
                None => mark::add_mark_id(id, &selection)?,
                Some(flavor) => mark::add_mark_id_for(&flavor, id, &selection)?,
            };
            ctx.log(vec![
                "addMarkID".into(),
                id.into(),
                Value::Array(nodes.iter().map(|n| n.get_key().into()).collect()),
            ]);
            Ok(())
        }
        Some("removeMarkID") => {
            let id = action.str("id").map_err(err)?;
            let selection = if action.get("inSelection").and_then(Value::as_bool) == Some(true) {
                Some(get_selection().ok_or_else(|| err("no selection"))?)
            } else {
                None
            };
            match extensions::mark_flavor_of(action)? {
                None => mark::remove_mark_id(id, selection.as_ref()),
                Some(flavor) => mark::remove_mark_id_for(&flavor, id, selection.as_ref()),
            }
        }
        Some("logNodeMarkIDs") => {
            let key = action.str("key").map_err(err)?;
            let ids = match extensions::mark_flavor_of(action)? {
                None => mark::get_node_mark_ids(&node(key)?)?,
                Some(flavor) => mark::get_node_mark_ids_for(&flavor, &node(key)?)?,
            };
            ctx.log(vec!["nodeMarkIDs".into(), key.into(), strings(&ids)]);
            Ok(())
        }
        Some("logMarkIDsAtSelection") => {
            let mode = action.get("mode").and_then(Value::as_str);
            let mode_of = match mode {
                Some("all") => mark::MarkIDsMode::All,
                _ => mark::MarkIDsMode::Any,
            };
            let selection = get_selection();
            let ids = match extensions::mark_flavor_of(action)? {
                None => mark::get_mark_ids_at_selection(selection.as_ref(), mode_of)?,
                Some(flavor) => {
                    mark::get_mark_ids_at_selection_for(&flavor, selection.as_ref(), mode_of)?
                }
            };
            ctx.log(vec![
                "markIDsAtSelection".into(),
                mode.map_or(Value::Null, Value::from),
                strings(&ids),
            ]);
            Ok(())
        }
        Some("migrateMarkNodesToMarkState") => {
            let target = match action.get("key").and_then(Value::as_str) {
                Some(key) => Some(node(key)?),
                None => None,
            };
            let count = match extensions::mark_flavor_of(action)? {
                None => mark::migrate_mark_nodes_to_mark_state(target.as_ref())?,
                Some(flavor) => {
                    mark::migrate_mark_nodes_to_mark_state_for(&flavor, target.as_ref())?
                }
            };
            ctx.log(vec![
                "migrateMarkNodesToMarkState".into(),
                (count as f64).into(),
            ]);
            Ok(())
        }
        Some("logMarkIDsByFlavor") => {
            let key = action.str("key").map_err(err)?;
            let mut entries = mark::get_mark_ids_by_flavor(&node(key)?)?;
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            ctx.log(vec![
                "markIDsByFlavor".into(),
                key.into(),
                Value::Array(
                    entries
                        .iter()
                        .map(|(state_key, ids)| {
                            Value::Array(vec![state_key.as_str().into(), strings(ids)])
                        })
                        .collect(),
                ),
            ]);
            Ok(())
        }
        Some("copy") => {
            let data_transfer = Rc::new(DataTransfer::new());
            // `$clipboardDataWithoutHtml`: the default stacks less `text/html`.
            let mut data = clipboard::get_clipboard_data_from_selection(get_selection().as_ref())?;
            data.retain(|(mime, _)| mime != "text/html");
            clipboard::set_lexical_clipboard_data_transfer(&data_transfer, &data);
            let entries = data_transfer
                .types()
                .into_iter()
                .map(|t| {
                    let data = data_transfer.get_data(&t);
                    Value::Array(vec![t.into(), data.into()])
                })
                .collect();
            ctx.log(vec!["copy".into(), Value::Array(entries)]);
            *ctx.clipboard.borrow_mut() = Some(data_transfer);
            Ok(())
        }
        Some("pasteClipboard") => {
            let data_transfer = ctx
                .clipboard
                .borrow()
                .clone()
                .ok_or_else(|| err("nothing copied"))?;
            let plain = action.get("plain").and_then(Value::as_bool) == Some(true);
            insert_data_transfer(&data_transfer, plain, editor)
        }
        Some("pasteLexical") => {
            let mut payload = Object::new();
            payload.insert("namespace", editor.get_namespace().into());
            payload.insert("nodes", action.get("nodes").cloned().unwrap_or(Value::Null));
            let data_transfer = DataTransfer::new();
            data_transfer.set_data(
                clipboard::LEXICAL_MIME_TYPE,
                &Value::from(payload).to_string(),
            );
            insert_data_transfer(&data_transfer, false, editor)
        }
        Some("pastePlainText") => {
            let data_transfer = DataTransfer::new();
            data_transfer.set_data(
                "text/plain",
                &string(action.get("text").unwrap_or(&Value::Null))?,
            );
            insert_data_transfer(&data_transfer, true, editor)
        }
        Some("pasteData") => {
            let data_transfer = data_transfer_of(action.get("data").unwrap_or(&Value::Null))?;
            let plain = action.get("plain").and_then(Value::as_bool) == Some(true);
            insert_data_transfer(&data_transfer, plain, editor)
        }
        Some("generateJSON") => {
            let selection = if action.get("whole").and_then(Value::as_bool) == Some(true) {
                None
            } else {
                get_selection()
            };
            let json = clipboard::generate_json_from_selected_nodes(editor, selection.as_ref())?;
            // The log is JSON, which leaves out the `undefined` properties
            // of JS's object (an element's `textFormat` in the legacy form).
            let json = lexical::json::parse(&json.to_json().to_string())
                .map_err(|e| err(format!("{e:?}")))?;
            ctx.log(vec!["generateJSON".into(), json]);
            Ok(())
        }
        Some("insertGeneratedNodes") => {
            let serialized = action
                .get("nodes")
                .and_then(Value::as_array)
                .ok_or_else(|| err("expected an array of nodes"))?;
            let nodes = clipboard::generate_nodes_from_serialized_nodes(serialized)?;
            let selection = get_selection().ok_or_else(|| err("no selection"))?;
            clipboard::insert_generated_nodes(editor, nodes, &selection)
        }
        Some("selectTypeaheadOption") => {
            let spec = action.get("typeahead").cloned().unwrap_or(Value::Null);
            let (_, text_match) = typeahead_match(&spec, editor)?;
            let node = match &text_match {
                None => None,
                Some(text_match) => split_node_containing_query(text_match)?,
            };
            ctx.log(vec![
                "splitNodeContainingQuery".into(),
                node.as_ref().map_or(Value::Null, |n| n.get_key().into()),
                node.as_ref()
                    .map_or(Value::Null, |n| n.get_text_content().into()),
            ]);
            if let Some(node) = node {
                let replacement = lexical::create_text_node(&string(
                    action.get("replacement").unwrap_or(&Value::Null),
                )?);
                node.replace(&replacement, false)?;
                replacement.select(None, None)?;
            }
            Ok(())
        }
        Some("logFullMatchOffset") => {
            let document_text = action.str("documentText").map_err(err)?;
            let entry_text = action.str("entryText").map_err(err)?;
            let offset = number(action.get("offset").unwrap_or(&Value::Null))? as usize;
            ctx.log(vec![
                "fullMatchOffset".into(),
                document_text.into(),
                entry_text.into(),
                (offset as f64).into(),
                (get_full_match_offset(document_text, entry_text, offset) as f64).into(),
            ]);
            Ok(())
        }
        Some("logDecoratorBlock") => {
            let key = action.str("key").map_err(err)?;
            let n = node(key)?;
            let format = n.get_decorator_block_format();
            ctx.log(vec![
                "decoratorBlock".into(),
                key.into(),
                format.is_some().into(),
                n.is_inline().into(),
                format.map_or(Value::Null, Value::from),
                format.map_or(Value::Null, |_| n.can_indent().into()),
            ]);
            Ok(())
        }
        Some("logUrlMatcher") => {
            let text = action.str("text").map_err(err)?;
            ctx.log(vec![
                "urlMatcher".into(),
                text.into(),
                url_matcher(text).map_or(Value::Null, |(index, matched)| {
                    Value::Array(vec![(index as f64).into(), matched.into()])
                }),
            ]);
            Ok(())
        }
        Some("logSingleTokenPaste") => {
            let text = action.str("text").map_err(err)?;
            ctx.log(vec![
                "singleTokenPaste".into(),
                text.into(),
                is_single_token(text).into(),
            ]);
            Ok(())
        }
        Some("findOffset") => {
            let text = action.str("text").map_err(err)?;
            let strlen_name = action.get("strlen").and_then(Value::as_str);
            let strlen = strlen_of(strlen_name)?;
            let offsets = action
                .arr("maxCharacters")
                .map_err(err)?
                .iter()
                .map(|max| {
                    let max = number(max)? as i64;
                    Ok((lexical::overflow::find_offset(text, max, &*strlen) as f64).into())
                })
                .collect::<Result<Vec<Value>, LexicalError>>()?;
            ctx.log(vec![
                "findOffset".into(),
                text.into(),
                strlen_name.unwrap_or("utf16").into(),
                Value::Array(offsets),
            ]);
            Ok(())
        }
        Some("wrapOverflowedNodes") => {
            let offset = number(action.get("offset").unwrap_or(&Value::Null))?;
            lexical::overflow::wrap_overflowed_nodes(offset as usize)
        }
        Some("mergePrevious") => {
            let overflow_node = node(action.str("key").map_err(err)?)?;
            if !lexical::overflow::is_overflow_node(&overflow_node) {
                return Err(err(format!(
                    "{} is not an overflow node",
                    overflow_node.get_key()
                )));
            }
            lexical::overflow::merge_previous(&overflow_node)
        }
        Some("dispatch") => {
            let name = action.get("command").and_then(Value::as_str).unwrap_or("");
            let payload = action.get("payload").cloned().unwrap_or(Value::Null);
            let dispatched = ctx.command(name).dispatch(editor, payload)?;
            log_dispatched(|entry| ctx.log(entry), name, dispatched);
            Ok(())
        }
        other => Err(err(format!("unsupported action {other:?}"))),
    }
}

/// `$create(block)`: a function making a new paragraph, quote or heading.
fn create_block(block: &Value) -> Result<Box<dyn Fn() -> NodeRef>, LexicalError> {
    Ok(match block.as_str().unwrap_or("") {
        "paragraph" => Box::new(create_paragraph_node),
        "quote" => Box::new(|| create_quote_node(false)),
        tag => {
            let tag =
                HeadingTagType::parse(tag).ok_or_else(|| err(format!("unknown block {tag}")))?;
            Box::new(move || create_heading_node(tag))
        }
    })
}

/// A list of node types as `ops.ts` writes it.
fn types(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| t.as_str().map(str::to_owned))
        .collect()
}

fn keys(nodes: impl IntoIterator<Item = NodeRef>) -> Value {
    Value::Array(nodes.into_iter().map(|n| n.get_key().into()).collect())
}

/// The actions of `@lexical/selection`'s and `@lexical/utils`'s functions
/// (`scripts/selection-utils`): whether `action` is one, once it ran.
fn run_selection_utils_action(
    action: &Value,
    ctx: &Ctx,
    editor: &Editor,
) -> Result<bool, LexicalError> {
    let key = || action.str("key").map_err(err);
    let keyed = || node(action.str("key").map_err(err)?);
    let any = || get_selection().ok_or_else(|| err("no selection"));
    let parse_one = |v: Option<&Value>| -> Result<NodeRef, LexicalError> {
        parse_serialized_node(v.unwrap_or(&Value::Null))
    };
    match action.get("do").and_then(Value::as_str) {
        Some("patchStyleText") => {
            let patch = action
                .get("patch")
                .and_then(Value::as_object)
                .ok_or_else(|| err("expected a patch"))?;
            let entries: Vec<(&str, selection::StylePatchValue)> = patch
                .iter()
                .map(|(k, v)| {
                    let value = match v {
                        Value::Null => selection::StylePatchValue::Remove,
                        Value::String(value) => selection::StylePatchValue::Value(value.clone()),
                        _ => selection::StylePatchValue::Compute(Rc::new(|prev, target| {
                            let kind = match target {
                                selection::StylePatchTarget::Selection(_) => "selection",
                                selection::StylePatchTarget::Text(_) => "text",
                                selection::StylePatchTarget::Element(_) => "element",
                            };
                            format!("{}@{kind}", prev.unwrap_or("null"))
                        })),
                    };
                    (k, value)
                })
                .collect();
            selection::patch_style_text(&any()?, &entries)?;
        }
        Some("logSelectionStyleValue") => {
            let property = action.str("property").map_err(err)?;
            let default_value = action.get("defaultValue").and_then(Value::as_str);
            let value = selection::get_selection_style_value_for_property(
                &any()?,
                property,
                default_value.unwrap_or(""),
            )?;
            ctx.log(vec!["styleValue".into(), property.into(), value.into()]);
        }
        Some("logStyleObject") => {
            let css = action.str("css").map_err(err)?;
            let object = get_style_object_from_css(css);
            ctx.log(vec![
                "styleObject".into(),
                css.into(),
                Value::Array(
                    object
                        .iter()
                        .map(|(k, v)| Value::Array(vec![k.into(), v.into()]))
                        .collect(),
                ),
                selection::get_css_from_style_object(&object).into(),
            ]);
        }
        Some("logAtNodeEnd") => {
            let range = sel()?;
            ctx.log(vec![
                "atNodeEnd".into(),
                selection::is_at_node_end(&range.anchor)?.into(),
                selection::is_at_node_end(&range.focus)?.into(),
            ]);
        }
        Some("trimTextContentFromAnchor") => {
            let count = number(action.get("count").unwrap_or(&Value::Null))?;
            selection::trim_text_content_from_anchor(editor, &sel()?.anchor, count as usize)?;
        }
        Some("forEachSelectedTextNode") => {
            let mut visited = Vec::new();
            selection::for_each_selected_text_node(|text_node| {
                visited.push(Value::Array(vec![
                    text_node.get_key().into(),
                    text_node.get_text_content().into(),
                ]));
                Ok(())
            })?;
            ctx.log(vec![
                "forEachSelectedTextNode".into(),
                Value::Array(visited),
            ]);
        }
        Some("ensureForwardRangeSelection") => {
            selection::ensure_forward_range_selection(&range_sel()?)?;
        }
        Some("wrapNodes") => {
            let create = create_block(action.get("block").unwrap_or(&Value::Null))?;
            let wrapping = match action.get("wrapping") {
                Some(block) => Some(create_block(block)?()),
                None => None,
            };
            selection::wrap_nodes(&any()?, create, wrapping.as_ref())?;
        }
        Some("logShouldOverride") => {
            let is_backward = flag(action.get("isBackward").unwrap_or(&Value::Null), false);
            let range = sel()?;
            ctx.log(vec![
                "shouldOverride".into(),
                is_backward.into(),
                selection::should_override_default_character_selection(&range, is_backward)?.into(),
                selection::is_parent_element_rtl(&range).into(),
            ]);
        }
        Some("moveCharacter") => {
            selection::move_character(
                &range_sel()?,
                flag(action.get("shift").unwrap_or(&Value::Null), false),
                flag(action.get("isBackward").unwrap_or(&Value::Null), false),
            )?;
        }
        Some("logSlice") => {
            let sliced = selection::slice_selected_text_node_content(&any()?, &text(keyed()?)?)?;
            ctx.log(vec![
                "slice".into(),
                key()?.into(),
                sliced.get_text_content().into(),
            ]);
        }
        Some("copyBlockFormatIndent") => {
            selection::copy_block_format_indent(
                &element(node(action.str("from").map_err(err)?)?)?,
                &element(node(action.str("to").map_err(err)?)?)?,
            )?;
        }
        Some("logCloneWithProperties") => {
            let n = keyed()?;
            if n.is_text() {
                #[allow(deprecated)]
                selection::add_node_style(&n);
            }
            let mut json = export_node_json(&*clone_with_properties(&n));
            // `exportJSON()` of an element has its own (empty) children.
            if n.is_element() {
                json.insert("children", Value::Array(Vec::new()));
            }
            ctx.log(vec!["clone".into(), key()?.into(), json.into()]);
        }
        Some("logDfs") => {
            let start = action
                .get("start")
                .and_then(Value::as_str)
                .map(node)
                .transpose()?;
            let end = action
                .get("end")
                .and_then(Value::as_str)
                .map(node)
                .transpose()?;
            let reverse = flag(action.get("reverse").unwrap_or(&Value::Null), false);
            let nodes = if reverse {
                utils::reverse_dfs(start.as_ref(), end.as_ref())
            } else {
                utils::dfs(start.as_ref(), end.as_ref())
            };
            ctx.log(vec![
                "dfs".into(),
                reverse.into(),
                Value::Array(
                    nodes
                        .into_iter()
                        .map(|d| {
                            Value::Array(vec![d.node.get_key().into(), (d.depth as f64).into()])
                        })
                        .collect(),
                ),
            ]);
        }
        Some("logDepth") => {
            let depth = utils::get_depth(Some(&keyed()?));
            ctx.log(vec!["depth".into(), key()?.into(), (depth as f64).into()]);
        }
        Some("logNextSiblingOrParentSibling") => {
            let found = utils::get_next_sibling_or_parent_sibling(&keyed()?);
            ctx.log(vec![
                "nextSiblingOrParentSibling".into(),
                key()?.into(),
                found.map_or(Value::Null, |(n, d)| {
                    Value::Array(vec![n.get_key().into(), (d as f64).into()])
                }),
            ]);
        }
        Some("logNextRightPreorderNode") => {
            let found = utils::get_next_right_preorder_node(&keyed()?);
            ctx.log(vec![
                "nextRightPreorderNode".into(),
                key()?.into(),
                found.map_or(Value::Null, |n| n.get_key().into()),
            ]);
        }
        Some("logNearestBlockElementAncestor") => {
            let found = utils::get_nearest_block_element_ancestor_or_throw(&keyed()?)?;
            ctx.log(vec![
                "nearestBlockElementAncestor".into(),
                key()?.into(),
                found.get_key().into(),
            ]);
        }
        Some("logChildren") => {
            let parent = element(keyed()?)?;
            let reverse = flag(action.get("reverse").unwrap_or(&Value::Null), false);
            let children = if reverse {
                keys(utils::last_to_first_iterator(&parent))
            } else {
                keys(utils::first_to_last_iterator(&parent))
            };
            ctx.log(vec![
                "children".into(),
                key()?.into(),
                reverse.into(),
                children,
            ]);
        }
        Some("unwrapAndFilterDescendants") => {
            let keep = types(action.get("keep"));
            let did_mutate = utils::unwrap_and_filter_descendants(&element(keyed()?)?, |n| {
                keep.iter().any(|t| t == n.get_type())
            })?;
            ctx.log(vec![
                "unwrapAndFilterDescendants".into(),
                key()?.into(),
                did_mutate.into(),
            ]);
        }
        Some("logDescendantsMatching") => {
            let types = types(action.get("types"));
            let found = utils::descendants_matching(&element(keyed()?)?.get_children(), |n| {
                types.iter().any(|t| t == n.get_type())
            });
            ctx.log(vec![
                "descendantsMatching".into(),
                key()?.into(),
                keys(found),
            ]);
        }
        Some("insertFirst") => {
            utils::insert_first(&element(keyed()?)?, &parse_one(action.get("node"))?)?;
        }
        Some("insertNodeToNearestRoot") => {
            let inserted = utils::insert_node_to_nearest_root(&parse_one(action.get("node"))?)?;
            ctx.log(vec![
                "insertNodeToNearestRoot".into(),
                inserted.get_key().into(),
            ]);
        }
        Some("insertNodeIntoLeaf") => {
            utils::insert_node_into_leaf(&parse_one(action.get("node"))?)?;
        }
        Some("splitNode") => {
            let offset = number(action.get("offset").unwrap_or(&Value::Null))?;
            let (left, right) = split_node(&element(keyed()?)?, offset as usize)?;
            ctx.log(vec![
                "splitNode".into(),
                left.map_or(Value::Null, |n| n.get_key().into()),
                right.get_key().into(),
            ]);
        }
        Some("wrapNodeInElement") => {
            let create = create_block(action.get("block").unwrap_or(&Value::Null))?;
            let wrapped = utils::wrap_node_in_element(&keyed()?, create)?;
            ctx.log(vec!["wrapNodeInElement".into(), wrapped.get_key().into()]);
        }
        Some("unwrapNode") => utils::unwrap_node(&element(keyed()?)?)?,
        Some("saveEditorState") => {
            let name = action.str("name").map_err(err)?;
            ctx.saved
                .borrow_mut()
                .insert(name.to_owned(), editor.get_editor_state());
        }
        Some("restoreEditorState") => {
            let name = action.str("name").map_err(err)?;
            let state = ctx
                .saved
                .borrow()
                .get(name)
                .cloned()
                .ok_or_else(|| err(format!("no saved state {name}")))?;
            utils::restore_editor_state(editor, &state);
        }
        Some("escape") => {
            let types = types(action.get("types"));
            let is_container =
                |n: &NodeRef| n.is_element() && types.iter().any(|t| t == n.get_type());
            let direction = action.str("direction").map_err(err)?;
            let result = if direction == "up" {
                utils::on_escape_up(is_container, None)?
            } else {
                utils::on_escape_down(is_container, None)?
            };
            ctx.log(vec!["escape".into(), direction.into(), result.into()]);
        }
        Some("logBlockFullySelected") => {
            let result = is_block_fully_selected(&element(keyed()?)?, &sel()?)?;
            ctx.log(vec![
                "blockFullySelected".into(),
                key()?.into(),
                result.into(),
            ]);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `$selectionSnapshot()`.
pub(crate) fn selection_snapshot() -> Value {
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
    let session_ctx = ctx.clone();
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
        "mark:nested" => {
            let ctx = session_ctx;
            utils::register_nested_element_resolver(
                editor,
                "mark",
                |from| mark::create_mark_node(&from.get_ids().unwrap_or_default()),
                move |from, to| {
                    ctx.log(vec![
                        "nestedOverlap".into(),
                        from.get_key().into(),
                        to.get_key().into(),
                    ]);
                    for id in from.get_ids().unwrap_or_default() {
                        to.add_id(&id);
                    }
                    Ok(())
                },
            )
        }
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
        "mark:nested",
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

/// `ops.ts`'s `NAMESPACE`: every session's editor namespace, which a
/// clipboard payload names.
const NAMESPACE: &str = "conformance";

/// `ScriptDataTransfer.of(data)`: a `DataTransfer` with `data` set in key
/// order.
fn data_transfer_of(data: &Value) -> Result<DataTransfer, LexicalError> {
    let data_transfer = DataTransfer::new();
    for (format, value) in data
        .as_object()
        .ok_or_else(|| err(format!("expected an object, got {data}")))?
        .iter()
    {
        data_transfer.set_data(format, &string(value)?);
    }
    Ok(data_transfer)
}

/// `$insertDataTransfer(dataTransfer, plain)` in ops.ts.
fn insert_data_transfer(
    data_transfer: &DataTransfer,
    plain: bool,
    editor: &Editor,
) -> Result<(), LexicalError> {
    let selection = get_selection().ok_or_else(|| err("no selection"))?;
    if plain {
        clipboard::insert_data_transfer_for_plain_text(data_transfer, &selection)
    } else {
        clipboard::insert_data_transfer_for_rich_text(data_transfer, &selection, editor)
    }
}

/// A script's editor, set up with its transforms, command handlers and
/// listeners, that runs steps one at a time (`ops.ts`'s `Session`). The
/// conformance suite replays the vectors with it; the playground drives it
/// step by step alongside JS Lexical.
pub struct Session {
    editor: Editor,
    /// The editor built from `extensions`, when the script has them.
    built: Option<lexical::extension::LexicalEditorWithDispose>,
    ctx: Rc<Ctx>,
    /// The session's clock, in milliseconds: what the history merges by.
    /// It starts at 0, and a step's `now` sets it.
    clock: Rc<Cell<f64>>,
    /// Unregisters the history, whose entries hold the editor.
    unregister_history: Option<Unregister>,
    /// `script.liveRegion`.
    live_region: bool,
    /// Whether the script has `codeHighlight`.
    code_highlight: bool,
    /// What `registerCodeHighlighting` returned, until the step
    /// `unregisterCodeHighlighting` calls it.
    unregister_code_highlighting: RefCell<Option<Unregister>>,
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

/// `snapshotOf(editor, log, snapshot)`: the log, the selection and the
/// text, and with `snapshot` the state in both forms, in that key order.
fn snapshot_of(editor: &Editor, ctx: &Ctx, snapshot: bool) -> Result<Object, String> {
    // A dispatch outside an update commits at the next read, whose
    // listeners log too.
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
    Ok(json)
}

/// The package flags a script with `extensions` can't have.
const PACKAGE_FLAGS: [&str; 10] = [
    "richText",
    "plainText",
    "list",
    "link",
    "code",
    "overflow",
    "hashtag",
    "extension",
    "mark",
    "decoratorBlock",
];

impl Session {
    /// `startSession(script)`: a new headless editor, with keys restarted.
    pub fn start(script: &Value) -> Result<Session, String> {
        Self::start_with_build(script)?
            .0
            .ok_or_else(|| "the editor failed to build".to_owned())
    }

    /// `startSession(script)`, with what building the editor from the
    /// script's `extensions` recorded (`session.build`). When that build
    /// fails there is no session.
    pub fn start_with_build(script: &Value) -> Result<(Option<Session>, Option<Value>), String> {
        reset_random_key();
        if let Some(extensions) = script.get("extensions").and_then(Value::as_array) {
            if let Some(flag) = PACKAGE_FLAGS
                .iter()
                .find(|f| script.get(f).and_then(Value::as_bool) == Some(true))
            {
                return Err(format!("extensions and {flag} don't mix"));
            }
            let ctx = Rc::new(Ctx::default());
            if let Some(defs) = script.get("extensionDefs").and_then(Value::as_object) {
                *ctx.extension_defs.borrow_mut() = defs.clone();
            }
            let parent_read = script.get("parentRead").and_then(Value::as_bool) != Some(false);
            let built = match script.get("parent").and_then(Value::as_array) {
                None => extensions::build(&ctx, extensions),
                Some(parent_extensions) => {
                    extensions::build(&ctx, parent_extensions).and_then(|parent| {
                        *ctx.parent.borrow_mut() = Some(parent.clone());
                        if parent_read {
                            parent.read(|| extensions::build(&ctx, extensions))?
                        } else {
                            extensions::build(&ctx, extensions)
                        }
                    })
                }
            };
            return match built {
                Err(e) => {
                    let mut build = Object::new();
                    build.insert("error", e.message().into());
                    build.insert("log", Value::Array(ctx.log.borrow().clone()));
                    Ok((None, Some(Value::Object(build))))
                }
                Ok(built) => {
                    if script.get("liveRegion").and_then(Value::as_bool) == Some(true) {
                        let entry = ctx.live_region_entry();
                        ctx.log.borrow_mut().push(entry);
                    }
                    let parent = ctx.parent.borrow().clone();
                    if let Some(parent) = parent {
                        let entry = nested_entry(&built, &parent)?;
                        ctx.log.borrow_mut().push(entry);
                    }
                    let build = snapshot_of(&built, &ctx, true)?;
                    let mut session = Self::start_on(script, built.editor().clone(), ctx)?;
                    session.built = Some(built);
                    Ok((Some(session), Some(Value::Object(build))))
                }
            };
        }
        Ok((Some(Self::start_plain(script)?), None))
    }

    /// `startSession(script)` on `createEditor`.
    fn start_plain(script: &Value) -> Result<Session, String> {
        let rich_text = script.get("richText").and_then(Value::as_bool) == Some(true);
        let with_list = script.get("list").and_then(Value::as_bool) == Some(true);
        let with_link = script.get("link").and_then(Value::as_bool) == Some(true);
        let with_code = script.get("code").and_then(Value::as_bool) == Some(true);
        let with_overflow = script.get("overflow").and_then(Value::as_bool) == Some(true);
        let with_hashtag = script.get("hashtag").and_then(Value::as_bool) == Some(true);
        let with_extension = script.get("extension").and_then(Value::as_bool) == Some(true);
        let with_mark = script.get("mark").and_then(Value::as_bool) == Some(true);
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
        if with_code {
            nodes.extend(lexical::code::code_nodes());
        }
        if with_overflow {
            nodes.extend(lexical::overflow::overflow_nodes());
        }
        if with_hashtag {
            nodes.extend(lexical::hashtag::hashtag_nodes());
        }
        if with_extension {
            nodes.extend(lexical::extension::extension_nodes());
        }
        if with_mark {
            nodes.extend(mark::mark_nodes());
        }
        if script.get("decoratorBlock").and_then(Value::as_bool) == Some(true) {
            nodes.extend(test_decorator_block_nodes());
        }
        let editor = create_editor(CreateEditorArgs {
            nodes,
            namespace: NAMESPACE.to_owned(),
            ..Default::default()
        });
        let ctx = Rc::new(Ctx::default());
        Self::start_on(script, editor, ctx)
    }

    /// Registers the script's packages, transforms, handlers, listeners and
    /// history on `editor`.
    fn start_on(script: &Value, editor: Editor, ctx: Rc<Ctx>) -> Result<Session, String> {
        let flag = |key: &str| script.get(key).and_then(Value::as_bool) == Some(true);
        let (rich_text, with_list, with_link, with_code, with_hashtag) = (
            flag("richText"),
            flag("list"),
            flag("link"),
            flag("code"),
            flag("hashtag"),
        );
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
            if script.get("autoLink").and_then(Value::as_bool) == Some(true) {
                let log = ctx.clone();
                let on_change: link::ChangeHandler = Rc::new(move |url, prev_url| {
                    let or_null = |url: Option<&str>| url.map_or(Value::Null, Value::from);
                    log.log(vec!["autoLink".into(), or_null(url), or_null(prev_url)]);
                });
                let _unregister = link::register_auto_link(
                    &editor,
                    link::AutoLinkConfig {
                        change_handlers: vec![on_change],
                        matchers: vec![
                            Rc::new(link::auto_link_url_matcher),
                            Rc::new(link::auto_link_email_matcher),
                        ],
                        ..link::AutoLinkConfig::default()
                    },
                );
            }
        }
        let code_highlight = script.get("codeHighlight");
        let mut unregister_code_highlighting = None;
        if with_code {
            let _unregister = lexical::code::register_code(&editor);
            match code_highlight {
                None => {
                    let _unregister =
                        lexical::code::register_code_indentation(&editor, None, false);
                }
                Some(spec) => {
                    unregister_code_highlighting = Some(
                        lexical::code::register_code_highlighting(
                            &editor,
                            code_highlight::tokenizer(spec)?,
                        )
                        .map_err(|e| e.message().to_owned())?,
                    );
                }
            }
        }
        if with_hashtag {
            let _unregister = lexical::hashtag::register_lexical_hashtag(
                &editor,
                lexical::hashtag::HashtagConfig::default(),
            );
        }
        if script.get("markdown").and_then(Value::as_bool) == Some(true) {
            let _unregister = lexical::markdown::register_markdown_shortcuts(
                &editor,
                &lexical::markdown::transformers(),
            )
            .map_err(|e| e.to_string())?;
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
        for node_type in script
            .get("mutations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let node_type = node_type.as_str().unwrap_or("").to_owned();
            let log = ctx.clone();
            let _unregister = editor
                .register_mutation_listener(
                    &node_type.clone(),
                    move |mutations, payload| {
                        let mut tags = payload.update_tags.clone();
                        tags.sort();
                        log.log(vec![
                            "mutation".into(),
                            node_type.as_str().into(),
                            Value::Array(
                                mutations
                                    .iter()
                                    .map(|(key, mutation)| {
                                        Value::Array(vec![
                                            key.as_str().into(),
                                            mutation.as_str().into(),
                                        ])
                                    })
                                    .collect(),
                            ),
                            Value::Array(tags.into_iter().map(Value::from).collect()),
                        ]);
                    },
                    MutationListenerOptions {
                        skip_initialization: true,
                    },
                )
                .map_err(|e| e.message().to_owned())?;
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
            // Mutation listeners only hear from the reconciler, which runs
            // with a root element (`setRootElement`).
            for node_type in ["text", "paragraph"] {
                let log = ctx.clone();
                let _unregister = editor
                    .register_mutation_listener(
                        node_type,
                        move |mutations, _| {
                            log.log(vec![
                                "mutation".into(),
                                node_type.into(),
                                Value::Array(
                                    mutations
                                        .iter()
                                        .map(|(key, mutation)| {
                                            Value::Array(vec![
                                                key.as_str().into(),
                                                mutation.as_str().into(),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ]);
                        },
                        MutationListenerOptions {
                            skip_initialization: true,
                        },
                    )
                    .map_err(|e| e.to_string())?;
            }
            let log = ctx.clone();
            let _unregister = editor.register_text_content_listener(move |text| {
                log.log(vec!["textContent".into(), text.into()]);
            });
        }
        // `registerHistory(editor, createEmptyHistoryState(), delay)` on the
        // session's clock.
        let clock = ctx.clock.clone();
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
            built: None,
            ctx,
            clock,
            unregister_history,
            live_region: flag("liveRegion"),
            code_highlight: code_highlight.is_some(),
            unregister_code_highlighting: RefCell::new(unregister_code_highlighting),
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
        let ctx = &self.ctx;
        if let Some(now) = step.get("now").and_then(Value::as_f64) {
            self.clock.set(now);
        }
        ctx.log.borrow_mut().clear();
        let parent = ctx.parent.borrow().clone();
        let outcome = if step.get("inParent").and_then(Value::as_bool) == Some(true) {
            match &parent {
                Some(parent) => self.run_on(parent, Some(parent), step),
                None => Err(err("the script has no parent editor")),
            }
        } else {
            self.run_on(&self.editor, self.built.as_ref(), step)
        };
        let editor = &self.editor;
        if self.live_region {
            let entry = ctx.live_region_entry();
            ctx.log.borrow_mut().push(entry);
        }
        if self.code_highlight {
            let entry = code_highlight::code_blocks_entry(editor)?;
            ctx.log.borrow_mut().push(entry);
        }
        if let Some(parent) = &parent {
            let entry = nested_entry(editor, parent)?;
            ctx.log.borrow_mut().push(entry);
        }
        let mut json = snapshot_of(editor, ctx, snapshot)?;
        let error = outcome.err();
        if error.is_some() {
            json.insert("error", true.into());
        }
        Ok(StepResult {
            json: Value::Object(json),
            error,
        })
    }

    /// Runs `step` on `editor`: the session's, or (`inParent`) the parent,
    /// built as `built`.
    fn run_on(
        &self,
        editor: &Editor,
        built: Option<&lexical::extension::LexicalEditorWithDispose>,
        step: &Value,
    ) -> Result<(), LexicalError> {
        let ctx = &self.ctx;
        if let Some(actions) = step.get("update").and_then(Value::as_array) {
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
        } else if let Some(name) = step.get("dispatch").and_then(Value::as_str) {
            let payload = step.get("payload").cloned().unwrap_or(Value::Null);
            ctx.command(name)
                .dispatch(editor, payload)
                .map(|dispatched| log_dispatched(|entry| ctx.log(entry), name, dispatched))
        } else if let Some(on) = step.get("setRootElement").and_then(Value::as_bool) {
            let root = ctx.session_dom();
            // A JS editor with a root element has its jsdom window, which
            // gives no DOM selection: `Selection.modify` stops after the
            // model step, where a headless editor throws.
            editor.set_selection_movement(on.then(|| {
                Rc::new(crate::suites::property::WindowWithoutSelection)
                    as Rc<dyn lexical::SelectionMovement>
            }));
            editor.set_root_element(on.then(|| RootElementRef(root)))
        } else if let Some(styles) = step.get("computedStyles").and_then(Value::as_object) {
            // `stubComputedStyles`: `direction` and `writingMode`, CSS's
            // initial values when left out.
            let default = selection::ComputedStyle::default();
            *ctx.session_dom().computed_styles.borrow_mut() = styles
                .iter()
                .map(|(key, style)| {
                    let field = |name: &str, initial: &str| {
                        style
                            .get(name)
                            .and_then(Value::as_str)
                            .unwrap_or(initial)
                            .to_owned()
                    };
                    let computed = selection::ComputedStyle {
                        direction: field("direction", &default.direction),
                        writing_mode: field("writingMode", &default.writing_mode),
                    };
                    (key.to_owned(), computed)
                })
                .collect();
            Ok(())
        } else if let Some(editable) = step.get("setEditable").and_then(Value::as_bool) {
            editor.set_editable(editable);
            Ok(())
        } else if let Some(text) = step.get("announce").and_then(Value::as_str) {
            get_extension_dependency_from_editor(editor, &aria_live_region_extension()).and_then(
                |dep| {
                    dep.output
                        .ok_or_else(|| err("no output"))
                        .map(|announcer| announcer.announce(text))
                },
            )
        } else if step.get("unregisterCodeHighlighting").is_some() {
            match self.unregister_code_highlighting.borrow_mut().take() {
                Some(unregister) => {
                    unregister();
                    Ok(())
                }
                None => Err(err("no code highlighting to unregister")),
            }
        } else if let Some(spec) = step.get("typeahead") {
            editor.get_editor_state().read(|| {
                let (text, text_match) = typeahead_match(spec, editor)?;
                let on_entity_boundary = text_match
                    .as_ref()
                    .map(|m| is_selection_on_entity_boundary(editor, m.lead_offset));
                ctx.log(vec![
                    "typeahead".into(),
                    text.map_or(Value::Null, Value::from),
                    text_match.as_ref().map_or(Value::Null, text_match_json),
                    on_entity_boundary.map_or(Value::Null, Value::from),
                ]);
                Ok(())
            })
        } else if step.get("characterLimit").is_some() || step.get("tableOfContents").is_some() {
            mount_step(step, ctx, editor)
        } else if let Some(name) = step.get("registerNodeStructure").and_then(Value::as_str) {
            let unregister = lexical::extension::register_node_structure(editor);
            ctx.structures
                .borrow_mut()
                .insert(name.to_owned(), unregister);
            Ok(())
        } else if let Some(name) = step.get("disposeNodeStructure").and_then(Value::as_str) {
            let dispose = ctx.structures.borrow_mut().remove(name);
            match dispose {
                None => Err(err(format!("no registration {name}"))),
                Some(dispose) => {
                    dispose();
                    Ok(())
                }
            }
        } else if let Some(name) = step.get("unmount").and_then(Value::as_str) {
            let unmount = ctx.mounted.borrow_mut().remove(name);
            match unmount {
                None => Err(err(format!("nothing mounted as {name}"))),
                Some(unmount) => {
                    unmount();
                    Ok(())
                }
            }
        } else {
            extensions::run_step(step, ctx, built)
        }
    }
}

/// `nestedEntry(editor, parent)`: `['nested', isNested, parentIsParent,
/// sameNamespace, theme, editable, parentEditable, parentText,
/// parentSelection]`, the theme as its `[key, value]` entries sorted by key
/// (UTF-16 order, as JS's `sort()`), the parent read with `parent.read`.
fn nested_entry(editor: &Editor, parent: &Editor) -> Result<Value, String> {
    let (parent_text, parent_selection) = parent
        .read(|| (get_root().get_text_content(), selection_snapshot()))
        .map_err(|e| format!("committing the parent: {e}"))?;
    let mut theme: Vec<(&str, &Value)> = editor.config().theme.iter().collect();
    theme.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
    Ok(Value::Array(vec![
        "nested".into(),
        utils::is_editor_is_nested_editor(editor).into(),
        (editor.parent_editor() == Some(parent)).into(),
        (editor.get_namespace() == parent.get_namespace()).into(),
        Value::Array(
            theme
                .into_iter()
                .map(|(k, v)| Value::Array(vec![k.into(), v.clone()]))
                .collect(),
        ),
        editor.is_editable().into(),
        parent.is_editable().into(),
        parent_text.into(),
        parent_selection,
    ]))
}

/// `scripts/*.json`: each step's result matches what JS recorded, on the
/// platform the script's `isApple` names (not Apple's when left out).
pub fn check(case: &Value, file: &Value) -> Result<Outcome, String> {
    let is_apple = case.get("isApple").and_then(Value::as_bool) == Some(true);
    with_is_apple(is_apple, || check_case(case, file))
}

fn check_case(case: &Value, _file: &Value) -> Result<Outcome, String> {
    if let Some(part) = unported(case) {
        return Ok(Outcome::NotPorted(part));
    }
    let (session, build) = Session::start_with_build(case)?;
    same(
        "build recorded",
        build.is_some(),
        case.get("build").is_some(),
    )?;
    if let (Some(build), Some(expected)) = (&build, case.get("build")) {
        for field in ["log", "selection", "text", "state", "compact", "error"] {
            same(
                format!("build {field}"),
                build.get(field),
                expected.get(field),
            )?;
        }
    }
    let steps = case.arr("steps")?;
    let Some(session) = session else {
        same("steps after a failed build", steps.len(), 0)?;
        return Ok(Outcome::Pass);
    };
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
