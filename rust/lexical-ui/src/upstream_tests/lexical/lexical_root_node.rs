//! Ports of `packages/lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts`.
//!
//! Upstream's `beforeEach` makes `rootNode` with `$createRootNode()`, a
//! `RootNode` object outside the node map; here it is a [`RootNode`] value.
//! The `__cachedText` tests are skipped: Rust has no `__cachedText`
//! (rust/PARITY.md, the DOM half of the reconciler).

use crate::json::Value;
use crate::nodes::root::RootNode;
use crate::upstream_tests::support::{create_test_decorator_node, initialize_unit_test, update};
use crate::{
    CreateEditorArgs, EditorReadMode, LexicalNode, NodeClone, NodeRef, create_paragraph_node,
    create_text_node, get_root, get_selection,
};

// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.constructor
#[test]
fn root_node_constructor() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let root_node = RootNode::new();
        let other = RootNode::new();
        assert_eq!(format!("{root_node:?}"), format!("{other:?}"));
        assert_eq!(root_node.get_type(), "root");
        assert_eq!((&root_node as &dyn LexicalNode).get_key(), "root");
        // A new root has no children; the one in the state has none yet.
        assert_eq!(root_node.get_text_content(), "");
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.exportJSON() should return and object conforming to the expected schema
#[test]
fn root_node_export_json() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let json = RootNode::new().export_json(false);
        // Present-with-undefined, not absent.
        assert_eq!(json.get("textFormat"), Some(&Value::Undefined));
        assert_eq!(json.get("textStyle"), Some(&Value::Undefined));
        assert_eq!(
            Value::Object(json).to_string(),
            r#"{"children":[],"direction":null,"format":"","indent":0,"type":"root","version":1}"#
        );
        Ok(())
    });
}

/// `$cloneWithProperties` is the node's `clone_node`
/// ([`crate::clone_with_properties`]), which needs no editor state.
// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.clone()
#[test]
fn root_node_clone() {
    let root_node = RootNode::new();
    let root_node_clone = root_node.clone_node();
    assert!(!std::ptr::addr_eq(
        &*root_node_clone as *const dyn LexicalNode,
        &root_node as *const RootNode
    ));
    assert_eq!(format!("{root_node_clone:?}"), format!("{root_node:?}"));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.createDOM()
#[cfg(feature = "html")]
#[test]
fn root_node_create_dom() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let root_node = RootNode::new();
    assert!(
        root_node
            .create_dom(
                &NodeRef::new("root"),
                &crate::EditorConfig::default(),
                &editor
            )
            .is_err()
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.updateDOM()
#[test]
fn root_node_update_dom() {
    let root_node = RootNode::new();
    assert!(!root_node.update_dom(&root_node));
}

/// A `NodeRef` is read inside a read (`NodeRef`'s docs); JS's
/// `RootNode.isAttached` returns `true` without reading the state.
// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.isAttached()
#[test]
fn root_node_is_attached() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    assert!(editor.read(|| get_root().is_attached()).unwrap());
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.isRootNode()
#[test]
fn root_node_is_root_node() {
    let root_node = RootNode::new();
    assert!((&root_node as &dyn LexicalNode).is::<RootNode>());
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > Cached getTextContent with decorators
#[test]
fn cached_get_text_content_with_decorators() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(&[create_test_decorator_node()])
    });
    assert_eq!(
        editor
            .read_with_mode(EditorReadMode::Latest, || get_root().get_text_content())
            .unwrap(),
        "Hello world"
    );
}

/// The selection's anchor and focus after the second update, if it is a
/// range selection.
fn anchor_and_focus_nodes() -> Option<(NodeRef, NodeRef)> {
    let selection = get_selection()?.range()?;
    Some((
        selection.anchor.get_node().unwrap(),
        selection.focus.get_node().unwrap(),
    ))
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.clear() to handle selection update
#[test]
fn root_node_clear_to_handle_selection_update() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let text = create_text_node("Hello");
        paragraph.append(std::slice::from_ref(&text))?;
        text.select(None, None)?;
        Ok(())
    });
    update(&editor, || get_root().clear());
    update(&editor, || {
        if let Some((anchor, focus)) = anchor_and_focus_nodes() {
            assert_eq!(anchor, get_root());
            assert_eq!(focus, get_root());
        }
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode is selected when its selected child is removed
#[test]
fn root_node_is_selected_when_its_selected_child_is_removed() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let text = create_text_node("Hello");
        paragraph.append(std::slice::from_ref(&text))?;
        text.select(None, None)?;
        Ok(())
    });
    update(&editor, || {
        get_root().get_first_child().unwrap().remove(false)
    });
    update(&editor, || {
        if let Some((anchor, focus)) = anchor_and_focus_nodes() {
            assert_eq!(anchor, get_root());
            assert_eq!(focus, get_root());
        }
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode is not selected when all children are removed with no selection
#[test]
fn root_node_is_not_selected_when_all_children_are_removed_with_no_selection() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        assert!(get_selection().is_none());
        get_root().append(&[create_paragraph_node()])?;
        assert!(get_selection().is_none());
        Ok(())
    });
    update(&editor, || {
        assert!(get_selection().is_none());
        get_root().clear()?;
        assert!(get_selection().is_none());
        Ok(())
    });
    update(&editor, || {
        assert!(get_selection().is_none());
        Ok(())
    });
}
