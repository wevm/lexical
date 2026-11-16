//! Ports of `packages/lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts`.
//!
//! JS takes any number as an index; the Rust API takes a `usize`, so
//! `Infinity` is `usize::MAX` (which clamps the same way), and the negative,
//! fractional and NaN indices are skipped as deviations
//! (`rust/parity/skips.json`).

use crate::caret_utils::get_adjacent_nodes;
use crate::{
    CaretDirection, CreateEditorArgs, LexicalError, NodeRef, UpdateOptions, create_editor,
    create_paragraph_node, create_text_node, get_child_caret, get_child_caret_at_index, get_root,
    get_sibling_caret,
};

/// `buildEditorFromExtensions({name})`, then a discrete update.
fn run(name: &str, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    let editor = create_editor(CreateEditorArgs {
        namespace: name.into(),
        ..CreateEditorArgs::default()
    });
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

/// `$createTextNode(text).toggleUnmergeable()`.
fn unmergeable(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_unmergeable();
    node
}

/// The indices of upstream's `indices` a `usize` can hold.
const INDICES: [usize; 6] = [0, 1, 2, 4, 5, usize::MAX];

// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 0 preserves clamping and direction
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 1 preserves clamping and direction
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 2 preserves clamping and direction
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 4 preserves clamping and direction
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 5 preserves clamping and direction
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at Infinity preserves clamping and direction
#[test]
fn child_caret_preserves_clamping_and_direction() {
    for index in INDICES {
        run("caret-index", move || {
            let children: Vec<NodeRef> = ["a", "b", "c", "d"].map(unmergeable).into();
            let parent = create_paragraph_node();
            parent.append(&children)?;
            get_root().clear()?;
            get_root().append(std::slice::from_ref(&parent))?;
            let offset = if index > 0 { index.min(4) } else { 0 };
            let next = get_child_caret_at_index(&parent, index, CaretDirection::Next);
            let previous = get_child_caret_at_index(&parent, index, CaretDirection::Previous);
            assert_eq!(
                next.get_node_at_caret(),
                children.get(offset).cloned(),
                "{index}"
            );
            assert_eq!(
                previous.get_node_at_caret(),
                offset.checked_sub(1).map(|i| children[i].clone()),
                "{index}"
            );
            assert!(next.get_flipped()?.is_same_node_caret(&previous), "{index}");
            assert_eq!(next.get_parent_at_caret(), Some(parent.clone()));
            assert_eq!(previous.get_parent_at_caret(), Some(parent.clone()));
            Ok(())
        });
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 0 stays at its boundary
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 1 stays at its boundary
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 2 stays at its boundary
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 4 stays at its boundary
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 5 stays at its boundary
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at Infinity stays at its boundary
#[test]
fn child_caret_in_an_empty_element_stays_at_its_boundary() {
    for index in INDICES {
        run("caret-empty-index", move || {
            let parent = create_paragraph_node();
            get_root().clear()?;
            get_root().append(std::slice::from_ref(&parent))?;
            for direction in [CaretDirection::Next, CaretDirection::Previous] {
                assert!(
                    get_child_caret_at_index(&parent, index, direction)
                        .is_same_node_caret(&get_child_caret(&parent, direction).unwrap()),
                    "{index} {direction:?}"
                );
            }
            Ok(())
        });
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::node collectors and retained carets follow moved siblings in document order
#[test]
fn node_collectors_and_retained_carets_follow_moved_siblings_in_document_order() {
    run("caret-collect", || {
        let [a, b, c, d] = ["a", "b", "c", "d"].map(unmergeable);
        let parent = create_paragraph_node();
        parent.append(&[a.clone(), b.clone(), c.clone(), d.clone()])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&parent))?;
        let start = get_child_caret(&parent, CaretDirection::Next).unwrap();
        let end = get_child_caret(&parent, CaretDirection::Previous).unwrap();
        b.insert_before(&d, true)?;
        assert_eq!(
            parent.get_children(),
            [a.clone(), d.clone(), b.clone(), c.clone()]
        );
        assert_eq!(b.get_previous_siblings(), [a.clone(), d.clone()]);
        assert_eq!(d.get_next_siblings(), [b.clone(), c.clone()]);
        assert_eq!(
            get_adjacent_nodes(&start),
            [a.clone(), d.clone(), b.clone(), c.clone()]
        );
        assert_eq!(
            get_adjacent_nodes(&end),
            [c.clone(), b.clone(), d.clone(), a.clone()]
        );
        assert_eq!(
            get_adjacent_nodes(&get_sibling_caret(&b, CaretDirection::Previous)),
            [d, a]
        );
        b.remove(false)?;
        assert_eq!(b.get_previous_siblings(), []);
        assert_eq!(b.get_next_siblings(), []);
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::indexed descendants distinguish child starts, document end and empty children
#[test]
fn indexed_descendants_distinguish_child_starts_document_end_and_empty_children() {
    run("caret-descendant-index", || {
        let a = unmergeable("a");
        let b = unmergeable("b");
        let empty = create_paragraph_node();
        let root = get_root();
        root.clear()?;
        let first = create_paragraph_node();
        first.append(&[a.clone(), b.clone()])?;
        root.append(&[first, empty.clone()])?;
        assert_eq!(root.get_descendant_by_index(0), Some(a));
        assert_eq!(root.get_descendant_by_index(1), Some(empty.clone()));
        assert_eq!(root.get_descendant_by_index(2), Some(empty.clone()));
        empty.remove(false)?;
        assert_eq!(root.get_descendant_by_index(1), Some(b.clone()));
        assert_eq!(root.get_descendant_by_index(usize::MAX), Some(b));
        // JS also returns null for -1, 0.5 and NaN, which a `usize` can't be.
        root.clear()?;
        assert_eq!(root.get_descendant_by_index(0), None);
        Ok(())
    });
}
