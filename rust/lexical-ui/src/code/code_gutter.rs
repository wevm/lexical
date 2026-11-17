//! `packages/lexical-code-core/src/CodeGutter.ts`: the line numbers of each
//! `CodeNode`, kept in step with its line count.
//!
//! JS writes them to the `data-gutter` attribute of the code block's DOM
//! element, for a theme's `content: attr(data-gutter)`. A Rust editor has
//! no DOM of its nodes, so the app gets the same value from a callback and
//! draws it, as it draws the rest of the block.

use crate::editor::{MutationListenerOptions, NodeMutation};
use crate::lexical_utils::get_node_by_key;
use crate::node::NodeKey;
use crate::{Editor, LexicalError, Unregister};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// `$updateCodeGutter(node)`'s value: one line per `LineBreakNode` child
/// plus one, newline separated (`"1\n2\n3"`).
fn code_gutter(children: &[crate::NodeRef]) -> String {
    let mut gutter = String::from("1");
    let mut count = 1;
    for child in children {
        if child.get_type() == "linebreak" {
            count += 1;
            gutter.push('\n');
            gutter.push_str(&count.to_string());
        }
    }
    gutter
}

/// `registerCodeGutter(editor)` (`@internal` in JS, which
/// `registerCodeHighlighting` calls): calls `set_gutter` with a code
/// node's key and its line numbers (what JS writes to `data-gutter`) when
/// the node is rendered, and again when its number of children changes, as
/// JS rewrites the attribute only then. A mutation listener of `CodeNode`,
/// so it hears only while the editor has a root element; a headless editor
/// registers nothing, as in JS. Returns what unregisters it, or an error
/// when the editor has no `CodeNode`, as JS's `registerMutationListener`
/// throws.
pub fn register_code_gutter(
    editor: &Editor,
    set_gutter: impl Fn(&str, &str) + 'static,
) -> Result<Unregister, LexicalError> {
    if editor.is_headless() {
        return Ok(Box::new(|| {}));
    }
    // `codeElement.__cachedChildrenLength`, by key: JS caches it on the
    // element, so a destroyed or re-created element starts afresh.
    let cached_children_length: Rc<RefCell<HashMap<NodeKey, usize>>> = Rc::default();
    let weak = editor.weak();
    editor.register_mutation_listener(
        "code",
        move |mutations, _| {
            let Some(editor) = weak.upgrade().map(Editor) else {
                return;
            };
            let mut cache = cached_children_length.borrow_mut();
            let _ = editor.read_with_mode(crate::EditorReadMode::Latest, || {
                for (key, mutation) in mutations {
                    match mutation {
                        NodeMutation::Destroyed => {
                            cache.remove(key);
                            continue;
                        }
                        NodeMutation::Created => {
                            cache.remove(key);
                        }
                        NodeMutation::Updated => {}
                    }
                    let Some(node) = get_node_by_key(key) else {
                        continue;
                    };
                    let children = node.get_children();
                    if cache.get(key) == Some(&children.len()) {
                        continue;
                    }
                    cache.insert(key.clone(), children.len());
                    set_gutter(key, &code_gutter(&children));
                }
            });
        },
        MutationListenerOptions {
            skip_initialization: false,
        },
    )
}
