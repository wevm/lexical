//! serialization/schema-node: `ops.ts`'s `TestSchemaNode`, a block element
//! whose `$config` declares a `json` schema of every kind of property, and
//! its `flags` set NodeState.

use lexical::json::Value;
use lexical::schema::{
    NodeSchema, NumberValueOptions, aliased_value, array_set_value, array_value, enum_value,
    node_schema, nullable, number_value, optional, schema_getter, schema_setter, string_value,
    with_accessors, with_field,
};
use lexical::{
    ElementData, LexicalNode, NodeBase, NodeClass, RequiredNodeStateConfig, StateConfig,
    StateValueConfig, create_state_with,
};

thread_local! {
    /// `testSchemaFlagsState`: a set NodeState, kept in canonical order.
    static FLAGS: StateConfig<Vec<String>> = create_state_with(
        "flags",
        StateValueConfig::from_schema(array_set_value(string_value("")).expect("a set of strings")),
    );
    /// `testSchemaSchema`.
    static SCHEMA: NodeSchema = test_schema_schema();
}

fn test_schema_schema() -> NodeSchema {
    let count = number_value(
        1.0,
        NumberValueOptions {
            min: Some(0.0),
            max: Some(10.0),
            integer: true,
            clamp: true,
        },
    )
    .expect("count");
    let kind = enum_value(vec!["a".into(), "b".into()], None)
        .and_then(|inner| {
            aliased_value(
                inner,
                vec![("alpha".into(), "a".into()), ("beta".into(), "b".into())],
            )
        })
        .expect("kind");
    let width = nullable(
        number_value(0.0, NumberValueOptions::default()).expect("numberValue()"),
        false,
    )
    .expect("width");
    node_schema(vec![
        (
            "count",
            with_field(
                count,
                |n: &TestSchemaNode| &n.count,
                |n: &mut TestSchemaNode| &mut n.count,
            )
            .expect("count"),
        ),
        (
            "kind",
            with_field(
                kind,
                |n: &TestSchemaNode| &n.kind,
                |n: &mut TestSchemaNode| &mut n.kind,
            )
            .expect("kind"),
        ),
        (
            // `withAccessors(stringValue(), {setter: null})`: `getLabel`.
            "label",
            with_accessors(
                string_value(""),
                Some(schema_getter(|n: &TestSchemaNode| {
                    n.kind.to_uppercase().into()
                })),
                None,
            )
            .expect("label"),
        ),
        (
            "note",
            with_field(
                optional(string_value(""), false).expect("note"),
                |n: &TestSchemaNode| &n.note,
                |n: &mut TestSchemaNode| &mut n.note,
            )
            .expect("note"),
        ),
        (
            // `{when: 'isShown'}`: only for a count above 1.
            "shown",
            with_field(
                string_value(""),
                |n: &TestSchemaNode| &n.shown,
                |n: &mut TestSchemaNode| &mut n.shown,
            )
            .expect("shown")
            .when(|n| {
                n.downcast_ref::<TestSchemaNode>()
                    .expect("a TestSchemaNode")
                    .count
                    > 1.0
            }),
        ),
        (
            "tags",
            with_field(
                array_value(string_value("")).expect("tags"),
                |n: &TestSchemaNode| &n.tags,
                |n: &mut TestSchemaNode| &mut n.tags,
            )
            .expect("tags"),
        ),
        (
            // `withAccessors(nullable(numberValue()), {})`: `getWidth` and
            // `setWidth`.
            "width",
            with_accessors(
                width,
                Some(schema_getter(|n: &TestSchemaNode| {
                    n.width.map_or(Value::Null, Value::Number)
                })),
                Some(schema_setter(|n: &mut TestSchemaNode, v: Value| {
                    n.width = v.as_f64();
                })),
            )
            .expect("width"),
        ),
    ])
    .expect("testSchemaSchema")
}

/// `TestSchemaNode`'s fields, initialized as its class initializes them.
#[derive(Clone, Debug)]
pub(crate) struct TestSchemaNode {
    base: NodeBase,
    element: ElementData,
    count: f64,
    kind: String,
    note: Option<String>,
    shown: String,
    width: Option<f64>,
    tags: Vec<String>,
}

impl Default for TestSchemaNode {
    fn default() -> Self {
        Self {
            base: NodeBase::default(),
            element: ElementData::default(),
            count: 1.0,
            kind: "a".into(),
            note: None,
            shown: String::new(),
            width: None,
            tags: Vec::new(),
        }
    }
}

impl LexicalNode for TestSchemaNode {
    fn get_type(&self) -> &'static str {
        "test-schema"
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
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![FLAGS.with(|c| RequiredNodeStateConfig::new(c, false))]
    }
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            SCHEMA.with(Clone::clone),
            lexical::nodes::element::element_node_schema(),
        ]
    }
}

/// `TestSchemaNode`'s class.
pub(crate) fn test_schema_nodes() -> Vec<NodeClass> {
    vec![NodeClass::of::<TestSchemaNode>()]
}
