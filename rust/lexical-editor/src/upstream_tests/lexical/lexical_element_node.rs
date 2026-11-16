//! Ports of `packages/lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx`.
//!
//! `init()` mounts the editor and appends a `TestElementNode` holding
//! `Foo`, a segmented `Bar` and `Baz`, with the selection at the start of
//! `Foo`.

use crate::upstream_tests::support::{
    create_test_element_node, initialize_unit_test, update, update_with,
};
use crate::{
    CreateEditorArgs, Editor, NodeRef, PointType, Which, create_paragraph_node, create_text_node,
    get_root, get_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `init()`.
fn element_node_editor() -> Editor {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let block = create_test_element_node();
        let text = create_text_node("Foo");
        let text2 = create_text_node("Bar");
        // Prevent text nodes from combining.
        text2.set_mode("segmented")?;
        let text3 = create_text_node("Baz");
        // Some operations require a selection to exist, hence
        // we make a selection in the setup code.
        text.select(Some(0), Some(0))?;
        block.append(&[text, text2, text3])?;
        get_root().append(&[block])
    });
    editor
}

/// The block `init()` appends.
fn first_block() -> NodeRef {
    let block = get_root().get_first_child().unwrap();
    assert!(block.is_element());
    block
}

fn keys(nodes: &[NodeRef]) -> Vec<String> {
    nodes.iter().map(|n| n.get_key().to_owned()).collect()
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > exportJSON() > should return and object conforming to the expected schema
#[test]
fn export_json_conforms_to_the_expected_schema() {
    let editor = element_node_editor();
    update(&editor, || {
        let json = create_test_element_node().export_json(false);
        // Present-with-undefined, not absent.
        assert_eq!(json.get("textFormat"), Some(&crate::json::Value::Undefined));
        assert_eq!(json.get("textStyle"), Some(&crate::json::Value::Undefined));
        assert_eq!(
            crate::json::Value::Object(json).to_string(),
            r#"{"children":[],"direction":null,"format":"","indent":0,"type":"test_block","version":1}"#
        );
        Ok(())
    });
}

/// `$getRoot().clear().append(p(bold "a", "b" color:green))`.
fn initial_state() -> Result<(), crate::LexicalError> {
    let a = create_text_node("a");
    a.toggle_format("bold");
    let b = create_text_node("b");
    b.set_style("color:green;");
    let paragraph = create_paragraph_node();
    paragraph.append(&[a, b])?;
    get_root().clear()?;
    get_root().append(&[paragraph])
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > exportJSON() > serializes only the first TextNode style and format
#[test]
fn export_json_serializes_only_the_first_text_node_style_and_format() {
    let editor = element_node_editor();
    update(&editor, initial_state);
    let json = editor.get_editor_state().to_json(false);
    let paragraph = json
        .get("root")
        .and_then(|r| r.get("children"))
        .and_then(|c| c.as_array())
        .map(|c| c[0].clone())
        .unwrap();
    let expected = crate::json::parse(
        r#"{"children":[{"detail":0,"format":1,"mode":"normal","style":"","text":"a","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"color:green;","text":"b","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":1,"textStyle":"","type":"paragraph","version":1}"#,
    )
    .unwrap();
    assert_eq!(paragraph, expected);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > exportJSON() > serializes the same way without a root element
#[test]
fn export_json_serializes_the_same_way_without_a_root_element() {
    let editor = element_node_editor();
    update(&editor, initial_state);
    let headless = crate::create_editor(CreateEditorArgs::default());
    update(&headless, initial_state);
    assert_eq!(
        headless.get_editor_state().to_json(false),
        editor.get_editor_state().to_json(false)
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getChildren() > no children
#[test]
fn get_children_no_children() {
    let editor = element_node_editor();
    update(&editor, || {
        let children = create_test_element_node().get_children();
        assert_eq!(children.len(), 0);
        assert_eq!(children, []);
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getChildren() > some children
#[test]
fn get_children_some_children() {
    let editor = element_node_editor();
    update(&editor, || {
        assert_eq!(first_block().get_children().len(), 3);
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getAllTextNodes() > basic
#[test]
fn get_all_text_nodes_basic() {
    let editor = element_node_editor();
    update(&editor, || {
        assert_eq!(first_block().get_all_text_nodes().len(), 3);
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getAllTextNodes() > nested
#[test]
fn get_all_text_nodes_nested() {
    let editor = element_node_editor();
    update(&editor, || {
        let block = create_test_element_node();
        let inner_block = create_test_element_node();
        let text = create_text_node("Foo");
        text.select(Some(0), Some(0))?;
        let text2 = create_text_node("Bar");
        let text3 = create_text_node("Baz");
        let text4 = create_text_node("Qux");
        block.append(&[text.clone(), inner_block.clone(), text4.clone()])?;
        inner_block.append(&[text2.clone(), text3.clone()])?;
        let children = block.get_all_text_nodes();
        assert_eq!(children.len(), 4);
        assert_eq!(
            keys(&children),
            keys(&[text.clone(), text2.clone(), text3.clone(), text4.clone()])
        );
        let inner_inner_block = create_test_element_node();
        let text5 = create_text_node("More");
        let text6 = create_text_node("Stuff");
        inner_inner_block.append(&[text5.clone(), text6.clone()])?;
        inner_block.append(&[inner_inner_block])?;
        let children2 = block.get_all_text_nodes();
        assert_eq!(children2.len(), 6);
        assert_eq!(
            keys(&children2),
            keys(&[text, text2, text3, text5, text6, text4])
        );
        get_root().append(&[block])
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getFirstChild() > basic
#[test]
fn get_first_child_basic() {
    let editor = element_node_editor();
    update(&editor, || {
        assert_eq!(
            first_block().get_first_child().unwrap().get_text_content(),
            "Foo"
        );
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getFirstChild() > empty
#[test]
fn get_first_child_empty() {
    let editor = element_node_editor();
    update(&editor, || {
        assert_eq!(create_test_element_node().get_first_child(), None);
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getLastChild() > basic
#[test]
fn get_last_child_basic() {
    let editor = element_node_editor();
    update(&editor, || {
        assert_eq!(
            first_block().get_last_child().unwrap().get_text_content(),
            "Baz"
        );
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getLastChild() > empty
#[test]
fn get_last_child_empty() {
    let editor = element_node_editor();
    update(&editor, || {
        assert_eq!(create_test_element_node().get_last_child(), None);
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getTextContent() > basic
#[test]
fn get_text_content_basic() {
    let editor = element_node_editor();
    update(&editor, || {
        assert_eq!(
            get_root().get_first_child().unwrap().get_text_content(),
            "FooBarBaz"
        );
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getTextContent() > empty
#[test]
fn get_text_content_empty() {
    let editor = element_node_editor();
    update(&editor, || {
        assert_eq!(create_test_element_node().get_text_content(), "");
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getTextContent() > nested
#[test]
fn get_text_content_nested() {
    let editor = element_node_editor();
    update(&editor, || {
        let block = create_test_element_node();
        let inner_block = create_test_element_node();
        let text = create_text_node("Foo");
        text.select(Some(0), Some(0))?;
        let text2 = create_text_node("Bar");
        let text3 = create_text_node("Baz");
        text3.set_mode("token")?;
        let text4 = create_text_node("Qux");
        block.append(&[text, inner_block.clone(), text4])?;
        inner_block.append(&[text2, text3])?;
        assert_eq!(block.get_text_content(), "FooBarBaz\n\nQux");
        let inner_inner_block = create_test_element_node();
        let text5 = create_text_node("More");
        text5.set_mode("token")?;
        let text6 = create_text_node("Stuff");
        inner_inner_block.append(&[text5, text6])?;
        inner_block.append(&[inner_inner_block])?;
        assert_eq!(block.get_text_content(), "FooBarBazMoreStuff\n\nQux");
        get_root().append(&[block])
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getTextContentSize() > basic
#[test]
fn get_text_content_size_basic() {
    let editor = element_node_editor();
    update(&editor, || {
        let first = get_root().get_first_child().unwrap();
        assert_eq!(
            first.get_text_content_size(),
            first.get_text_content().len()
        );
        Ok(())
    });
}

/// `splice`'s `beforeEach`: the block of `init()`.
fn splice_editor() -> (Editor, NodeRef) {
    let editor = element_node_editor();
    let block = update_with(&editor, || Ok(first_block()));
    (editor, block)
}

/// `Plain text: %s`.
fn splice_plain_text(
    start: usize,
    delete_count: usize,
    delete_only: bool,
    expected_text: &'static str,
) {
    let (editor, block) = splice_editor();
    update(&editor, move || {
        let nodes = if delete_only {
            vec![]
        } else {
            vec![create_text_node("Qux"), create_text_node("Quuz")]
        };
        block.splice(start, delete_count, &nodes)?;
        assert_eq!(block.get_text_content(), expected_text);
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Do nothing
#[test]
fn splice_plain_text_do_nothing() {
    splice_plain_text(0, 0, true, "FooBarBaz");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Insert in the beginning
#[test]
fn splice_plain_text_insert_in_the_beginning() {
    splice_plain_text(0, 0, false, "QuxQuuzFooBarBaz");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Insert in the middle
#[test]
fn splice_plain_text_insert_in_the_middle() {
    splice_plain_text(1, 0, false, "FooQuxQuuzBarBaz");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Insert in the end
#[test]
fn splice_plain_text_insert_in_the_end() {
    splice_plain_text(3, 0, false, "FooBarBazQuxQuuz");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Delete in the beginning
#[test]
fn splice_plain_text_delete_in_the_beginning() {
    splice_plain_text(0, 1, true, "BarBaz");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Delete in the middle
#[test]
fn splice_plain_text_delete_in_the_middle() {
    splice_plain_text(1, 1, true, "FooBaz");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Delete in the end
#[test]
fn splice_plain_text_delete_in_the_end() {
    splice_plain_text(2, 1, true, "FooBar");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Delete all
#[test]
fn splice_plain_text_delete_all() {
    splice_plain_text(0, 3, true, "");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Replace in the beginning
#[test]
fn splice_plain_text_replace_in_the_beginning() {
    splice_plain_text(0, 1, false, "QuxQuuzBarBaz");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Replace in the middle
#[test]
fn splice_plain_text_replace_in_the_middle() {
    splice_plain_text(1, 1, false, "FooQuxQuuzBaz");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Replace in the end
#[test]
fn splice_plain_text_replace_in_the_end() {
    splice_plain_text(2, 1, false, "FooBarQuxQuuz");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Replace all
#[test]
fn splice_plain_text_replace_all() {
    splice_plain_text(0, 3, false, "QuxQuuz");
}

/// The nodes of the nested-elements setup.
struct Nested {
    text1: NodeRef,
    nested_text1: NodeRef,
}

/// `Nested elements: %s`; `expected` gives the selection's key (from the
/// nodes and the block), offset and type for both points.
fn splice_nested_elements(
    start: usize,
    delete_count: usize,
    delete_only: bool,
    expected_text: &'static str,
    expected: fn(&Nested, &NodeRef) -> (String, usize, PointType),
) {
    let (editor, block) = splice_editor();
    let b = block.clone();
    let nodes = update_with(&editor, move || {
        let text1 = create_text_node("Foo");
        let text2 = create_text_node("Bar");
        let nested_block1 = create_test_element_node();
        let nested_text1 = create_text_node("Wiz");
        nested_block1.append(std::slice::from_ref(&nested_text1))?;
        let nested_block2 = create_test_element_node();
        let nested_text2 = create_text_node("Fuz");
        nested_block2.append(std::slice::from_ref(&nested_text2))?;
        b.clear()?;
        b.append(&[text1.clone(), nested_block1, nested_block2, text2])?;
        nested_text1.select(Some(1), Some(1))?;
        assert_eq!(b.get_text_content(), "FooWiz\n\nFuz\n\nBar");
        Ok(Nested {
            text1,
            nested_text1,
        })
    });
    let b = block.clone();
    update(&editor, move || {
        let nodes = if delete_only {
            vec![]
        } else {
            vec![create_text_node("Qux"), create_text_node("Quuz")]
        };
        b.splice(start, delete_count, &nodes)
    });
    update(&editor, move || {
        assert_eq!(block.get_text_content(), expected_text);
        let Some(selection) = get_selection().filter(|s| s.is_range()) else {
            return Ok(());
        };
        let expected_point = expected(&nodes, &block);
        for which in [Which::Anchor, Which::Focus] {
            let p = selection.point(which).unwrap();
            assert_eq!((p.key, p.offset, p.point_type), expected_point.clone());
        }
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Nested elements: Do nothing
#[test]
fn splice_nested_elements_1() {
    splice_nested_elements(1, 0, true, "FooWiz\n\nFuz\n\nBar", |n, _block| {
        (n.nested_text1.get_key().to_owned(), 1, PointType::Text)
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Nested elements: Delete selected element (selection moves to the previous)
#[test]
fn splice_nested_elements_2() {
    splice_nested_elements(1, 1, true, "FooFuz\n\nBar", |n, _block| {
        (n.text1.get_key().to_owned(), 3, PointType::Text)
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Nested elements: Replace selected element (selection moves to the previous)
#[test]
fn splice_nested_elements_3() {
    splice_nested_elements(1, 1, false, "FooQuxQuuzFuz\n\nBar", |n, _block| {
        (n.text1.get_key().to_owned(), 3, PointType::Text)
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Nested elements: Delete selected with all siblings (selection moves up to the element)
#[test]
fn splice_nested_elements_5() {
    splice_nested_elements(0, 4, true, "", |_n, block| {
        (block.get_key().to_owned(), 0, PointType::Element)
    });
}
// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Running transforms for inserted nodes, their previous siblings and new siblings
#[test]
fn splice_running_transforms_for_inserted_nodes_their_previous_siblings_and_new_siblings() {
    let (editor, block) = splice_editor();
    let transforms: Rc<RefCell<Vec<String>>> = Rc::default();
    let t = transforms.clone();
    let remove_transform = editor.register_node_transform("text", move |node| {
        t.borrow_mut().push(node.get_key().to_owned());
        Ok(())
    });
    let b = block.clone();
    let expected_transforms = update_with(&editor, move || {
        let another_block = create_test_element_node();
        let text1 = create_text_node("1");
        // Prevent text nodes from combining.
        let text2 = create_text_node("2");
        text2.set_mode("segmented")?;
        let text3 = create_text_node("3");
        another_block.append(&[text1.clone(), text2.clone(), text3.clone()])?;
        get_root().append(&[another_block])?;
        // Expect inserted node, its old siblings and new siblings to receive
        // transformer calls.
        Ok(keys(&[
            text1,
            text2,
            text3,
            b.get_child_at_index(0).unwrap(),
            b.get_child_at_index(1).unwrap(),
        ]))
    });
    let b = block.clone();
    update(&editor, move || {
        let moved = get_root()
            .get_last_child()
            .unwrap()
            .get_child_at_index(1)
            .unwrap();
        b.splice(1, 0, &[moved])
    });
    remove_transform();
    update(&editor, move || {
        assert_eq!(block.get_text_content(), "Foo2BarBaz");
        for key in &expected_transforms {
            assert!(transforms.borrow().contains(key), "{key}");
        }
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Re-inserting the node after the range does not create a cycle
#[test]
fn splice_re_inserting_the_node_after_the_range_does_not_create_a_cycle() {
    let (editor, block) = splice_editor();
    update(&editor, move || {
        let children = block.get_children();
        let (first, second, third) = (&children[0], &children[1], &children[2]);
        // A no-op splice: the node already sits at this position.
        block.splice(1, 0, std::slice::from_ref(second))?;
        // Checked before any sibling walk so a cycle fails fast.
        assert_eq!(
            second.get_next_sibling().unwrap().get_key(),
            third.get_key()
        );
        assert_eq!(block.get_children_size(), 3);
        assert_eq!(
            block.get_children_keys(),
            keys(&[first.clone(), second.clone(), third.clone()])
        );
        assert_eq!(block.get_last_child().unwrap().get_key(), third.get_key());
        assert_eq!(block.get_text_content(), "FooBarBaz");
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Replacing a node with a later sibling does not create a cycle
#[test]
fn splice_replacing_a_node_with_a_later_sibling_does_not_create_a_cycle() {
    let (editor, block) = splice_editor();
    update(&editor, move || {
        let children = block.get_children();
        let (first, third) = (&children[0], &children[2]);
        // Delete "Bar" and move "Baz" into its place.
        block.splice(1, 1, std::slice::from_ref(third))?;
        // Checked before any sibling walk so a cycle fails fast.
        assert_eq!(third.get_next_sibling(), None);
        assert_eq!(block.get_children_size(), 2);
        assert_eq!(
            block.get_children_keys(),
            keys(&[first.clone(), third.clone()])
        );
        assert_eq!(block.get_last_child().unwrap().get_key(), third.get_key());
        assert_eq!(block.get_text_content(), "FooBaz");
        Ok(())
    });
}
