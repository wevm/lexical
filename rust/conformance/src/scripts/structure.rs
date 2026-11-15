//! scripts/node-structure: `ops.ts`'s test node classes with structure
//! declarations (`TestBoxNode`, `TestSubBoxNode`, `TestTagNode`,
//! `TestSubTagNode`) and the actions that read the node structure
//! (`LexicalStructure.ts`).

use super::err;
use lexical::json::{Object, Value};
use lexical::{
    ElementData, LexicalError, LexicalNode, MergeAdjacentDeclaration, MergeAdjacentEntry, NodeBase,
    NodeChildEntry, NodeChildrenConfig, NodeClass, NodeConfigLink, NodeRef,
    RequiredNodeStateConfig, StateConfig, StaticNodeConfig, create_state,
};

thread_local! {
    /// `testTagColorState`: a NodeState that `TestTagNode`'s
    /// `mergeAdjacent` names.
    static COLOR: StateConfig<String> = create_state("color", |v: &Value| {
        v.as_str().unwrap_or("").to_owned()
    });
    /// `testTagCountState`: a derived NodeState of `TestTagNode`.
    static COUNT: StateConfig<f64> = create_state("count", |v: &Value| v.as_f64().unwrap_or(0.0));
}

/// The chain of `TestBoxNode`: `children: 'block'`.
fn box_link() -> NodeConfigLink {
    NodeConfigLink::new(
        "test-box",
        StaticNodeConfig {
            children: Some(NodeChildrenConfig::Block),
            ..StaticNodeConfig::default()
        },
    )
}

/// `TestBoxNode`: a block that holds blocks (`children: 'block'`).
#[derive(Clone, Debug, Default)]
pub(crate) struct TestBoxNode {
    base: NodeBase,
    element: ElementData,
}

/// `TestSubBoxNode`: a `TestBoxNode` that declares nothing of its own.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestSubBoxNode {
    base: NodeBase,
    element: ElementData,
}

macro_rules! element_basics {
    ($type:literal) => {
        fn get_type(&self) -> &'static str {
            $type
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
    };
}

impl LexicalNode for TestBoxNode {
    element_basics!("test-box");
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![box_link()]
    }
    fn export_json(&self, compact: bool) -> Object {
        lexical::nodes::element::export_json(self, compact)
    }
    fn update_from_json(&mut self, json: &Object) {
        lexical::nodes::element::update_from_json(&mut self.element, json);
    }
}

impl LexicalNode for TestSubBoxNode {
    element_basics!("test-sub-box");
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![
            NodeConfigLink::new("test-sub-box", StaticNodeConfig::default()),
            box_link(),
        ]
    }
    fn export_json(&self, compact: bool) -> Object {
        lexical::nodes::element::export_json(self, compact)
    }
    fn update_from_json(&mut self, json: &Object) {
        lexical::nodes::element::update_from_json(&mut self.element, json);
    }
}

/// The properties of a `TestTagNode`: its schema's `label` (derived) and
/// `tag`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Tag {
    tag: String,
    label: String,
}

/// The link of `TestTagNode`: inline, `children: 'inline'`, merged by
/// `['tag', testTagColorState]`, `label` and `count` derived.
fn tag_link() -> NodeConfigLink {
    NodeConfigLink::new(
        "test-tag",
        StaticNodeConfig {
            inline: Some(true),
            children: Some(NodeChildrenConfig::Inline),
            merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![
                MergeAdjacentEntry::Field("tag"),
                MergeAdjacentEntry::State(COLOR.with(StateConfig::erased)),
            ])),
            derived: vec!["label", "count"],
        },
    )
}

fn tag_state_configs() -> Vec<RequiredNodeStateConfig> {
    vec![
        COLOR.with(|c| RequiredNodeStateConfig::new(c, false)),
        COUNT.with(|c| RequiredNodeStateConfig::new(c, false)),
    ]
}

/// The JSON of a `TestTagNode`: the element's, with `label` and `tag` after
/// `children` (the schema's properties, as `$config` serializes them).
fn tag_export_json(node: &dyn LexicalNode, tag: &Tag, compact: bool) -> Object {
    let element = lexical::nodes::element::export_json(node, compact);
    let mut json = Object::new();
    for (key, value) in element.iter() {
        json.insert(key, value.clone());
        if key == "children" {
            if !compact || !tag.label.is_empty() {
                json.insert("label", tag.label.as_str().into());
            }
            if !compact || !tag.tag.is_empty() {
                json.insert("tag", tag.tag.as_str().into());
            }
        }
    }
    json
}

fn tag_update_from_json(element: &mut ElementData, tag: &mut Tag, json: &Object) {
    lexical::nodes::element::update_from_json(element, json);
    let string = |key: &str| {
        json.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    tag.label = string("label");
    tag.tag = string("tag");
}

/// `TestTagNode`: an inline element holding inline content, which cannot
/// be empty; equal adjacent ones (same `tag`, same `color` state) merge.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestTagNode {
    base: NodeBase,
    element: ElementData,
    tag: Tag,
}

/// `TestSubTagNode`: a `TestTagNode` that inherits its declarations.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestSubTagNode {
    base: NodeBase,
    element: ElementData,
    tag: Tag,
}

impl LexicalNode for TestTagNode {
    element_basics!("test-tag");
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![tag_link()]
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        tag_state_configs()
    }
    fn can_be_empty(&self) -> bool {
        false
    }
    fn export_json(&self, compact: bool) -> Object {
        tag_export_json(self, &self.tag, compact)
    }
    fn update_from_json(&mut self, json: &Object) {
        tag_update_from_json(&mut self.element, &mut self.tag, json);
    }
}

impl LexicalNode for TestSubTagNode {
    element_basics!("test-sub-tag");
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![
            NodeConfigLink::new("test-sub-tag", StaticNodeConfig::default()),
            tag_link(),
        ]
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        tag_state_configs()
    }
    fn can_be_empty(&self) -> bool {
        false
    }
    fn export_json(&self, compact: bool) -> Object {
        tag_export_json(self, &self.tag, compact)
    }
    fn update_from_json(&mut self, json: &Object) {
        tag_update_from_json(&mut self.element, &mut self.tag, json);
    }
}

/// The test class an `ExtensionSpec`'s `nodes` names, if it is one.
pub(super) fn test_node_class(node_type: &str) -> Option<NodeClass> {
    Some(match node_type {
        "test-box" => NodeClass::of::<TestBoxNode>(),
        "test-sub-box" => NodeClass::of::<TestSubBoxNode>(),
        "test-tag" => NodeClass::of::<TestTagNode>(),
        "test-sub-tag" => NodeClass::of::<TestSubTagNode>(),
        _ => return None,
    })
}

/// `$create($registeredClass(type))` of a test class.
pub(super) fn create_test_node(node_type: &str) -> Result<NodeRef, LexicalError> {
    Ok(match node_type {
        "test-box" => lexical::create::<TestBoxNode>(),
        "test-sub-box" => lexical::create::<TestSubBoxNode>(),
        "test-tag" => lexical::create::<TestTagNode>(),
        "test-sub-tag" => lexical::create::<TestSubTagNode>(),
        other => return Err(err(format!("no $create for {other}"))),
    })
}

/// `STRUCTURE_JSON($registeredClass(type))`.
pub(super) fn structure_json(
    editor: &lexical::Editor,
    node_type: &str,
) -> Result<Value, LexicalError> {
    let class = editor
        .get_registered_node(node_type)
        .ok_or_else(|| err(format!("unknown node type {node_type}")))?;
    let declared = class.get_declared_node_structure();
    let mut out = Object::new();
    if let Some(inline) = declared.inline {
        out.insert("inline", inline.into());
    }
    if let Some(children) = &declared.children {
        out.insert(
            "children",
            match children {
                NodeChildrenConfig::Inline => "inline".into(),
                NodeChildrenConfig::Block => "block".into(),
                NodeChildrenConfig::List(entries) => Value::Array(
                    entries
                        .iter()
                        .map(|e| match e {
                            NodeChildEntry::Inline => "inline".into(),
                            NodeChildEntry::Class(class) => class.node_type().into(),
                        })
                        .collect(),
                ),
            },
        );
    }
    if let Some(merge) = &declared.merge_adjacent {
        out.insert(
            "mergeAdjacent",
            Value::Array(
                merge
                    .by
                    .iter()
                    .map(|e| match e {
                        MergeAdjacentEntry::Field(name) => (*name).into(),
                        MergeAdjacentEntry::State(state) => {
                            let mut o = Object::new();
                            o.insert("state", state.key().into());
                            o.into()
                        }
                    })
                    .collect(),
            ),
        );
    }
    let mut derived = declared.derived.clone();
    derived.sort_unstable();
    out.insert(
        "derived",
        Value::Array(derived.into_iter().map(Value::from).collect()),
    );
    let mut by = Object::new();
    let declared_by = &declared.declared_by;
    for (key, value) in [
        ("inline", declared_by.inline),
        ("children", declared_by.children),
        ("mergeAdjacent", declared_by.merge_adjacent),
    ] {
        if let Some(t) = value {
            by.insert(key, t.into());
        }
    }
    out.insert("declaredBy", by.into());
    Ok(out.into())
}
