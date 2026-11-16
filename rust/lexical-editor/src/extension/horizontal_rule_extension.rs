//! `HorizontalRuleExtension.ts`: `HorizontalRuleNode`,
//! `$createHorizontalRuleNode`, `$isHorizontalRuleNode`,
//! `INSERT_HORIZONTAL_RULE_COMMAND` and `HorizontalRuleExtension`.

use super::editor_state_extension::editor_state_extension;
use super::node_selection_extension::node_selection_extension;
use crate::extension_core::{LexicalExtension, define_extension};
use crate::generated_json::{export_compact_line_break_node, export_line_break_node};
use crate::json::Object;
use crate::lexical_utils::set_node_key;
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::utils::insert_node_to_nearest_root;
use crate::{
    CLICK_COMMAND, COMMAND_PRIORITY_EDITOR, COMMAND_PRIORITY_LOW, LexicalCommand, LexicalError,
    NodeClass, create_node_selection, get_node_by_key, get_selection, merge_register,
    set_selection,
};
use std::rc::Rc;

/// `INSERT_HORIZONTAL_RULE_COMMAND`: inserts a [`HorizontalRuleNode`] at the
/// current selection.
pub const INSERT_HORIZONTAL_RULE_COMMAND: LexicalCommand<()> =
    LexicalCommand::constant(u64::MAX - 57, "INSERT_HORIZONTAL_RULE_COMMAND");

/// `HorizontalRuleNode`: a horizontal rule, a block decorator whose text is
/// a line break.
#[derive(Clone, Debug, Default)]
pub struct HorizontalRuleNode {
    base: NodeBase,
}

/// `$createHorizontalRuleNode()`.
pub fn create_horizontal_rule_node() -> NodeRef {
    set_node_key(Box::new(HorizontalRuleNode::default()))
}

/// `$isHorizontalRuleNode(node)`.
pub fn is_horizontal_rule_node(node: &NodeRef) -> bool {
    node.get_latest().is::<HorizontalRuleNode>()
}

impl LexicalNode for HorizontalRuleNode {
    fn get_type(&self) -> &'static str {
        "horizontalrule"
    }
    /// `HorizontalRuleNode.createDOM(config)`: an `<hr>` with the theme's
    /// `hr` class names.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, crate::LexicalError> {
        let element = crate::lexical_dom::get_document().create_element("hr");
        crate::lexical_dom::add_theme_class_names(&element, &config.theme, "hr");
        Ok(element)
    }
    /// `HorizontalRuleNode.exportDOM()`: a bare `<hr>`.
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        _this: &NodeRef,
        _editor: &crate::Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, crate::LexicalError> {
        Ok(crate::lexical_dom::DomExportOutput::element(
            crate::lexical_dom::get_document().create_element("hr"),
        ))
    }
    /// `HorizontalRuleNode`'s `importDOM`: `<hr>`.
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(vec![(
            "hr".to_owned(),
            crate::lexical_dom::importer(0, |_| {
                Ok(Some(crate::lexical_dom::DomConversionOutput::node(Some(
                    create_horizontal_rule_node(),
                ))))
            }),
        )])
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    /// `HorizontalRuleNode.isInline()`.
    fn is_inline(&self) -> bool {
        false
    }
    /// `HorizontalRuleNode.getTextContent()`.
    fn get_text_content(&self) -> String {
        "\n".to_owned()
    }
    /// `LexicalNode`'s generated JSON: the type and the version.
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            export_compact_line_break_node(self.get_type())
        } else {
            export_line_break_node(self.get_type())
        })
    }
    fn generated_update_from_json(&mut self, _json: &Object) -> bool {
        true
    }
}

/// `$toggleNodeSelection(node, shiftKey)`: selects `node` alone, or with
/// `shift_key` adds it to the node selection there is; a node that was
/// selected is taken out of that selection instead.
fn toggle_node_selection(node: &NodeRef, shift_key: bool) -> Result<(), LexicalError> {
    let selection = get_selection();
    let was_selected = node.is_selected(None)?;
    let key = node.get_key();
    let node_selection = match selection {
        Some(selection) if shift_key && selection.is_node() => selection,
        _ => {
            let node_selection = create_node_selection();
            set_selection(Some(node_selection.clone()));
            node_selection
        }
    };
    if was_selected {
        node_selection.delete(key);
    } else {
        node_selection.add(key);
    }
    Ok(())
}

/// `HorizontalRuleExtension`.
pub type HorizontalRuleExtensionType = LexicalExtension;

fn make() -> Rc<HorizontalRuleExtensionType> {
    define_extension(LexicalExtension {
        dependencies: vec![
            editor_state_extension().into(),
            node_selection_extension().into(),
        ],
        nodes: vec![NodeClass::of::<HorizontalRuleNode>()],
        register: Some(Rc::new(|editor, _, _| {
            Ok(merge_register(vec![
                editor.register_command(
                    &INSERT_HORIZONTAL_RULE_COMMAND,
                    |_, _| {
                        let Some(range) = get_selection().and_then(|s| s.range()) else {
                            return Ok(false);
                        };
                        // `selection.focus.getNode()`, which throws for a
                        // focus on no node.
                        range.focus.get_node()?;
                        insert_node_to_nearest_root(&create_horizontal_rule_node())?;
                        Ok(true)
                    },
                    COMMAND_PRIORITY_EDITOR,
                ),
                editor.register_command(
                    &CLICK_COMMAND,
                    |event, _| {
                        // `$getNodeFromDOMNode(event.target)`: the node the
                        // app resolved the target to.
                        let node = event.target_key.as_deref().and_then(get_node_by_key);
                        if let Some(node) = node.filter(is_horizontal_rule_node) {
                            toggle_node_selection(&node, event.shift_key)?;
                            return Ok(true);
                        }
                        Ok(false)
                    },
                    COMMAND_PRIORITY_LOW,
                ),
            ]))
        })),
        ..LexicalExtension::new("@lexical/extension/HorizontalRule", ())
    })
}

thread_local! {
    static HORIZONTAL_RULE_EXTENSION: Rc<HorizontalRuleExtensionType> = make();
}

/// `HorizontalRuleExtension`: registers [`HorizontalRuleNode`], inserts one
/// at the selection's focus with [`INSERT_HORIZONTAL_RULE_COMMAND`]
/// (`$insertNodeToNearestRoot`), and on [`CLICK_COMMAND`] whose
/// [`target_key`](crate::MouseEvent::target_key) is a rule selects it in a
/// new node selection, or with Shift toggles it in the node selection there
/// is (`$toggleNodeSelection`).
///
/// Not ported: JS also keeps the theme's `hrSelected` class (`selected` by
/// default) on the `<hr>` of each rule while
/// `NodeSelectionExtension`'s `watchNodeKey` signal of it is true, through a
/// mutation listener that reads `editor.getElementByKey`. A Rust editor has
/// no DOM of its nodes: an app draws a rule's selected state from the same
/// signal (rust/PARITY.md).
pub fn horizontal_rule_extension() -> Rc<HorizontalRuleExtensionType> {
    HORIZONTAL_RULE_EXTENSION.with(Rc::clone)
}
