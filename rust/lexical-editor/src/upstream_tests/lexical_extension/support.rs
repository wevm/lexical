//! Helpers shared by the ports of `@lexical/extension`'s BlockId tests,
//! which each upstream file defines alike.

#![allow(dead_code)]

use crate::extension::GenerateId;
use crate::{
    Editor, HISTORY_MERGE_TAG, LexicalError, NodeRef, UpdateOptions, create_paragraph_node,
    create_text_node, get_root,
};
use std::cell::Cell;
use std::rc::Rc;

/// `counter(prefix)`: `${prefix}1`, `${prefix}2`, ...
pub(crate) fn counter(prefix: &str) -> GenerateId {
    let prefix = prefix.to_owned();
    let n = Cell::new(0);
    GenerateId(Rc::new(move || {
        n.set(n.get() + 1);
        format!("{prefix}{}", n.get())
    }))
}

/// `{discrete: true, tag}`.
pub(crate) fn tagged(tags: &[&str]) -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        ..UpdateOptions::default()
    }
}

/// `editor.update(fn, {discrete: true, tag})`, which panics if it fails.
pub(crate) fn update_tagged(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
    tags: &[&str],
) {
    editor.update(f, tagged(tags)).unwrap();
}

/// `asEdited(editor)`: the editor has had content once, so the first
/// blocks a test adds are edits, not a load.
pub(crate) fn as_edited<T: std::ops::Deref<Target = Editor>>(editor: T) -> T {
    update_tagged(
        &editor,
        || {
            get_root().append(&[create_paragraph_node()])?;
            Ok(())
        },
        &[HISTORY_MERGE_TAG],
    );
    update_tagged(&editor, || get_root().clear(), &[HISTORY_MERGE_TAG]);
    editor
}

/// `$paragraph(text)`.
pub(crate) fn paragraph(text: &str) -> NodeRef {
    let p = create_paragraph_node();
    p.append(&[create_text_node(text)]).unwrap();
    p
}

/// `$nth(index)`: the root's child at `index`, an element.
pub(crate) fn nth(index: usize) -> NodeRef {
    let node = get_root()
        .get_child_at_index(index)
        .unwrap_or_else(|| panic!("child {index}"));
    assert!(node.is_element(), "child {index} is an element");
    node
}

/// `$caret(index, offset)`: a collapsed selection `offset` characters into
/// block `index`.
pub(crate) fn caret(index: usize, offset: usize) -> Result<(), LexicalError> {
    let block = nth(index);
    match block.get_first_descendant().filter(NodeRef::is_text) {
        Some(text) => text.select(Some(offset), Some(offset))?,
        None => block.select(Some(0), Some(0))?,
    };
    Ok(())
}

/// `$getRoot().clear().append(...nodes)`.
pub(crate) fn set_root_children(nodes: &[NodeRef]) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(nodes)?;
    Ok(())
}

/// `seed(editor, texts)`: the root holds a paragraph per text.
pub(crate) fn seed(editor: &Editor, texts: &[&str]) {
    let texts: Vec<String> = texts.iter().map(|t| (*t).to_owned()).collect();
    super::super::support::update(editor, move || {
        set_root_children(&texts.iter().map(|t| paragraph(t)).collect::<Vec<_>>())
    });
}

/// The strings of `ids`, as `Some`.
pub(crate) fn some(ids: &[&str]) -> Vec<Option<String>> {
    ids.iter().map(|id| Some((*id).to_owned())).collect()
}

/// The strings, owned.
pub(crate) fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}
