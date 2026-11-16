//! `TabIndentationExtension.ts`: `registerTabIndentation`'s model handler
//! and `TabIndentationExtension`.

use super::named_signals::named_signals_config;
use super::signals::{ReadonlySignal, effect};
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::utils::handle_indent_and_outdent;
use crate::{
    COMMAND_PRIORITY_CRITICAL, Editor, INDENT_CONTENT_COMMAND, NodeRef, Unregister, get_selection,
};
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

/// `registerTabIndentation(editor, maxIndent, $canIndent)`'s model part:
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
    // KEY_TAB_COMMAND: Tab and Shift+Tab dispatch INDENT_CONTENT_COMMAND or
    // OUTDENT_CONTENT_COMMAND when `$indentOverTab(selection)`, and
    // INSERT_TAB_COMMAND otherwise, at COMMAND_PRIORITY_EDITOR, first. The
    // core's KEY_DOWN_COMMAND handler dispatches KEY_TAB_COMMAND now; the
    // handler isn't ported yet (rust/PARITY.md).
    editor.register_command(
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
    )
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
