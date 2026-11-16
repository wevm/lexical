//! Ports of `packages/lexical/src/caret/__tests__/unit/LexicalCaret.test.ts`.
//!
//! Upstream's `initializeUnitTest` editor registers the packages' nodes and
//! the test nodes of `__tests__/utils`; here that is [`test_editor`], with
//! [`TestDecoratorNode`] and [`TestElementNode`] ported. The table cases need
//! `@lexical/table` and are skipped (`rust/parity/skips.json`). A JS
//! `beforeEach` update is an update of its own before the test's.
#![cfg(all(
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "mark"
))]

use crate::caret_utils::SliceMode;
use crate::link::{LinkAttributes, LinkNode, create_link_node};
use crate::list::{ListItemNode, ListNode, ListType, create_list_item_node, create_list_node};
use crate::mark::{MarkNode, create_mark_node};
use crate::node::NodeBase;
use crate::nodes::element::ElementData;
use crate::nodes::line_break::{LineBreakNode, create_line_break_node};
use crate::nodes::paragraph::ParagraphNode;
use crate::rich_text::{
    HeadingNode, HeadingTagType, QuoteNode, create_heading_node, is_heading_node,
};
use crate::{
    CaretDirection, CaretRange, CommonAncestorResult, CreateEditorArgs, Editor, EditorReadMode,
    LexicalError, LexicalNode, NodeCaret, NodeClass, NodeRef, PointCaret, PointType, RootMode,
    SelectionRef, TextPointCaretSlice, UpdateOptions, Which, caret_range_from_selection,
    compare_point_caret_next, create, create_editor, create_paragraph_node, create_range_selection,
    create_text_node, extend_caret_to_range, get_caret_range, get_child_caret,
    get_child_caret_at_index, get_common_ancestor, get_root, get_selection, get_sibling_caret,
    get_text_point_caret, remove_text_from_caret_range, rewind_sibling_caret, select_all,
    set_point_from_caret, set_selection, set_selection_from_caret_range, split_at_point_caret_next,
};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

/// A `$makeSelection` of the collapsed cases.
type MakeSelection<'a> = dyn Fn() -> Result<SelectionRef, LexicalError> + 'a;

const NEXT: CaretDirection = CaretDirection::Next;
const PREVIOUS: CaretDirection = CaretDirection::Previous;
const DIRECTIONS: [CaretDirection; 2] = [NEXT, PREVIOUS];

/// `TestDecoratorNode` (`__tests__/utils`): a decorator whose text is
/// "Hello world", inline unless `setIsInline(false)`.
#[derive(Clone, Debug, Default)]
struct TestDecoratorNode {
    base: NodeBase,
    block: bool,
}

impl LexicalNode for TestDecoratorNode {
    fn get_type(&self) -> &'static str {
        "test_decorator"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        !self.block
    }
    fn get_text_content(&self) -> String {
        "Hello world".to_owned()
    }
}

/// `$createTestDecoratorNode()`.
fn create_test_decorator_node() -> NodeRef {
    create::<TestDecoratorNode>()
}

/// `setIsInline(inline)`.
fn set_is_inline(node: &NodeRef, inline: bool) -> NodeRef {
    crate::lexical_utils::with_writable(node.get_key(), |n| {
        n.as_any_mut()
            .downcast_mut::<TestDecoratorNode>()
            .expect("a TestDecoratorNode")
            .block = !inline;
    });
    node.clone()
}

/// `TestElementNode` (`__tests__/utils`): a block element, `test_block`.
#[derive(Clone, Debug, Default)]
struct TestElementNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for TestElementNode {
    fn get_type(&self) -> &'static str {
        "test_block"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
}

/// `$createTestElementNode()`.
fn create_test_element_node() -> NodeRef {
    create::<TestElementNode>()
}

/// `initializeUnitTest`'s editor, with the nodes these tests use.
fn test_editor() -> Editor {
    create_editor(CreateEditorArgs {
        namespace: "test".into(),
        nodes: vec![
            NodeClass::of::<HeadingNode>(),
            NodeClass::of::<QuoteNode>(),
            NodeClass::of::<ListNode>(),
            NodeClass::of::<ListItemNode>(),
            NodeClass::of::<LinkNode>(),
            NodeClass::of::<MarkNode>(),
            NodeClass::of::<TestDecoratorNode>(),
            NodeClass::of::<TestElementNode>(),
        ],
        ..CreateEditorArgs::default()
    })
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// `editor.update(f, {discrete: true})`.
fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

/// `editor.update(f)`, committed at the next read or update.
fn update_later(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, UpdateOptions::default()).unwrap();
}

/// A discrete update of a fresh [`test_editor`].
fn run(f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    update(&test_editor(), f);
}

/// `$createParagraphNode().append(...children)`.
fn paragraph_of(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(children)?;
    Ok(paragraph)
}

/// `node.append(...children)`, returning the node.
fn append(node: NodeRef, children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    node.append(children)?;
    Ok(node)
}

/// `$getRoot().clear().append(...children)`.
fn set_root(children: &[NodeRef]) -> Result<(), LexicalError> {
    get_root().clear()?;
    get_root().append(children)
}

/// `$createTextNode(text).setMode(mode)`.
fn text_with_mode(text: &str, mode: &str) -> NodeRef {
    let node = create_text_node(text);
    node.set_mode(mode).unwrap();
    node
}

fn texts_of(nodes: &[NodeRef]) -> Vec<String> {
    nodes.iter().map(NodeRef::get_text_content).collect()
}

fn child(node: &NodeRef, direction: CaretDirection) -> PointCaret {
    get_child_caret(node, direction).expect("an element")
}

fn range_of(selection: &SelectionRef) -> Result<CaretRange, LexicalError> {
    caret_range_from_selection(&selection.range().expect("a range selection"))
}

/// `range.getTextSlices().filter($isTextPointCaretSlice)`.
fn slices(range: &CaretRange) -> Vec<TextPointCaretSlice> {
    range.get_text_slices().into_iter().flatten().collect()
}

/// `[...range.iterNodeCarets('root')]`.
fn node_carets(range: &CaretRange) -> Vec<NodeCaret> {
    range.iter_node_carets(RootMode::Root).collect()
}

/// `toMatchObject({key, offset, type})` on a selection's point.
fn assert_point(selection: &SelectionRef, which: Which, key: &str, offset: usize, t: PointType) {
    let point = selection.point(which).expect("a range selection");
    assert_eq!(
        (point.key.as_str(), point.offset, point.point_type),
        (key, offset, t),
        "{which:?}"
    );
}

/// `toMatchObject` on a caret: each given field.
fn assert_caret(
    caret: &PointCaret,
    direction: Option<CaretDirection>,
    offset: Option<usize>,
    origin: Option<&NodeRef>,
    caret_type: Option<&str>,
) {
    if let Some(direction) = direction {
        assert_eq!(caret.direction(), direction, "{caret:?}");
    }
    if let Some(offset) = offset {
        assert_eq!(caret.offset(), Some(offset), "{caret:?}");
    }
    if let Some(origin) = origin {
        assert_eq!(caret.origin(), origin, "{caret:?}");
    }
    if let Some(caret_type) = caret_type {
        assert_eq!(caret.caret_type(), caret_type, "{caret:?}");
    }
}

/// `toMatchObject` on a text slice: its caret's fields and its distance.
fn assert_slice(
    slice: &TextPointCaretSlice,
    direction: Option<CaretDirection>,
    offset: usize,
    origin: Option<&NodeRef>,
    distance: isize,
) {
    assert_caret(&slice.caret, direction, Some(offset), origin, Some("text"));
    assert_eq!(slice.distance, distance, "{slice:?}");
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $getChildCaret > direction next
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $getChildCaret > direction previous
#[test]
fn get_child_caret_direction() {
    for direction in DIRECTIONS {
        run(move || {
            let paragraph = create_paragraph_node();
            let root = get_root();
            root.clear()?;
            root.append(std::slice::from_ref(&paragraph))?;
            let caret = child(&root, direction);
            assert_eq!(caret.origin(), &root);
            assert_eq!(caret.direction(), direction);
            assert_eq!(caret.caret_type(), "child");
            assert_eq!(caret.get_node_at_caret(), Some(paragraph.clone()));
            assert_eq!(caret.get_parent_at_caret(), Some(root.clone()));

            let flipped = caret.get_flipped()?;
            assert_ne!(flipped, caret);
            assert!(flipped.get_flipped()?.is_same_node_caret(&caret));
            assert_ne!(flipped.direction(), direction);
            assert_eq!(flipped.caret_type(), "sibling");
            assert_eq!(flipped.get_node_at_caret(), None);
            assert_eq!(flipped.get_adjacent_caret(), None);
            for mode in [RootMode::Root, RootMode::ShadowRoot] {
                assert_eq!(caret.get_parent_caret(mode), None);
                assert_eq!(flipped.get_parent_caret(mode), None);
            }
            let adjacent = caret
                .get_adjacent_caret()
                .expect("depth caret of a non-empty element must always have an adjacent caret");
            assert_eq!(adjacent.origin(), &paragraph);
            assert_eq!(adjacent.caret_type(), "sibling");
            assert_eq!(adjacent.get_adjacent_caret(), None);

            assert_eq!(root.get_children_size(), 1);
            caret.remove()?;
            assert!(root.is_empty());
            caret.replace_or_insert(&paragraph, false)?;
            assert_eq!(root.get_children_size(), 1);
            caret.remove()?;
            caret.insert(&paragraph)?;
            assert_eq!(root.get_children_size(), 1);

            // When direction === 'next' we are prepending the second node,
            // otherwise we are appending it
            let second_paragraph = create_paragraph_node();
            caret.insert(&second_paragraph)?;
            assert_eq!(root.get_children_size(), 2);
            let paragraph_keys = vec![
                paragraph.get_key().to_owned(),
                second_paragraph.get_key().to_owned(),
            ];
            let reversed: Vec<String> = paragraph_keys.iter().rev().cloned().collect();
            let (p, s) = (paragraph.clone(), second_paragraph.clone());
            assert_eq!(
                root.get_children_keys(),
                if direction == NEXT {
                    &reversed
                } else {
                    &paragraph_keys
                }
                .clone()
            );

            caret.splice(2, &[], NEXT)?;
            assert_eq!(root.get_children_size(), 0);
            caret.splice(0, &[p.clone(), s.clone()], NEXT)?;
            assert_eq!(root.get_children_keys(), paragraph_keys);
            caret.splice(0, &[s.clone(), p.clone()], NEXT)?;
            assert_eq!(root.get_children_keys(), reversed);
            caret.splice(0, &[p.clone(), s.clone()], NEXT)?;
            assert_eq!(root.get_children_keys(), paragraph_keys);
            caret.splice(2, &[s.clone(), p.clone()], NEXT)?;
            assert_eq!(root.get_children_keys(), reversed);
            caret.splice(2, &[p.clone(), s.clone()], NEXT)?;
            assert_eq!(root.get_children_keys(), paragraph_keys);
            caret.splice(20, std::slice::from_ref(&p), NEXT)?;
            assert_eq!(root.get_children_keys(), [p.get_key().to_owned()]);
            // JS's deleteCount of -1 deletes nothing, as 0 does.
            caret.splice(0, std::slice::from_ref(&s), NEXT)?;
            assert_eq!(
                root.get_children_keys(),
                if direction == NEXT {
                    reversed.clone()
                } else {
                    paragraph_keys.clone()
                }
            );
            caret.splice(usize::MAX, &[p.clone(), s.clone()], direction)?;
            assert_eq!(
                root.get_children_keys(),
                if direction == NEXT {
                    paragraph_keys.clone()
                } else {
                    reversed.clone()
                }
            );

            let mut keys = root.get_children_keys();
            if direction == PREVIOUS {
                keys.reverse();
            }
            let visited: Vec<String> = caret
                .iter()
                .map(|c| c.origin().get_key().to_owned())
                .collect();
            assert_eq!(visited, keys);
            Ok(())
        });
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $getSiblingCaret > direction next
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $getSiblingCaret > direction previous
#[test]
fn get_sibling_caret_direction() {
    for direction in DIRECTIONS {
        run(move || {
            let paragraph = create_paragraph_node();
            let tokens: Vec<NodeRef> = ["-2", "-1", "0", "1", "2"]
                .iter()
                .map(|t| text_with_mode(t, "token"))
                .collect();
            let token_texts = texts_of(&tokens);
            let root = get_root();
            root.clear()?;
            root.append(&[append(paragraph.clone(), &tokens)?])?;
            const ZERO_INDEX: usize = 2;
            let z_token = &tokens[ZERO_INDEX];
            let next_token = if direction == NEXT {
                z_token.get_next_sibling()
            } else {
                z_token.get_previous_sibling()
            }
            .expect("nextToken must exist");
            let caret = get_sibling_caret(z_token, direction);
            assert_eq!(caret.origin(), z_token);
            assert_eq!(caret.direction(), direction);
            assert_eq!(caret.caret_type(), "sibling");
            assert_eq!(caret.get_node_at_caret(), Some(next_token));
            assert_eq!(caret.get_parent_at_caret(), Some(paragraph.clone()));

            let sign: i32 = if direction == NEXT { 1 } else { -1 };
            let values = |caret: &PointCaret| -> Vec<i32> {
                caret
                    .iter()
                    .map(|c| sign * c.origin().get_text_content().parse::<i32>().unwrap())
                    .collect()
            };
            assert_eq!(values(&caret), [1, 2]);

            let flipped = caret.get_flipped()?;
            assert_ne!(flipped, caret);
            // Upstream's `expect(flipped.getFlipped().isSameNodeCaret(caret))`
            // has no matcher, so it asserts nothing.
            let _ = flipped.get_flipped()?.is_same_node_caret(&caret);
            assert_eq!(Some(flipped.origin().clone()), caret.get_node_at_caret());
            assert_ne!(flipped.direction(), direction);
            assert_eq!(flipped.caret_type(), "sibling");
            assert_eq!(flipped.get_node_at_caret().as_ref(), Some(z_token));
            let flipped_adjacent = flipped.get_adjacent_caret().expect(
                "A flipped SiblingCaret always has an adjacent caret because it points back to the origin",
            );
            assert_eq!(flipped_adjacent.origin(), caret.origin());

            for mode in [RootMode::Root, RootMode::ShadowRoot] {
                assert!(caret.get_parent_caret(mode).is_some_and(|p| {
                    get_sibling_caret(&paragraph, caret.direction()).is_same_node_caret(&p)
                }));
                assert!(flipped.get_parent_caret(mode).is_some_and(|p| {
                    get_sibling_caret(&paragraph, flipped.direction()).is_same_node_caret(&p)
                }));
            }

            let adjacent = caret
                .get_adjacent_caret()
                .expect("expecting adjacent caret");
            let offset: isize = if direction == NEXT { 1 } else { -1 };
            let at = |delta: isize| ZERO_INDEX.checked_add_signed(delta).unwrap();
            let neighbor = tokens[at(offset)].clone();
            assert_eq!(adjacent.origin(), &neighbor);
            assert_eq!(adjacent.caret_type(), "sibling");
            assert_eq!(adjacent.origin().get_text_content(), offset.to_string());

            assert!(neighbor.is_attached());
            assert_eq!(caret.get_node_at_caret().as_ref(), Some(&neighbor));
            assert_eq!(caret.get_node_at_caret().as_ref(), Some(&neighbor));
            assert_eq!(paragraph.get_children_size(), tokens.len());
            caret.remove()?;
            assert_eq!(paragraph.get_children_size(), tokens.len() - 1);
            assert!(!neighbor.is_attached());
            assert_eq!(
                caret.get_node_at_caret().as_ref(),
                Some(&tokens[at(2 * offset)])
            );
            assert_eq!(values(&caret), [2]);
            let children_texts = || texts_of(&paragraph.get_children());
            assert_eq!(
                children_texts(),
                token_texts
                    .iter()
                    .filter(|t| **t != offset.to_string())
                    .cloned()
                    .collect::<Vec<_>>()
            );
            caret.insert(&neighbor)?;
            assert_eq!(children_texts(), token_texts);
            caret.replace_or_insert(&neighbor, false)?;
            assert_eq!(children_texts(), token_texts);

            caret.replace_or_insert(&create_text_node("replaced!"), false)?;
            assert_eq!(
                children_texts(),
                token_texts
                    .iter()
                    .enumerate()
                    .map(|(i, t)| if i == at(offset) {
                        "replaced!".to_owned()
                    } else {
                        t.clone()
                    })
                    .collect::<Vec<_>>()
            );
            caret.replace_or_insert(&neighbor, false)?;

            // The texts of the tokens with `zero` in place of token 0, and
            // without the token at `skip`.
            let around_zero = |zero: [&str; 3], skip: Option<usize>| -> Vec<String> {
                token_texts
                    .iter()
                    .enumerate()
                    .flat_map(|(i, t)| {
                        if Some(i) == skip {
                            vec![]
                        } else if i != ZERO_INDEX {
                            vec![t.clone()]
                        } else {
                            zero.iter().map(|z| (*z).to_owned()).collect()
                        }
                    })
                    .collect()
            };
            let ab_nodes = [create_text_node("a"), create_text_node("b")];
            let remove_ab = || -> Result<(), LexicalError> {
                for n in &ab_nodes {
                    n.remove(false)?;
                }
                Ok(())
            };
            caret.splice(0, &ab_nodes, NEXT)?;
            assert_eq!(
                children_texts(),
                around_zero(
                    if direction == NEXT {
                        ["0", "a", "b"]
                    } else {
                        ["a", "b", "0"]
                    },
                    None
                )
            );
            remove_ab()?;

            caret.splice(0, &ab_nodes, PREVIOUS)?;
            assert_eq!(
                children_texts(),
                around_zero(
                    if direction == NEXT {
                        ["0", "b", "a"]
                    } else {
                        ["b", "a", "0"]
                    },
                    None
                )
            );
            remove_ab()?;

            caret.splice(1, &ab_nodes, NEXT)?;
            assert_eq!(
                children_texts(),
                around_zero(
                    if direction == NEXT {
                        ["0", "a", "b"]
                    } else {
                        ["a", "b", "0"]
                    },
                    Some(at(offset))
                )
            );
            paragraph.clear()?;
            paragraph.append(&tokens)?;

            caret.splice(1, &ab_nodes[..1], NEXT)?;
            assert_eq!(
                children_texts(),
                token_texts
                    .iter()
                    .enumerate()
                    .map(|(i, t)| if i == at(offset) {
                        "a".to_owned()
                    } else {
                        t.clone()
                    })
                    .collect::<Vec<_>>()
            );
            paragraph.clear()?;
            paragraph.append(&tokens)?;

            caret.splice(2, &ab_nodes[..1], NEXT)?;
            assert_eq!(
                children_texts(),
                if direction == NEXT {
                    ["-2", "-1", "0", "a"]
                } else {
                    ["a", "0", "1", "2"]
                }
            );
            paragraph.clear()?;
            paragraph.append(&tokens)?;

            caret.splice(usize::MAX, &ab_nodes, NEXT)?;
            assert_eq!(
                children_texts(),
                if direction == NEXT {
                    ["-2", "-1", "0", "a", "b"]
                } else {
                    ["a", "b", "0", "1", "2"]
                }
            );
            paragraph.clear()?;
            paragraph.append(&tokens)?;
            Ok(())
        });
    }
}

/// `['first', 'second', 'third']` in token mode, in a paragraph at the root.
fn token_text_nodes() -> Result<Vec<NodeRef>, LexicalError> {
    let text_nodes: Vec<NodeRef> = ["first", "second", "third"]
        .iter()
        .map(|t| text_with_mode(t, "token"))
        .collect();
    set_root(&[paragraph_of(&text_nodes)?])?;
    Ok(text_nodes)
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > collapsed text point selection
#[test]
fn caret_range_from_selection_collapsed_text_point_selection() {
    update_later(&test_editor(), || {
        let text_nodes = token_text_nodes()?;
        let node = text_nodes[1].clone();
        let size = node.get_text_content_size();
        let cases: [(&MakeSelection<'_>, usize); 3] = [
            (&|| node.select_start(), 0),
            (&|| node.select_end(), size),
            (&|| node.select(Some(3), Some(3)), 3),
        ];
        for (make_selection, offset) in cases {
            let key = node.get_key();
            let selection = make_selection()?;
            assert_point(&selection, Which::Anchor, key, offset, PointType::Text);
            assert_point(&selection, Which::Focus, key, offset, PointType::Text);
            let range = range_of(&selection)?;
            assert!(range.is_collapsed());
            assert!(range.anchor.is_text_point_caret());
            assert!(range.focus.is_text_point_caret());
            assert_caret(&range.anchor, Some(NEXT), Some(offset), None, None);
            assert_caret(&range.focus, Some(NEXT), Some(offset), None, None);
            let slices = slices(&range);
            assert_eq!(slices.len(), 1);
            assert_slice(&slices[0], Some(NEXT), offset, Some(&node), 0);
            assert_eq!(node_carets(&range), []);
        }
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > full text node selection (next)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > full text node selection (previous)
#[test]
fn caret_range_from_selection_full_text_node_selection() {
    for direction in DIRECTIONS {
        update_later(&test_editor(), move || {
            for node in token_text_nodes()? {
                let key = node.get_key();
                let text_size = node.get_text_content_size();
                let (anchor_offset, focus_offset) = if direction == NEXT {
                    (0, text_size)
                } else {
                    (text_size, 0)
                };
                let selection = node.select(Some(anchor_offset), Some(focus_offset))?;
                assert_point(
                    &selection,
                    Which::Anchor,
                    key,
                    anchor_offset,
                    PointType::Text,
                );
                assert_point(&selection, Which::Focus, key, focus_offset, PointType::Text);
                let range = range_of(&selection)?;
                assert!(range.anchor.is_text_point_caret());
                assert!(range.focus.is_text_point_caret());
                assert_caret(
                    &range.anchor,
                    Some(direction),
                    Some(anchor_offset),
                    Some(&node),
                    None,
                );
                assert_eq!(range.direction(), direction);
                assert_caret(
                    &range.focus,
                    Some(direction),
                    Some(focus_offset),
                    Some(&node),
                    None,
                );
                let slices = slices(&range);
                assert_eq!(slices.len(), 1);
                assert_slice(
                    &slices[0],
                    Some(direction),
                    anchor_offset,
                    Some(&node),
                    focus_offset as isize - anchor_offset as isize,
                );
                assert_eq!(node_carets(&range), []);
                assert!(!range.is_collapsed());
            }
            Ok(())
        });
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > single text node non-empty selection
#[test]
fn caret_range_from_selection_single_text_node_non_empty_selection() {
    update_later(&test_editor(), || {
        for node in token_text_nodes()? {
            // Test all non-empty selections
            let text_size = node.get_text_content_size();
            for index_start in 0..text_size {
                for index_end in index_start + 1..=text_size {
                    for direction in DIRECTIONS {
                        let (offset, size) = if direction == NEXT {
                            (index_start, index_end as isize - index_start as isize)
                        } else {
                            (index_end, index_start as isize - index_end as isize)
                        };
                        let selection = if direction == NEXT {
                            node.select(Some(index_start), Some(index_end))?
                        } else {
                            node.select(Some(index_end), Some(index_start))?
                        };
                        let range = range_of(&selection)?;
                        assert!(range.anchor.is_text_point_caret());
                        assert!(range.focus.is_text_point_caret());
                        assert_eq!(range.direction(), direction);
                        let slices = slices(&range);
                        assert_eq!(slices.len(), 1);
                        assert_slice(&slices[0], Some(direction), offset, Some(&node), size);
                        assert_eq!(node_carets(&range), []);
                    }
                }
            }
        }
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > multiple text node non-empty selection (next)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > multiple text node non-empty selection (previous)
#[test]
fn caret_range_from_selection_multiple_text_node_non_empty_selection() {
    for direction in DIRECTIONS {
        update_later(&test_editor(), move || {
            let text_nodes = token_text_nodes()?;
            let selection = get_root().select(None, None)?;

            // test all start and end nodes (where different)
            let node_count = text_nodes.len();
            for index_node_start in 0..node_count {
                for index_node_end in index_node_start + 1..node_count {
                    let start_node = &text_nodes[index_node_start];
                    let end_node = &text_nodes[index_node_end];
                    for index_start in [0, 1, start_node.get_text_content_size()] {
                        for index_end in [0, 1, end_node.get_text_content_size()] {
                            let (anchor_node, anchor_offset, focus_node, focus_offset) =
                                if direction == NEXT {
                                    (start_node, index_start, end_node, index_end)
                                } else {
                                    (end_node, index_end, start_node, index_start)
                                };
                            selection.set_text_node_range(
                                anchor_node,
                                anchor_offset,
                                focus_node,
                                focus_offset,
                            )?;
                            let range = range_of(&selection)?;
                            assert!(range.anchor.is_text_point_caret());
                            assert!(range.focus.is_text_point_caret());
                            assert_eq!(range.direction(), direction);
                            let text_slice_carets = slices(&range);
                            assert_eq!(text_slice_carets.len(), 2);
                            let anchor_size = anchor_node.get_text_content_size() as isize;
                            let focus_size = focus_node.get_text_content_size() as isize;
                            let (anchor_offset_i, focus_offset_i) =
                                (anchor_offset as isize, focus_offset as isize);
                            assert_slice(
                                &text_slice_carets[0],
                                Some(direction),
                                anchor_offset,
                                Some(anchor_node),
                                if direction == NEXT {
                                    anchor_size - anchor_offset_i
                                } else {
                                    -anchor_offset_i
                                },
                            );
                            assert_slice(
                                &text_slice_carets[1],
                                Some(direction),
                                focus_offset,
                                Some(focus_node),
                                if direction == NEXT {
                                    -focus_offset_i
                                } else {
                                    focus_size - focus_offset_i
                                },
                            );
                            let carets = node_carets(&range);
                            let expected = &text_nodes[index_node_start + 1..index_node_end];
                            assert_eq!(carets.len(), expected.len());
                            for (caret, origin) in carets.iter().zip(expected) {
                                assert_caret(
                                    caret,
                                    Some(direction),
                                    None,
                                    Some(origin),
                                    Some("sibling"),
                                );
                            }
                        }
                    }
                }
            }
            Ok(())
        });
    }
}

/// `$removeTextFromCaretRange(range)`.
fn remove_text(range: &CaretRange) -> Result<CaretRange, LexicalError> {
    remove_text_from_caret_range(range, SliceMode::RemoveEmptySlices)
}

const TEXTS: [&str; 3] = ["first", "second", "third"];

/// `$createLinkNode(url)`.
fn link(url: &str) -> NodeRef {
    create_link_node(url, &LinkAttributes::default())
}

fn is_paragraph(node: Option<&NodeRef>) -> bool {
    node.is_some_and(|n| n.get_latest().is::<ParagraphNode>())
}

fn is_line_break(node: Option<&NodeRef>) -> bool {
    node.is_some_and(|n| n.get_latest().is::<LineBreakNode>())
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > ported Headings e2e tests > Pressing return in the middle of a heading creates a new heading below
#[test]
fn pressing_return_in_the_middle_of_a_heading_creates_a_new_heading_below() {
    run(|| {
        let initial_text_node = create_text_node("[before][after]");
        let heading_node = append(
            create_heading_node(HeadingTagType::H1),
            std::slice::from_ref(&initial_text_node),
        )?;
        set_root(std::slice::from_ref(&heading_node))?;
        let offset = "[before]".len();
        let new_heading_node = initial_text_node
            .select(Some(offset), Some(offset))?
            .insert_paragraph()?;
        assert_eq!(
            texts_of(&get_root().get_all_text_nodes()),
            ["[before]", "[after]"]
        );
        let new_heading_node = new_heading_node.expect("paragraph inserted");
        assert!(is_heading_node(&new_heading_node), "paragraph inserted");
        assert_eq!(
            get_root().get_children(),
            [heading_node.clone(), new_heading_node.clone()]
        );
        assert_eq!(initial_text_node.get_text_content(), "[before]");
        assert_eq!(initial_text_node.get_parent(), Some(heading_node));
        let new_text_nodes = new_heading_node.get_all_text_nodes();
        assert_eq!(new_text_nodes.len(), 1);
        assert!(new_text_nodes[0].is_text(), "new text node created");
        assert_eq!(new_text_nodes[0].get_text_content(), "[after]");
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > ported File e2e tests > $selectAll() with nesting and a trailing decorator
#[test]
fn select_all_with_nesting_and_a_trailing_decorator() {
    run(|| {
        let bold = create_text_node("Hello");
        bold.set_format_type("bold");
        let paragraph_node = paragraph_of(&[bold, create_text_node("World")])?;
        let list_node = append(
            create_list_node(ListType::Number, 1.0),
            &[
                append(create_list_item_node(None), &[create_text_node("one")])?,
                append(create_list_item_node(None), &[create_text_node("two")])?,
                append(create_list_item_node(None), &[create_test_decorator_node()])?,
            ],
        )?;
        set_root(&[paragraph_node.clone(), list_node])?;
        assert_eq!(get_root().get_children_size(), 2);
        let range = range_of(&select_all()?)?;
        let result_range = remove_text(&range)?;
        assert_eq!(get_root().get_all_text_nodes(), []);
        assert_eq!(
            get_root().get_children(),
            std::slice::from_ref(&paragraph_node)
        );
        assert_caret(
            &result_range.anchor,
            Some(NEXT),
            None,
            Some(&paragraph_node),
            Some("child"),
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > ported LexicalSelection tests > remove partial initial TextNode and partial segmented TextNode
#[test]
fn remove_partial_initial_text_node_and_partial_segmented_text_node() {
    let editor = test_editor();
    let nodes: Rc<RefCell<Option<(NodeRef, NodeRef)>>> = Rc::default();
    let out = nodes.clone();
    update(&editor, move || {
        let sel = create_range_selection();
        let leading_text = create_text_node("leading text");
        let trailing_segmented_text = text_with_mode("segmented text", "segmented");
        set_root(&[paragraph_of(&[
            leading_text.clone(),
            trailing_segmented_text.clone(),
        ])?])?;
        sel.point_set(
            Which::Anchor,
            leading_text.get_key(),
            "lead".len(),
            PointType::Text,
        )?;
        sel.point_set(
            Which::Focus,
            trailing_segmented_text.get_key(),
            "segmented ".len(),
            PointType::Text,
        )?;
        set_selection(Some(sel.clone()));
        let result_range = remove_text(&range_of(&sel)?)?;
        set_selection_from_caret_range(&result_range)?;
        assert_caret(
            &result_range.anchor,
            None,
            Some("lead".len()),
            Some(&leading_text),
            None,
        );
        assert_eq!(result_range.direction(), NEXT);
        assert!(leading_text.is_attached());
        assert!(!trailing_segmented_text.is_attached());
        // These should get merged in reconciliation
        assert_eq!(texts_of(&get_root().get_all_text_nodes()), ["lead", "text"]);
        let selection = get_selection().filter(SelectionRef::is_range).unwrap();
        assert!(selection.range().unwrap().is_collapsed());
        assert_point(
            &selection,
            Which::Anchor,
            leading_text.get_key(),
            "lead".len(),
            PointType::Text,
        );
        *out.borrow_mut() = Some((leading_text, trailing_segmented_text));
        Ok(())
    });
    let (leading_text, trailing_segmented_text) = nodes.borrow().clone().unwrap();
    // Reconciliation has happened
    editor
        .read_with_mode(EditorReadMode::Latest, || {
            // These should get merged in reconciliation
            assert_eq!(texts_of(&get_root().get_all_text_nodes()), ["leadtext"]);
            assert!(leading_text.is_attached());
            assert!(!trailing_segmented_text.is_attached());
        })
        .unwrap();
}

/// The `single block` suite's `beforeEach`: the three texts, styled so they
/// don't merge, in one paragraph.
fn single_block_editor() -> Editor {
    let editor = test_editor();
    update_later(&editor, || {
        // Ensure that the separate texts don't get merged
        let text_nodes: Vec<NodeRef> = TEXTS
            .iter()
            .map(|text| {
                let node = create_text_node(text);
                node.set_style(&format!("color: --color-{text}"));
                node
            })
            .collect();
        set_root(&[paragraph_of(&text_nodes)?])
    });
    editor
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > remove second TextNode when wrapped in a LinkNode that will become empty
#[test]
fn remove_second_text_node_when_wrapped_in_a_link_node_that_will_become_empty() {
    update(&single_block_editor(), || {
        let sel = create_range_selection();
        let original_nodes = get_root().get_all_text_nodes();
        let (leading_text, trailing_link_text) = (&original_nodes[0], &original_nodes[1]);
        let link_wrapper = link("https://lexical.dev");
        trailing_link_text.replace(&link_wrapper, false)?;
        link_wrapper.append(std::slice::from_ref(trailing_link_text))?;
        sel.point_set(
            Which::Anchor,
            trailing_link_text.get_key(),
            0,
            PointType::Text,
        )?;
        sel.point_set(
            Which::Focus,
            trailing_link_text.get_key(),
            trailing_link_text.get_text_content_size(),
            PointType::Text,
        )?;
        let direction = NEXT;
        let range = range_of(&sel)?;
        set_selection(Some(sel.clone()));
        assert_caret(
            &range.anchor,
            Some(direction),
            Some(0),
            Some(trailing_link_text),
            None,
        );
        assert_caret(
            &range.focus,
            Some(direction),
            Some(trailing_link_text.get_text_content_size()),
            Some(trailing_link_text),
            None,
        );
        let result_range = remove_text(&range)?;
        set_selection_from_caret_range(&result_range)?;
        assert!(leading_text.is_attached());
        assert!(!trailing_link_text.is_attached());
        assert_eq!(get_root().get_all_text_nodes().len(), 2);
        assert!(result_range.is_collapsed());
        assert!(sel.range().unwrap().is_collapsed());
        let anchor = sel.point(Which::Anchor).unwrap();
        assert_eq!(anchor.get_node()?, *leading_text);
        assert_eq!(anchor.key, leading_text.get_key());
        assert_eq!(anchor.offset, leading_text.get_text_content_size());
        assert_caret(
            &result_range.anchor,
            Some(direction),
            Some(leading_text.get_text_content_size()),
            Some(leading_text),
            None,
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > remove first TextNode with second in token mode
#[test]
fn remove_first_text_node_with_second_in_token_mode() {
    update(&single_block_editor(), || {
        let sel = create_range_selection();
        let original_nodes = get_root().get_all_text_nodes();
        let (leading_text, trailing_token_text) = (&original_nodes[0], &original_nodes[1]);
        trailing_token_text.set_mode("token")?;
        sel.point_set(Which::Anchor, leading_text.get_key(), 0, PointType::Text)?;
        sel.point_set(
            Which::Focus,
            leading_text.get_key(),
            leading_text.get_text_content_size(),
            PointType::Text,
        )?;
        let direction = NEXT;
        let range = range_of(&sel)?;
        set_selection(Some(sel.clone()));
        assert_caret(
            &range.anchor,
            Some(direction),
            Some(0),
            Some(leading_text),
            None,
        );
        assert_caret(
            &range.focus,
            Some(direction),
            Some(leading_text.get_text_content_size()),
            Some(leading_text),
            None,
        );
        let result_range = remove_text(&range)?;
        set_selection_from_caret_range(&result_range)?;
        assert!(!leading_text.is_attached());
        assert!(trailing_token_text.is_attached());
        assert_eq!(get_root().get_all_text_nodes().len(), 2);
        assert!(result_range.is_collapsed());
        assert!(sel.range().unwrap().is_collapsed());
        assert_point(
            &sel,
            Which::Anchor,
            trailing_token_text.get_key(),
            0,
            PointType::Text,
        );
        assert_caret(
            &result_range.anchor,
            Some(direction),
            Some(0),
            Some(trailing_token_text),
            None,
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > collapsed text point selection
#[test]
fn remove_text_collapsed_text_point_selection() {
    update_later(&single_block_editor(), || {
        let text_nodes = get_root().get_all_text_nodes();
        let original_text = get_root().get_text_content();
        let node = text_nodes[1].clone();
        let size = node.get_text_content_size();
        let cases: [(&MakeSelection<'_>, usize); 3] = [
            (&|| node.select_start(), 0),
            (&|| node.select_end(), size),
            (&|| node.select(Some(3), Some(3)), 3),
        ];
        for (make_selection, offset) in cases {
            let key = node.get_key();
            let selection = make_selection()?;
            assert_point(&selection, Which::Anchor, key, offset, PointType::Text);
            assert_point(&selection, Which::Focus, key, offset, PointType::Text);
            let range = range_of(&selection)?;
            assert!(range.is_collapsed());
            assert!(range.anchor.is_text_point_caret());
            assert!(range.focus.is_text_point_caret());
            let original_range_match = |range: &CaretRange| {
                assert_caret(&range.anchor, Some(NEXT), Some(offset), None, None);
                assert_caret(&range.focus, Some(NEXT), Some(offset), None, None);
            };
            original_range_match(&range);
            let slices = slices(&range);
            assert_eq!(slices.len(), 1);
            assert_slice(&slices[0], Some(NEXT), offset, Some(&node), 0);
            assert_eq!(node_carets(&range), []);
            original_range_match(&remove_text(&range)?);
            assert_eq!(get_root().get_text_content(), original_text);
            set_selection(None);
        }
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > first node (next)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > second node (next)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > third node (next)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > first node (previous)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > second node (previous)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > third node (previous)
#[test]
fn full_text_node_internal_selection() {
    for direction in DIRECTIONS {
        for i in 0..TEXTS.len() {
            update_later(&single_block_editor(), move || {
                let mut original_nodes = get_root().get_all_text_nodes();
                let node = original_nodes.remove(i);
                assert!(node.is_text(), "Missing TextNode {i}");
                let key = node.get_key();
                let size = node.get_text_content_size();
                let (anchor_offset, focus_offset) = if direction == NEXT {
                    (0, size)
                } else {
                    (size, 0)
                };
                let selection = node.select(Some(anchor_offset), Some(focus_offset))?;
                assert_point(
                    &selection,
                    Which::Anchor,
                    key,
                    anchor_offset,
                    PointType::Text,
                );
                assert_point(&selection, Which::Focus, key, focus_offset, PointType::Text);
                let range = range_of(&selection)?;
                assert!(range.anchor.is_text_point_caret());
                assert!(range.focus.is_text_point_caret());
                assert_caret(&range.anchor, None, Some(anchor_offset), Some(&node), None);
                assert_eq!(range.direction(), direction);
                assert_caret(&range.focus, None, Some(focus_offset), Some(&node), None);
                let slices = slices(&range);
                assert_eq!(slices.len(), 1);
                let sign: isize = if direction == NEXT { 1 } else { -1 };
                assert_slice(
                    &slices[0],
                    None,
                    if direction == NEXT { 0 } else { size },
                    Some(&node),
                    sign * size as isize,
                );
                assert_eq!(node_carets(&range), []);
                assert!(!range.is_collapsed());
                let result_range = remove_text(&range)?;
                set_selection(None);
                assert_after_whole_text_node_removal(&result_range, &original_nodes, i, direction);
                Ok(())
            });
        }
    }
}

/// The assertions after the text node `i` of [`TEXTS`] is removed whole:
/// the other nodes remain, and the range is collapsed at the end of the
/// node before it (or the start of the first).
fn assert_after_whole_text_node_removal(
    result_range: &CaretRange,
    original_nodes: &[NodeRef],
    i: usize,
    direction: CaretDirection,
) {
    let remaining_nodes = get_root().get_all_text_nodes();
    assert_eq!(remaining_nodes, original_nodes);
    let expected_texts: Vec<&str> = TEXTS
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != i)
        .map(|(_, t)| *t)
        .collect();
    assert_eq!(texts_of(&remaining_nodes), expected_texts);
    assert!(result_range.is_collapsed());
    // bias towards the start
    let new_origin = &remaining_nodes[i.saturating_sub(1)];
    let offset = if i == 0 {
        0
    } else {
        new_origin.get_text_content_size()
    };
    for caret in [&result_range.anchor, &result_range.focus] {
        assert_caret(
            caret,
            Some(direction),
            Some(offset),
            Some(new_origin),
            Some("text"),
        );
    }
    assert_eq!(result_range.direction(), direction);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bias {
    Inside,
    Outside,
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (next inside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (next inside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (next inside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (next outside inside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (next outside inside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (next outside inside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (next outside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (next outside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (next outside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (previous inside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (previous inside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (previous inside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (previous outside inside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (previous outside inside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (previous outside inside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (previous outside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (previous outside outside)
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (previous outside outside)
#[test]
fn full_text_node_biased_selection() {
    use Bias::{Inside, Outside};
    for direction in DIRECTIONS {
        for (anchor_bias, focus_bias) in [(Inside, Outside), (Outside, Inside), (Outside, Outside)]
        {
            for i in 0..TEXTS.len() {
                update_later(&single_block_editor(), move || {
                    full_text_node_biased_selection_case(direction, anchor_bias, focus_bias, i)
                });
            }
        }
    }
}

fn full_text_node_biased_selection_case(
    direction: CaretDirection,
    anchor_bias: Bias,
    focus_bias: Bias,
    i: usize,
) -> Result<(), LexicalError> {
    let case = format!("{i} ({direction:?} {anchor_bias:?} {focus_bias:?})");
    let mut original_nodes = get_root().get_all_text_nodes();
    let node = original_nodes.remove(i);
    assert!(node.is_text(), "Missing TextNode {i}");
    let size = node.get_text_content_size();
    let (anchor_offset, focus_offset) = if direction == NEXT {
        (0, size)
    } else {
        (size, 0)
    };
    // Create the inside selection, will mutate for outside
    let selection = node.select(Some(anchor_offset), Some(focus_offset))?;
    let node_caret = get_sibling_caret(&node, direction);
    let last = TEXTS.len() - 1;
    let parent = node.get_parent_or_throw()?;
    // The point at the start (`at_start`) or end of `parent`, or at the end
    // of the node before `node` or the start of the node after it.
    let assert_outside = |which: Which, before: bool| {
        let at_edge = if before { i == 0 } else { i == last };
        if at_edge {
            let offset = if before {
                0
            } else {
                parent.get_children_size()
            };
            assert_point(
                &selection,
                which,
                parent.get_key(),
                offset,
                PointType::Element,
            );
        } else if before {
            let adj = &original_nodes[i - 1];
            assert_point(
                &selection,
                which,
                adj.get_key(),
                adj.get_text_content_size(),
                PointType::Text,
            );
        } else {
            let adj = &original_nodes[i];
            assert_point(&selection, which, adj.get_key(), 0, PointType::Text);
        }
    };
    if anchor_bias == Bias::Outside {
        set_point_from_caret(
            &selection,
            Which::Anchor,
            &rewind_sibling_caret(&node_caret)?,
        )?;
        assert_outside(Which::Anchor, direction == NEXT);
    }
    if focus_bias == Bias::Outside {
        set_point_from_caret(
            &selection,
            Which::Focus,
            &get_sibling_caret(&node, direction).get_flipped()?,
        )?;
        assert_outside(Which::Focus, direction != NEXT);
    }
    let range = range_of(&selection)?;
    assert!(!range.is_collapsed(), "{case}");
    let both_outside = anchor_bias == Bias::Outside && focus_bias == Bias::Outside;
    assert_eq!(
        node_carets(&range).len(),
        usize::from(both_outside),
        "{case}"
    );
    let non_empty: Vec<TextPointCaretSlice> = slices(&range)
        .into_iter()
        .filter(|slice| slice.distance != 0)
        .collect();
    if both_outside {
        assert_eq!(non_empty, [], "{case}");
    } else {
        assert_eq!(non_empty.len(), 1, "{case}");
        let (offset, distance) = if (anchor_bias == Bias::Inside) == (direction == NEXT) {
            (0, size as isize)
        } else {
            (size, -(size as isize))
        };
        assert_eq!(non_empty[0].caret.offset(), Some(offset), "{case}");
        assert_eq!(non_empty[0].distance, distance, "{case}");
    }
    let result_range = remove_text(&range)?;
    set_selection(None);
    assert_after_whole_text_node_removal(&result_range, &original_nodes, i, direction);
    Ok(())
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > next 0:-1
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > next 1:-1
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > next 1:0
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > previous 0:-1
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > previous 1:-1
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > previous 1:0
#[test]
fn single_text_node_non_empty_partial_selection() {
    const EDGE_OFFSETS: [(usize, usize); 3] = [(0, 1), (1, 1), (1, 0)];
    for direction in DIRECTIONS {
        for (anchor_edge_offset, focus_edge_offset) in EDGE_OFFSETS {
            update_later(&single_block_editor(), move || {
                let original_nodes = get_root().get_all_text_nodes();
                let i = 0;
                let text = TEXTS[i];
                let node = &original_nodes[i];
                assert!(node.is_text(), "Missing TextNode 0");
                let size = node.get_text_content_size();
                let anchor = get_text_point_caret(
                    node,
                    direction,
                    if direction == NEXT {
                        anchor_edge_offset
                    } else {
                        size - anchor_edge_offset
                    },
                )?;
                let focus = get_text_point_caret(
                    node,
                    direction,
                    if direction == NEXT {
                        size - focus_edge_offset
                    } else {
                        focus_edge_offset
                    },
                )?;
                let (a, f) = (anchor.offset().unwrap(), focus.offset().unwrap());
                let (offset_start, offset_end) = (a.min(f), a.max(f));
                let range = get_caret_range(anchor, focus)?;
                let slices: Vec<TextPointCaretSlice> = slices(&range)
                    .into_iter()
                    .filter(|slice| slice.distance != 0)
                    .collect();
                assert_eq!(node_carets(&range), []);
                assert_eq!(slices.len(), 1);
                let sign: isize = if direction == NEXT { 1 } else { -1 };
                assert_eq!(
                    slices[0].distance,
                    sign * (size - anchor_edge_offset - focus_edge_offset) as isize
                );
                assert_eq!(
                    slices[0].get_text_content(),
                    &text[offset_start..offset_end]
                );
                let result_range = remove_text(&range)?;
                set_selection(None);
                assert!(result_range.is_collapsed());
                assert_caret(
                    &result_range.anchor,
                    Some(direction),
                    Some(offset_start),
                    Some(node),
                    None,
                );
                let remaining_nodes = get_root().get_all_text_nodes();
                assert_eq!(remaining_nodes.len(), TEXTS.len());
                let expected: Vec<String> = TEXTS
                    .iter()
                    .enumerate()
                    .map(|(j, v)| {
                        if i == j {
                            format!("{}{}", &v[..offset_start], &v[offset_end..])
                        } else {
                            (*v).to_owned()
                        }
                    })
                    .collect();
                assert_eq!(texts_of(&remaining_nodes), expected);
                Ok(())
            });
        }
    }
}

/// A `multiple text node selection` case.
type MultipleTextNodeCase = (CaretDirection, (usize, usize), [fn(usize) -> usize; 2]);

/// `startOfNode`, `insideNode` and `endOfNode`: an offset in a text of
/// `size`.
const OFFSETS: [fn(usize) -> usize; 3] = [|_| 0, |_| 1, |size| size];
const NODE_PAIRS: [(usize, usize); 3] = [(0, 1), (0, 2), (1, 2)];

/// The cases of a `multiple text node selection` suite, in upstream's order:
/// the direction, the start and end nodes, and the offsets in each.
fn multiple_text_node_cases() -> Vec<MultipleTextNodeCase> {
    let mut cases = Vec::new();
    for direction in DIRECTIONS {
        for pair in NODE_PAIRS {
            for start_fn in OFFSETS {
                for end_fn in OFFSETS {
                    cases.push((direction, pair, [start_fn, end_fn]));
                }
            }
        }
    }
    cases
}

/// The carets a `multiple text node selection` case removes between, with
/// the slices' texts checked: `(start_caret, end_caret, range)`.
fn multiple_text_node_range(
    original_nodes: &[NodeRef],
    direction: CaretDirection,
    (node_index_start, node_index_end): (usize, usize),
    [start_fn, end_fn]: [fn(usize) -> usize; 2],
) -> Result<(PointCaret, PointCaret, CaretRange), LexicalError> {
    let start_node = &original_nodes[node_index_start];
    let end_node = &original_nodes[node_index_end];
    assert_ne!(start_node, end_node);
    assert!(start_node.is_text(), "text node");
    assert!(end_node.is_text(), "text node");
    assert!(start_node.is_before(end_node));
    let start_caret = get_text_point_caret(
        start_node,
        direction,
        start_fn(start_node.get_text_content_size()),
    )?;
    let end_caret = get_text_point_caret(
        end_node,
        direction,
        end_fn(end_node.get_text_content_size()),
    )?;
    let (anchor, focus) = if direction == NEXT {
        (start_caret.clone(), end_caret.clone())
    } else {
        (end_caret.clone(), start_caret.clone())
    };
    let range = get_caret_range(anchor, focus)?;
    Ok((start_caret, end_caret, range))
}

/// The slices of a `multiple text node selection` case: the start's text
/// from its offset and the end's up to its offset, in the range's order.
fn assert_multiple_text_node_slices(
    range: &CaretRange,
    start_caret: &PointCaret,
    end_caret: &PointCaret,
) {
    let slices = slices(range);
    assert_eq!(slices.len(), 2);
    let start_text = crate::utf16::slice_from(
        &start_caret.origin().get_text_content(),
        start_caret.offset().unwrap(),
    );
    let end_text = crate::utf16::slice(
        &end_caret.origin().get_text_content(),
        0,
        end_caret.offset().unwrap(),
    );
    let expected = if range.direction() == NEXT {
        [start_text, end_text]
    } else {
        [end_text, start_text]
    };
    let actual: Vec<String> = slices
        .iter()
        .map(TextPointCaretSlice::get_text_content)
        .collect();
    assert_eq!(actual, expected);
}

/// The text nodes after a `multiple text node selection` case: the start
/// keeps the text before its offset, the end the text after its offset,
/// the nodes between are removed, and the rest are kept as they were.
fn assert_multiple_text_node_remaining(
    original_nodes: &[NodeRef],
    (node_index_start, node_index_end): (usize, usize),
    start_caret: &PointCaret,
    end_caret: &PointCaret,
) {
    let remaining_nodes = get_root().get_all_text_nodes();
    let mut new_index = 0;
    for (original_index, original_node) in original_nodes.iter().enumerate() {
        let original_text = TEXTS[original_index];
        let deleted = if original_index == node_index_start {
            let offset = start_caret.offset().unwrap();
            let deleted = offset == 0;
            if !deleted {
                assert_eq!(original_node.get_text_content(), &original_text[..offset]);
            }
            deleted
        } else if original_index > node_index_start && original_index < node_index_end {
            true
        } else if original_index == node_index_end {
            let offset = end_caret.offset().unwrap();
            let deleted = offset == original_text.len();
            if !deleted {
                assert_eq!(original_node.get_text_content(), &original_text[offset..]);
            }
            deleted
        } else {
            assert_eq!(original_node.get_text_content(), original_text);
            false
        };
        assert_eq!(original_node.is_attached(), !deleted);
        if !deleted {
            assert_eq!(Some(original_node), remaining_nodes.get(new_index));
        }
        new_index += usize::from(!deleted);
    }
    assert_eq!(remaining_nodes.len(), new_index);
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second startOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second startOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second startOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second insideNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second insideNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second insideNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second endOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second endOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second endOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second startOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second startOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second startOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second insideNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second insideNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second insideNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second endOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second endOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second endOfNode third endOfNode
#[test]
fn single_block_multiple_text_node_selection() {
    for (direction, pair, fns) in multiple_text_node_cases() {
        update_later(&single_block_editor(), move || {
            let (node_index_start, node_index_end) = pair;
            let original_nodes = get_root().get_all_text_nodes();
            let (start_caret, end_caret, range) =
                multiple_text_node_range(&original_nodes, direction, pair, fns)?;
            assert_eq!(
                node_carets(&range).len(),
                (node_index_end - node_index_start).saturating_sub(1)
            );
            assert_multiple_text_node_slices(&range, &start_caret, &end_caret);
            let result_range = remove_text(&range)?;
            assert_eq!(result_range.anchor.direction(), direction);
            assert_eq!(result_range.direction(), direction);
            assert_eq!(result_range.focus.direction(), direction);
            let anchor = &result_range.anchor;
            if start_caret.offset() != Some(0) {
                // Part of the start remains
                assert_caret(
                    anchor,
                    None,
                    start_caret.offset(),
                    Some(start_caret.origin()),
                    None,
                );
            } else if node_index_start > 0 {
                // The anchor was removed so bias towards the previous node
                let prev_node = &original_nodes[node_index_start - 1];
                assert_caret(
                    anchor,
                    None,
                    Some(prev_node.get_text_content_size()),
                    Some(prev_node),
                    None,
                );
            } else if end_caret.offset() != Some(TEXTS[node_index_end].len()) {
                // The focus was not deleted and there is no previous node so
                // the new anchor will be set to the focus origin
                assert_caret(
                    anchor,
                    None,
                    Some(0),
                    Some(&original_nodes[node_index_end]),
                    None,
                );
            } else if node_index_end != TEXTS.len() - 1 {
                // The anchor was at the start and the focus was removed but
                // there is another text node to use as the anchor caret
                assert_caret(
                    anchor,
                    None,
                    Some(0),
                    Some(&original_nodes[node_index_end + 1]),
                    None,
                );
            } else {
                // All text has been removed so we have to use a depth caret
                assert_caret(
                    anchor,
                    None,
                    None,
                    get_root().get_first_child().as_ref(),
                    Some("child"),
                );
            }
            assert_multiple_text_node_remaining(&original_nodes, pair, &start_caret, &end_caret);
            Ok(())
        });
    }
}

/// The `multiple blocks` suite's `beforeEach`: each text, styled so they
/// don't merge, in a paragraph of its own.
fn multiple_blocks_editor() -> Editor {
    let editor = test_editor();
    update_later(&editor, || {
        // Ensure that the separate texts don't get merged
        let mut paragraphs = Vec::new();
        for text in TEXTS {
            let node = create_text_node(text);
            node.set_style(&format!("color: --color-{text}"));
            paragraphs.push(paragraph_of(&[node])?);
        }
        set_root(&paragraphs)
    });
    editor
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second startOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second startOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second startOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second insideNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second insideNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second insideNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second endOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second endOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second endOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode second startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode second insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode second endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second startOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second startOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second startOfNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second insideNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second insideNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second insideNode third endOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second endOfNode third startOfNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second endOfNode third insideNode
// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second endOfNode third endOfNode
#[test]
fn multiple_blocks_multiple_text_node_selection() {
    for (direction, pair, fns) in multiple_text_node_cases() {
        update_later(&multiple_blocks_editor(), move || {
            let (_, node_index_end) = pair;
            let original_nodes = get_root().get_all_text_nodes();
            let (start_caret, end_caret, range) =
                multiple_text_node_range(&original_nodes, direction, pair, fns)?;
            let (anchor, focus) = (range.anchor.clone(), range.focus.clone());
            assert_multiple_text_node_slices(&range, &start_caret, &end_caret);
            let original_start_parent = start_caret.get_parent_at_caret().unwrap();
            let original_end_parent = end_caret.get_parent_at_caret().unwrap();
            let result_range = remove_text(&range)?;
            if direction == NEXT {
                assert_eq!(result_range.direction(), direction);
                let result_anchor = &result_range.anchor;
                if anchor.offset() != Some(0) {
                    // Part of the anchor remains
                    assert_caret(
                        result_anchor,
                        Some(direction),
                        anchor.offset(),
                        Some(anchor.origin()),
                        None,
                    );
                } else if focus.offset() != Some(TEXTS[node_index_end].len()) {
                    // The focus was not deleted and there is no previous
                    // node so the new anchor will be set to the focus origin
                    assert_caret(
                        result_anchor,
                        Some(direction),
                        Some(0),
                        Some(&original_nodes[node_index_end]),
                        None,
                    );
                } else {
                    // The anchor and focus were removed so we have an empty
                    // paragraph at the anchor
                    assert_caret(
                        result_anchor,
                        Some(direction),
                        None,
                        Some(&original_start_parent),
                        Some("child"),
                    );
                }
            }
            // Check that the containing block is always that of the anchor
            assert_eq!(
                result_range.anchor.get_parent_at_caret(),
                Some(original_start_parent.clone())
            );
            // Check that the focus parent has always been removed
            assert!(!original_end_parent.is_attached());
            // Check that the focus has been removed or moved to the anchor
            // parent
            assert!(
                !focus.origin().is_attached()
                    || focus.origin().get_parent() == Some(original_start_parent.clone())
            );
            assert_multiple_text_node_remaining(&original_nodes, pair, &start_caret, &end_caret);
            Ok(())
        });
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > remove range between list and nested list
#[test]
fn remove_range_between_list_and_nested_list() {
    let editor = test_editor();
    update(&editor, || {
        let list_text_node = create_text_node("list node");
        let list_node = append(
            create_list_node(ListType::Number, 1.0),
            &[append(
                create_list_item_node(None),
                std::slice::from_ref(&list_text_node),
            )?],
        )?;
        let nested_text_node = create_text_node("be nested");
        let nested_list_node = append(
            create_list_node(ListType::Number, 1.0),
            &[append(
                create_list_item_node(None),
                std::slice::from_ref(&nested_text_node),
            )?],
        )?;
        list_node.append(&[append(create_list_item_node(None), &[nested_list_node])?])?;
        set_root(std::slice::from_ref(&list_node))?;
        let sel = create_range_selection();
        sel.point_set(
            Which::Anchor,
            list_text_node.get_key(),
            "list".len(),
            PointType::Text,
        )?;
        sel.point_set(
            Which::Focus,
            nested_text_node.get_key(),
            "be ".len(),
            PointType::Text,
        )?;
        set_selection(Some(sel.clone()));
        let result_range = remove_text(&range_of(&sel)?)?;
        set_selection_from_caret_range(&result_range)?;
        assert_eq!(
            texts_of(&get_root().get_all_text_nodes()),
            ["list", "nested"]
        );
        assert_eq!(list_node.get_children().len(), 1);
        Ok(())
    });
    editor
        .read_with_mode(EditorReadMode::Latest, || {
            // These should get merged in reconciliation
            assert_eq!(texts_of(&get_root().get_all_text_nodes()), ["listnested"]);
        })
        .unwrap();
}

/// `hello` and `world` in two paragraphs: `(t1, t2, p1, p2)`.
fn hello_world() -> Result<(NodeRef, NodeRef, NodeRef, NodeRef), LexicalError> {
    let t1 = create_text_node("hello");
    let t2 = create_text_node("world");
    let p1 = paragraph_of(std::slice::from_ref(&t1))?;
    let p2 = paragraph_of(std::slice::from_ref(&t2))?;
    set_root(&[p1.clone(), p2.clone()])?;
    Ok((t1, t2, p1, p2))
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $extendCaretToRange > next direction: focus can be flipped and used to delete to end of document (regression for #8927)
#[test]
fn extend_caret_to_range_next_direction_deletes_to_end_of_document() {
    run(|| {
        let (t1, _, p1, p2) = hello_world()?;
        let range = extend_caret_to_range(&get_text_point_caret(&t1, NEXT, 2)?)?;
        assert_eq!(range.focus, get_sibling_caret(&p2, NEXT));
        assert_eq!(range.focus.get_flipped()?, child(&get_root(), PREVIOUS));
        assert_eq!(range.focus.get_flipped()?.get_flipped()?, range.focus);
        remove_text(&range)?;
        assert_eq!(get_root().get_children(), [p1]);
        assert_eq!(get_root().get_all_text_nodes(), [t1]);
        assert_eq!(get_root().get_text_content(), "he");
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $extendCaretToRange > previous direction: focus can be flipped and used to delete to start of document
#[test]
fn extend_caret_to_range_previous_direction_deletes_to_start_of_document() {
    run(|| {
        let (_, t2, p1, p2) = hello_world()?;
        let range = extend_caret_to_range(&get_text_point_caret(&t2, PREVIOUS, 2)?)?;
        assert_eq!(range.focus, get_sibling_caret(&p1, PREVIOUS));
        assert_eq!(range.focus.get_flipped()?, child(&get_root(), NEXT));
        assert_eq!(range.focus.get_flipped()?.get_flipped()?, range.focus);
        remove_text(&range)?;
        assert_eq!(get_root().get_children(), [p2]);
        assert_eq!(get_root().get_all_text_nodes(), [t2]);
        assert_eq!(get_root().get_text_content(), "rld");
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $extendCaretToRange > doesn't change which nodes plain iteration visits
#[test]
fn extend_caret_to_range_does_not_change_which_nodes_plain_iteration_visits() {
    update_later(&test_editor(), || {
        let (t1, t2, p1, p2) = hello_world()?;
        let visited: Vec<NodeCaret> = extend_caret_to_range(&get_text_point_caret(&t1, NEXT, 2)?)?
            .iter()
            .collect();
        assert_eq!(
            visited,
            [
                get_sibling_caret(&p1, NEXT),
                child(&p2, NEXT),
                get_sibling_caret(&t2, NEXT),
                get_sibling_caret(&p2, NEXT),
            ]
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $extendCaretToRange > empty document: focus can still be flipped without throwing
#[test]
fn extend_caret_to_range_empty_document_focus_can_be_flipped() {
    update_later(&test_editor(), || {
        get_root().clear()?;
        let range = extend_caret_to_range(&child(&get_root(), NEXT))?;
        assert!(range.focus.get_flipped().is_ok());
        Ok(())
    });
}

/// The `Ordering` suite's document.
#[derive(Clone)]
struct OrderingDoc {
    root_node: NodeRef,
    paragraph_node: NodeRef,
    paragraph_text: NodeRef,
    link_node: NodeRef,
    link_text: NodeRef,
    list_node: NodeRef,
    list_item_text1: NodeRef,
    list_item_text2: NodeRef,
    list_item1: NodeRef,
    list_item2: NodeRef,
}

/// The `Ordering` suite's `beforeEach`, then `f` in a discrete update.
fn with_ordering_doc(f: impl FnOnce(OrderingDoc) -> Result<(), LexicalError> + 'static) {
    let editor = test_editor();
    let doc: Rc<RefCell<Option<OrderingDoc>>> = Rc::default();
    let out = doc.clone();
    update_later(&editor, move || {
        let d = OrderingDoc {
            root_node: get_root(),
            paragraph_text: create_text_node("paragraph text"),
            link_text: create_text_node("link text"),
            link_node: link(""),
            paragraph_node: create_paragraph_node(),
            list_item_text1: create_text_node("item 1"),
            list_item_text2: create_text_node("item 2"),
            list_item1: create_list_item_node(None),
            list_item2: create_list_item_node(None),
            list_node: create_list_node(ListType::Bullet, 1.0),
        };
        let empty_paragraph = create_paragraph_node();
        d.link_node.append(std::slice::from_ref(&d.link_text))?;
        d.paragraph_node
            .append(&[d.paragraph_text.clone(), d.link_node.clone()])?;
        d.list_item1
            .append(std::slice::from_ref(&d.list_item_text1))?;
        d.list_item2
            .append(std::slice::from_ref(&d.list_item_text2))?;
        d.list_node
            .append(&[d.list_item1.clone(), d.list_item2.clone()])?;
        set_root(&[
            d.paragraph_node.clone(),
            d.list_node.clone(),
            empty_paragraph,
        ])?;
        *out.borrow_mut() = Some(d);
        Ok(())
    });
    let d = doc.borrow().clone().unwrap();
    update(&editor, move || f(d));
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $comparePointCaretNext > trivial caret checks
#[test]
fn compare_point_caret_next_trivial_caret_checks() {
    with_ordering_doc(|_| {
        let mut seen_carets: Vec<NodeCaret> = Vec::new();
        let range = get_caret_range(
            child(&get_root(), NEXT),
            get_sibling_caret(&get_root(), NEXT),
        )?;
        let cmp = |a: &PointCaret, b: &PointCaret| compare_point_caret_next(a, b).unwrap();
        assert_eq!(cmp(&range.anchor, &range.focus), Ordering::Less);
        assert_eq!(cmp(&range.anchor, &range.anchor), Ordering::Equal);
        assert_eq!(cmp(&range.focus, &range.focus), Ordering::Equal);
        assert_eq!(cmp(&range.focus, &range.anchor), Ordering::Greater);
        for caret in get_caret_range(
            child(&get_root(), NEXT),
            get_sibling_caret(&get_root(), NEXT),
        )?
        .iter()
        {
            assert_eq!(cmp(&caret, &caret), Ordering::Equal);
            for seen_caret in &seen_carets {
                assert_eq!(cmp(&caret, seen_caret), Ordering::Greater);
                assert_eq!(cmp(seen_caret, &caret), Ordering::Less);
            }
            seen_carets.push(caret);
        }
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $comparePointCaretNext > TextPointCaret checks single origin
#[test]
fn compare_point_caret_next_text_point_caret_single_origin() {
    with_ordering_doc(|d| {
        let t = &d.paragraph_text;
        let text = |offset: usize| get_text_point_caret(t, NEXT, offset).unwrap();
        let cmp = |a: &PointCaret, b: &PointCaret| compare_point_caret_next(a, b).unwrap();
        let sibling = get_sibling_caret(t, NEXT);
        let size = t.get_text_content_size();
        assert_eq!(cmp(&text(0), &text(0)), Ordering::Equal);
        assert_eq!(cmp(&text(0), &text(2)), Ordering::Less);
        assert_eq!(cmp(&text(2), &text(0)), Ordering::Greater);
        // next sibling carets always come after points inside the text
        assert_eq!(cmp(&text(2), &sibling), Ordering::Less);
        assert_eq!(cmp(&sibling, &text(2)), Ordering::Greater);
        assert_eq!(cmp(&text(size), &sibling), Ordering::Less);
        assert_eq!(cmp(&sibling, &text(size)), Ordering::Greater);
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $comparePointCaretNext > TextPointCaret multiple origin
#[test]
fn compare_point_caret_next_text_point_caret_multiple_origin() {
    with_ordering_doc(|d| {
        let p0 = get_text_point_caret(&d.paragraph_text, NEXT, 0)?;
        let l0 = get_text_point_caret(&d.link_text, NEXT, 0)?;
        assert_eq!(compare_point_caret_next(&p0, &l0)?, Ordering::Less);
        assert_eq!(
            compare_point_caret_next(&p0, &get_sibling_caret(&d.link_text, NEXT))?,
            Ordering::Less
        );
        assert_eq!(compare_point_caret_next(&l0, &p0)?, Ordering::Greater);
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > trivial node checks
#[test]
fn get_common_ancestor_trivial_node_checks() {
    with_ordering_doc(|_| {
        let root = get_root();
        assert_eq!(
            get_common_ancestor(&root, &root),
            Some(CommonAncestorResult::Same {
                common_ancestor: root.clone()
            })
        );
        for caret in get_caret_range(child(&root, NEXT), get_sibling_caret(&root, NEXT))?.iter() {
            if caret.is_sibling_caret() {
                let origin = caret.origin();
                assert_eq!(
                    get_common_ancestor(origin, origin),
                    Some(CommonAncestorResult::Same {
                        common_ancestor: origin.clone()
                    })
                );
                assert_eq!(
                    get_common_ancestor(&root, origin),
                    Some(CommonAncestorResult::Ancestor {
                        common_ancestor: root.clone()
                    })
                );
                assert_eq!(
                    get_common_ancestor(origin, &root),
                    Some(CommonAncestorResult::Descendant {
                        common_ancestor: root.clone()
                    })
                );
                let parent = caret.get_parent_at_caret().unwrap();
                assert_eq!(
                    get_common_ancestor(&parent, origin),
                    Some(CommonAncestorResult::Ancestor {
                        common_ancestor: parent.clone()
                    })
                );
                assert_eq!(
                    get_common_ancestor(origin, &parent),
                    Some(CommonAncestorResult::Descendant {
                        common_ancestor: parent.clone()
                    })
                );
            }
        }
        Ok(())
    });
}

/// `testBranch(title, nodes, ancestors, commonAncestor)`.
fn test_branch(get: fn(&OrderingDoc) -> [NodeRef; 5]) {
    with_ordering_doc(move |d| {
        let [n0, n1, a0, a1, common_ancestor] = get(&d);
        assert_eq!(
            get_common_ancestor(&n0, &n1),
            Some(CommonAncestorResult::Branch {
                a: a0.clone(),
                b: a1.clone(),
                common_ancestor: common_ancestor.clone(),
            })
        );
        assert_eq!(
            get_common_ancestor(&n1, &n0),
            Some(CommonAncestorResult::Branch {
                a: a1,
                b: a0,
                common_ancestor,
            })
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > paragraphNode and listNode are siblings in root
#[test]
fn paragraph_node_and_list_node_are_siblings_in_root() {
    test_branch(|d| {
        [
            d.paragraph_node.clone(),
            d.list_node.clone(),
            d.paragraph_node.clone(),
            d.list_node.clone(),
            d.root_node.clone(),
        ]
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > paragraphText and linkNode->linkText are siblings in paragraphNode
#[test]
fn paragraph_text_and_link_text_are_siblings_in_paragraph_node() {
    test_branch(|d| {
        [
            d.paragraph_text.clone(),
            d.link_text.clone(),
            d.paragraph_text.clone(),
            d.link_node.clone(),
            d.paragraph_node.clone(),
        ]
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > paragraphNode->linkNode->linkText and listNode->listItem2->listItemText2 are siblings in root
#[test]
fn link_text_and_list_item_text2_are_siblings_in_root() {
    test_branch(|d| {
        [
            d.link_text.clone(),
            d.list_item_text2.clone(),
            d.paragraph_node.clone(),
            d.list_node.clone(),
            d.root_node.clone(),
        ]
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > listItem1 and listItem2->listItemText2 are siblings in listNode
#[test]
fn list_item1_and_list_item_text2_are_siblings_in_list_node() {
    test_branch(|d| {
        [
            d.list_item1.clone(),
            d.list_item_text2.clone(),
            d.list_item1.clone(),
            d.list_item2.clone(),
            d.list_node.clone(),
        ]
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > listItem1->listItemText1 and listItem2->listItemText2 are siblings in listNode
#[test]
fn list_item_text1_and_list_item_text2_are_siblings_in_list_node() {
    test_branch(|d| {
        [
            d.list_item_text1.clone(),
            d.list_item_text2.clone(),
            d.list_item1.clone(),
            d.list_item2.clone(),
            d.list_node.clone(),
        ]
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > with a fully-selected text node preceded by an inline element > a single text node
#[test]
fn a_fully_selected_text_node_preceded_by_an_inline_element() {
    let editor = test_editor();
    let out: Rc<RefCell<Option<NodeRef>>> = Rc::default();
    let paragraph_out = out.clone();
    update_later(&editor, move || {
        let root = get_root();
        let paragraph = create_paragraph_node();
        root.append(std::slice::from_ref(&paragraph))?;
        let link = link("https://");
        link.append(&[create_text_node("link")])?;
        paragraph.append(std::slice::from_ref(&link))?;
        let text = create_text_node("Existing text...");
        paragraph.append(std::slice::from_ref(&text))?;
        let range = get_caret_range(
            get_text_point_caret(&text, NEXT, 0)?,
            get_text_point_caret(&text, NEXT, NEXT)?,
        )?;
        let new_range = remove_text(&range)?;
        assert_caret(
            &new_range.anchor,
            Some(NEXT),
            None,
            Some(&link),
            Some("sibling"),
        );
        new_range.focus.insert(&create_text_node("foo"))?;
        *paragraph_out.borrow_mut() = Some(paragraph);
        Ok(())
    });
    // Upstream checks the reconciled innerHTML,
    // `<p dir="auto"><a href="https://"><span data-lexical-text="true">link</span></a><span data-lexical-text="true">foo</span></p>`;
    // the reconciler here writes no DOM, so the committed tree it renders
    // from is checked: the paragraph holds the link with "link", then "foo".
    let paragraph = out.borrow().clone().unwrap();
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children(), std::slice::from_ref(&paragraph));
            let children = paragraph.get_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].get_type(), "link");
            assert_eq!(texts_of(&children[0].get_children()), ["link"]);
            assert!(children[0].get_children()[0].is_text());
            assert!(children[1].is_text());
            assert_eq!(children[1].get_text_content(), "foo");
        })
        .unwrap();
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > canBeEmpty()=false parent cleanup > MarkNode is removed when all children are deleted via element-type range
#[test]
fn mark_node_is_removed_when_all_children_are_deleted_via_element_type_range() {
    run(|| {
        let root = get_root();
        root.clear()?;
        let before = create_text_node("before ");
        let mark = create_mark_node(&["test-id".to_owned()]);
        let after = create_text_node(" after");
        mark.append(&[create_text_node("aaa"), create_text_node("bbb")])?;
        root.append(&[paragraph_of(&[
            before.clone(),
            mark.clone(),
            after.clone(),
        ])?])?;
        let range = get_caret_range(
            get_child_caret_at_index(&mark, 0, NEXT),
            get_child_caret_at_index(&mark, 2, NEXT),
        )?;
        remove_text(&range)?;
        assert!(!mark.is_attached());
        assert!(before.is_attached());
        assert!(after.is_attached());
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > canBeEmpty()=false parent cleanup > LinkNode is removed when all children are deleted via element-type range
#[test]
fn link_node_is_removed_when_all_children_are_deleted_via_element_type_range() {
    run(|| {
        let root = get_root();
        root.clear()?;
        let before = create_text_node("before ");
        let link = link("https://lexical.dev");
        let after = create_text_node(" after");
        link.append(&[create_text_node("click")])?;
        root.append(&[paragraph_of(&[
            before.clone(),
            link.clone(),
            after.clone(),
        ])?])?;
        let range = get_caret_range(
            get_child_caret_at_index(&link, 0, NEXT),
            get_child_caret_at_index(&link, 1, NEXT),
        )?;
        remove_text(&range)?;
        assert!(!link.is_attached());
        assert!(before.is_attached());
        assert!(after.is_attached());
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > canBeEmpty()=false parent cleanup > canBeEmpty()=false parent survives when only some children are removed
#[test]
fn can_be_empty_false_parent_survives_when_only_some_children_are_removed() {
    run(|| {
        let root = get_root();
        root.clear()?;
        let mark = create_mark_node(&["test-id".to_owned()]);
        let t2 = create_text_node("bbb");
        mark.append(&[create_text_node("aaa"), t2.clone(), create_text_node("ccc")])?;
        root.append(&[paragraph_of(std::slice::from_ref(&mark))?])?;
        let range = get_caret_range(
            get_child_caret_at_index(&mark, 0, NEXT),
            get_sibling_caret(&t2, NEXT),
        )?;
        remove_text(&range)?;
        assert!(mark.is_attached());
        assert!(mark.get_children_size() > 0);
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > canBeEmpty()=false parent cleanup > multiple canBeEmpty()=false parents in same range are cleaned up
#[test]
fn multiple_can_be_empty_false_parents_in_same_range_are_cleaned_up() {
    run(|| {
        let root = get_root();
        root.clear()?;
        let mark1 = create_mark_node(&["id1".to_owned()]);
        let mark2 = create_mark_node(&["id2".to_owned()]);
        mark1.append(&[create_text_node("aaa")])?;
        mark2.append(&[create_text_node("bbb")])?;
        let p = paragraph_of(&[mark1.clone(), create_text_node(" "), mark2.clone()])?;
        root.append(std::slice::from_ref(&p))?;
        let range = get_caret_range(
            get_child_caret_at_index(&p, 0, NEXT),
            get_child_caret_at_index(&p, 3, NEXT),
        )?;
        remove_text(&range)?;
        assert!(!mark1.is_attached());
        assert!(!mark2.is_attached());
        Ok(())
    });
}

/// A block decorator, then `last`, at the root; then `$selectAll()` and
/// `removeText()`, each in a discrete update.
fn select_all_delete_after_block_decorator(last: fn() -> Result<NodeRef, LexicalError>) -> Editor {
    let editor = test_editor();
    update(&editor, move || {
        let root = get_root();
        root.clear()?;
        let decorator = set_is_inline(&create_test_decorator_node(), false);
        root.append(&[decorator, last()?])
    });
    update(&editor, || select_all()?.remove_text());
    editor
}

fn bullet_list_with_item() -> Result<NodeRef, LexicalError> {
    append(
        create_list_node(ListType::Bullet, 1.0),
        &[append(
            create_list_item_node(None),
            &[create_text_node("item")],
        )?],
    )
}

/// The root holds one empty paragraph.
fn assert_root_is_one_empty_paragraph(editor: &Editor) {
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 1);
            assert!(is_paragraph(root.get_first_child().as_ref()));
            assert_eq!(root.get_text_content(), "");
        })
        .unwrap();
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > the root's last child after select all delete > a list is replaced with an empty paragraph
#[test]
fn a_list_is_replaced_with_an_empty_paragraph() {
    let editor = select_all_delete_after_block_decorator(bullet_list_with_item);
    assert_root_is_one_empty_paragraph(&editor);
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > the root's last child after select all delete > an inline insert after the list deletion does not throw
#[test]
fn an_inline_insert_after_the_list_deletion_does_not_throw() {
    let editor = select_all_delete_after_block_decorator(bullet_list_with_item);
    let result = editor.update(
        || {
            get_selection()
                .filter(SelectionRef::is_range)
                .expect("a range selection")
                .insert_nodes(vec![create_line_break_node()])
        },
        discrete(),
    );
    assert!(result.is_ok(), "{result:?}");
    editor
        .read(|| {
            let paragraph = get_root().get_first_child();
            assert!(is_paragraph(paragraph.as_ref()), "Expected a ParagraphNode");
            assert!(is_line_break(paragraph.unwrap().get_first_child().as_ref()));
        })
        .unwrap();
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > the root's last child after select all delete > a container that may be empty is kept
#[test]
fn a_container_that_may_be_empty_is_kept() {
    let editor = select_all_delete_after_block_decorator(|| {
        append(
            create_test_element_node(),
            &[paragraph_of(&[create_text_node("inner")])?],
        )
    });
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 1);
            let container = root.get_first_child().unwrap();
            assert!(container.get_latest().is::<TestElementNode>());
            assert!(!is_paragraph(Some(&container)));
        })
        .unwrap();
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > the root's last child after select all delete > a paragraph is kept
#[test]
fn a_paragraph_is_kept() {
    let editor =
        select_all_delete_after_block_decorator(|| paragraph_of(&[create_text_node("text")]));
    assert_root_is_one_empty_paragraph(&editor);
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::$splitAtPointCaretNext > Does not split a TextNode at the beginning
#[test]
fn split_at_point_caret_next_does_not_split_a_text_node_at_the_beginning() {
    run(|| {
        let text_node = create_text_node("test");
        let paragraph_node = paragraph_of(std::slice::from_ref(&text_node))?;
        set_root(std::slice::from_ref(&paragraph_node))?;
        let caret = get_text_point_caret(&text_node, NEXT, 0)?;
        let after = split_at_point_caret_next(&caret, &Default::default())?;
        assert_eq!(text_node.get_text_content(), "test");
        assert!(after.is_some_and(|a| child(&paragraph_node, NEXT).is_same_point_caret(&a)));
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::$splitAtPointCaretNext > Splits a TextNode in the middle
#[test]
fn split_at_point_caret_next_splits_a_text_node_in_the_middle() {
    run(|| {
        let text_node = create_text_node("test");
        let paragraph_node = paragraph_of(std::slice::from_ref(&text_node))?;
        set_root(std::slice::from_ref(&paragraph_node))?;
        let caret = get_text_point_caret(&text_node, NEXT, 2)?;
        let after = split_at_point_caret_next(&caret, &Default::default())?;
        assert_eq!(text_node.get_text_content(), "te");
        let next_caret = get_sibling_caret(&text_node, NEXT);
        assert!(after.is_some_and(|a| next_caret.is_same_point_caret(&a)));
        let split_node = next_caret.get_node_at_caret();
        assert_eq!(
            split_node
                .filter(NodeRef::is_text)
                .map(|n| n.get_text_content()),
            Some("st".to_owned())
        );
        Ok(())
    });
}

// upstream: lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::$splitAtPointCaretNext > Splits a ParagraphNode
#[test]
fn split_at_point_caret_next_splits_a_paragraph_node() {
    run(|| {
        let before_text_node = create_text_node("before");
        let after_text_node = create_text_node("after");
        let paragraph_node = paragraph_of(&[before_text_node.clone(), after_text_node.clone()])?;
        set_root(std::slice::from_ref(&paragraph_node))?;
        let caret = get_sibling_caret(&before_text_node, NEXT);
        let after = split_at_point_caret_next(&caret, &Default::default())?;
        assert_eq!(paragraph_node.get_all_text_nodes(), [before_text_node]);
        let next_caret = get_sibling_caret(&paragraph_node, NEXT);
        assert!(after.is_some_and(|a| next_caret.is_same_point_caret(&a)));
        let split_node = next_caret.get_node_at_caret();
        assert_eq!(
            split_node
                .filter(|n| is_paragraph(Some(n)))
                .map(|n| n.get_all_text_nodes()),
            Some(vec![after_text_node])
        );
        Ok(())
    });
}
