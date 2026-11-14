//! A Rust port of [Lexical](https://lexical.dev)'s model and headless
//! packages, at parity with the JS implementation: the same documents give
//! the same editor state and the same JSON, byte for byte, in the legacy and
//! compact forms. Parity is checked against the conformance vectors that
//! `packages/lexical-conformance` records from the JS packages (see
//! `rust/PARITY.md`).
//!
//! The internals mirror the upstream sources file by file. JS's `$`
//! functions drop the `$` and take snake case: `$getRoot()` is
//! [`get_root()`], and like JS they only work inside [`Editor::read`] or
//! [`EditorState::read`]. A node is read through a [`NodeRef`], which
//! resolves the latest version on every call, as JS node methods do.
//!
//! ```
//! use lexical_editor::{create_editor, get_root, CreateEditorArgs};
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
//! ```

mod caret_tree;
pub mod constants;
mod editor;
mod editor_state;
mod error;
mod generated_json;
pub mod json;
mod node;
pub mod nodes;
mod serialized_export;
mod updates;
mod utils;

pub use editor::{CreateEditorArgs, Editor, NodeClass, create_editor};
pub use editor_state::EditorState;
pub use error::LexicalError;
pub use node::{LexicalNode, NodeBase, NodeClone, NodeKey, NodeRef};
pub use nodes::element::{Direction, ElementData};
pub use nodes::text::TextData;
pub use serialized_export::{export_node_json, is_compact_export, with_compact_export};
pub use utils::{get_node_by_key, get_root, reset_random_key};

/// The upstream Lexical release this port conforms to. The conformance
/// vectors (`meta.json`) name the same one, so an upstream sync can't move
/// one without the other.
pub const UPSTREAM_VERSION: &str = "0.53.0";
