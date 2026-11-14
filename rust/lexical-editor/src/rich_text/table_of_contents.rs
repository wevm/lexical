//! `packages/lexical-react/src/LexicalTableOfContentsPlugin.tsx`: the logic
//! of `@lexical/react`'s `TableOfContentsPlugin`, which keeps the
//! document's headings in order for a view to list. React keeps them in a
//! component whose children render the entries; here, as in Swift's
//! `registerTableOfContents`, a registration reports them to a closure each
//! time the plugin would set its state.
//!
//! Its mutation listeners only hear from the reconciler, so an editor
//! reports created, destroyed and updated headings once it has a root
//! element ([`Editor::set_root_element`]), as in JS.

use super::{HeadingTagType, is_heading_node};
use crate::utils::get_next_right_preorder_node;
use crate::{
    Editor, LexicalError, MutationListenerOptions, NodeKey, NodeMutation, NodeRef, Unregister,
    get_node_by_key, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

/// The entries, in document order.
type TableOfContents = Vec<TableOfContentsEntry>;

/// What hears the entries each time they change.
type OnChange = dyn Fn(&[TableOfContentsEntry]);

/// `TableOfContentsEntry`: a heading's key, its text content and its tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableOfContentsEntry {
    pub key: NodeKey,
    pub text: String,
    pub tag: HeadingTagType,
}

/// `toEntry(heading)`.
fn to_entry(heading: &NodeRef) -> TableOfContentsEntry {
    TableOfContentsEntry {
        key: heading.get_key().to_owned(),
        text: heading.get_text_content(),
        tag: heading.get_heading_tag().expect("a heading"),
    }
}

/// `$insertHeadingIntoTableOfContents(prevHeading, newHeading,
/// currentTableOfContents)`.
fn insert_heading_into_table_of_contents(
    prev_heading: Option<&NodeRef>,
    new_heading: &NodeRef,
    current_table_of_contents: &[TableOfContentsEntry],
) -> Vec<TableOfContentsEntry> {
    let new_entry = to_entry(new_heading);
    let Some(prev_heading) = prev_heading else {
        // Check if the key already exists.
        if current_table_of_contents
            .first()
            .is_some_and(|e| e.key == new_heading.get_key())
        {
            return current_table_of_contents.to_vec();
        }
        let mut new_table_of_contents = vec![new_entry];
        new_table_of_contents.extend_from_slice(current_table_of_contents);
        return new_table_of_contents;
    };
    let mut new_table_of_contents = Vec::new();
    for (i, entry) in current_table_of_contents.iter().enumerate() {
        new_table_of_contents.push(entry.clone());
        if entry.key == prev_heading.get_key() && entry.key != new_heading.get_key() {
            // Check if the key already exists.
            if current_table_of_contents
                .get(i + 1)
                .is_some_and(|e| e.key == new_heading.get_key())
            {
                return current_table_of_contents.to_vec();
            }
            new_table_of_contents.push(new_entry.clone());
        }
    }
    new_table_of_contents
}

/// `$deleteHeadingFromTableOfContents(key, currentTableOfContents)`.
fn delete_heading_from_table_of_contents(
    key: &str,
    current_table_of_contents: &[TableOfContentsEntry],
) -> Vec<TableOfContentsEntry> {
    current_table_of_contents
        .iter()
        .filter(|e| e.key != key)
        .cloned()
        .collect()
}

/// `$updateHeadingInTableOfContents(heading, currentTableOfContents)`.
fn update_heading_in_table_of_contents(
    heading: &NodeRef,
    current_table_of_contents: &[TableOfContentsEntry],
) -> Vec<TableOfContentsEntry> {
    current_table_of_contents
        .iter()
        .map(|e| {
            if e.key == heading.get_key() {
                to_entry(heading)
            } else {
                e.clone()
            }
        })
        .collect()
}

/// `$updateHeadingPosition(prevHeading, heading, currentTableOfContents)`:
/// the entries with `heading` after `prev_heading`, or first without one.
fn update_heading_position(
    prev_heading: Option<&NodeRef>,
    heading: &NodeRef,
    current_table_of_contents: &[TableOfContentsEntry],
) -> Vec<TableOfContentsEntry> {
    let mut new_table_of_contents = Vec::new();
    let new_entry = to_entry(heading);
    if prev_heading.is_none() {
        new_table_of_contents.push(new_entry.clone());
    }
    for entry in current_table_of_contents {
        if entry.key == heading.get_key() {
            continue;
        }
        new_table_of_contents.push(entry.clone());
        if prev_heading.is_some_and(|p| entry.key == p.get_key()) {
            new_table_of_contents.push(new_entry.clone());
        }
    }
    new_table_of_contents
}

/// `$getPreviousHeading(node)`: the heading before `node` in the
/// document, by `$getNextRightPreorderNode`.
fn get_previous_heading(node: &NodeRef) -> Option<NodeRef> {
    let mut prev_heading = get_next_right_preorder_node(node);
    while let Some(node) = prev_heading.as_ref().filter(|n| !is_heading_node(n)) {
        prev_heading = get_next_right_preorder_node(node);
    }
    prev_heading
}

/// `TableOfContentsPlugin`: reports the document's headings, in order, to
/// `on_change` at once and each time they change, as the plugin sets its
/// state. Fails when the editor doesn't register `HeadingNode`. Returns
/// the clean-up.
pub fn register_table_of_contents(
    editor: &Editor,
    on_change: impl Fn(&[TableOfContentsEntry]) + 'static,
) -> Result<Unregister, LexicalError> {
    // JS's `registerMutationListener` throws for a class the editor doesn't
    // register; checked first, so nothing is left registered.
    if !editor.registered_nodes().contains_key("heading") {
        return Err(LexicalError::new(
            "Node HeadingNode has not been registered. Ensure node has been passed to createEditor.",
        ));
    }
    let on_change: Rc<OnChange> = Rc::new(on_change);
    // Set the table of contents' initial state.
    let current: Rc<RefCell<TableOfContents>> = Rc::default();
    editor.get_editor_state().read(|| {
        fn update_current_table_of_contents(
            node: &NodeRef,
            current: &mut Vec<TableOfContentsEntry>,
        ) {
            for child in node.get_children() {
                if is_heading_node(&child) {
                    current.push(to_entry(&child));
                } else if child.is_element() {
                    update_current_table_of_contents(&child, current);
                }
            }
        }
        update_current_table_of_contents(&get_root(), &mut current.borrow_mut());
    });
    let set = {
        let current = current.clone();
        move |next: Vec<TableOfContentsEntry>| {
            *current.borrow_mut() = next.clone();
            on_change(&next);
        }
    };
    let set: Rc<dyn Fn(Vec<TableOfContentsEntry>)> = Rc::new(set);
    let initial = current.borrow().clone();
    set(initial);

    let remove_root_update_listener = {
        let (current, set) = (current.clone(), set.clone());
        editor.register_update_listener(move |payload| {
            payload.editor_state.read(|| {
                fn update_child_headings(
                    node: &NodeRef,
                    current: &RefCell<Vec<TableOfContentsEntry>>,
                    set: &dyn Fn(Vec<TableOfContentsEntry>),
                ) {
                    for child in node.get_children() {
                        if is_heading_node(&child) {
                            let prev_heading = get_previous_heading(&child);
                            let next = update_heading_position(
                                prev_heading.as_ref(),
                                &child,
                                &current.borrow(),
                            );
                            set(next);
                        } else if child.is_element() {
                            update_child_headings(&child, current, set);
                        }
                    }
                }
                // If a node changes, all of its child headings' positions
                // need to be updated.
                for node in get_root().get_children() {
                    let dirty = payload
                        .dirty_elements
                        .iter()
                        .any(|(key, intentional)| key == node.get_key() && *intentional);
                    if node.is_element() && dirty {
                        update_child_headings(&node, &current, &*set);
                    }
                }
            });
        })
    };

    // Listen to heading mutations and update the state.
    let remove_header_mutation_listener = {
        let (current, set) = (current.clone(), set.clone());
        let weak = editor.weak();
        editor.register_mutation_listener(
            "heading",
            move |mutated_nodes, _| {
                let Some(editor) = weak.upgrade().map(Editor) else {
                    return;
                };
                editor.get_editor_state().read(|| {
                    let mut next = current.borrow().clone();
                    for (node_key, mutation) in mutated_nodes {
                        match mutation {
                            NodeMutation::Created => {
                                if let Some(new_heading) =
                                    get_node_by_key(node_key).filter(is_heading_node)
                                {
                                    let prev_heading = get_previous_heading(&new_heading);
                                    next = insert_heading_into_table_of_contents(
                                        prev_heading.as_ref(),
                                        &new_heading,
                                        &next,
                                    );
                                }
                            }
                            NodeMutation::Destroyed => {
                                next = delete_heading_from_table_of_contents(node_key, &next);
                            }
                            NodeMutation::Updated => {
                                if let Some(new_heading) =
                                    get_node_by_key(node_key).filter(is_heading_node)
                                {
                                    let prev_heading = get_previous_heading(&new_heading);
                                    next = update_heading_position(
                                        prev_heading.as_ref(),
                                        &new_heading,
                                        &next,
                                    );
                                }
                            }
                        }
                    }
                    set(next);
                });
            },
            // Initialization is handled separately.
            MutationListenerOptions {
                skip_initialization: true,
            },
        )?
    };

    // Listen to text node mutations.
    let remove_text_node_mutation_listener = {
        let (current, set) = (current.clone(), set.clone());
        let weak = editor.weak();
        editor.register_mutation_listener(
            "text",
            move |mutated_nodes, _| {
                let Some(editor) = weak.upgrade().map(Editor) else {
                    return;
                };
                editor.get_editor_state().read(|| {
                    for (node_key, mutation) in mutated_nodes {
                        if *mutation != NodeMutation::Updated {
                            continue;
                        }
                        // JS's `getParentOrThrow` throws out of the listener
                        // for a node without a parent; a Rust listener has no
                        // error channel, so such a node is skipped.
                        if let Some(parent_node) = get_node_by_key(node_key)
                            .and_then(|n| n.get_parent_or_throw().ok())
                            .filter(is_heading_node)
                        {
                            let next = update_heading_in_table_of_contents(
                                &parent_node,
                                &current.borrow(),
                            );
                            set(next);
                        }
                    }
                });
            },
            // Initialization is handled separately.
            MutationListenerOptions {
                skip_initialization: true,
            },
        )?
    };

    Ok(Box::new(move || {
        remove_header_mutation_listener();
        remove_text_node_mutation_listener();
        remove_root_update_listener();
    }))
}
