//! A Rust port of [Lexical](https://lexical.dev)'s model and headless
//! packages, at parity with the JS implementation: the same documents give
//! the same editor state and the same JSON, byte for byte, in the legacy and
//! compact forms. Parity is checked against the conformance vectors that
//! `packages/lexical-conformance` records from the JS packages (see
//! `rust/PARITY.md`).
//!
//! The internals mirror the upstream sources file by file. JS's `$`
//! functions drop the `$` and take snake case: `$getRoot()` is
//! [`get_root()`], and like JS they only work inside [`Editor::update`],
//! [`Editor::read`] or [`EditorState::read`]. A node is read and changed
//! through a [`NodeRef`], which resolves the latest version on every call,
//! as JS node methods do. Text offsets are UTF-16 code units, as in JS.
//!
//! ```
//! use lexical_editor::{create_editor, get_root, CreateEditorArgs, UpdateOptions};
//!
//! let editor = create_editor(CreateEditorArgs::default());
//! let state = editor
//!     .parse_editor_state(r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","text":"Hi"}]}]}}"#)
//!     .unwrap();
//! assert_eq!(state.read(|| get_root().get_text_content()), "Hi");
//! assert_eq!(
//!     state.to_json(true).to_string(),
//!     r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","text":"Hi"}]}]}}"#,
//! );
//!
//! // An update: split the text and bold the second half.
//! editor.set_editor_state(&state, None).unwrap();
//! editor
//!     .update(
//!         || {
//!             let text = get_root().get_first_descendant().unwrap();
//!             let parts = text.split_text(&[1])?;
//!             parts[1].toggle_format("bold");
//!             Ok(())
//!         },
//!         UpdateOptions { discrete: true, ..UpdateOptions::default() },
//!     )
//!     .unwrap();
//! assert_eq!(
//!     editor.get_editor_state().to_json(true).to_string(),
//!     r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","text":"H"},{"type":"text","format":1,"text":"i"}]}]}}"#,
//! );
//! ```

mod caret;
mod caret_tree;
mod caret_utils;
#[cfg(feature = "code")]
pub mod code;
mod commands;
pub mod constants;
#[cfg(feature = "html")]
pub mod dom;
mod editor;
mod editor_state;
mod error;
#[cfg(feature = "extension")]
pub mod extension;
mod gc;
mod generated_json;
#[cfg(feature = "hashtag")]
pub mod hashtag;
#[cfg(feature = "history")]
pub mod history;
#[cfg(feature = "html")]
pub mod html;
pub mod json;
#[cfg(feature = "html")]
pub mod lexical_dom;
mod lexical_selection;
mod lexical_utils;
#[cfg(feature = "link")]
pub mod link;
#[cfg(feature = "list")]
pub mod list;
mod node;
mod node_state;
pub mod nodes;
mod normalization;
mod ordered;
#[cfg(feature = "overflow")]
pub mod overflow;
#[cfg(feature = "plain-text")]
pub mod plain_text;
#[cfg(feature = "rich-text")]
pub mod rich_text;
#[cfg(feature = "selection")]
pub mod selection;
mod selection_deletion;
mod selection_editing;
mod selection_format;
mod selection_insert_nodes;
mod selection_insert_text;
mod serialized_export;
mod set_dom_style;
#[cfg(feature = "text")]
pub mod text;
mod updates;
pub mod utf16;
#[cfg(feature = "utils")]
pub mod utils;

pub use caret::{
    CaretDirection, CaretKind, CaretRange, ChildCaret, CommonAncestorResult, NodeCaret, PointCaret,
    RootMode, SiblingCaret, TextOffset, TextPointCaret, TextPointCaretSlice,
    compare_point_caret_next, extend_caret_to_range, flip_direction, get_adjacent_child_caret,
    get_caret_in_direction, get_caret_range, get_child_caret, get_child_caret_or_self,
    get_collapsed_caret_range, get_common_ancestor, get_common_ancestor_result_branch_order,
    get_sibling_caret, get_text_node_offset, get_text_point_caret, get_text_point_caret_slice,
    make_stepwise_iterator,
};
pub use caret_utils::{
    SliceMode, SplitAtPointCaretNextOptions, caret_from_point, caret_range_from_selection,
    get_adjacent_sibling_or_parent_sibling_caret, get_caret_range_in_direction,
    get_child_caret_at_index, get_text_point_caret_slice_for_node,
    insert_node_to_nearest_root_at_caret, is_extendable_text_point_caret, normalize_caret,
    remove_text_from_caret_range, rewind_sibling_caret, set_point_from_caret,
    set_selection_from_caret_range, split_at_point_caret_next, split_text_point_caret,
    update_range_selection_from_caret_range,
};
pub use commands::{
    CAN_REDO_COMMAND, CAN_UNDO_COMMAND, CLEAR_EDITOR_COMMAND, CLEAR_HISTORY_COMMAND,
    CONTROLLED_TEXT_INSERTION_COMMAND, DELETE_CHARACTER_COMMAND, DELETE_LINE_COMMAND,
    DELETE_WORD_COMMAND, FORMAT_ELEMENT_COMMAND, FORMAT_TEXT_COMMAND, INDENT_CONTENT_COMMAND,
    INSERT_LINE_BREAK_COMMAND, INSERT_PARAGRAPH_COMMAND, INSERT_TAB_COMMAND,
    OUTDENT_CONTENT_COMMAND, REDO_COMMAND, REMOVE_TEXT_COMMAND, SELECT_ALL_COMMAND,
    SET_TEXT_FORMAT_COMMAND, UNDO_COMMAND,
};
pub use editor::{
    COMMAND_PRIORITY_CRITICAL, COMMAND_PRIORITY_EDITOR, COMMAND_PRIORITY_HIGH,
    COMMAND_PRIORITY_LOW, COMMAND_PRIORITY_NORMAL, COMPOSITION_END_TAG, COMPOSITION_START_TAG,
    CUT_TAG, CreateEditorArgs, Editor, EditorConfig, HISTORIC_TAG, HISTORY_MERGE_TAG,
    HISTORY_PUSH_TAG, LexicalCommand, NodeClass, PASTE_TAG, Transform, Unregister,
    UpdateListenerPayload, UpdateOptions, create_command, create_editor,
};
pub use editor_state::EditorState;
pub use error::LexicalError;
pub use lexical_selection::{
    NodeSelection, Point, PointType, RangeSelection, Selection, SelectionRef, Which,
    create_node_selection, create_range_selection, get_selection, set_selection,
};
pub use lexical_utils::{
    copy_node, get_node_by_key, get_root, get_text_direction, reset_random_key,
};
pub use node::{LexicalNode, NodeBase, NodeClone, NodeKey, NodeRef};
pub use node_state::{
    AnyStateConfig, RequiredNodeStateConfig, StateConfig, StateIsEqual, StateValue,
    StateValueConfig, create_state, create_state_with, get_state, get_state_change,
    get_state_direct, set_state, set_state_with,
};
pub use nodes::element::{Direction, ElementData};
pub use nodes::paragraph::create_paragraph_node;
pub use nodes::text::{TextData, create_text_node, toggle_text_format_type};
pub use selection_deletion::{SelectionAlter, SelectionGranularity, SelectionMovement};
pub use selection_insert_nodes::{insert_nodes, select_all};
pub use serialized_export::{export_node_json, is_compact_export, with_compact_export};
pub use set_dom_style::{StyleObject, get_style_object_from_css, js_trim};
#[cfg(feature = "html")]
pub use set_dom_style::{set_dom_style_from_css, set_dom_style_object};
pub use updates::parse_serialized_node;

/// The upstream Lexical release this port conforms to. The conformance
/// vectors (`meta.json`) name the same one, so an upstream sync can't move
/// one without the other.
pub const UPSTREAM_VERSION: &str = "0.53.0";
