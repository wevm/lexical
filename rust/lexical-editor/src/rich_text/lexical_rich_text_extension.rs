//! `LexicalRichTextExtension.ts`: `RichTextExtension` and its config
//! (`RichTextConfig`, `mergeRichTextConfig`).

use super::{
    EscapeFormatTriggerConfig, RichTextConfig, ShouldHandlePasteAsFiles, register_rich_text_with,
    rich_text_nodes,
};
use crate::extension::normalize_inline_elements_extension;
use crate::extension::signals::{Signal, effect, signal};
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, ExtensionRegisterState, LexicalExtension,
    define_extension,
};
use crate::{Editor, Unregister};
use std::rc::Rc;

/// `Partial<RichTextConfig>`.
#[derive(Clone, Default)]
pub struct RichTextConfigOverrides {
    pub escape_format_triggers: Option<EscapeFormatTriggerConfig>,
    pub should_handle_paste_as_files: Option<ShouldHandlePasteAsFiles>,
}

impl ExtensionConfigBase for RichTextConfig {
    type Partial = RichTextConfigOverrides;

    fn shallow_merge(&self, overrides: &RichTextConfigOverrides) -> Self {
        RichTextConfig {
            escape_format_triggers: overrides
                .escape_format_triggers
                .clone()
                .unwrap_or_else(|| self.escape_format_triggers.clone()),
            should_handle_paste_as_files: overrides
                .should_handle_paste_as_files
                .clone()
                .unwrap_or_else(|| self.should_handle_paste_as_files.clone()),
        }
    }
}

/// `mergeEscapeFormatTriggers(config, overrides)`: each format the
/// overrides name merges key by key with the configured one
/// (`mergeTriggerConfig`); a format the config doesn't have, or one the
/// overrides set to `None` (JS `null`), takes the override as it is.
fn merge_escape_format_triggers(
    config: &EscapeFormatTriggerConfig,
    overrides: &EscapeFormatTriggerConfig,
) -> EscapeFormatTriggerConfig {
    let mut merged = config.clone();
    for (format, override_) in overrides {
        let base = config
            .iter()
            .find(|(f, _)| f == format)
            .and_then(|(_, triggers)| triggers.as_ref());
        let value = match (base, override_) {
            (Some(base), Some(override_)) => Some(base.merged(override_)),
            _ => override_.clone(),
        };
        match merged.iter_mut().find(|(f, _)| f == format) {
            Some(entry) => entry.1 = value,
            None => merged.push((format.clone(), value)),
        }
    }
    merged
}

/// `mergeRichTextConfig(config, overrides)`: a shallow merge, except that
/// `escapeFormatTriggers` merge format by format.
fn merge_rich_text_config(
    config: &RichTextConfig,
    overrides: &RichTextConfigOverrides,
) -> RichTextConfig {
    let mut merged = config.shallow_merge(overrides);
    if let Some(triggers) = &overrides.escape_format_triggers {
        merged.escape_format_triggers =
            merge_escape_format_triggers(&config.escape_format_triggers, triggers);
    }
    merged
}

/// `namedSignals(RichTextConfig)`: `RichTextExtension`'s output.
#[derive(Clone)]
pub struct RichTextOutput {
    pub escape_format_triggers: Signal<EscapeFormatTriggerConfig>,
    pub should_handle_paste_as_files: Signal<ShouldHandlePasteAsFiles>,
}

/// `RichTextExtension`.
pub type RichTextExtensionType = LexicalExtension<RichTextConfig, RichTextOutput, ()>;

fn make() -> Rc<RichTextExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &RichTextConfig, _: &ExtensionBuildState<()>| {
                Ok(RichTextOutput {
                    escape_format_triggers: signal(config.escape_format_triggers.clone()),
                    should_handle_paste_as_files: signal(
                        config.should_handle_paste_as_files.clone(),
                    ),
                })
            },
        )),
        conflicts_with: vec!["@lexical/plain-text".to_owned()],
        // Of JS's dependencies, `NormalizeInlineElementsExtension` and
        // `NormalizeTripleClickSelectionExtension` (on the app's
        // `CLICK_COMMAND`). The others act on the DOM:
        // `HeadingAnnounceExtension` (a mutation listener that announces
        // headings, with `a11y`) and `DragonExtension` (root element
        // listeners). `CoreImportExtension` and `DOMImportExtension` with
        // `RichTextImportRules` (HTML import) come with `html`.
        dependencies: {
            #[allow(unused_mut)]
            let mut dependencies = vec![
                normalize_inline_elements_extension().into(),
                crate::extension::normalize_triple_click_selection_extension().into(),
            ];
            #[cfg(feature = "html")]
            dependencies.extend(crate::html::import::package_import_dependencies(
                super::rich_text_import_rules(),
                Vec::new(),
            ));
            dependencies
        },
        merge_config: Some(Rc::new(merge_rich_text_config)),
        nodes: rich_text_nodes(),
        // `effect(() => registerRichText(editor, escapeFormatTriggers,
        // shouldHandlePasteAsFiles))`: the handlers peek the signals, so
        // the effect reads none and runs once.
        register: Some(Rc::new(
            |editor: &Editor,
             _: &RichTextConfig,
             state: &ExtensionRegisterState<(), RichTextOutput>| {
                let output = state.get_output().expect("RichTextExtension has a build");
                let editor = editor.weak();
                Ok(effect(move || {
                    let editor = editor.upgrade().map(Editor)?;
                    let triggers = output.escape_format_triggers.clone();
                    let paste_as_files = output.should_handle_paste_as_files.clone();
                    Some(register_rich_text_with(
                        &editor,
                        Rc::new(move || triggers.peek()),
                        Rc::new(move || paste_as_files.peek()),
                    ))
                }) as Unregister)
            },
        )),
        ..LexicalExtension::new("@lexical/rich-text", RichTextConfig::default())
    })
}

thread_local! {
    static RICH_TEXT_EXTENSION: Rc<RichTextExtensionType> = make();
}

/// `RichTextExtension`: headings, quotes and the rich text commands
/// ([`register_rich_text`](super::register_rich_text)) with its config's
/// `escapeFormatTriggers` and `shouldHandlePasteAsFiles`, read from the
/// signals of its output when a handler needs them, with
/// `NormalizeInlineElementsExtension`. It can't be built with
/// `PlainTextExtension`.
pub fn rich_text_extension() -> Rc<RichTextExtensionType> {
    RICH_TEXT_EXTENSION.with(Rc::clone)
}
