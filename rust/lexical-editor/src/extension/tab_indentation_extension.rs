//! `TabIndentationExtension.ts`: `registerTabIndentation` and
//! `TabIndentationExtension`.

use super::named_signals::named_signals_config;
use super::signals::{ReadonlySignal, effect};
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::lexical_selection::{PointType, SelectionRef, Which, create_range_selection};
use crate::merge_register::merge_register;
use crate::selection_insert_nodes::normalize_selection;
use crate::utils::{get_nearest_block_element_ancestor_or_throw, handle_indent_and_outdent};
use crate::{
    COMMAND_PRIORITY_CRITICAL, COMMAND_PRIORITY_EDITOR, Editor, INDENT_CONTENT_COMMAND,
    INSERT_TAB_COMMAND, KEY_TAB_COMMAND, LexicalError, NodeRef, OUTDENT_CONTENT_COMMAND,
    Unregister, get_selection,
};

/// `$indentOverTab(selection)`: whether Tab indents rather than inserts a
/// tab: when the selection spans a block that can indent, or starts at
/// the start of one.
fn indent_over_tab(selection: &SelectionRef) -> Result<bool, LexicalError> {
    let nodes = selection.get_nodes()?;
    // 1. If selection spans across canIndent block nodes: indent
    if nodes
        .iter()
        .any(|node| node.is_element() && !node.is_inline() && node.can_indent())
    {
        return Ok(true);
    }
    // 2. If first (anchor/focus) is at block start: indent
    let range = selection.range().expect("a range selection");
    let (anchor, focus) = (&range.anchor, &range.focus);
    let first = if focus.is_before(anchor)? {
        focus
    } else {
        anchor
    };
    let first_block = get_nearest_block_element_ancestor_or_throw(&first.get_node()?)?;
    if first_block.can_indent() {
        let first_block_key = first_block.get_key();
        let selection_at_start = create_range_selection();
        selection_at_start.point_set(Which::Anchor, first_block_key, 0, PointType::Element)?;
        selection_at_start.point_set(Which::Focus, first_block_key, 0, PointType::Element)?;
        normalize_selection(&selection_at_start)?;
        if selection_at_start.point(Which::Anchor).as_ref() == Some(first) {
            return Ok(true);
        }
    }
    // 3. Else: tab
    Ok(false)
}
use std::rc::Rc;

/// `CanIndentPredicate`: whether a block may be indented, compared by
/// identity as JS compares functions.
#[derive(Clone)]
pub struct CanIndentPredicate(pub Rc<dyn Fn(&NodeRef) -> bool>);

impl PartialEq for CanIndentPredicate {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// `$defaultCanIndent(node)`: `node.canIndent()`.
pub fn default_can_indent(node: &NodeRef) -> bool {
    node.can_indent()
}

/// `registerTabIndentation(editor, maxIndent, $canIndent)`: Tab and
/// Shift+Tab (`KEY_TAB_COMMAND`, at `COMMAND_PRIORITY_EDITOR`) at a range
/// selection dispatch `INDENT_CONTENT_COMMAND` or `OUTDENT_CONTENT_COMMAND`
/// where `$indentOverTab` says so, and `INSERT_TAB_COMMAND` otherwise; and
/// `INDENT_CONTENT_COMMAND`, at `COMMAND_PRIORITY_CRITICAL`, indents each
/// selected block `$canIndent` accepts, by one, while that stays below
/// `maxIndent` (`None`, or 0 as JS's `!currentMaxIndent` reads it, is no
/// maximum). Both are read when the command is handled, as JS `peek`s
/// signals. Returns what unregisters it.
pub fn register_tab_indentation(
    editor: &Editor,
    max_indent: ReadonlySignal<Option<f64>>,
    can_indent: ReadonlySignal<CanIndentPredicate>,
) -> Unregister {
    let unregister_tab = editor.register_command(
        &KEY_TAB_COMMAND,
        |event, editor| {
            let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
                return Ok(false);
            };
            event.prevent_default();
            let command = if indent_over_tab(&selection)? {
                if event.shift_key {
                    OUTDENT_CONTENT_COMMAND
                } else {
                    INDENT_CONTENT_COMMAND
                }
            } else {
                INSERT_TAB_COMMAND
            };
            editor.dispatch_command(&command, ())
        },
        COMMAND_PRIORITY_EDITOR,
    );
    let unregister_indent = editor.register_command(
        &INDENT_CONTENT_COMMAND,
        move |_, _| {
            let current_max_indent = max_indent.peek();
            if !get_selection().is_some_and(|s| s.is_range()) {
                return Ok(false);
            }
            let current_can_indent = can_indent.peek();
            handle_indent_and_outdent(|block| {
                if (current_can_indent.0)(block) {
                    let new_indent = block.get_indent()? + 1.0;
                    let no_max = current_max_indent.is_none_or(|m| m == 0.0 || m.is_nan());
                    if no_max || current_max_indent.is_some_and(|m| new_indent < m) {
                        block.set_indent(new_indent)?;
                    }
                }
                Ok(())
            })
        },
        COMMAND_PRIORITY_CRITICAL,
    );
    merge_register(vec![unregister_tab, unregister_indent])
}

named_signals_config! {
    /// `TabIndentationConfig`.
    pub struct TabIndentationConfig;
    /// `Partial<TabIndentationConfig>`.
    pub struct TabIndentationConfigOverrides;
    /// `namedSignals(TabIndentationConfig)`: the extension's output.
    pub struct TabIndentationOutput {
        /// `$canIndent`: which blocks may be indented,
        /// [`default_can_indent`] by default.
        can_indent: CanIndentPredicate,
        /// `disabled`.
        disabled: bool,
        /// `maxIndent`: indentation stops below it; `None` by default.
        max_indent: Option<f64>,
    }
}

/// `TabIndentationExtension`.
pub type TabIndentationExtensionType =
    LexicalExtension<TabIndentationConfig, TabIndentationOutput, ()>;

fn make() -> Rc<TabIndentationExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &TabIndentationConfig, _: &ExtensionBuildState<()>| {
                Ok(TabIndentationOutput::of(config))
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &TabIndentationConfig,
             state: &ExtensionRegisterState<(), TabIndentationOutput>| {
                let stores = state
                    .get_output()
                    .expect("TabIndentationExtension has a build");
                let editor = editor.weak();
                Ok(effect(move || {
                    if stores.disabled.value() {
                        return None;
                    }
                    let editor = editor.upgrade().map(Editor)?;
                    Some(register_tab_indentation(
                        &editor,
                        stores.max_indent.as_readonly(),
                        stores.can_indent.as_readonly(),
                    ))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/extension/TabIndentation",
            TabIndentationConfig {
                can_indent: CanIndentPredicate(Rc::new(default_can_indent)),
                disabled: false,
                max_indent: None,
            },
        )
    })
}

thread_local! {
    static TAB_INDENTATION_EXTENSION: Rc<TabIndentationExtensionType> = make();
}

/// `TabIndentationExtension`: [`register_tab_indentation`] with its config
/// as signals, unless `disabled`.
pub fn tab_indentation_extension() -> Rc<TabIndentationExtensionType> {
    TAB_INDENTATION_EXTENSION.with(Rc::clone)
}
