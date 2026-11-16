//! Ports of `packages/lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx`,
//! on `initializeUnitTest`'s editor, whose root element makes it reconcile.

use crate::upstream_tests::support::{create_test_element_node, initialize_unit_test, update};
use crate::{
    CreateEditorArgs, Editor, NodeRef, create_paragraph_node, create_text_node, get_node_by_key,
    get_root,
};

fn node_map_size(editor: &Editor) -> usize {
    editor.get_editor_state().0.node_map.borrow().len()
}

fn editor() -> Editor {
    initialize_unit_test(CreateEditorArgs::default())
}

fn unmergeable(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_unmergeable();
    node
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and subchild
#[test]
fn root_node_clear_with_a_child_and_subchild() {
    let editor = editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("foo")])?;
        get_root().append(&[paragraph])
    });
    assert_eq!(node_map_size(&editor), 3);
    update(&editor, || get_root().clear());
    assert_eq!(node_map_size(&editor), 1);
}

/// `collects newly created dirty subtrees, parent created first: ${parentFirst}`.
fn collects_newly_created_dirty_subtrees(parent_first: bool) {
    let editor = editor();
    update(&editor, move || {
        let first = create_test_element_node();
        let second = create_test_element_node();
        let (parent, child) = if parent_first {
            (first, second)
        } else {
            (second, first)
        };
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("temporary")])?;
        child.append(&[paragraph])?;
        parent.append(&[child])?;
        get_root().append(std::slice::from_ref(&parent))?;
        parent.remove(false)
    });
    let keys: Vec<String> = editor
        .get_editor_state()
        .0
        .node_map
        .borrow()
        .keys()
        .map(|k| k.to_string())
        .collect();
    assert_eq!(keys, ["root"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > collects newly created dirty subtrees, parent created first: false
#[test]
fn collects_newly_created_dirty_subtrees_parent_created_first_false() {
    collects_newly_created_dirty_subtrees(false);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > collects newly created dirty subtrees, parent created first: true
#[test]
fn collects_newly_created_dirty_subtrees_parent_created_first_true() {
    collects_newly_created_dirty_subtrees(true);
}

/// A paragraph of `foo`, `bar` and `zzz` (keys 1 to 4).
fn three_subchildren() -> Result<(), crate::LexicalError> {
    let text1 = create_text_node("foo");
    let text2 = unmergeable("bar");
    let text3 = unmergeable("zzz");
    let paragraph = create_paragraph_node();
    paragraph.append(&[text1, text2, text3])?;
    get_root().append(&[paragraph])
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and three subchildren
#[test]
fn root_node_clear_with_a_child_and_three_subchildren() {
    let editor = editor();
    update(&editor, three_subchildren);
    assert_eq!(node_map_size(&editor), 5);
    update(&editor, || get_root().clear());
    assert_eq!(node_map_size(&editor), 1);
}

/// `RootNode.clear() with a child and three subchildren, subchild ${i} removed first`.
fn root_node_clear_subchild_removed_first(i: usize) {
    let editor = editor();
    update(&editor, three_subchildren);
    assert_eq!(node_map_size(&editor), 5);
    update(&editor, move || {
        let root = get_root();
        let first_child = root.get_first_child().filter(NodeRef::is_element);
        let first_child = first_child.expect("an element");
        let subchild = first_child.get_child_at_index(i).unwrap();
        assert_eq!(subchild.get_text_content(), ["foo", "bar", "zzz"][i]);
        subchild.remove(false)?;
        root.clear()
    });
    assert_eq!(node_map_size(&editor), 1);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and three subchildren, subchild 0 removed first
#[test]
fn root_node_clear_with_a_child_and_three_subchildren_subchild_0_removed_first() {
    root_node_clear_subchild_removed_first(0);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and three subchildren, subchild 1 removed first
#[test]
fn root_node_clear_with_a_child_and_three_subchildren_subchild_1_removed_first() {
    root_node_clear_subchild_removed_first(1);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and three subchildren, subchild 2 removed first
#[test]
fn root_node_clear_with_a_child_and_three_subchildren_subchild_2_removed_first() {
    root_node_clear_subchild_removed_first(2);
}

/// `RootNode.clear() with a complex tree, nodes ${removeKeys} removed first`:
///
/// ```text
///          R
///          P
///     T   TE    T
///        T  T
/// ```
fn root_node_clear_with_a_complex_tree(remove_keys: &'static [&'static str]) {
    let editor = editor();
    update(&editor, || {
        let test_element = create_test_element_node(); // 1
        let test_element_text1 = unmergeable("te1"); // 2
        let test_element_text2 = unmergeable("te2"); // 3
        let text1 = unmergeable("a"); // 4
        let text2 = unmergeable("b"); // 5
        let paragraph = create_paragraph_node(); // 6
        test_element.append(&[test_element_text1, test_element_text2])?;
        paragraph.append(&[text1, test_element, text2])?;
        get_root().append(&[paragraph])
    });
    assert_eq!(node_map_size(&editor), 7);
    update(&editor, move || {
        for key in remove_keys {
            get_node_by_key(key).unwrap().remove(false)?;
        }
        get_root().clear()
    });
    assert_eq!(node_map_size(&editor), 1);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes  removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_none_removed_first() {
    root_node_clear_with_a_complex_tree(&[]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_1_removed_first() {
    root_node_clear_with_a_complex_tree(&["1"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,2 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_1_2_removed_first() {
    root_node_clear_with_a_complex_tree(&["1", "2"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,3 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_1_3_removed_first() {
    root_node_clear_with_a_complex_tree(&["1", "3"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,4 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_1_4_removed_first() {
    root_node_clear_with_a_complex_tree(&["1", "4"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,5 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_1_5_removed_first() {
    root_node_clear_with_a_complex_tree(&["1", "5"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,6 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_1_6_removed_first() {
    root_node_clear_with_a_complex_tree(&["1", "6"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_2_removed_first() {
    root_node_clear_with_a_complex_tree(&["2"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,1 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_2_1_removed_first() {
    root_node_clear_with_a_complex_tree(&["2", "1"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,3 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_2_3_removed_first() {
    root_node_clear_with_a_complex_tree(&["2", "3"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,4 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_2_4_removed_first() {
    root_node_clear_with_a_complex_tree(&["2", "4"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,5 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_2_5_removed_first() {
    root_node_clear_with_a_complex_tree(&["2", "5"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,6 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_2_6_removed_first() {
    root_node_clear_with_a_complex_tree(&["2", "6"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_3_removed_first() {
    root_node_clear_with_a_complex_tree(&["3"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,1 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_3_1_removed_first() {
    root_node_clear_with_a_complex_tree(&["3", "1"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,2 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_3_2_removed_first() {
    root_node_clear_with_a_complex_tree(&["3", "2"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,4 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_3_4_removed_first() {
    root_node_clear_with_a_complex_tree(&["3", "4"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,5 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_3_5_removed_first() {
    root_node_clear_with_a_complex_tree(&["3", "5"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,6 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_3_6_removed_first() {
    root_node_clear_with_a_complex_tree(&["3", "6"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_4_removed_first() {
    root_node_clear_with_a_complex_tree(&["4"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,1 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_4_1_removed_first() {
    root_node_clear_with_a_complex_tree(&["4", "1"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,2 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_4_2_removed_first() {
    root_node_clear_with_a_complex_tree(&["4", "2"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,3 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_4_3_removed_first() {
    root_node_clear_with_a_complex_tree(&["4", "3"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,5 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_4_5_removed_first() {
    root_node_clear_with_a_complex_tree(&["4", "5"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,6 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_4_6_removed_first() {
    root_node_clear_with_a_complex_tree(&["4", "6"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_5_removed_first() {
    root_node_clear_with_a_complex_tree(&["5"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,1 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_5_1_removed_first() {
    root_node_clear_with_a_complex_tree(&["5", "1"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,2 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_5_2_removed_first() {
    root_node_clear_with_a_complex_tree(&["5", "2"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,3 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_5_3_removed_first() {
    root_node_clear_with_a_complex_tree(&["5", "3"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,4 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_5_4_removed_first() {
    root_node_clear_with_a_complex_tree(&["5", "4"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,6 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_5_6_removed_first() {
    root_node_clear_with_a_complex_tree(&["5", "6"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_6_removed_first() {
    root_node_clear_with_a_complex_tree(&["6"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,1 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_6_1_removed_first() {
    root_node_clear_with_a_complex_tree(&["6", "1"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,2 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_6_2_removed_first() {
    root_node_clear_with_a_complex_tree(&["6", "2"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,3 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_6_3_removed_first() {
    root_node_clear_with_a_complex_tree(&["6", "3"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,4 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_6_4_removed_first() {
    root_node_clear_with_a_complex_tree(&["6", "4"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,5 removed first
#[test]
fn root_node_clear_with_a_complex_tree_nodes_6_5_removed_first() {
    root_node_clear_with_a_complex_tree(&["6", "5"]);
}
