//! Ports of `packages/lexical-link/src/__tests__/unit/LinkGeneratedJSON.test.ts`.

use crate::json::Value;
use crate::link::{AutoLinkAttributes, LinkAttributes, create_auto_link_node, create_link_node};
use crate::schema::walk_export_json;
use crate::upstream_tests::support::{initialize_unit_test, json_to_equal};
use crate::{CreateEditorArgs, Editor, NodeRef, UpdateOptions};

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

fn some(s: &str) -> Option<Option<String>> {
    Some(Some(s.to_owned()))
}

// upstream: lexical-link/src/__tests__/unit/LinkGeneratedJSON.test.ts::link generated exportJSON > LinkNode agrees with the schema-driven walk
#[test]
fn link_node_agrees_with_the_schema_driven_walk() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        expect_same_json(&create_link_node("", &LinkAttributes::default()));
        let link = create_link_node(
            "https://example.com/",
            &LinkAttributes {
                rel: some("noreferrer"),
                target: some("_blank"),
                title: some("Example"),
            },
        );
        link.set_indent(1.0)?;
        expect_same_json(&link);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LinkGeneratedJSON.test.ts::link generated exportJSON > AutoLinkNode agrees with the schema-driven walk
#[test]
fn auto_link_node_agrees_with_the_schema_driven_walk() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        expect_same_json(&create_auto_link_node("", &AutoLinkAttributes::default()));
        expect_same_json(&create_auto_link_node(
            "https://example.com/",
            &AutoLinkAttributes {
                is_unlinked: Some(true),
                target: some("_blank"),
                ..AutoLinkAttributes::default()
            },
        ));
        Ok(())
    });
}
