//! `ClickAfterLastBlockExtension.ts`: `$defaultShouldInsertAfter` and
//! `ClickAfterLastBlockExtension`.

use super::named_signals::named_signals_config;
use super::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::{
    CLICK_COMMAND, COMMAND_PRIORITY_CRITICAL, Editor, NodeRef, create_paragraph_node, get_root,
};
use std::rc::Rc;

/// `$defaultShouldInsertAfter(node)`: a paragraph goes after a decorator or
/// a shadow root (a table), which a caret can't be placed after.
pub fn default_should_insert_after(node: &NodeRef) -> bool {
    node.is_decorator() || crate::lexical_utils::is_shadow_root_node(node)
}

/// `$shouldInsertAfter`: whether a click below the root's last child
/// inserts a paragraph after it, compared by identity as JS compares
/// functions.
#[derive(Clone)]
pub struct ShouldInsertAfter(pub Rc<dyn Fn(&NodeRef) -> bool>);

impl PartialEq for ShouldInsertAfter {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

named_signals_config! {
    /// `ClickAfterLastBlockConfig`.
    pub struct ClickAfterLastBlockConfig;
    /// `Partial<ClickAfterLastBlockConfig>`.
    pub struct ClickAfterLastBlockConfigOverrides;
    /// `ClickAfterLastBlockOutput`: `namedSignals(config)`.
    pub struct ClickAfterLastBlockOutput {
        /// `disabled`.
        disabled: bool,
        /// `$shouldInsertAfter`: called in the update with the root's last
        /// child; [`default_should_insert_after`] by default.
        should_insert_after: ShouldInsertAfter,
    }
}

/// `ClickAfterLastBlockExtension`.
pub type ClickAfterLastBlockExtensionType =
    LexicalExtension<ClickAfterLastBlockConfig, ClickAfterLastBlockOutput, ()>;

fn make() -> Rc<ClickAfterLastBlockExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &ClickAfterLastBlockConfig, _: &ExtensionBuildState<()>| {
                Ok(ClickAfterLastBlockOutput::of(config))
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &ClickAfterLastBlockConfig,
             state: &ExtensionRegisterState<(), ClickAfterLastBlockOutput>| {
                let output = state
                    .get_output()
                    .expect("ClickAfterLastBlockExtension has a build");
                let editor = editor.weak();
                Ok(effect(move || {
                    if output.disabled.value() {
                        return None;
                    }
                    let editor = editor.upgrade().map(Editor)?;
                    let should_insert_after = output.should_insert_after.clone();
                    Some(editor.register_command(
                        &CLICK_COMMAND,
                        move |event, editor| {
                            // `shouldClaimClick`: only a click on the root
                            // itself, which the view reports below the last
                            // block, of an editable editor.
                            if !editor.is_editable() || event.target_key.as_deref() != Some("root")
                            {
                                return Ok(false);
                            }
                            let Some(last_child) = get_root().get_last_child() else {
                                return Ok(false);
                            };
                            if !(should_insert_after.peek().0)(&last_child) {
                                return Ok(false);
                            }
                            event.prevent_default();
                            let paragraph = create_paragraph_node();
                            last_child.insert_after(&paragraph, true)?;
                            paragraph.select(None, None)?;
                            Ok(true)
                        },
                        COMMAND_PRIORITY_CRITICAL,
                    ))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/ClickAfterLastBlock",
            ClickAfterLastBlockConfig {
                disabled: false,
                should_insert_after: ShouldInsertAfter(Rc::new(default_should_insert_after)),
            },
        )
    })
}

thread_local! {
    static CLICK_AFTER_LAST_BLOCK_EXTENSION: Rc<ClickAfterLastBlockExtensionType> = make();
}

/// `ClickAfterLastBlockExtension`: a click in the empty area below the last
/// block, when that block is one a caret can't be placed after (a block
/// decorator, a table), inserts a paragraph after it and selects it.
///
/// JS listens to `mousedown` and `click` on the root element, in the
/// capture phase, measures the last block's DOM and stops Lexical's own
/// click handling. Here the app reports such a click as [`CLICK_COMMAND`]
/// with `root` as its [`target_key`](crate::MouseEvent::target_key) (the
/// view measures the click against the last block), which the extension
/// handles at [`COMMAND_PRIORITY_CRITICAL`], before the other click
/// handlers, as Swift's does. The predicate runs once, in that update,
/// where JS runs it in a read and again in its update.
pub fn click_after_last_block_extension() -> Rc<ClickAfterLastBlockExtensionType> {
    CLICK_AFTER_LAST_BLOCK_EXTENSION.with(Rc::clone)
}
