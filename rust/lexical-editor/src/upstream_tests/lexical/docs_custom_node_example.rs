//! Ports of `packages/lexical/src/__tests__/unit/DocsCustomNodeExample.test.ts`:
//! the `CalloutNode` of `docs/concepts/nodes.mdx`, its schema declared with
//! `with_field` as the example declares it.
#![cfg(feature = "extension")]

use crate::extension::build_editor_from_extensions;
use crate::json::{Object, Value, parse};
use crate::node::NodeBase;
use crate::schema::{
    NodeSchema, NumberValueOptions, enum_value, node_schema, number_value, string_value, with_field,
};
use crate::{
    ElementData, LexicalExtension, LexicalNode, NodeClass, UpdateOptions, create, define_extension,
    get_root,
};

#[derive(Clone, Debug)]
struct CalloutNode {
    base: NodeBase,
    element: ElementData,
    label: String,
    level: f64,
    tone: String,
}

impl Default for CalloutNode {
    fn default() -> Self {
        Self {
            base: NodeBase::default(),
            element: ElementData::default(),
            label: String::new(),
            level: 1.0,
            tone: "info".to_owned(),
        }
    }
}

thread_local! {
    static CALLOUT_NODE_SCHEMA: NodeSchema = node_schema(vec![
        (
            "label",
            with_field(string_value(""), |n: &CalloutNode| &n.label, |n: &mut CalloutNode| &mut n.label)
                .unwrap(),
        ),
        (
            "level",
            with_field(
                number_value(
                    1.0,
                    NumberValueOptions {
                        integer: true,
                        max: Some(3.0),
                        min: Some(1.0),
                        ..NumberValueOptions::default()
                    },
                )
                .unwrap(),
                |n: &CalloutNode| &n.level,
                |n: &mut CalloutNode| &mut n.level,
            )
            .unwrap(),
        ),
        (
            "tone",
            with_field(
                enum_value(vec!["info".into(), "warning".into(), "danger".into()], None).unwrap(),
                |n: &CalloutNode| &n.tone,
                |n: &mut CalloutNode| &mut n.tone,
            )
            .unwrap(),
        ),
    ])
    .unwrap();
}

impl LexicalNode for CalloutNode {
    fn get_type(&self) -> &'static str {
        "callout"
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
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            CALLOUT_NODE_SCHEMA.with(Clone::clone),
            crate::nodes::element::element_node_schema(),
        ]
    }
}

fn object(json: &str) -> Object {
    parse(json).unwrap().as_object().unwrap().clone()
}

// upstream: lexical/src/__tests__/unit/DocsCustomNodeExample.test.ts::the CalloutNode from docs/concepts/nodes.mdx > does what the section claims it does
#[test]
fn does_what_the_section_claims_it_does() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        nodes: vec![NodeClass::of::<CalloutNode>()],
        ..LexicalExtension::<(), (), ()>::new("[doc-callout]", ())
    })
    .into()])
    .unwrap();
    editor
        .update(
            || {
                let callout = create::<CalloutNode>();
                get_root().clear()?;
                get_root().append(std::slice::from_ref(&callout))?;
                // "level: 9 or level: "big" or a missing level all land on 1 here"
                callout
                    .update_from_json(&object(r#"{"label":"Note","level":9,"tone":"warning"}"#))?;
                let json = callout.export_json(false);
                assert_eq!(json.get("label"), Some(&Value::from("Note")));
                assert_eq!(json.get("level"), Some(&Value::from(1.0)));
                assert_eq!(json.get("tone"), Some(&Value::from("warning")));
                callout.update_from_json(&object(r#"{"level":"big"}"#))?;
                assert_eq!(callout.export_json(false).get("level"), Some(&1.0.into()));
                callout.update_from_json(&Object::new())?;
                assert_eq!(callout.export_json(false).get("level"), Some(&1.0.into()));
                // …and an in-domain value is kept.
                callout.update_from_json(&object(r#"{"level":3}"#))?;
                assert_eq!(callout.export_json(false).get("level"), Some(&3.0.into()));
                // An out-of-domain enum falls back to the first value.
                callout.update_from_json(&object(r#"{"tone":"banana"}"#))?;
                assert_eq!(
                    callout.export_json(false).get("tone"),
                    Some(&Value::from("info"))
                );
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    editor.dispose();
}
