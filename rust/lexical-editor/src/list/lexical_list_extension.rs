//! `LexicalListExtension.ts`: `ListExtension` and `CheckListExtension`.

use super::{
    RegisterListOptions, list_nodes, register_check_list, register_list,
    register_list_strict_indent_transform,
};
use crate::extension::named_signals_config;
use crate::extension::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::merge_register::merge_register;
use crate::{Editor, Unregister};
use std::rc::Rc;

named_signals_config! {
    /// `ListConfig`.
    pub struct ListConfig;
    /// `Partial<ListConfig>`.
    pub struct ListConfigOverrides;
    /// `namedSignals(ListConfig)`: `ListExtension`'s output.
    pub struct ListOutput {
        /// `hasStrictIndent`: registers
        /// [`register_list_strict_indent_transform`]. `false` by default.
        has_strict_indent: bool,
        /// `shouldPreserveNumbering`: a list an Enter in an empty item
        /// splits continues the numbering (`restoreNumbering`). `false` by
        /// default.
        should_preserve_numbering: bool,
    }
}

/// `ListExtension`.
pub type ListExtensionType = LexicalExtension<ListConfig, ListOutput, ()>;

fn make_list_extension() -> Rc<ListExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &ListConfig, _: &ExtensionBuildState<()>| {
                Ok(ListOutput::of(config))
            },
        )),
        // JS's dependencies, `CoreImportExtension` and `DOMImportExtension`
        // with `ListImportRules`, are the HTML import, which comes with
        // `html`.
        nodes: list_nodes(),
        register: Some(Rc::new(
            |editor: &Editor, _: &ListConfig, state: &ExtensionRegisterState<(), ListOutput>| {
                let stores = state.get_output().expect("ListExtension has a build");
                let (weak, list_stores) = (editor.weak(), stores.clone());
                let list = effect(move || {
                    let restore_numbering = list_stores.should_preserve_numbering.value();
                    let editor = weak.upgrade().map(Editor)?;
                    Some(register_list(
                        &editor,
                        RegisterListOptions { restore_numbering },
                    ))
                });
                let weak = editor.weak();
                let strict = effect(move || {
                    if !stores.has_strict_indent.value() {
                        return None;
                    }
                    let editor = weak.upgrade().map(Editor)?;
                    Some(register_list_strict_indent_transform(&editor))
                });
                Ok(merge_register(vec![list, strict]))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/list/List",
            ListConfig {
                has_strict_indent: false,
                should_preserve_numbering: false,
            },
        )
    })
}

named_signals_config! {
    /// `CheckListConfig`.
    pub struct CheckListConfig;
    /// `Partial<CheckListConfig>`.
    pub struct CheckListConfigOverrides;
    /// `namedSignals(CheckListConfig)`: `CheckListExtension`'s output.
    pub struct CheckListOutput {
        /// `disableTakeFocusOnClick`: a click on an item's marker doesn't
        /// focus the editor. Clicks need an input layer, so nothing reads
        /// it yet.
        disable_take_focus_on_click: bool,
    }
}

/// `CheckListExtension`.
pub type CheckListExtensionType = LexicalExtension<CheckListConfig, CheckListOutput, ()>;

fn make_check_list_extension() -> Rc<CheckListExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &CheckListConfig, _: &ExtensionBuildState<()>| {
                Ok(CheckListOutput::of(config))
            },
        )),
        dependencies: vec![list_extension().into()],
        // `registerCheckList(editor, state.getOutput())`, whose option
        // only the click handler reads.
        register: Some(Rc::new(
            |editor: &Editor,
             _: &CheckListConfig,
             _: &ExtensionRegisterState<(), CheckListOutput>| {
                Ok(register_check_list(editor) as Unregister)
            },
        )),
        ..LexicalExtension::new(
            "@lexical/list/CheckList",
            CheckListConfig {
                disable_take_focus_on_click: false,
            },
        )
    })
}

thread_local! {
    static LIST_EXTENSION: Rc<ListExtensionType> = make_list_extension();
    static CHECK_LIST_EXTENSION: Rc<CheckListExtensionType> = make_check_list_extension();
}

/// `ListExtension`: `ListNode` and `ListItemNode`, the list commands
/// ([`register_list`]), and the strict indent transform with
/// `has_strict_indent`.
pub fn list_extension() -> Rc<ListExtensionType> {
    LIST_EXTENSION.with(Rc::clone)
}

/// `CheckListExtension`: [`list_extension`] and
/// `INSERT_CHECK_LIST_COMMAND` ([`register_check_list`]).
pub fn check_list_extension() -> Rc<CheckListExtensionType> {
    CHECK_LIST_EXTENSION.with(Rc::clone)
}
