//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts`.
//!
//! Upstream's `toEqual` compares each `{depth, node}` with the node's
//! latest version (`getLatest()`); a [`NodeRef`] is a key that always
//! resolves to the node's latest version, so comparing [`DFSNode`]s
//! compares the same thing.

use crate::upstream_tests::support::{create_test_element_node, initialize_unit_test, update};
use crate::utils::{
    DFSNode, dfs, first_to_last_iterator, get_next_sibling_or_parent_sibling,
    last_to_first_iterator, reverse_dfs,
};
use crate::{
    CreateEditorArgs, Editor, EditorReadMode, NodeKey, NodeRef, create_paragraph_node,
    create_text_node, get_node_by_key, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `DFSKeyPair`.
type KeyPair = (usize, NodeKey);

fn dfs_node(depth: usize, node: &NodeRef) -> DFSNode {
    DFSNode {
        depth,
        node: node.clone(),
    }
}

fn text(text: &str) -> NodeRef {
    create_text_node(text)
}

fn bold(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_format("bold");
    node
}

fn element_of(element: NodeRef, children: &[NodeRef]) -> NodeRef {
    element.append(children).unwrap();
    element
}

/// `keysForNode(depth, node, $getChildren)`.
fn keys_for_node(
    depth: usize,
    node: &NodeRef,
    get_children: fn(&NodeRef) -> Vec<NodeRef>,
    out: &mut Vec<KeyPair>,
) {
    out.push((depth, node.get_key().to_owned()));
    if node.is_element() {
        for child in get_children(node) {
            keys_for_node(depth + 1, &child, get_children, out);
        }
    }
}

/// The `dfs order` describe's `beforeEach`: the tree
///
/// ```text
///               R
///        P1            P2
///     B1     B2     T4 T5 B3
///     T1   T2 T3          T6
/// ```
///
/// and its keys in DFS and reverse DFS order, checked as upstream does.
fn dfs_order_editor() -> (Editor, Vec<KeyPair>, Vec<KeyPair>) {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let keys = Rc::new(RefCell::new((Vec::new(), Vec::new())));
    let out = keys.clone();
    update(&editor, move || {
        let root = get_root();
        let paragraph1 = create_paragraph_node();
        let paragraph2 = create_paragraph_node();
        let block1 = create_test_element_node();
        let block2 = create_test_element_node();
        let block3 = create_test_element_node();
        let text1 = text("text1");
        let text2 = text("text2");
        let text3 = text("text3");
        let text4 = text("text4");
        let text5 = text("text5");
        let text6 = text("text6");
        root.append(&[paragraph1.clone(), paragraph2.clone()])?;
        paragraph1.append(&[block1.clone(), block2.clone()])?;
        paragraph2.append(&[text4.clone(), text5.clone()])?;
        text5.toggle_format("bold"); // Prevent from merging with text 4
        paragraph2.append(std::slice::from_ref(&block3))?;
        block1.append(std::slice::from_ref(&text1))?;
        block2.append(&[text2.clone(), text3.clone()])?;
        text3.toggle_format("bold"); // Prevent from merging with text2
        block3.append(std::slice::from_ref(&text6))?;

        let mut expected_keys = Vec::new();
        keys_for_node(
            0,
            &root,
            |n| first_to_last_iterator(n).collect(),
            &mut expected_keys,
        );
        let mut reverse_expected_keys = Vec::new();
        keys_for_node(
            0,
            &root,
            |n| last_to_first_iterator(n).collect(),
            &mut reverse_expected_keys,
        );
        let pairs = |nodes: &[&NodeRef]| -> Vec<KeyPair> {
            nodes
                .iter()
                .map(|n| (n.get_parent_keys().len(), n.get_key().to_owned()))
                .collect()
        };
        // R, P1, B1, T1, B2, T2, T3, P2, T4, T5, B3, T6
        assert_eq!(
            expected_keys,
            pairs(&[
                &root,
                &paragraph1,
                &block1,
                &text1,
                &block2,
                &text2,
                &text3,
                &paragraph2,
                &text4,
                &text5,
                &block3,
                &text6
            ])
        );
        // R, P2, B3, T6, T5, T4, P1, B2, T3, T2, B1, T1
        assert_eq!(
            reverse_expected_keys,
            pairs(&[
                &root,
                &paragraph2,
                &block3,
                &text6,
                &text5,
                &text4,
                &paragraph1,
                &block2,
                &text3,
                &text2,
                &block1,
                &text1
            ])
        );
        *out.borrow_mut() = (expected_keys, reverse_expected_keys);
        Ok(())
    });
    let (expected_keys, reverse_expected_keys) = keys.take();
    (editor, expected_keys, reverse_expected_keys)
}

fn nodes_of(keys: &[KeyPair]) -> Vec<DFSNode> {
    keys.iter()
        .map(|(depth, key)| dfs_node(*depth, &get_node_by_key(key).unwrap()))
        .collect()
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > DFS node order
#[test]
fn dfs_node_order() {
    let (editor, expected_keys, _) = dfs_order_editor();
    editor
        .read_with_mode(EditorReadMode::Latest, || {
            let expected_nodes = nodes_of(&expected_keys);
            let n = expected_nodes.len();
            let (first, second) = (&expected_nodes[0].node, &expected_nodes[1].node);
            let (last, second_to_last) = (&expected_nodes[n - 1].node, &expected_nodes[n - 2].node);
            assert_eq!(dfs(Some(first), Some(last)), expected_nodes);
            assert_eq!(
                dfs(Some(second), Some(second_to_last)),
                expected_nodes[1..n - 1]
            );
            assert_eq!(dfs(None, None), expected_nodes);
            assert_eq!(dfs(Some(&get_root()), None), expected_nodes);
        })
        .unwrap();
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > Reverse DFS node order
#[test]
fn reverse_dfs_node_order() {
    let (editor, _, reverse_expected_keys) = dfs_order_editor();
    editor
        .read_with_mode(EditorReadMode::Latest, || {
            let expected_nodes = nodes_of(&reverse_expected_keys);
            let n = expected_nodes.len();
            let (first, second) = (&expected_nodes[0].node, &expected_nodes[1].node);
            let (last, second_to_last) = (&expected_nodes[n - 1].node, &expected_nodes[n - 2].node);
            assert_eq!(reverse_dfs(Some(first), Some(last)), expected_nodes);
            assert_eq!(
                reverse_dfs(Some(second), Some(second_to_last)),
                expected_nodes[1..n - 1]
            );
            assert_eq!(reverse_dfs(None, None), expected_nodes);
            assert_eq!(reverse_dfs(Some(&get_root()), None), expected_nodes);
        })
        .unwrap();
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > DFS from middle leaf node should only include that node
#[test]
fn dfs_from_middle_leaf_node_should_only_include_that_node() {
    let (editor, _, _) = dfs_order_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[element_of(
            create_paragraph_node(),
            &[text("Hello"), bold("world"), text("!")],
        )])?;
        let paragraph = root.get_first_child().unwrap();
        assert!(paragraph.is_element());
        let children = paragraph.get_children();
        assert_eq!(dfs(Some(&children[1]), None), [dfs_node(2, &children[1])]);
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > DFS starting at last child of element node should not include parent's siblings
#[cfg(feature = "mark")]
#[test]
fn dfs_starting_at_last_child_of_element_node_should_not_include_parents_siblings() {
    let (editor, _, _) = dfs_order_editor();
    update(&editor, || {
        let root = get_root();
        let p1 = element_of(
            create_paragraph_node(),
            &[
                text("Hello"),
                element_of(crate::mark::create_mark_node(&[]), &[bold("world")]),
            ],
        );
        let p2 = element_of(create_paragraph_node(), &[text("!!")]);
        root.clear()?;
        root.append(&[p1.clone(), p2])?;
        let mark_node = p1.get_child_at_index(1).unwrap();
        assert!(
            crate::mark::is_mark_node(&mark_node),
            "first child must be MarkNode"
        );
        let text_nodes = mark_node.get_children();
        assert_eq!(
            dfs(Some(&mark_node), None),
            [dfs_node(2, &mark_node), dfs_node(3, &text_nodes[0])]
        );
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > DFS with endNode as last descendant should not include parent siblings
#[test]
fn dfs_with_end_node_as_last_descendant_should_not_include_parent_siblings() {
    let (editor, _, _) = dfs_order_editor();
    update(&editor, || {
        let root = get_root();
        let element_node1 = element_of(create_test_element_node(), &[text("word1"), text("word2")]);
        let element_node2 = element_of(create_test_element_node(), &[text("word3")]);
        root.clear()?;
        root.append(&[element_node1.clone(), element_node2])?;
        let results = dfs(
            Some(&element_node1),
            Some(&element_node1.get_last_descendant().unwrap()),
        );
        // Should include: elementNode1, word1, word2
        // Should NOT include: elementNode2, word3 (parent's sibling)
        assert_eq!(
            results,
            [
                dfs_node(1, &element_node1),
                dfs_node(2, &element_node1.get_first_descendant().unwrap()),
                dfs_node(2, &element_node1.get_last_descendant().unwrap()),
            ]
        );
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > DFS with endNode as ElementNode should stop at the ElementNode, not include children
#[test]
fn dfs_with_end_node_as_element_node_should_stop_at_the_element_node() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let root = get_root();
        let element_node1 = element_of(create_test_element_node(), &[text("word1"), text("word2")]);
        let element_node2 = element_of(create_test_element_node(), &[text("word3")]);
        root.clear()?;
        root.append(&[element_node1.clone(), element_node2.clone()])?;
        let first = element_node1.get_first_descendant().unwrap();
        let last = element_node1.get_last_descendant().unwrap();
        assert_eq!(
            dfs(None, Some(&element_node2)),
            [
                dfs_node(0, &get_root()),
                dfs_node(1, &element_node1),
                dfs_node(2, &first),
                dfs_node(2, &last),
                dfs_node(1, &element_node2),
            ]
        );
        assert_eq!(
            dfs(Some(&element_node1), Some(&element_node2)),
            [
                dfs_node(1, &element_node1),
                dfs_node(2, &first),
                dfs_node(2, &last),
                dfs_node(1, &element_node2),
            ]
        );
        assert_eq!(
            dfs(Some(&element_node1), Some(&element_node1)),
            [dfs_node(1, &element_node1)]
        );
        Ok(())
    });
}

/// The two updates' first half of `DFS triggers getLatest()`: a paragraph
/// with two blocks; the keys of the root, the paragraph and the blocks.
fn get_latest_editor() -> (Editor, [NodeKey; 4]) {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let keys = crate::upstream_tests::support::update_with(&editor, || {
        let root = get_root();
        let paragraph = create_paragraph_node();
        let block1 = create_test_element_node();
        let block2 = create_test_element_node();
        let keys = [&root, &paragraph, &block1, &block2].map(|n| n.get_key().to_owned());
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.append(&[block1, block2])?;
        Ok(keys)
    });
    (editor, keys)
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > DFS triggers getLatest()
#[test]
fn dfs_triggers_get_latest() {
    let (editor, keys) = get_latest_editor();
    update(&editor, move || {
        let [root, paragraph, block1, block2] = keys.map(|k| get_node_by_key(&k).unwrap());
        let block3 = create_test_element_node();
        assert!(block1.is_element());
        // this will (only) change the latest state of block1
        block1.append(std::slice::from_ref(&block3))?;
        assert_eq!(
            dfs(Some(&root), None),
            [
                dfs_node(0, &root),
                dfs_node(1, &paragraph),
                dfs_node(2, &block1),
                dfs_node(3, &block3),
                dfs_node(2, &block2),
            ]
        );
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > reverse DFS triggers getLatest()
#[test]
fn reverse_dfs_triggers_get_latest() {
    let (editor, keys) = get_latest_editor();
    update(&editor, move || {
        let [root, paragraph, block1, block2] = keys.map(|k| get_node_by_key(&k).unwrap());
        let block3 = create_test_element_node();
        assert!(block1.is_element());
        // this will (only) change the latest state of block1
        block1.append(std::slice::from_ref(&block3))?;
        assert_eq!(
            reverse_dfs(None, None),
            [
                dfs_node(0, &root),
                dfs_node(1, &paragraph),
                dfs_node(2, &block2),
                dfs_node(2, &block1),
                dfs_node(3, &block3),
            ]
        );
        Ok(())
    });
}

/// An empty paragraph before a paragraph with text; the empty one's key.
fn empty_paragraph_editor() -> (Editor, NodeKey) {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let key = crate::upstream_tests::support::update_with(&editor, || {
        let root = get_root();
        let paragraph = create_paragraph_node();
        let paragraph2 = create_paragraph_node();
        paragraph2.append(&[text("test")])?;
        root.append(&[paragraph.clone(), paragraph2])?;
        Ok(paragraph.get_key().to_owned())
    });
    (editor, key)
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > DFS of empty ParagraphNode returns only itself
#[test]
fn dfs_of_empty_paragraph_node_returns_only_itself() {
    let (editor, key) = empty_paragraph_editor();
    update(&editor, move || {
        let paragraph = get_node_by_key(&key).unwrap();
        assert_eq!(dfs(Some(&paragraph), None), [dfs_node(1, &paragraph)]);
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > reverse DFS of empty ParagraphNode returns only itself
#[test]
fn reverse_dfs_of_empty_paragraph_node_returns_only_itself() {
    let (editor, key) = empty_paragraph_editor();
    update(&editor, move || {
        let paragraph = get_node_by_key(&key).unwrap();
        assert_eq!(
            reverse_dfs(Some(&paragraph), Some(&paragraph)),
            [dfs_node(1, &paragraph)]
        );
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > reverse DFS of empty RootNode returns only itself
#[test]
fn reverse_dfs_of_empty_root_node_returns_only_itself() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        assert_eq!(reverse_dfs(None, None), [dfs_node(0, &get_root())]);
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > $getNextSiblingOrParentSibling
#[test]
fn get_next_sibling_or_parent_sibling_test() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let root = get_root();
        let paragraph = create_paragraph_node();
        let paragraph2 = create_paragraph_node();
        let text1 = text("text1");
        let text2 = text("text2");
        text2.toggle_unmergeable();
        paragraph.append(&[text1.clone(), text2.clone()])?;
        root.append(&[paragraph.clone(), paragraph2.clone()])?;
        // Sibling
        assert_eq!(
            get_next_sibling_or_parent_sibling(&paragraph),
            Some((paragraph2.clone(), 0))
        );
        assert_eq!(
            get_next_sibling_or_parent_sibling(&text1),
            Some((text2.clone(), 0))
        );
        // Parent
        assert_eq!(
            get_next_sibling_or_parent_sibling(&text2),
            Some((paragraph2.clone(), -1))
        );
        // Null (end of the tree)
        assert_eq!(get_next_sibling_or_parent_sibling(&paragraph2), None);
        Ok(())
    });
}
