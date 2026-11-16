//! `SelectBlockExtension.ts` and its dependency
//! `PreventSelectAllExtension.ts`.

use super::named_signals::named_signals_config;
use super::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::merge_register::merge_register;
use crate::nodes::element::is_root_or_shadow_root;
use crate::selection_deletion::is_block_fully_selected;
use crate::{
    COMMAND_PRIORITY_LOW, Editor, LexicalError, NodeRef, SELECT_ALL_COMMAND, Which, get_root,
    get_selection, select_all,
};
use std::rc::Rc;

named_signals_config! {
    /// `PreventSelectAllConfig`.
    pub struct PreventSelectAllConfig;
    /// `Partial<PreventSelectAllConfig>`.
    pub struct PreventSelectAllConfigOverrides;
    /// `namedSignals(PreventSelectAllConfig)`: the extension's output.
    pub struct PreventSelectAllOutput {
        /// `disabled`.
        disabled: bool,
    }
}

/// `PreventSelectAllExtension`.
pub type PreventSelectAllExtensionType =
    LexicalExtension<PreventSelectAllConfig, PreventSelectAllOutput, ()>;

fn make_prevent_select_all() -> Rc<PreventSelectAllExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &PreventSelectAllConfig, _: &ExtensionBuildState<()>| {
                Ok(PreventSelectAllOutput::of(config))
            },
        )),
        // JS's `register` adds a capturing `keydown` listener to the root
        // element that keeps Ctrl/Cmd+A in an `<input>` or `<textarea>`
        // from dispatching `SELECT_ALL_COMMAND`; a headless editor has no
        // root element, so only the config is ported.
        ..LexicalExtension::new(
            "@lexical/extension/PreventSelectAll",
            PreventSelectAllConfig { disabled: false },
        )
    })
}

/// `node.getTopLevelElement()`: the ancestor (or `node`) whose parent is
/// the root or a shadow root, or that is a slot value (the top of its
/// isolated scope).
fn get_top_level_element(node: &NodeRef) -> Option<NodeRef> {
    let mut node = Some(node.clone());
    while let Some(n) = node {
        let parent = n.get_parent();
        if crate::slot::get_slot_host_key(&n).is_some() {
            return Some(n);
        }
        if parent
            .as_ref()
            .is_some_and(|p| is_root_or_shadow_root(&*p.get_latest()))
        {
            return Some(n);
        }
        node = parent;
    }
    None
}

/// `$hasCommonTopParent(nodes, commonTopParent)`.
fn has_common_top_parent(nodes: &[NodeRef], common_top_parent: &NodeRef) -> bool {
    nodes
        .iter()
        .all(|node| get_top_level_element(node).as_ref() == Some(common_top_parent))
}

/// `SelectBlockExtension`'s `SELECT_ALL_COMMAND` handler.
fn handle_select_all() -> Result<bool, LexicalError> {
    // JS first handles the command bubbling up from a nested editor
    // (`triggerEditor !== editor`, with `cascadeSelection`); a Rust editor
    // has no parent, so it is always its own trigger.
    let Some(selection) = get_selection() else {
        return Ok(false);
    };
    if selection.is_node() {
        let selected_nodes = selection.get_nodes()?;
        let Some(first_node) = selected_nodes.first() else {
            // An empty NodeSelection, defer to the default handlers.
            return Ok(false);
        };
        let top_parent = get_top_level_element(first_node);
        match top_parent {
            Some(top_parent)
                if !top_parent.is_root()
                    && &top_parent != first_node
                    && !(selected_nodes.len() > 1
                        && !has_common_top_parent(&selected_nodes, &top_parent)) =>
            {
                if top_parent.is_element() {
                    top_parent.select(Some(0), Some(top_parent.get_children_size()))?;
                }
            }
            _ => {
                select_all()?;
            }
        }
        return Ok(true);
    }
    let Some(range) = selection.range() else {
        return Ok(false);
    };
    let anchor_node = range.point(Which::Anchor).get_node()?;
    let focus_node = range.point(Which::Focus).get_node()?;
    if let Some(block_node) = get_top_level_element(&anchor_node)
        // A selection that crosses block boundaries expands to the next
        // enclosing scope instead of shrinking to the anchor's block.
        && Some(&block_node) == get_top_level_element(&focus_node).as_ref()
        // An empty block is fully selected by its caret.
        && !is_block_fully_selected(&block_node, &range)?
    {
        block_node.select(Some(0), Some(block_node.get_children_size()))?;
        return Ok(true);
    }
    // Named slots are isolated scopes between the block and the document:
    // expand to the innermost enclosing slot frame not yet fully selected,
    // then outward.
    let mut frame = crate::slot::get_slot_frame(&anchor_node);
    while let Some(f) = frame {
        if f.is_element() && !is_block_fully_selected(&f, &range)? {
            f.select(Some(0), Some(f.get_children_size()))?;
            return Ok(true);
        }
        frame = crate::slot::get_slot_host(&f).and_then(|host| crate::slot::get_slot_frame(&host));
    }
    if !is_block_fully_selected(&get_root(), &range)? {
        // Don't select all if the document is already fully selected.
        select_all()?;
    }
    Ok(true)
}

named_signals_config! {
    /// `SelectBlockConfig`.
    pub struct SelectBlockConfig;
    /// `Partial<SelectBlockConfig>`.
    pub struct SelectBlockConfigOverrides;
    /// `namedSignals(SelectBlockConfig)`: the extension's output.
    pub struct SelectBlockOutput {
        /// `cascadeSelection`: select all of a parent editor once a nested
        /// editor's content is fully selected. Rust editors have no parent,
        /// so nothing reads it.
        cascade_selection: bool,
        /// `disabled`, which `PreventSelectAllExtension`'s follows.
        disabled: bool,
    }
}

/// `SelectBlockExtension`.
pub type SelectBlockExtensionType = LexicalExtension<SelectBlockConfig, SelectBlockOutput, ()>;

fn make_select_block() -> Rc<SelectBlockExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &SelectBlockConfig, _: &ExtensionBuildState<()>| {
                Ok(SelectBlockOutput::of(config))
            },
        )),
        dependencies: vec![prevent_select_all_extension().into()],
        register: Some(Rc::new(
            |editor: &Editor,
             _: &SelectBlockConfig,
             state: &ExtensionRegisterState<(), SelectBlockOutput>| {
                let stores = state
                    .get_output()
                    .expect("SelectBlockExtension has a build");
                let prevent_select_all_stores = state
                    .get_dependency(&prevent_select_all_extension())?
                    .output
                    .expect("PreventSelectAllExtension has a build");
                // `PreventSelectAllExtension` is only a dependency in
                // support of this one, so its disabled state follows.
                let sync = {
                    let stores = stores.clone();
                    effect(move || {
                        prevent_select_all_stores
                            .disabled
                            .set_value(stores.disabled.value());
                        None
                    })
                };
                let editor = editor.weak();
                let handler = effect(move || {
                    if stores.disabled.value() {
                        return None;
                    }
                    let editor = editor.upgrade().map(Editor)?;
                    // In a higher priority bucket than
                    // COMMAND_PRIORITY_EDITOR, so it runs before
                    // RichTextExtension's handler.
                    Some(editor.register_command(
                        &SELECT_ALL_COMMAND,
                        |_, _| handle_select_all(),
                        COMMAND_PRIORITY_LOW,
                    ))
                });
                Ok(merge_register(vec![sync, handler]))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/extension/SelectBlock",
            SelectBlockConfig {
                cascade_selection: false,
                disabled: false,
            },
        )
    })
}

thread_local! {
    static PREVENT_SELECT_ALL_EXTENSION: Rc<PreventSelectAllExtensionType> =
        make_prevent_select_all();
    static SELECT_BLOCK_EXTENSION: Rc<SelectBlockExtensionType> = make_select_block();
}

/// `PreventSelectAllExtension`: its config, as signals; its root element
/// listener needs a DOM.
pub fn prevent_select_all_extension() -> Rc<PreventSelectAllExtensionType> {
    PREVENT_SELECT_ALL_EXTENSION.with(Rc::clone)
}

/// `SelectBlockExtension`: `SELECT_ALL_COMMAND` selects the block holding
/// the selection first, then the document.
pub fn select_block_extension() -> Rc<SelectBlockExtensionType> {
    SELECT_BLOCK_EXTENSION.with(Rc::clone)
}
