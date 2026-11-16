//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextGeneratedJSON.test.ts`.
//! A class's generated code is its `LexicalNode::generated_export_json` and
//! `generated_update_from_json` (ported by hand), which the walk of its
//! schema (`schema::walk_export_json`, `walk_update_from_json`) checks.

use crate::json::{Object, Value, parse};
use crate::lexical_utils::with_writable;
use crate::rich_text::{
    HeadingNode, HeadingTagType, QuoteNode, create_heading_node, create_quote_node,
};
use crate::schema::{walk_export_json, walk_update_from_json};
use crate::upstream_tests::support::{json_to_equal, update};
use crate::{
    CreateEditorArgs, Direction, Editor, LexicalError, LexicalNode, NodeClass, NodeRef, create,
    create_editor,
};

/// `initializeUnitTest` with `{namespace: 'test', nodes: [HeadingNode,
/// QuoteNode], theme: {}}`.
fn test_editor() -> Editor {
    create_editor(CreateEditorArgs {
        namespace: "test".to_owned(),
        nodes: vec![NodeClass::of::<HeadingNode>(), NodeClass::of::<QuoteNode>()],
        ..CreateEditorArgs::default()
    })
}

/// `$expectSameJSON(node)`: the generated exporter agrees with the walk in
/// both forms, key order too.
fn expect_same_json(node: &NodeRef) {
    let latest = node.get_latest();
    assert!(latest.generated_export_json(false).is_some());
    for compact in [false, true] {
        let Some(from_generated) = latest.generated_export_json(compact) else {
            continue;
        };
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

/// `$expectSameParse(klass, json)`: `$applyJSONSetters` (the flat NodeState,
/// then the generated parser) and `$walkJSONSetters` (the flat NodeState,
/// then the schema walk) on fresh nodes export the same.
fn expect_same_parse<T: LexicalNode + Default>(json: &str) -> Result<NodeRef, LexicalError> {
    let json: Object = parse(json).unwrap().as_object().unwrap().clone();
    assert!(T::default().generated_update_from_json(&json));
    let via_generated = create::<T>();
    via_generated.update_from_json(&json)?;
    let via_walk = create::<T>();
    crate::node_state::apply_flat_states(&via_walk, &json)?;
    with_writable(via_walk.get_key(), |n| {
        walk_update_from_json(n.as_node_mut(), &json)
    });
    assert!(json_to_equal(
        &Value::Object(via_generated.export_json(false)),
        &Value::Object(via_walk.export_json(false))
    ));
    Ok(via_generated)
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextGeneratedJSON.test.ts::rich-text generated JSON > HeadingNode agrees with the schema-driven walk
#[test]
fn heading_node_agrees_with_the_schema_driven_walk() {
    let editor = test_editor();
    update(&editor, || {
        expect_same_json(&create_heading_node(HeadingTagType::H1));
        let heading = create_heading_node(HeadingTagType::H3);
        heading.set_direction(Some(Direction::Rtl));
        heading.set_indent(2.0)?;
        heading.set_element_format("center");
        heading.set_text_format(1.0);
        heading.set_text_style("color: red");
        expect_same_json(&heading);
        expect_same_parse::<HeadingNode>(
            r#"{"direction":"rtl","format":"center","indent":2,"tag":"h3","textFormat":1,"textStyle":"color: red"}"#,
        )?;
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextGeneratedJSON.test.ts::rich-text generated JSON > QuoteNode agrees with the schema-driven walk
#[test]
fn quote_node_agrees_with_the_schema_driven_walk() {
    let editor = test_editor();
    update(&editor, || {
        expect_same_json(&create_quote_node(false));
        let quote = create_quote_node(false);
        quote.set_direction(Some(Direction::Rtl));
        quote.set_indent(1.0)?;
        quote.set_element_format("end");
        expect_same_json(&quote);
        // The shadowRoot flat state is appended by the dispatch; it has to
        // survive the composition.
        let shadow = create_quote_node(false);
        shadow.set_quote_is_shadow_root(true)?;
        assert_eq!(
            shadow.export_json(false).get("shadowRoot"),
            Some(&Value::Bool(true))
        );
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextGeneratedJSON.test.ts::rich-text generated JSON > QuoteNode parses through its generated parser, flat state included
#[test]
fn quote_node_parses_through_its_generated_parser_flat_state_included() {
    let editor = test_editor();
    update(&editor, || {
        let node = expect_same_parse::<QuoteNode>(
            r#"{"direction":"rtl","format":"center","indent":2,"shadowRoot":true,"textFormat":1,"textStyle":"color: red"}"#,
        )?;
        assert!(crate::lexical_utils::is_shadow_root_node(&node));
        assert_eq!(node.get_indent()?, 2.0);
        Ok(())
    });
}
