//! `ClearEditorExtension.ts`: `CLEAR_EDITOR_COMMAND`'s implementation.

use super::signals::{Signal, effect, signal};
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, ExtensionRegisterState, LexicalExtension,
    define_extension,
};
use crate::{
    CLEAR_EDITOR_COMMAND, COMMAND_PRIORITY_EDITOR, Editor, LexicalError, Unregister,
    create_paragraph_node, get_root, get_selection,
};
use std::rc::Rc;

/// `ClearEditorConfig.$onClear`: what clearing does, compared by identity
/// as JS compares functions.
#[derive(Clone)]
pub struct OnClear(pub Rc<dyn Fn() -> Result<(), LexicalError>>);

impl PartialEq for OnClear {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// `ClearEditorConfig`.
#[derive(Clone)]
pub struct ClearEditorConfig {
    pub on_clear: OnClear,
}

/// `Partial<ClearEditorConfig>`.
#[derive(Clone, Default)]
pub struct ClearEditorConfigOverrides {
    pub on_clear: Option<OnClear>,
}

impl ExtensionConfigBase for ClearEditorConfig {
    type Partial = ClearEditorConfigOverrides;

    fn shallow_merge(&self, overrides: &ClearEditorConfigOverrides) -> Self {
        ClearEditorConfig {
            on_clear: overrides
                .on_clear
                .clone()
                .unwrap_or_else(|| self.on_clear.clone()),
        }
    }
}

/// `namedSignals(ClearEditorConfig)`: `ClearEditorExtension`'s output.
#[derive(Clone)]
pub struct ClearEditorOutput {
    pub on_clear: Signal<OnClear>,
}

/// `ClearEditorExtension`.
pub type ClearEditorExtensionType = LexicalExtension<ClearEditorConfig, ClearEditorOutput, ()>;

/// `$defaultOnClear`: the root gets one empty paragraph, selected if
/// anything was, with a range selection's format reset.
fn default_on_clear() -> Result<(), LexicalError> {
    let root = get_root();
    let selection = get_selection();
    let paragraph = create_paragraph_node();
    root.clear()?;
    root.append(std::slice::from_ref(&paragraph))?;
    if selection.is_some() {
        paragraph.select(None, None)?;
    }
    if let Some(selection) = selection.filter(|s| s.is_range()) {
        selection.set_format(0.0);
    }
    Ok(())
}

/// `registerClearEditor(editor, $onClear)`: handles `CLEAR_EDITOR_COMMAND`
/// by running `on_clear` (by default `$defaultOnClear`) in the command's
/// update.
pub fn register_clear_editor(editor: &Editor, on_clear: Option<OnClear>) -> Unregister {
    let on_clear = on_clear.map_or_else(|| Rc::new(default_on_clear) as Rc<_>, |c| c.0);
    editor.register_command(
        &CLEAR_EDITOR_COMMAND,
        move |_, _| {
            on_clear()?;
            Ok(true)
        },
        COMMAND_PRIORITY_EDITOR,
    )
}

fn make() -> Rc<ClearEditorExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_editor: &Editor, config: &ClearEditorConfig, _state: &ExtensionBuildState<()>| {
                Ok(ClearEditorOutput {
                    on_clear: signal(config.on_clear.clone()),
                })
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _config: &ClearEditorConfig,
             state: &ExtensionRegisterState<(), ClearEditorOutput>| {
                let output = state
                    .get_output()
                    .expect("ClearEditorExtension has a build");
                // Weakly, as the editor keeps the effect through the signal.
                let editor = editor.weak();
                Ok(effect(move || {
                    let on_clear = output.on_clear.value();
                    let editor = editor.upgrade().map(Editor)?;
                    Some(register_clear_editor(&editor, Some(on_clear)))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/extension/ClearEditor",
            ClearEditorConfig {
                on_clear: OnClear(Rc::new(default_on_clear)),
            },
        )
    })
}

thread_local! {
    static CLEAR_EDITOR_EXTENSION: Rc<ClearEditorExtensionType> = make();
}

/// `ClearEditorExtension`: an implementation of `CLEAR_EDITOR_COMMAND`,
/// whose `$onClear` is a signal of its output.
pub fn clear_editor_extension() -> Rc<ClearEditorExtensionType> {
    CLEAR_EDITOR_EXTENSION.with(Rc::clone)
}
