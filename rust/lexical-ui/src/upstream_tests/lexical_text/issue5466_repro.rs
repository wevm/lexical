//! Ports of `packages/lexical-text/src/__tests__/unit/Issue5466Repro.test.ts`.

use super::support::{entity_class, regex_match};
use crate::extension::build_editor_from_extensions;
use crate::text::register_lexical_text_entity;
use crate::{
    LexicalExtension, NodeClass, UpdateOptions, apply_node_replacement, create_paragraph_node,
    create_text_node, define_extension, get_root, json, merge_register,
};
use std::rc::Rc;

entity_class!(TestEntityNode, "test-entity-5466");

/// `$describeTextNodes()`: `E:"…"` for an entity, `T:"…"` for plain text.
fn describe_text_nodes() -> String {
    get_root()
        .get_all_text_nodes()
        .iter()
        .map(|node| {
            format!(
                "{}:{}",
                if node.get_type() == "test-entity-5466" {
                    "E"
                } else {
                    "T"
                },
                json::Value::String(node.get_text_content())
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// The second text node is bold so that it is not normalized into the first
// one. That leaves the match at the end of a text node that still has a
// TextNode sibling, which is the shape produced by $insertNodes.
// upstream: lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > replaces a match that ends the node and does not start at offset 0
// upstream: lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > replaces the last of several matches in the same node
// upstream: lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > replaces every match when the first one starts at offset 0
// upstream: lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > replaces a match that does not end the node (control)
// upstream: lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > leaves a match that only completes inside the sibling (control)
#[test]
fn register_lexical_text_entity_5466() {
    let cases: [(&str, &str, &str); 5] = [
        ("x ${A}", " tail", r#"T:"x " E:"${A}" T:" tail""#),
        (
            "x ${A} y ${B}",
            " tail",
            r#"T:"x " E:"${A}" T:" y " E:"${B}" T:" tail""#,
        ),
        (
            "${A} ${B} ${C}",
            " tail",
            r#"E:"${A}" T:" " E:"${B}" T:" " E:"${C}" T:" tail""#,
        ),
        ("x ${A} y", " tail", r#"T:"x " E:"${A}" T:" y" T:" tail""#),
        ("x ${A", "B} tail", r#"T:"x ${A" T:"B} tail""#),
    ];
    for (first, second, expected) in cases {
        let editor = build_editor_from_extensions([define_extension(LexicalExtension {
            nodes: vec![NodeClass::of::<TestEntityNode>()],
            register: Some(Rc::new(|editor_: &crate::Editor, _, _| {
                Ok(merge_register(register_lexical_text_entity(
                    editor_,
                    regex_match(r"\$\{[^}]*\}", ""),
                    "test-entity-5466",
                    |text_node| {
                        apply_node_replacement(&TestEntityNode::new_node(
                            &text_node.get_text_content(),
                        ))
                        .unwrap()
                    },
                )))
            })),
            ..LexicalExtension::<(), (), ()>::new("test", ())
        })
        .into()])
        .unwrap();
        editor
            .update(
                move || {
                    let paragraph = create_paragraph_node();
                    let bold = create_text_node(second);
                    bold.toggle_format("bold");
                    paragraph.append(&[create_text_node(first), bold])?;
                    get_root().clear()?;
                    get_root().append(&[paragraph])
                },
                UpdateOptions::default(),
            )
            .unwrap();
        assert_eq!(
            editor.read(describe_text_nodes).unwrap(),
            expected,
            "{first}|{second}"
        );
        editor.dispose();
    }
}
