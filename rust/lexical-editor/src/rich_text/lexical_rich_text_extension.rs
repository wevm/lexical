//! `LexicalRichTextExtension.ts`: `RichTextExtension`.

use super::{register_rich_text, rich_text_nodes};
use crate::extension::normalize_inline_elements_extension;
use crate::extension_core::{ExtensionRegisterState, LexicalExtension, define_extension};
use crate::{Editor, Unregister};
use std::rc::Rc;

/// `RichTextExtension`.
pub type RichTextExtensionType = LexicalExtension;

fn make() -> Rc<RichTextExtensionType> {
    define_extension(LexicalExtension {
        conflicts_with: vec!["@lexical/plain-text".to_owned()],
        // Of JS's dependencies, `NormalizeInlineElementsExtension`. The
        // others act on the DOM: `HeadingAnnounceExtension` (a mutation
        // listener that announces headings), `DragonExtension` and
        // `NormalizeTripleClickSelectionExtension` (root element
        // listeners), and `CoreImportExtension` and `DOMImportExtension`
        // with `RichTextImportRules` (HTML import), which come with `html`.
        dependencies: vec![normalize_inline_elements_extension().into()],
        nodes: rich_text_nodes(),
        // JS's `register` runs `registerRichText(editor,
        // escapeFormatTriggers, shouldHandlePasteAsFiles)` in an effect of
        // its output; both options belong to parts that aren't ported (the
        // click and arrow-key handlers, and paste's file branch;
        // `register_rich_text`), so neither is the config (`RichTextConfig`,
        // `mergeRichTextConfig`), and the effect would run once.
        register: Some(Rc::new(
            |editor: &Editor, _: &(), _: &ExtensionRegisterState<(), ()>| {
                Ok(register_rich_text(editor) as Unregister)
            },
        )),
        ..LexicalExtension::new("@lexical/rich-text", ())
    })
}

thread_local! {
    static RICH_TEXT_EXTENSION: Rc<RichTextExtensionType> = make();
}

/// `RichTextExtension`: headings, quotes and the rich text commands
/// ([`register_rich_text`]), with `NormalizeInlineElementsExtension`. It
/// can't be built with `PlainTextExtension`.
pub fn rich_text_extension() -> Rc<RichTextExtensionType> {
    RICH_TEXT_EXTENSION.with(Rc::clone)
}
