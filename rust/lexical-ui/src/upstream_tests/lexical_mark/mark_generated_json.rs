//! Ports of `packages/lexical-mark/src/__tests__/unit/MarkGeneratedJSON.test.ts`.
//! `MarkNode`'s generated code is its `generated_export_json` (by hand, in
//! `mark/generated_json.rs`), which the walk of its schema checks. The
//! `DedupeMarkNode` subclass test is skipped (rust/parity/skips.json).

use crate::json::{Value, parse};
use crate::mark::{MarkNode, create_mark_node};
use crate::schema::walk_export_json;
use crate::upstream_tests::support::json_to_equal;
use crate::{CreateEditorArgs, Editor, NodeClass, NodeRef, UpdateOptions, create_editor};

/// `initializeUnitTest` with `{namespace: 'test', nodes: [MarkNode]}`.
fn test_editor() -> Editor {
    create_editor(CreateEditorArgs {
        namespace: "test".to_owned(),
        nodes: vec![NodeClass::of::<MarkNode>()],
        ..CreateEditorArgs::default()
    })
}

fn update(editor: &Editor, f: impl FnOnce() -> Result<(), crate::LexicalError> + 'static) {
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

/// `new MarkNode(ids)`.
fn mark(ids: &[&str]) -> NodeRef {
    let ids: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
    create_mark_node(&ids)
}

/// `$expectSameJSON(node)`: the generated exporter agrees with the walk in
/// both forms, key order too.
fn expect_same_json(node: &NodeRef) {
    let latest = node.get_latest();
    for compact in [false, true] {
        let from_generated = latest
            .generated_export_json(compact)
            .expect("generated code");
        let from_walk = walk_export_json(&*latest, compact);
        assert!(
            json_to_equal(
                &Value::Object(from_generated.clone()),
                &Value::Object(from_walk.clone())
            ),
            "{compact}"
        );
        assert_eq!(
            from_generated.keys().collect::<Vec<_>>(),
            from_walk.keys().collect::<Vec<_>>()
        );
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkGeneratedJSON.test.ts::mark generated exportJSON > MarkNode agrees with the schema-driven walk
#[test]
fn mark_node_agrees_with_the_schema_driven_walk() {
    let editor = test_editor();
    update(&editor, || {
        // `new MarkNode()` and `new MarkNode([])` both have no ids.
        expect_same_json(&mark(&[]));
        expect_same_json(&create_mark_node(&Vec::new()));
        expect_same_json(&mark(&["a", "b"]));
        Ok(())
    });
}

// upstream: lexical-mark/src/__tests__/unit/MarkGeneratedJSON.test.ts::mark generated exportJSON > the compact form compares an empty-array default by content
#[test]
fn the_compact_form_compares_an_empty_array_default_by_content() {
    let editor = test_editor();
    update(&editor, || {
        // `getGeneratedJSON(MarkNode)?.exportCompactJSON` is defined.
        assert!(crate::LexicalNode::generated_export_json(&MarkNode::default(), true).is_some());
        let compact = |node: &NodeRef| Value::Object(node.export_json(true));
        let expected = |json: &str| parse(json).unwrap();
        assert!(json_to_equal(
            &compact(&mark(&[])),
            &expected(r#"{"children":[],"type":"mark"}"#)
        ));
        assert!(json_to_equal(
            &compact(&create_mark_node(&Vec::new())),
            &expected(r#"{"children":[],"type":"mark"}"#)
        ));
        assert!(json_to_equal(
            &compact(&mark(&["x"])),
            &expected(r#"{"children":[],"ids":["x"],"type":"mark"}"#)
        ));
        // The legacy form writes it unconditionally either way.
        assert_eq!(
            mark(&[]).export_json(false).get("ids"),
            Some(&Value::Array(Vec::new()))
        );
        Ok(())
    });
}
