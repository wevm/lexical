//! `@lexical/list` (the `list` feature), from `packages/lexical-list/src`:
//! `ListNode` and `ListItemNode` with their generated JSON
//! (`LexicalListGeneratedJSON.ts`), the model functions of `formatList.ts`
//! and `utils.ts`, `registerList`'s command handlers and transforms
//! ([`register_list`]), and the model part of `registerCheckList`
//! ([`register_check_list`]).
//!
//! Not ported: the DOM parts (`createDOM`, `updateDOM`, `exportDOM`,
//! `importDOM`, `ListImportExtension`, `WordListImportExtension`), which come
//! with `html`; and `registerList`'s `KEY_BACKSPACE_COMMAND` handler and
//! `registerCheckList`'s pointer, focus and key handlers, which wait for an
//! input layer (rust/PARITY.md). With the `extension` feature,
//! [`list_extension`] and [`check_list_extension`] are `ListExtension` and
//! `CheckListExtension`.

mod format_list;
mod generated_json;
#[cfg(feature = "extension")]
mod lexical_list_extension;
mod list_item_node;
mod list_node;
mod register_list;
mod utils;

pub use format_list::{
    handle_indent, handle_list_insert_paragraph, handle_outdent, insert_list, merge_lists,
    merge_next_sibling_list_if_same_type, remove_list, update_children_list_item_value,
};
#[cfg(feature = "extension")]
pub use lexical_list_extension::{
    CheckListConfig, CheckListConfigOverrides, CheckListExtensionType, CheckListOutput, ListConfig,
    ListConfigOverrides, ListExtensionType, ListOutput, check_list_extension, list_extension,
};
pub use list_item_node::{ListItemNode, create_list_item_node, is_list_item_node};
pub use list_node::{ListNode, ListType, create_list_node, is_list_node};
pub use register_list::{
    INSERT_CHECK_LIST_COMMAND, INSERT_ORDERED_LIST_COMMAND, INSERT_UNORDERED_LIST_COMMAND,
    REMOVE_LIST_COMMAND, RegisterListOptions, UPDATE_LIST_START_COMMAND, UpdateListStartPayload,
    register_check_list, register_list, register_list_strict_indent_transform,
};
pub use utils::{
    get_all_list_items, get_list_depth, get_new_list_start, get_top_list_node, is_nested_list_node,
    remove_highest_empty_list_parent,
};

use crate::NodeClass;

/// The node classes this package registers (`ListExtension`'s `nodes`):
/// `ListNode` and `ListItemNode`.
pub fn list_nodes() -> Vec<NodeClass> {
    vec![NodeClass::of::<ListNode>(), NodeClass::of::<ListItemNode>()]
}
