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
pub mod constants;
mod editor;
mod editor_state;
mod error;
mod gc;
mod generated_json;
pub mod json;
mod node;
pub mod nodes;
mod normalization;
mod ordered;
mod selection;
mod selection_deletion;
mod selection_editing;
mod selection_format;
mod selection_insert_nodes;
mod selection_insert_text;
mod serialized_export;
mod updates;
pub mod utf16;
mod utils;

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
    SliceMode, caret_from_point, caret_range_from_selection,
    get_adjacent_sibling_or_parent_sibling_caret, get_caret_range_in_direction,
    get_child_caret_at_index, get_text_point_caret_slice_for_node, is_extendable_text_point_caret,
    normalize_caret, remove_text_from_caret_range, rewind_sibling_caret, set_point_from_caret,
    set_selection_from_caret_range, update_range_selection_from_caret_range,
};
pub use editor::{
    COMMAND_PRIORITY_CRITICAL, COMMAND_PRIORITY_EDITOR, COMMAND_PRIORITY_HIGH,
    COMMAND_PRIORITY_LOW, COMMAND_PRIORITY_NORMAL, CreateEditorArgs, Editor, HISTORY_MERGE_TAG,
    LexicalCommand, NodeClass, Transform, Unregister, UpdateListenerPayload, UpdateOptions,
    create_command, create_editor,
};
pub use editor_state::EditorState;
pub use error::LexicalError;
pub use node::{LexicalNode, NodeBase, NodeClone, NodeKey, NodeRef};
pub use nodes::element::{Direction, ElementData};
pub use nodes::paragraph::create_paragraph_node;
pub use nodes::text::{TextData, create_text_node, toggle_text_format_type};
pub use selection::{
    NodeSelection, Point, PointType, RangeSelection, Selection, SelectionRef, Which,
    create_node_selection, create_range_selection, get_selection, set_selection,
};
pub use selection_deletion::{SelectionAlter, SelectionGranularity, SelectionMovement};
pub use selection_insert_nodes::{insert_nodes, select_all};
pub use serialized_export::{export_node_json, is_compact_export, with_compact_export};
pub use updates::parse_serialized_node;
pub use utils::{get_node_by_key, get_root, reset_random_key};

/// The upstream Lexical release this port conforms to. The conformance
/// vectors (`meta.json`) name the same one, so an upstream sync can't move
/// one without the other.
pub const UPSTREAM_VERSION: &str = "0.53.0";
