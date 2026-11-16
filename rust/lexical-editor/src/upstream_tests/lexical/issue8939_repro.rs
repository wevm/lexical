//! Ports of `packages/lexical/src/__tests__/unit/Issue8939Repro.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::{
    TestBlockableDecoratorNode, TestShadowRootNode, create_test_decorator_node_inline,
    create_test_shadow_root_node, range_selection,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::selection_deletion::needs_block_cursor_beside;
use crate::{
    LexicalError, LexicalExtension, NodeClass, NodeRef, PointType, UpdateOptions, Which,
    create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `buildEditorFromExtensions(ext)`.
fn ext() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        nodes: vec![
            NodeClass::of::<TestBlockableDecoratorNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[8939]", ())
    })
    .into()])
    .unwrap()
}

fn update(
    editor: &LexicalEditorWithDispose,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
) {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

/// `$blockDecorator()`.
fn block_decorator() -> NodeRef {
    create_test_decorator_node_inline(false)
}

/// `$shadowRoot(label)`.
fn shadow_root(label: &str) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(label)])?;
    let shadow = create_test_shadow_root_node();
    shadow.append(&[paragraph])?;
    Ok(shadow)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Decorator,
    ShadowRoot,
}

fn make(kind: Kind, label: &str) -> Result<NodeRef, LexicalError> {
    match kind {
        Kind::Decorator => Ok(block_decorator()),
        Kind::ShadowRoot => shadow_root(label),
    }
}

fn set_root(children: &[NodeRef]) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(children)
}

/// `selection.deleteCharacter(isBackward)` on the range selection.
fn delete_character(editor: &LexicalEditorWithDispose, is_backward: bool) {
    update(editor, move || {
        range_selection().delete_character(is_backward)
    });
}

/// The key of the node the range selection's anchor is in.
fn anchor_key() -> String {
    range_selection().range().unwrap().anchor.key
}

/// `deletes the adjacent $kind (isBackward: $isBackward)`.
fn deletes_the_adjacent(kind: Kind, is_backward: bool) {
    let editor = ext();
    let keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let k = keys.clone();
    update(&editor, move || {
        let a = make(kind, "x")?;
        let b = make(kind, "x")?;
        k.borrow_mut()
            .extend([a.get_key().to_owned(), b.get_key().to_owned()]);
        set_root(&[a, b])?;
        // The block cursor position between the two nodes.
        get_root().select(Some(1), Some(1))?;
        Ok(())
    });
    let keys = keys.take();
    editor
        .read(|| {
            // Both sides render a block cursor, so both must delete the same way.
            let root = get_root();
            assert!(needs_block_cursor_beside(
                &root.get_child_at_index(0).unwrap()
            ));
            assert!(needs_block_cursor_beside(
                &root.get_child_at_index(1).unwrap()
            ));
        })
        .unwrap();
    delete_character(&editor, is_backward);
    editor
        .read(|| {
            assert_eq!(get_root().get_children_size(), 1);
            // Backspace removes the node before the cursor, Delete the one after.
            assert_eq!(
                get_root().get_child_at_index(0).unwrap().get_key(),
                if is_backward { &keys[1] } else { &keys[0] }
            );
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deletes the adjacent decorator (isBackward: true)
#[test]
fn deletes_the_adjacent_decorator_backward() {
    deletes_the_adjacent(Kind::Decorator, true);
}

// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deletes the adjacent decorator (isBackward: false)
#[test]
fn deletes_the_adjacent_decorator_forward() {
    deletes_the_adjacent(Kind::Decorator, false);
}

// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deletes the adjacent shadowRoot (isBackward: true)
#[test]
fn deletes_the_adjacent_shadow_root_backward() {
    deletes_the_adjacent(Kind::ShadowRoot, true);
}

// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deletes the adjacent shadowRoot (isBackward: false)
#[test]
fn deletes_the_adjacent_shadow_root_forward() {
    deletes_the_adjacent(Kind::ShadowRoot, false);
}

// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > Backspace after the last shadow root deletes it
#[test]
fn backspace_after_the_last_shadow_root_deletes_it() {
    let editor = ext();
    update(&editor, || {
        set_root(&[shadow_root("A")?, shadow_root("B")?])?;
        get_root().select(Some(2), Some(2))?;
        Ok(())
    });
    delete_character(&editor, true);
    editor
        .read(|| {
            assert_eq!(get_root().get_children_size(), 1);
            assert_eq!(get_root().get_text_content(), "A");
        })
        .unwrap();
}

// Removing the last top-level node would otherwise leave the root with no
// children at all, so there is nowhere left to put a caret.
/// `deleting the only $kind leaves the root editable`.
fn deleting_the_only_node_leaves_the_root_editable(kind: Kind) {
    let editor = ext();
    update(&editor, move || {
        set_root(&[make(kind, "A")?])?;
        get_root().select(Some(1), Some(1))?;
        Ok(())
    });
    delete_character(&editor, true);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            let paragraph = &children[0];
            assert_eq!(paragraph.get_type(), "paragraph");
            assert!(paragraph.is_empty());
            // The caret is inside the restored paragraph, not on the root before
            // it, so the next keystroke acts on the paragraph.
            assert_eq!(anchor_key(), paragraph.get_key());
        })
        .unwrap();
    // Enter therefore behaves as it does in any other document holding a
    // single empty paragraph: the caret ends up in the second one.
    update(&editor, || {
        let selection = range_selection();
        selection.insert_paragraph()?;
        selection.insert_text("X")
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert!(children.iter().all(|c| c.get_type() == "paragraph"));
            let texts: Vec<String> = children.iter().map(NodeRef::get_text_content).collect();
            assert_eq!(texts, ["", "X"]);
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deleting the only decorator leaves the root editable
#[test]
fn deleting_the_only_decorator_leaves_the_root_editable() {
    deleting_the_only_node_leaves_the_root_editable(Kind::Decorator);
}

// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deleting the only shadowRoot leaves the root editable
#[test]
fn deleting_the_only_shadow_root_leaves_the_root_editable() {
    deleting_the_only_node_leaves_the_root_editable(Kind::ShadowRoot);
}

// The same rule one level down: a shadow root emptied by the deletion has
// nowhere to put a caret either.
// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deleting the only block inside a shadow root keeps it editable
#[test]
fn deleting_the_only_block_inside_a_shadow_root_keeps_it_editable() {
    let editor = ext();
    update(&editor, || {
        let shadow = create_test_shadow_root_node();
        shadow.append(&[block_decorator()])?;
        set_root(std::slice::from_ref(&shadow))?;
        shadow.select(Some(1), Some(1))?;
        Ok(())
    });
    delete_character(&editor, true);
    editor
        .read(|| {
            let shadow = get_root().get_first_child().unwrap();
            assert_eq!(shadow.get_type(), "test_shadow_root");
            // Previously the shadow root was left with no children at all and the
            // caret on the shadow root itself.
            assert_eq!(shadow.get_children_size(), 1);
            let paragraph = shadow.get_first_child().unwrap();
            assert_eq!(paragraph.get_type(), "paragraph");
            assert!(paragraph.is_empty());
            assert_eq!(anchor_key(), paragraph.get_key());
        })
        .unwrap();
}

// Guard the boundary of the new branch: an empty block whose caret merely
// sits next to a shadow root is not a block cursor, so the existing
// "remove the empty block, keep the shadow root" behaviour must survive.
// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > Backspace in an empty block after a shadow root removes the block, not the shadow root
#[test]
fn backspace_in_an_empty_block_after_a_shadow_root_removes_the_block() {
    let editor = ext();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        set_root(&[shadow_root("A")?, paragraph.clone()])?;
        paragraph.select(Some(0), Some(0))?;
        Ok(())
    });
    delete_character(&editor, true);
    editor
        .read(|| {
            assert_eq!(get_root().get_children_size(), 1);
            assert_eq!(get_root().get_text_content(), "A");
        })
        .unwrap();
}

// Deleting all of a shadow root's content restores the empty paragraph the
// same way the document root does, so Enter over that selection does not
// depend on whether the selection is described with element or text points
// ($selectAll scoped to a top-level shadow root produces the former).
/// `Enter over all of a shadow root splits an empty paragraph ($points points)`.
fn enter_over_all_of_a_shadow_root(element_points: bool) {
    let editor = ext();
    update(&editor, move || {
        let shadow = create_test_shadow_root_node();
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("inside")])?;
        shadow.append(std::slice::from_ref(&paragraph))?;
        set_root(std::slice::from_ref(&shadow))?;
        if element_points {
            shadow.select(Some(0), Some(shadow.get_children_size()))?;
        } else {
            let selection = paragraph.select(Some(0), Some(0))?;
            selection.point_set(
                Which::Focus,
                paragraph.get_first_child_or_throw()?.get_key(),
                "inside".len(),
                PointType::Text,
            )?;
        }
        Ok(())
    });
    update(&editor, || {
        range_selection().insert_paragraph()?;
        Ok(())
    });
    editor
        .read(|| {
            let shadow = get_root().get_first_child().unwrap();
            assert_eq!(shadow.get_type(), "test_shadow_root");
            let paragraphs = shadow.get_children();
            assert!(paragraphs.iter().all(|p| p.get_type() == "paragraph"));
            assert_eq!(
                paragraphs.iter().map(NodeRef::is_empty).collect::<Vec<_>>(),
                [true, true]
            );
            assert_eq!(anchor_key(), paragraphs[1].get_key());
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > Enter over all of a shadow root splits an empty paragraph (element points)
#[test]
fn enter_over_all_of_a_shadow_root_with_element_points() {
    enter_over_all_of_a_shadow_root(true);
}

// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > Enter over all of a shadow root splits an empty paragraph (text points)
#[test]
fn enter_over_all_of_a_shadow_root_with_text_points() {
    enter_over_all_of_a_shadow_root(false);
}

// A plain paragraph never renders a block cursor, so it must not be
// removed as a unit.
// upstream: lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > a paragraph does not need a block cursor beside it
#[test]
fn a_paragraph_does_not_need_a_block_cursor_beside_it() {
    let editor = ext();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("A")])?;
        set_root(&[paragraph])
    });
    editor
        .read(|| {
            assert!(!needs_block_cursor_beside(
                &get_root().get_child_at_index(0).unwrap()
            ));
        })
        .unwrap();
}
