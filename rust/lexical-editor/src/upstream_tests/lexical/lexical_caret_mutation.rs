//! Ports of `packages/lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts`.
//!
//! `caret insertion dispatches to the %s node method` spies on
//! `TextNode.prototype` and is skipped as a deviation
//! (`rust/parity/skips.json`).

use crate::{
    CaretDirection, CreateEditorArgs, Editor, LexicalError, NodeRef, PointType, UpdateOptions,
    Which, create_editor, create_paragraph_node, create_range_selection, create_text_node,
    get_root, get_selection, get_sibling_caret, set_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// `buildEditorFromExtensions({name})`.
fn editor(name: &str) -> Editor {
    create_editor(CreateEditorArgs {
        namespace: name.into(),
        ..CreateEditorArgs::default()
    })
}

fn key(node: Option<&NodeRef>) -> Option<String> {
    node.map(|n| n.get_key().to_owned())
}

/// `$expectChildren(parent, children)`.
fn expect_children(parent: &NodeRef, children: &[NodeRef]) {
    assert_eq!(parent.get_children_size(), children.len());
    assert_eq!(
        key(parent.get_first_child().as_ref()),
        key(children.first())
    );
    assert_eq!(key(parent.get_last_child().as_ref()), key(children.last()));
    for (index, node) in children.iter().enumerate() {
        assert_eq!(key(node.get_parent().as_ref()), key(Some(parent)));
        assert_eq!(
            key(node.get_previous_sibling().as_ref()),
            key(index.checked_sub(1).and_then(|i| children.get(i)))
        );
        assert_eq!(
            key(node.get_next_sibling().as_ref()),
            key(children.get(index + 1))
        );
    }
    let keys: Vec<String> = children.iter().map(|n| n.get_key().to_owned()).collect();
    assert_eq!(parent.get_children_keys(), keys);
}

#[derive(Clone)]
struct Fixture {
    children: Vec<NodeRef>,
    parent: NodeRef,
}

/// `$createFixture()`: four unmergeable text nodes "a" to "d" in a
/// paragraph at the root.
fn create_fixture() -> Result<Fixture, LexicalError> {
    let children: Vec<NodeRef> = ["a", "b", "c", "d"].map(unmergeable).into();
    let parent = create_paragraph_node();
    parent.append(&children)?;
    get_root().clear()?;
    get_root().append(std::slice::from_ref(&parent))?;
    Ok(Fixture { children, parent })
}

/// `$createTextNode(text).toggleUnmergeable()`.
fn unmergeable(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_unmergeable();
    node
}

/// `editor.update(() => { fixture = $createFixture(); }, {discrete: true})`.
fn fixture_in(editor: &Editor) -> Fixture {
    let fixture: Rc<RefCell<Option<Fixture>>> = Rc::default();
    let out = fixture.clone();
    editor
        .update(
            move || {
                *out.borrow_mut() = Some(create_fixture()?);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    fixture.borrow().clone().unwrap()
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::replacement with an existing previous sibling preserves both links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::replacement with an existing next sibling preserves both links
#[test]
fn replacement_with_an_existing_sibling_preserves_both_links() {
    for direction in [CaretDirection::Previous, CaretDirection::Next] {
        let editor = editor("caret-mutation-links");
        let Fixture { parent, children } = fixture_in(&editor);
        editor
            .update(
                move || {
                    let [a, b, c, d] = <[NodeRef; 4]>::try_from(children).unwrap();
                    let replacement = if direction == CaretDirection::Previous {
                        &a
                    } else {
                        &c
                    };
                    b.replace(replacement, false)?;
                    expect_children(&parent, &[a.clone(), c.clone(), d]);
                    assert!(!b.is_attached());
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 0 previous from 1 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 2 previous from 1 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 3 previous from 1 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 0 previous from 2 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 1 previous from 2 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 3 previous from 2 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 0 next from 1 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 2 next from 1 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 3 next from 1 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 0 next from 2 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 1 next from 2 preserves committed links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 3 next from 2 preserves committed links
#[test]
fn moving_a_sibling_preserves_committed_links() {
    for direction in [CaretDirection::Previous, CaretDirection::Next] {
        for origin_index in [1, 2] {
            for source_index in (0..4).filter(|&i| i != origin_index) {
                let editor = editor("caret-mutation-move");
                let fixture = fixture_in(&editor);
                let previous_state = editor.get_editor_state();
                let Fixture { parent, children } = fixture.clone();
                editor
                    .update(
                        move || {
                            let source = children[source_index].clone();
                            let origin = children[origin_index].clone();
                            let mut expected: Vec<NodeRef> =
                                children.iter().filter(|&n| n != &source).cloned().collect();
                            let at = expected.iter().position(|n| n == &origin).unwrap()
                                + usize::from(direction == CaretDirection::Next);
                            expected.insert(at, source.clone());
                            get_sibling_caret(&origin, direction).insert(&source)?;
                            expect_children(&parent, &expected);
                            Ok(())
                        },
                        discrete(),
                    )
                    .unwrap();
                previous_state.read(|| expect_children(&fixture.parent, &fixture.children));
            }
        }
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(0, 0, [ 3 ]) repairs both boundary links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(2, 1, [ 1, 3 ]) repairs both boundary links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(0, 2, [ 1, 0 ]) repairs both boundary links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(0, 4, []) repairs both boundary links
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(1, 2, [ 2, 1 ]) repairs both boundary links
#[test]
fn splice_repairs_both_boundary_links() {
    type Case = (usize, usize, &'static [usize], &'static [usize]);
    let cases: [Case; 5] = [
        (0, 0, &[3], &[3, 0, 1, 2]),
        (2, 1, &[1, 3], &[0, 1, 3]),
        (0, 2, &[1, 0], &[1, 0, 2, 3]),
        (0, 4, &[], &[]),
        (1, 2, &[2, 1], &[0, 2, 1, 3]),
    ];
    for (start, count, insert, expected) in cases {
        let editor = editor("caret-mutation-splice");
        let fixture = fixture_in(&editor);
        let previous_state = editor.get_editor_state();
        let Fixture { parent, children } = fixture.clone();
        editor
            .update(
                move || {
                    let nodes: Vec<NodeRef> = insert.iter().map(|&i| children[i].clone()).collect();
                    parent.splice(start, count, &nodes)?;
                    let expected_nodes: Vec<NodeRef> =
                        expected.iter().map(|&i| children[i].clone()).collect();
                    expect_children(&parent, &expected_nodes);
                    for (index, child) in children.iter().enumerate() {
                        if !expected.contains(&index) {
                            assert_eq!(child.get_parent(), None);
                            assert_eq!(child.get_previous_sibling(), None);
                            assert_eq!(child.get_next_sibling(), None);
                        }
                    }
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
        previous_state.read(|| expect_children(&fixture.parent, &fixture.children));
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertBefore, backward=false, offset=0)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertBefore, backward=false, offset=2)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertBefore, backward=true, offset=0)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertBefore, backward=true, offset=2)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertAfter, backward=false, offset=0)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertAfter, backward=false, offset=2)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertAfter, backward=true, offset=0)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertAfter, backward=true, offset=2)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (replace, backward=false, offset=0)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (replace, backward=false, offset=2)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (replace, backward=true, offset=0)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (replace, backward=true, offset=2)
#[test]
fn moving_a_text_endpoint_beside_its_parent_endpoint() {
    for operation in ["insertBefore", "insertAfter", "replace"] {
        for backward in [false, true] {
            for offset in [0, 2] {
                let case = format!("{operation}, backward={backward}, offset={offset}");
                editor("move-mixed-endpoints")
                    .update(
                        move || {
                            let first = unmergeable("first");
                            let moved = unmergeable("second");
                            let parent = create_paragraph_node();
                            parent.append(&[first.clone(), moved.clone()])?;
                            get_root().append(std::slice::from_ref(&parent))?;
                            let selection = create_range_selection();
                            let (text_point, element_point) = if backward {
                                (Which::Focus, Which::Anchor)
                            } else {
                                (Which::Anchor, Which::Focus)
                            };
                            selection.point_set(text_point, moved.get_key(), 1, PointType::Text)?;
                            selection.point_set(
                                element_point,
                                parent.get_key(),
                                offset,
                                PointType::Element,
                            )?;
                            set_selection(Some(selection));
                            match operation {
                                "insertBefore" => first.insert_before(&moved, true).map(drop),
                                "insertAfter" => first.insert_after(&moved, true).map(drop),
                                _ => first.replace(&moved, false).map(drop),
                            }?;
                            expect_children(
                                &parent,
                                &match operation {
                                    "replace" => vec![moved.clone()],
                                    "insertBefore" => vec![moved.clone(), first],
                                    _ => vec![first, moved.clone()],
                                },
                            );
                            let after = get_selection();
                            assert!(after.as_ref().is_some_and(|s| s.is_range()), "{case}");
                            let after = after.unwrap();
                            let retained = after.point(text_point).unwrap();
                            assert_eq!(retained.key, moved.get_key(), "{case}");
                            assert_eq!(retained.point_type, PointType::Text, "{case}");
                            assert_eq!(retained.offset, 1, "{case}");
                            for which in [Which::Anchor, Which::Focus] {
                                assert!(after.point(which).unwrap().get_node()?.is_attached());
                            }
                            // A primed direction cache must not survive the
                            // structural move.
                            assert_eq!(
                                after.is_backward()?,
                                after.clone_selection().is_backward()?,
                                "{case}"
                            );
                            Ok(())
                        },
                        discrete(),
                    )
                    .unwrap();
            }
        }
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::invalidates text-only selection direction after insertBefore reorders siblings
// upstream: lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::invalidates text-only selection direction after insertAfter reorders siblings
#[test]
fn invalidates_text_only_selection_direction_after_reordering_siblings() {
    for operation in ["insertBefore", "insertAfter"] {
        editor("move-text-endpoints")
            .update(
                move || {
                    let first = unmergeable("first");
                    let second = unmergeable("second");
                    let parent = create_paragraph_node();
                    parent.append(&[first.clone(), second.clone()])?;
                    get_root().append(std::slice::from_ref(&parent))?;
                    let selection = create_range_selection();
                    selection.point_set(Which::Anchor, first.get_key(), 1, PointType::Text)?;
                    selection.point_set(Which::Focus, second.get_key(), 1, PointType::Text)?;
                    set_selection(Some(selection.clone()));
                    assert!(!selection.is_backward()?);
                    if operation == "insertAfter" {
                        second.insert_after(&first, true)?;
                    } else {
                        first.insert_before(&second, true)?;
                    }
                    expect_children(&parent, &[second, first]);
                    assert!(selection.is_backward()?, "{operation}");
                    assert_eq!(
                        selection.is_backward()?,
                        selection.clone_selection().is_backward()?
                    );
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
    }
}
