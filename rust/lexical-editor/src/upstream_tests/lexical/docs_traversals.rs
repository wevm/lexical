//! Ports of `packages/lexical/src/caret/__tests__/unit/docs-traversals.test.ts`,
//! the examples of `docs/concepts/traversals.md`. Upstream's
//! `initializeUnitTest` editor is an editor with `LinkNode` registered; its
//! `beforeEach` update builds the document, then each test runs a discrete
//! update.
#![cfg(feature = "link")]

use crate::link::{LinkAttributes, LinkNode, create_link_node};
use crate::{
    CaretDirection, CreateEditorArgs, LexicalError, NodeCaret, NodeClass, NodeRef, PointCaret,
    RootMode, UpdateOptions, create_editor, create_paragraph_node, create_text_node,
    get_caret_range, get_child_caret, get_root, get_sibling_caret,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

const NEXT: CaretDirection = CaretDirection::Next;
const PREVIOUS: CaretDirection = CaretDirection::Previous;

#[derive(Clone)]
struct Doc {
    paragraph_a: NodeRef,
    text_a1: NodeRef,
    link_a2: NodeRef,
    text_a3: NodeRef,
    text_a4: NodeRef,
    paragraph_b: NodeRef,
    text_b1: NodeRef,
    paragraph_c: NodeRef,
}

/// The `beforeEach` update, then `body` in a discrete update.
fn with_doc(body: impl FnOnce(Doc) -> Result<(), LexicalError> + 'static) {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<LinkNode>()],
        ..CreateEditorArgs::default()
    });
    let doc: Rc<RefCell<Option<Doc>>> = Rc::default();
    let out = doc.clone();
    editor
        .update(
            move || {
                let paragraph_a = create_paragraph_node();
                let text_a1 = create_text_node("Text A1");
                let link_a2 = create_link_node(
                    "https://lexical.dev/docs/concepts/traversals",
                    &LinkAttributes::default(),
                );
                let text_a3 = create_text_node("Text A3");
                let text_a4 = create_text_node("Text A4");
                let paragraph_b = create_paragraph_node();
                let text_b1 = create_text_node("Text B1");
                let paragraph_c = create_paragraph_node();
                // Root
                // * Paragraph A
                //   * Text A1
                //   * Link A2
                //     * Text A3
                //   * Text A4
                // * Paragraph B
                //   * Text B1
                // * Paragraph C
                link_a2.append(std::slice::from_ref(&text_a3))?;
                paragraph_a.append(&[text_a1.clone(), link_a2.clone(), text_a4.clone()])?;
                paragraph_b.append(std::slice::from_ref(&text_b1))?;
                get_root().clear()?;
                get_root().append(&[
                    paragraph_a.clone(),
                    paragraph_b.clone(),
                    paragraph_c.clone(),
                ])?;
                *out.borrow_mut() = Some(Doc {
                    paragraph_a,
                    text_a1,
                    link_a2,
                    text_a3,
                    text_a4,
                    paragraph_b,
                    text_b1,
                    paragraph_c,
                });
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    let doc = doc.borrow().clone().unwrap();
    editor
        .update(
            move || body(doc),
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

fn child(node: &NodeRef, direction: CaretDirection) -> PointCaret {
    get_child_caret(node, direction).expect("an element")
}

fn sibling(node: &NodeRef, direction: CaretDirection) -> PointCaret {
    get_sibling_caret(node, direction)
}

/// The depth first carets of the whole document, from the root's child
/// caret.
fn depth_first(d: &Doc) -> Vec<PointCaret> {
    vec![
        child(&d.paragraph_a, NEXT), // enter Paragraph A
        sibling(&d.text_a1, NEXT),
        child(&d.link_a2, NEXT), // enter Link A2
        sibling(&d.text_a3, NEXT),
        sibling(&d.link_a2, NEXT), // leave Link A2
        sibling(&d.text_a4, NEXT),
        sibling(&d.paragraph_a, NEXT), // leave Paragraph A
        child(&d.paragraph_b, NEXT),   // enter Paragraph B
        sibling(&d.text_b1, NEXT),
        sibling(&d.paragraph_b, NEXT), // leave Paragraph B
        child(&d.paragraph_c, NEXT),   // enter Paragraph C
        sibling(&d.paragraph_c, NEXT), // leave Paragraph C
    ]
}

// upstream: lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Adjacent Caret Traversals > $iterSiblings
#[test]
fn iter_siblings() {
    // Note that a caret already iterates its siblings this way, so this
    // function is not very useful. It starts at the adjacent caret: the
    // start caret points away from its origin, which is not included.
    fn iter_siblings(start_caret: &NodeCaret) -> Vec<NodeCaret> {
        let mut carets = Vec::new();
        let mut caret = start_caret.get_adjacent_caret();
        while let Some(c) = caret {
            caret = c.get_adjacent_caret();
            carets.push(c);
        }
        carets
    }
    with_doc(|d| {
        assert_eq!(
            iter_siblings(&child(&get_root(), NEXT)),
            [
                sibling(&d.paragraph_a, NEXT),
                sibling(&d.paragraph_b, NEXT),
                sibling(&d.paragraph_c, NEXT),
            ]
        );
        // iterSiblings is the same as iterating the caret
        assert_eq!(
            iter_siblings(&child(&get_root(), NEXT)),
            child(&get_root(), NEXT).iter().collect::<Vec<_>>()
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Adjacent Caret Traversals > root has no siblings
#[test]
fn root_has_no_siblings() {
    with_doc(|_| {
        // The root does not have sibling nodes
        let carets: Vec<_> = sibling(&get_root(), NEXT).iter().collect();
        assert_eq!(carets, []);
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Adjacent Caret Traversals > root has paragraph children
#[test]
fn root_has_paragraph_children() {
    with_doc(|d| {
        // The adjacent node to a ChildNode is its first or last child and
        // is always a SiblingNode. It does not traverse deeper.
        let carets: Vec<_> = child(&get_root(), NEXT).iter().collect();
        // next starts at the first child
        assert_eq!(
            carets,
            [
                sibling(&d.paragraph_a, NEXT),
                sibling(&d.paragraph_b, NEXT),
                sibling(&d.paragraph_c, NEXT),
            ]
        );
        // previous starts at the last child
        let prev_carets: Vec<_> = child(&get_root(), PREVIOUS).iter().collect();
        assert_eq!(
            prev_carets,
            [
                sibling(&d.paragraph_c, PREVIOUS),
                sibling(&d.paragraph_b, PREVIOUS),
                sibling(&d.paragraph_a, PREVIOUS),
            ]
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Adjacent Caret Traversals > iteration does not include the origin
#[test]
fn iteration_does_not_include_the_origin() {
    with_doc(|d| {
        // The iteration starts at the node where the head of the "arrow" is
        // pointing, which is away from the origin (the tail of the "arrow").
        let carets: Vec<_> = sibling(&d.paragraph_b, NEXT).iter().collect();
        assert_eq!(carets, [sibling(&d.paragraph_c, NEXT)]);
        let prev_carets: Vec<_> = sibling(&d.paragraph_b, PREVIOUS).iter().collect();
        assert_eq!(prev_carets, [sibling(&d.paragraph_a, PREVIOUS)]);
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Depth First Caret Traversals > $iterCaretsDepthFirst > via generator
#[test]
fn iter_carets_depth_first_via_generator() {
    fn iter_carets_depth_first(start_caret: &NodeCaret) -> Vec<NodeCaret> {
        fn step(prev_caret: &NodeCaret) -> Option<NodeCaret> {
            // Get the adjacent SiblingCaret
            let next_caret = prev_caret.get_adjacent_caret();
            // If there is a sibling, try and get a ChildCaret from it
            next_caret
                .as_ref()
                .and_then(PointCaret::get_child_caret)
                // Return the sibling if there is one
                .or(next_caret)
                // Return a SiblingCaret of the parent, if there is one
                .or_else(|| prev_caret.get_parent_caret(RootMode::Root))
        }
        let mut carets = Vec::new();
        let mut caret = step(start_caret);
        while let Some(c) = caret {
            caret = step(&c);
            carets.push(c);
        }
        carets
    }
    with_doc(|d| {
        assert_eq!(
            iter_carets_depth_first(&child(&get_root(), NEXT)),
            depth_first(&d)
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Depth First Caret Traversals > $iterCaretsDepthFirst > via CaretRange
#[test]
fn iter_carets_depth_first_via_caret_range() {
    fn iter_carets_depth_first(
        start_caret: &NodeCaret,
        end_caret: Option<NodeCaret>,
    ) -> Result<Vec<NodeCaret>, LexicalError> {
        // Use the root as the default end caret, but you might choose to
        // use startCaret.getParentCaret('root') for example
        let end_caret = end_caret.unwrap_or_else(|| sibling(&get_root(), start_caret.direction()));
        Ok(get_caret_range(start_caret.clone(), end_caret)?
            .iter()
            .collect())
    }
    with_doc(|d| {
        assert_eq!(
            iter_carets_depth_first(&child(&get_root(), NEXT), None)?,
            depth_first(&d)
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Depth First Caret Traversals > $iterNodesDepthFirst > includes only wholly included nodes
#[test]
fn iter_nodes_depth_first_includes_only_wholly_included_nodes() {
    fn iter_nodes_depth_first(
        start_caret: &NodeCaret,
        end_caret: Option<NodeCaret>,
    ) -> Result<Vec<NodeRef>, LexicalError> {
        let end_caret = end_caret.unwrap_or_else(|| child(&get_root(), start_caret.direction()));
        let mut seen = HashSet::new();
        let mut nodes = Vec::new();
        for caret in get_caret_range(start_caret.clone(), end_caret)?.iter() {
            let origin = caret.origin();
            if caret.is_child_caret() {
                seen.insert(origin.get_key().to_owned());
            } else if !origin.is_element() || seen.contains(origin.get_key()) {
                // If the origin is an element and we have not seen it as a
                // ChildCaret then it was not entirely in the CaretRange
                nodes.push(origin.clone());
            }
        }
        Ok(nodes)
    }
    with_doc(|d| {
        assert_eq!(
            iter_nodes_depth_first(
                &child(&d.paragraph_a, NEXT),
                Some(child(&d.paragraph_c, NEXT))
            )?,
            [
                // already starting inside paragraphA
                d.text_a1,
                // linkA2 is entered here
                d.text_a3,
                // linkA2 is exited and included
                d.link_a2,
                d.text_a4,
                // paragraphA is exited but not included because it was never
                // entered; paragraphB is entered here
                d.text_b1,
                // paragraphB is exited and included
                d.paragraph_b,
                // paragraphC is entered but never exited so not included
            ]
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Depth First Caret Traversals > full traversal
#[test]
fn full_traversal() {
    with_doc(|d| {
        // A full traversal of the document from root: start with the arrow
        // pointing towards the first child of root, end when the arrow
        // points away from root.
        let carets: Vec<_> = get_caret_range(child(&get_root(), NEXT), sibling(&get_root(), NEXT))?
            .iter()
            .collect();
        assert_eq!(carets, depth_first(&d));
        Ok(())
    });
}
