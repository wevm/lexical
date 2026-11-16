//! Ports of `packages/lexical/src/__tests__/unit/LexicalNodeState.test.ts`.
//! A JS node object is a version of a node; here a version is the `Rc` an
//! editor state's node map holds, whose `__state` is `base().state`.

use crate::json::{Object, Value, parse};
use crate::lexical_utils::set_node_key;
use crate::node::NodeBase;
use crate::node_state::node_states_are_equivalent;
use crate::nodes::paragraph::ParagraphNode;
use crate::schema::{
    SerializationSchema, SerializationSchemaMeta, aliased_value, array_set_value, array_value,
    enum_value, nullable, number_value, object_value, optional, string_value, transform_value,
    union_value, with_accessors, with_field,
};
use crate::upstream_tests::support::{json_to_equal, panic_message};
use crate::{
    CreateEditorArgs, Editor, ElementData, LexicalError, LexicalNode, LexicalNodeReplacement,
    NodeClass, NodeRef, RequiredNodeStateConfig, RootElement, RootElementRef, StateConfig,
    StateValueConfig, UpdateOptions, copy_node, copy_node_with, create, create_editor,
    create_paragraph_node, create_state, create_state_with, create_text_node, get_root, get_state,
    get_state_change, get_state_direct, set_state, set_state_with,
};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

fn json(text: &str) -> Value {
    parse(text).unwrap()
}

fn object(text: &str) -> Object {
    json(text).as_object().unwrap().clone()
}

/// JS's `Boolean(v)`.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Undefined | Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => *n != 0.0 && !n.is_nan(),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn number_or(default: f64) -> impl Fn(&Value) -> f64 {
    move |v| v.as_f64().unwrap_or(default)
}

fn string_or_empty(v: &Value) -> String {
    v.as_str().unwrap_or("").to_owned()
}

thread_local! {
    static NUMBER_STATE: StateConfig<f64> = create_state("numberState", number_or(0.0));
    static BOOL_STATE: StateConfig<bool> = create_state("boolState", truthy);
}

fn number_state() -> StateConfig<f64> {
    NUMBER_STATE.with(Clone::clone)
}

fn bool_state() -> StateConfig<bool> {
    BOOL_STATE.with(Clone::clone)
}

/// `TestNode` (`LexicalNode.test.ts`): a plain `LexicalNode`.
#[derive(Clone, Debug, Default)]
struct TestNode {
    base: NodeBase,
}

impl LexicalNode for TestNode {
    fn get_type(&self) -> &'static str {
        "test"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
}

/// `StateNode extends TestNode`: `numberState` flat, `boolState` nested.
#[derive(Clone, Debug, Default)]
struct StateNode {
    base: NodeBase,
}

impl LexicalNode for StateNode {
    fn get_type(&self) -> &'static str {
        "state"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["test"]
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![
            RequiredNodeStateConfig::new(&number_state(), true),
            RequiredNodeStateConfig::new(&bool_state(), false),
        ]
    }
}

fn create_state_node() -> NodeRef {
    create::<StateNode>()
}

struct TestRoot;
impl RootElement for TestRoot {}

/// `initializeUnitTest`'s editor, with `TestNode` and `StateNode`.
fn state_editor() -> Editor {
    crate::reset_random_key();
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<TestNode>(), NodeClass::of::<StateNode>()],
        ..CreateEditorArgs::default()
    });
    editor
        .set_root_element(Some(RootElementRef(Rc::new(TestRoot))))
        .unwrap();
    editor
}

/// The class's `importJSON`.
fn import_json(node_type: &str, json: &Object) -> NodeRef {
    let import = crate::get_editor()
        .get_registered_node(node_type)
        .unwrap()
        .import_json;
    import(json).unwrap()
}

/// The version of `key` in `state`.
fn version_in(state: &crate::EditorState, key: &str) -> Rc<dyn LexicalNode> {
    state.0.node_map.borrow()[key].clone()
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > state.$set() and state.$get() need to be inside an update
#[test]
fn state_set_and_get_need_to_be_inside_an_update() {
    let editor = state_editor();
    let root = get_root_ref();
    let string_state = create_state("stringState", string_or_empty);
    let f = {
        let (root, s) = (root.clone(), string_state.clone());
        move || {
            set_state(&root, &s, "hello".to_owned()).unwrap();
        }
    };
    assert!(catch_unwind(AssertUnwindSafe(f.clone())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(f.clone())).is_err());
    editor
        .update(
            move || {
                f();
                f();
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    editor.read(|| ()).unwrap();
}

/// `root`, as `beforeEach` takes it.
fn get_root_ref() -> NodeRef {
    NodeRef::new("root")
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > getState and setState
#[test]
fn get_state_and_set_state() {
    let editor = state_editor();
    update(&editor, || {
        let root = get_root();
        let string_state = create_state("stringState", string_or_empty);
        assert_eq!(get_state(&root, &string_state)?, "");
        set_state(&root, &string_state, "hello".to_owned())?;
        assert_eq!(get_state(&root, &string_state)?, "hello");
        let maybe_string_state =
            create_state("maybeStringState", |v| v.as_str().map(str::to_owned));
        assert_eq!(get_state(&root, &maybe_string_state)?, None);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > import and export state
#[test]
fn import_and_export_state() {
    let editor = state_editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let json1 = paragraph.export_json(false);
        // State is not exported as an empty object.
        assert!(json1.get("$").is_none());
        set_state(&paragraph, &number_state(), 1.0)?;
        let json2 = paragraph.export_json(false);
        assert_eq!(json2.get("$"), Some(&json(r#"{"numberState":1}"#)));
        let paragraph2 = import_json("paragraph", &json2);
        let json3 = paragraph2.export_json(false);
        assert_eq!(json3.get("$"), Some(&json(r#"{"numberState":1}"#)));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > importJSON ignores prototype-polluting state keys
#[test]
fn import_json_ignores_prototype_polluting_state_keys() {
    let editor = state_editor();
    update(&editor, || {
        let malicious =
            object(r#"{"type":"paragraph","version":1,"$":{"__proto__":{"injectedKey":"evil"}}}"#);
        let paragraph = import_json("paragraph", &malicious);
        // Reading the key the smuggled record names returns the default.
        let injected_state = create_state("injectedKey", |v| {
            v.as_str().unwrap_or("default").to_owned()
        });
        assert_eq!(get_state(&paragraph, &injected_state)?, "default");
        // The dangerous key is dropped, so no node state is emitted.
        assert!(paragraph.export_json(false).get("$").is_none());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > states cannot be registered with the same key string
#[test]
fn states_cannot_be_registered_with_the_same_key_string() {
    let editor = state_editor();
    update(&editor, || {
        let root = get_root();
        let k0 = create_state("foo", truthy);
        let k1 = create_state("foo", |_| "foo".to_owned());
        assert!(!get_state(&root, &k0)?);
        set_state(&root, &k0, true)?;
        assert!(get_state(&root, &k0)?);
        let message = "State key collision \"foo\" detected in RootNode node with type root and key root. Only one StateConfig with a given key should be used on a node.";
        let error = get_state(&root, &k1).unwrap_err();
        assert!(error.message().contains(message), "{error}");
        let error = set_state(&root, &k1, "foo".to_owned()).unwrap_err();
        assert!(error.message().contains(message), "{error}");
        assert!(get_state(&root, &k0)?);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeGetter() and nodeSetter()
#[test]
fn node_getter_and_node_setter() {
    let editor = state_editor();
    update(&editor, || {
        let state_node = create_state_node();
        assert_eq!(get_state(&state_node, &number_state())?, 0.0);
        set_state(&state_node, &number_state(), 1.0)?;
        assert_eq!(get_state(&state_node, &number_state())?, 1.0);
        set_state_with(&state_node, &number_state(), |n| n + 1.0)?;
        assert_eq!(get_state(&state_node, &number_state())?, 2.0);
        set_state(&state_node, &bool_state(), true)?;
        let exported = state_node.export_json(false);
        assert_eq!(exported.get("$"), Some(&json(r#"{"boolState":true}"#)));
        assert_eq!(exported.get("numberState"), Some(&2.0.into()));
        assert_eq!(exported.get("type"), Some(&"state".into()));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > flat config serialization round-trip
#[test]
fn flat_config_serialization_round_trip() {
    let editor = state_editor();
    update(&editor, || {
        let flat = r#"{"$":{"boolState":true},"numberState":2,"type":"state","version":1}"#;
        let nested = r#"{"$":{"boolState":true,"numberState":2},"type":"state","version":1}"#;
        let both = r#"{"$":{"boolState":true,"numberState":3},"numberState":2,"type":"state","version":1}"#;
        for doc in [flat, nested, both] {
            let node = import_json("state", &object(doc));
            assert!(
                json_to_equal(&Value::Object(node.export_json(false)), &json(flat)),
                "{doc}"
            );
            assert!(get_state(&node, &bool_state())?);
            assert_eq!(get_state(&node, &number_state())?, 2.0);
        }
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > an absent flat key leaves the nested value alone
#[test]
fn an_absent_flat_key_leaves_the_nested_value_alone() {
    let editor = state_editor();
    update(&editor, || {
        let node = import_json(
            "state",
            &object(r#"{"$":{"numberState":7},"type":"state","version":1}"#),
        );
        assert_eq!(get_state(&node, &number_state())?, 7.0);
        // A flat key that is present wins over the blob's value.
        let both = import_json(
            "state",
            &object(r#"{"$":{"numberState":7},"numberState":9,"type":"state","version":1}"#),
        );
        assert_eq!(get_state(&both, &number_state())?, 9.0);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > default value should not be exported
#[test]
fn default_value_should_not_be_exported() {
    let editor = state_editor();
    update(&editor, || {
        let indent_state = create_state("indent", number_or(0.0));
        let paragraph = create_paragraph_node();
        assert_eq!(get_state(&paragraph, &indent_state)?, 0.0);
        assert!(paragraph.export_json(false).get("$").is_none());
        set_state(&paragraph, &indent_state, 1.0)?;
        assert_eq!(
            paragraph.export_json(false).get("$"),
            Some(&json(r#"{"indent":1}"#))
        );
        // Set the default value explicitly.
        set_state(&paragraph, &indent_state, 0.0)?;
        assert!(paragraph.export_json(false).get("$").is_none());
        let foo = create_state("foo", string_or_empty);
        set_state(&paragraph, &foo, "fooValue".to_owned())?;
        assert_eq!(
            paragraph.export_json(false).get("$"),
            Some(&json(r#"{"foo":"fooValue"}"#))
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > getState returns immutable values, setState require an Object literal
#[test]
fn get_state_returns_immutable_values_set_state_require_an_object_literal() {
    // The type assertions are the compiler's; what runs is the same calls.
    let editor = state_editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let object_state = create_state_with(
            "object",
            StateValueConfig {
                parse: Rc::new(|v: &Value| {
                    if truthy(v) {
                        v.clone()
                    } else {
                        json(r#"{"bar":0,"foo":""}"#)
                    }
                }),
                unparse: Rc::new(Value::clone),
                is_equal: Rc::new(|a, b| a == b),
                reset_on_copy_node: false,
                derived: false,
                schema: None,
            },
        );
        let paragraph_object = get_state(&paragraph, &object_state)?;
        assert_eq!(paragraph_object, json(r#"{"bar":0,"foo":""}"#));
        set_state(&paragraph, &object_state, json(r#"{"bar":1}"#))?;
        set_state(
            &paragraph,
            &object_state,
            json(r#"{"bar":1,"baz":"baz","foo":"foo"}"#),
        )?;
        set_state(&paragraph, &object_state, paragraph_object.clone())?;
        let mut spread = paragraph_object.as_object().unwrap().clone();
        spread.insert("foo", "foo".into());
        set_state(&paragraph, &object_state, Value::Object(spread))?;
        Ok(())
    });
}

fn vk_state() -> StateConfig<Option<f64>> {
    create_state("vk", |v| v.as_f64())
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > setting state shouldn’t affect previous reconciled versions of the node
#[test]
fn setting_state_should_not_affect_previous_reconciled_versions_of_the_node() {
    let editor = state_editor();
    let vk = vk_state();
    let versions: Rc<RefCell<Vec<Rc<dyn LexicalNode>>>> = Rc::default();
    {
        let (vk, versions) = (vk.clone(), versions.clone());
        update(&editor, move || {
            let initial_root = get_root().get_latest();
            let v0 = set_state(&get_root(), &vk, Some(0.0))?;
            assert_eq!(get_state(&v0, &vk)?, Some(0.0));
            let v0 = v0.get_latest();
            assert_eq!(
                get_state_change(&*v0, &*initial_root, &vk)?,
                Some((Some(0.0), None))
            );
            versions.borrow_mut().extend([initial_root, v0]);
            Ok(())
        });
    }
    let state0 = editor.get_editor_state();
    {
        let (vk, versions) = (vk.clone(), versions.clone());
        update(&editor, move || {
            let v0 = versions.borrow()[1].clone();
            let v1 = set_state(&get_root(), &vk, Some(1.0))?.get_latest();
            assert!(!Rc::ptr_eq(&v1, &v0));
            assert_eq!(v1.base().key, v0.base().key);
            // getLatest().
            assert_eq!(get_state(&get_root(), &vk)?, Some(1.0));
            assert_eq!(get_state_direct(&*v0, &vk)?, Some(0.0));
            assert_eq!(get_state_direct(&*v1, &vk)?, Some(1.0));
            assert_eq!(
                get_state_change(&*v1, &*v0, &vk)?,
                Some((Some(1.0), Some(0.0)))
            );
            versions.borrow_mut().push(v1);
            Ok(())
        });
    }
    let state1 = editor.get_editor_state();
    // Each state reads its own version of the node (getLatest).
    let root = get_root_ref();
    assert_eq!(state0.read(|| get_state(&root, &vk).unwrap()), Some(0.0));
    assert_eq!(state1.read(|| get_state(&root, &vk).unwrap()), Some(1.0));
    // `NODE_STATE_DIRECT` needs no editor state.
    let versions = versions.borrow();
    assert_eq!(get_state_direct(&*versions[0], &vk).unwrap(), None);
    assert_eq!(get_state_direct(&*versions[1], &vk).unwrap(), Some(0.0));
    assert_eq!(get_state_direct(&*versions[2], &vk).unwrap(), Some(1.0));
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeStatesAreEquivalent > undefined states are equivalent
#[test]
fn undefined_states_are_equivalent() {
    assert!(node_states_are_equivalent(None, None));
}

fn paragraph_texts() -> Vec<NodeRef> {
    get_root().get_all_text_nodes()
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeStatesAreEquivalent > merges text nodes with different number of default state values
#[test]
fn merges_text_nodes_with_different_number_of_default_state_values() {
    let editor = state_editor();
    update(&editor, || {
        let first = create_text_node("hello");
        let second = create_text_node("world");
        set_state(&first, &number_state(), 1.0)?;
        set_state(&first, &bool_state(), true)?;
        set_state(&second, &bool_state(), true)?;
        get_root().clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[first, second])?;
        get_root().append(&[paragraph])
    });
    editor
        .read(|| assert_eq!(paragraph_texts().len(), 2))
        .unwrap();
    // Revert the number state to its default.
    update(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        assert_eq!(paragraph.get_type(), "paragraph");
        let first = paragraph.get_children()[0].clone();
        set_state(&first, &number_state(), 0.0)?;
        Ok(())
    });
    editor
        .read(|| {
            let texts = paragraph_texts();
            assert_eq!(texts.len(), 1);
            assert_eq!(get_state(&texts[0], &number_state()).unwrap(), 0.0);
            assert!(get_state(&texts[0], &bool_state()).unwrap());
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeStatesAreEquivalent > TextNode merging only with equivalent state
#[test]
fn text_node_merging_only_with_equivalent_state() {
    let editor = state_editor();
    let class_name_state = create_state("className", string_or_empty);
    let state = class_name_state.clone();
    update(&editor, move || {
        let with_class = |name: &str| -> Result<NodeRef, LexicalError> {
            set_state(&create_text_node(name), &state, name.to_owned())
        };
        get_root().clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[
            with_class("red")?,
            create_text_node("none!"),
            with_class("red")?,
            with_class("blue")?,
            with_class("blue")?,
            with_class("red")?,
            with_class("blue")?,
        ])?;
        get_root().append(&[paragraph])
    });
    let read = |states: &[&str], texts: &[&str]| {
        let state = class_name_state.clone();
        editor
            .read(move || {
                let nodes = paragraph_texts();
                assert_eq!(
                    nodes
                        .iter()
                        .map(|n| get_state(n, &state).unwrap())
                        .collect::<Vec<_>>(),
                    states
                );
                assert_eq!(
                    nodes
                        .iter()
                        .map(NodeRef::get_text_content)
                        .collect::<Vec<_>>(),
                    texts
                );
            })
            .unwrap();
    };
    read(
        &["red", "", "red", "blue", "red", "blue"],
        &["red", "none!", "red", "blueblue", "red", "blue"],
    );
    let state = class_name_state.clone();
    update(&editor, move || {
        let paragraph = get_root().get_first_child_or_throw()?;
        assert_eq!(paragraph.get_type(), "paragraph");
        // Change none! to red.
        set_state(
            &paragraph.get_child_at_index(1).unwrap(),
            &state,
            "red".to_owned(),
        )?;
        Ok(())
    });
    read(
        &["red", "blue", "red", "blue"],
        &["rednone!red", "blueblue", "red", "blue"],
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeStatesAreEquivalent > different versions of the same state are not equivalent
#[test]
fn different_versions_of_the_same_state_are_not_equivalent() {
    let editor = state_editor();
    let vk = vk_state();
    let mut versions: Vec<Rc<dyn LexicalNode>> = Vec::new();
    let initial: Rc<RefCell<Option<Rc<dyn LexicalNode>>>> = Rc::default();
    {
        let (vk, initial) = (vk.clone(), initial.clone());
        update(&editor, move || {
            *initial.borrow_mut() = Some(get_root().get_latest());
            set_state(&get_root(), &vk, Some(0.0))?;
            Ok(())
        });
    }
    let initial_root = initial.borrow().clone().unwrap();
    versions.push(version_in(&editor.get_editor_state(), "root"));
    for value in [Some(1.0), Some(0.0), None] {
        let vk = vk.clone();
        update(&editor, move || {
            set_state(&get_root(), &vk, value)?;
            Ok(())
        });
        versions.push(version_in(&editor.get_editor_state(), "root"));
    }
    for state_json in [r#"{"vk":1}"#, r#"{"unknown":true,"vk":1}"#] {
        let state_json = json(state_json);
        update(&editor, move || {
            let mut exported = get_root().export_json(false);
            exported.insert("$", state_json);
            get_root().update_from_json(&exported)
        });
        versions.push(version_in(&editor.get_editor_state(), "root"));
    }
    let [v0, v1, v2, v3, v4, v5] = <[_; 6]>::try_from(versions).ok().unwrap();
    let state = |v: &Rc<dyn LexicalNode>| v.base().state.clone();
    // Each set's states are equivalent, and no state of one set is
    // equivalent to one of another.
    let equivalences = [
        vec![state(&initial_root), state(&v3), None],
        vec![state(&v0), state(&v2)],
        vec![state(&v1), state(&v4)],
        vec![state(&v5)],
    ];
    for i in 0..equivalences.len() {
        for j in i + 1..equivalences.len() {
            for a in &equivalences[i] {
                for b in &equivalences[j] {
                    assert!(
                        !node_states_are_equivalent(a.as_ref(), b.as_ref()),
                        "{i} {j}"
                    );
                    assert!(
                        !node_states_are_equivalent(b.as_ref(), a.as_ref()),
                        "{i} {j}"
                    );
                }
            }
        }
        let set = &equivalences[i];
        for j in 0..set.len() {
            for k in j + 1..set.len() {
                assert!(node_states_are_equivalent(set[j].as_ref(), set[k].as_ref()));
                assert!(node_states_are_equivalent(set[k].as_ref(), set[j].as_ref()));
            }
        }
    }
}

fn number_state_with_reset(key: &str, reset: bool) -> StateConfig<f64> {
    let mut config = StateValueConfig::new(number_or(0.0));
    config.reset_on_copy_node = reset;
    create_state_with(key, config)
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > resetOnCopyNode > state with resetOnCopyNode: true is reset when using $copyNode
#[test]
fn state_with_reset_on_copy_node_true_is_reset_when_using_copy_node() {
    let editor = state_editor();
    update(&editor, || {
        let reset_state = number_state_with_reset("resetState", true);
        let node = create_state_node();
        set_state(&node, &reset_state, 42.0)?;
        assert_eq!(get_state(&node, &reset_state)?, 42.0);
        let copy = copy_node(&node);
        assert_eq!(get_state(&copy, &reset_state)?, 0.0);
        assert_eq!(get_state(&node, &reset_state)?, 42.0);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > resetOnCopyNode > state with resetOnCopyNode: false is preserved when using $copyNode
#[test]
fn state_with_reset_on_copy_node_false_is_preserved_when_using_copy_node() {
    let editor = state_editor();
    update(&editor, || {
        let persist_state = number_state_with_reset("persistState", false);
        let node = create_state_node();
        set_state(&node, &persist_state, 42.0)?;
        assert_eq!(get_state(&node, &persist_state)?, 42.0);
        let copy = copy_node(&node);
        assert_eq!(get_state(&copy, &persist_state)?, 42.0);
        assert_eq!(get_state(&node, &persist_state)?, 42.0);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > resetOnCopyNode > state without resetOnCopyNode option is preserved when using $copyNode
#[test]
fn state_without_reset_on_copy_node_option_is_preserved_when_using_copy_node() {
    let editor = state_editor();
    update(&editor, || {
        let default_state = create_state("defaultState", number_or(0.0));
        let node = create_state_node();
        set_state(&node, &default_state, 42.0)?;
        assert_eq!(get_state(&node, &default_state)?, 42.0);
        let copy = copy_node(&node);
        assert_eq!(get_state(&copy, &default_state)?, 42.0);
        assert_eq!(get_state(&node, &default_state)?, 42.0);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > resetOnCopyNode > multiple states with different resetOnCopyNode configurations
#[test]
fn multiple_states_with_different_reset_on_copy_node_configurations() {
    let editor = state_editor();
    update(&editor, || {
        let reset_state = number_state_with_reset("resetState", true);
        let mut persist = StateValueConfig::new(string_or_empty);
        persist.reset_on_copy_node = false;
        let persist_state = create_state_with("persistState", persist);
        let default_state = create_state("defaultState", |v| v.as_bool().unwrap_or(false));
        let node = create_state_node();
        set_state(&node, &reset_state, 100.0)?;
        set_state(&node, &persist_state, "hello".to_owned())?;
        set_state(&node, &default_state, true)?;
        let values = |n: &NodeRef| -> (f64, String, bool) {
            (
                get_state(n, &reset_state).unwrap(),
                get_state(n, &persist_state).unwrap(),
                get_state(n, &default_state).unwrap(),
            )
        };
        let original = (100.0, "hello".to_owned(), true);
        assert_eq!(values(&node), original);
        let copy = copy_node(&node);
        assert_eq!(values(&copy), (0.0, "hello".to_owned(), true));
        assert_eq!(values(&node), original);
        let full_copy = copy_node_with(&node, true);
        assert_eq!(values(&node), original);
        assert_eq!(values(&full_copy), original);
        Ok(())
    });
}

thread_local! {
    static IP_STATE: StateConfig<f64> = create_state("ipState", number_or(0.0));
    static IA_STATE: StateConfig<f64> = create_state("iaState", number_or(0.0));
    static IC_STATE: StateConfig<f64> = create_state("icState", number_or(0.0));
    static INTERLEAVE_TRANSFORMS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// `InterleaveConcrete`: the leaf of `ElementNode -> InterleaveParagraph ->
/// InterleaveAbstract -> InterleaveConcrete`, each level declaring a flat
/// state, the abstract one a `$transform`. A Rust class lists what it
/// inherits itself.
#[derive(Clone, Debug, Default)]
struct InterleaveConcrete {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for InterleaveConcrete {
    fn get_type(&self) -> &'static str {
        "interleave-concrete"
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
    fn super_types(&self) -> Vec<&'static str> {
        vec!["interleave-paragraph"]
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![
            IC_STATE.with(|s| RequiredNodeStateConfig::new(s, true)),
            IA_STATE.with(|s| RequiredNodeStateConfig::new(s, true)),
            IP_STATE.with(|s| RequiredNodeStateConfig::new(s, true)),
        ]
    }
    fn transform(&self, this: &NodeRef) -> Result<(), LexicalError> {
        INTERLEAVE_TRANSFORMS.with(|t| t.borrow_mut().push(this.get_type().to_owned()));
        Ok(())
    }
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::$config interleaved abstract/concrete classes > serializes, transforms, and types every interleaved level’s state
#[test]
fn serializes_transforms_and_types_every_interleaved_levels_state() {
    INTERLEAVE_TRANSFORMS.with(|t| t.borrow_mut().clear());
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<InterleaveConcrete>()],
        ..CreateEditorArgs::default()
    });
    update(&editor, || {
        let node = create::<InterleaveConcrete>();
        set_state(&node, &IP_STATE.with(Clone::clone), 1.0)?;
        set_state(&node, &IA_STATE.with(Clone::clone), 2.0)?;
        set_state(&node, &IC_STATE.with(Clone::clone), 3.0)?;
        get_root().append(std::slice::from_ref(&node))?;
        let json = node.export_json(false);
        for (key, value) in [("iaState", 2.0), ("icState", 3.0), ("ipState", 1.0)] {
            assert_eq!(json.get(key), Some(&value.into()), "{key}");
        }
        assert_eq!(json.get("type"), Some(&"interleave-concrete".into()));
        Ok(())
    });
    // The abstract base's transform ran for the concrete leaf.
    assert!(INTERLEAVE_TRANSFORMS.with(|t| t.borrow().contains(&"interleave-concrete".to_owned())));
}

thread_local! {
    static REPLACED_FLAT_STATE: StateConfig<String> =
        create_state("replacedFlatKey", string_or_empty);
}

/// A paragraph subclass with a flat state, of a type of its own or of
/// `paragraph`'s.
macro_rules! flat_paragraph {
    ($name:ident, $type:literal) => {
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            element: ElementData,
        }
        impl LexicalNode for $name {
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
            fn super_types(&self) -> Vec<&'static str> {
                vec!["paragraph"]
            }
            fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
                vec![REPLACED_FLAT_STATE.with(|s| RequiredNodeStateConfig::new(s, true))]
            }
        }
    };
}

flat_paragraph!(FlatParagraph, "flat-paragraph");
flat_paragraph!(SameTypeParagraph, "paragraph");

/// `{replace: ParagraphNode, with: () => new N(), withKlass: N}`.
fn replacement<N: LexicalNode + Default>() -> NodeClass {
    LexicalNodeReplacement {
        replace: NodeClass::of::<ParagraphNode>(),
        with: Rc::new(|_| Ok(set_node_key(Box::new(N::default())))),
        with_klass: Some(NodeClass::of::<N>()),
    }
    .into()
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::flat NodeState through a node replacement > the subclass writes and reads its own flat state top-level
#[test]
fn the_subclass_writes_and_reads_its_own_flat_state_top_level() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<FlatParagraph>(),
            replacement::<FlatParagraph>(),
        ],
        ..CreateEditorArgs::default()
    });
    update(&editor, || {
        let p = create_paragraph_node();
        assert!(p.get_latest().is::<FlatParagraph>());
        assert_eq!(p.get_type(), "flat-paragraph");
        p.append(&[create_text_node("x")])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&p))?;
        set_state(
            &p,
            &REPLACED_FLAT_STATE.with(Clone::clone),
            "hello".to_owned(),
        )?;
        Ok(())
    });
    let json = editor.get_editor_state().to_json(false);
    let exported = &json
        .get("root")
        .unwrap()
        .get("children")
        .unwrap()
        .as_array()
        .unwrap()[0];
    // Written top-level, not nested under the NodeState key.
    assert_eq!(exported.get("replacedFlatKey"), Some(&"hello".into()));
    assert!(exported.get("$").is_none_or(Value::is_undefined));
    let reparsed = editor.parse_editor_state(&json.to_string()).unwrap();
    assert_eq!(
        reparsed.read(|| get_state(
            &get_root().get_first_child_or_throw().unwrap(),
            &REPLACED_FLAT_STATE.with(Clone::clone)
        )
        .unwrap()),
        "hello"
    );
}

/// What a failed update with an `onError` that throws throws.
fn update_fails(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) -> String {
    match catch_unwind(AssertUnwindSafe(|| editor.update(f, discrete()))) {
        Ok(result) => result.expect_err("a failed update").message().to_owned(),
        Err(payload) => panic_message(&*payload),
    }
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::flat NodeState through a node replacement > withKlass must itself be registered
#[test]
fn with_klass_must_itself_be_registered() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![replacement::<FlatParagraph>()],
        ..CreateEditorArgs::default()
    });
    let message = update_fails(&editor, || {
        create_paragraph_node();
        Ok(())
    });
    assert!(
        message.contains("not configured to be used on the editor"),
        "{message}"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::flat NodeState through a node replacement > a replacement class cannot reuse the replaced type
#[test]
fn a_replacement_class_cannot_reuse_the_replaced_type() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![replacement::<SameTypeParagraph>()],
        ..CreateEditorArgs::default()
    });
    let message = update_fails(&editor, || {
        create_paragraph_node();
        Ok(())
    });
    assert!(
        message.contains("does not match registered node ParagraphNode"),
        "{message}"
    );
}

thread_local! {
    static FLAT_KEY: StateConfig<String> = create_state("importFlatKey", string_or_empty);
    static DIRECT_KEY: StateConfig<f64> = create_state("importDirectKey", number_or(0.0));
}

/// `StatefulParagraph extends ParagraphNode`, with a flat state.
#[derive(Clone, Debug, Default)]
struct StatefulParagraph {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for StatefulParagraph {
    fn get_type(&self) -> &'static str {
        "stateful-paragraph"
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
    fn super_types(&self) -> Vec<&'static str> {
        vec!["paragraph"]
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![FLAT_KEY.with(|s| RequiredNodeStateConfig::new(s, true))]
    }
}

fn stateful_paragraph_editor() -> Editor {
    create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<StatefulParagraph>()],
        ..CreateEditorArgs::default()
    })
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::importJSON applies state to the node it just built > both flat and nested state survive a round trip through importJSON
#[test]
fn both_flat_and_nested_state_survive_a_round_trip_through_import_json() {
    let editor = stateful_paragraph_editor();
    update(&editor, || {
        let p = create::<StatefulParagraph>();
        p.append(&[create_text_node("x")])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&p))?;
        set_state(&p, &FLAT_KEY.with(Clone::clone), "flat-value".to_owned())?;
        set_state(&p, &DIRECT_KEY.with(Clone::clone), 42.0)?;
        Ok(())
    });
    let json = editor.get_editor_state().to_json(false).to_string();
    editor.parse_editor_state(&json).unwrap().read(|| {
        let p = get_root().get_first_child_or_throw().unwrap();
        assert_eq!(
            get_state(&p, &FLAT_KEY.with(Clone::clone)).unwrap(),
            "flat-value"
        );
        assert_eq!(get_state(&p, &DIRECT_KEY.with(Clone::clone)).unwrap(), 42.0);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::importJSON applies state to the node it just built > a node whose JSON carries no state still imports its own properties
#[test]
fn a_node_whose_json_carries_no_state_still_imports_its_own_properties() {
    let editor = stateful_paragraph_editor();
    update(&editor, || {
        let p = create::<StatefulParagraph>();
        p.append(&[create_text_node("hello")])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&p))?;
        p.set_indent(2.0)
    });
    let json = editor.get_editor_state().to_json(false).to_string();
    assert!(!json.contains('$'));
    editor.parse_editor_state(&json).unwrap().read(|| {
        let p = get_root().get_first_child_or_throw().unwrap();
        assert!(p.get_latest().super_types().contains(&"paragraph"));
        assert_eq!(p.get_indent().unwrap(), 2.0);
        assert_eq!(p.get_text_content(), "hello");
        assert_eq!(get_state(&p, &FLAT_KEY.with(Clone::clone)).unwrap(), "");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::importJSON applies state to the node it just built > the node importJSON returns is writable, so its properties stick
#[test]
fn the_node_import_json_returns_is_writable_so_its_properties_stick() {
    let editor = stateful_paragraph_editor();
    update(&editor, || {
        let node = import_json(
            "paragraph",
            &object(
                r#"{"children":[],"direction":null,"format":"","indent":3,"type":"paragraph","version":1}"#,
            ),
        );
        assert_eq!(node.get_type(), "paragraph");
        node.append(&[create_text_node("y")])?;
        get_root().clear()?;
        get_root().append(&[node])
    });
    editor
        .read(|| {
            let p = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(p.get_type(), "paragraph");
            assert_eq!(p.get_indent().unwrap(), 3.0);
            assert_eq!(p.get_text_content(), "y");
        })
        .unwrap();
}

thread_local! {
    static TAGS_STATE: StateConfig<Value> = create_state_with(
        "tags",
        StateValueConfig::from_schema(array_set_value(string_value("")).unwrap()),
    );
}

fn tags_state() -> StateConfig<Value> {
    TAGS_STATE.with(Clone::clone)
}

fn paragraph_with_tags(tags: Value) -> Result<NodeRef, LexicalError> {
    let p = set_state(&create_paragraph_node(), &tags_state(), tags)?;
    get_root().clear()?;
    get_root().append(std::slice::from_ref(&p))?;
    Ok(p)
}

fn create_set_editor() -> Editor {
    create_editor(CreateEditorArgs::default())
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the value is a readonly array, and the schema says it is a set
#[test]
fn the_value_is_a_readonly_array_and_the_schema_says_it_is_a_set() {
    // A Rust value is immutable unless owned, so there is nothing to freeze.
    let tags = tags_state();
    assert_eq!(tags.default_value(), json("[]"));
    let schema = tags.schema().expect("a schema");
    assert_eq!(schema.meta().kind(), "set");
    let SerializationSchemaMeta::Set { item } = schema.meta() else {
        panic!("a set schema");
    };
    assert_eq!(item.meta().kind(), "string");
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > parsing drops duplicates and rejected entries, sorts and freezes
#[test]
fn parsing_drops_duplicates_and_rejected_entries_sorts_and_freezes() {
    let tags = tags_state();
    let input = Value::Array(vec![
        "b".into(),
        "a".into(),
        "b".into(),
        1.0.into(),
        Value::Null,
        Value::Undefined,
        "c".into(),
    ]);
    let parsed = tags.parse(&input);
    assert_eq!(parsed, json(r#"["a","b","c"]"#));
    // Sorted by UTF-16 code unit, as Array.prototype.sort does.
    assert_eq!(
        tags.parse(&json(r#"["b","B","a","é","e"]"#)),
        json(r#"["B","a","b","e","é"]"#)
    );
    for not_an_array in [
        json(r#""a""#),
        Value::Undefined,
        Value::Null,
        json("1"),
        json(r#"{"a":true}"#),
    ] {
        assert_eq!(tags.parse(&not_an_array), json("[]"));
    }
    // A value that is already canonical parses to itself.
    assert_eq!(tags.parse(&parsed), parsed);
    let size_state = create_state_with(
        "size",
        StateValueConfig::<Value>::from_schema(
            array_set_value(enum_value(vec!["s".into(), "m".into(), "l".into()], None).unwrap())
                .unwrap(),
        ),
    );
    assert_eq!(
        size_state.parse(&json(r#"["xl","l","s","l"]"#)),
        json(r#"["l","s"]"#)
    );
    assert_eq!(size_state.parse(&json(r#"["l","xl"]"#)), json(r#"["l"]"#));
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the value is frozen, so mutating it in place throws
#[test]
fn the_value_is_frozen_so_mutating_it_in_place_throws() {
    // A Rust value can't be changed through what get_state returns: a copy
    // changed in place leaves the stored value as it was.
    let mut parsed = tags_state().parse(&json(r#"["b","a"]"#));
    if let Value::Array(items) = &mut parsed {
        items.push("c".into());
    }
    assert_eq!(
        tags_state().parse(&json(r#"["b","a"]"#)),
        json(r#"["a","b"]"#)
    );
    let editor = create_set_editor();
    update(&editor, || {
        let p = paragraph_with_tags(json(r#"["b","a"]"#))?;
        let mut stored = get_state(&p, &tags_state())?;
        if let Value::Array(items) = &mut stored {
            items.push("c".into());
        }
        assert_eq!(get_state(&p, &tags_state())?, json(r#"["a","b"]"#));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > $setState stores unsorted or duplicated input in the canonical form
#[test]
fn set_state_stores_unsorted_or_duplicated_input_in_the_canonical_form() {
    let editor = create_set_editor();
    let key = Rc::new(RefCell::new(String::new()));
    let k = key.clone();
    update(&editor, move || {
        let p = paragraph_with_tags(json(r#"["b","a","a"]"#))?;
        *k.borrow_mut() = p.get_key().to_owned();
        assert_eq!(get_state(&p, &tags_state())?, json(r#"["a","b"]"#));
        // An updater's result is canonicalized too.
        set_state_with(&p, &tags_state(), |prev| {
            let mut items = prev.as_array().unwrap().clone();
            items.extend(["c".into(), "a".into(), "c".into()]);
            Value::Array(items)
        })?;
        assert_eq!(get_state(&p, &tags_state())?, json(r#"["a","b","c"]"#));
        assert_eq!(
            p.export_json(false).get("$"),
            Some(&json(r#"{"tags":["a","b","c"]}"#))
        );
        Ok(())
    });
    let key = key.borrow().clone();
    editor
        .read(|| {
            assert_eq!(
                get_state(&NodeRef::new(key), &tags_state()).unwrap(),
                json(r#"["a","b","c"]"#)
            );
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > equality is element by element on the canonical form
#[test]
fn equality_is_element_by_element_on_the_canonical_form() {
    let tags = tags_state();
    let ab = tags.parse(&json(r#"["a","b"]"#));
    assert!(tags.is_equal(&ab, &tags.parse(&json(r#"["b","a","b"]"#))));
    assert!(tags.is_equal(&ab, &json(r#"["a","b"]"#)));
    assert!(!tags.is_equal(&ab, &tags.parse(&json(r#"["a"]"#))));
    assert!(!tags.is_equal(&ab, &tags.parse(&json(r#"["a","c"]"#))));
    // Total: not an array is not equal.
    assert!(!tags.is_equal(&ab, &"a,b".into()));
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the JSON is one sorted array, and the empty set is omitted
#[test]
fn the_json_is_one_sorted_array_and_the_empty_set_is_omitted() {
    let editor = create_set_editor();
    update(&editor, || {
        let p = paragraph_with_tags(json(r#"["c","a","b"]"#))?;
        assert_eq!(
            p.export_json(false).get("$"),
            Some(&json(r#"{"tags":["a","b","c"]}"#))
        );
        set_state(&p, &tags_state(), json("[]"))?;
        assert!(p.export_json(false).get("$").is_none());
        set_state(&p, &tags_state(), json(r#"["a","a"]"#))?;
        assert_eq!(
            p.export_json(false).get("$"),
            Some(&json(r#"{"tags":["a"]}"#))
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > setting the same members in another order does not mark the node dirty
#[test]
fn setting_the_same_members_in_another_order_does_not_mark_the_node_dirty() {
    let editor = create_set_editor();
    let key = Rc::new(RefCell::new(String::new()));
    let k = key.clone();
    update(&editor, move || {
        *k.borrow_mut() = paragraph_with_tags(json(r#"["a","b"]"#))?
            .get_key()
            .to_owned();
        Ok(())
    });
    let dirty: Rc<RefCell<Vec<String>>> = Rc::default();
    let d = dirty.clone();
    let _unregister = editor.register_update_listener(move |payload| {
        let mut dirty = d.borrow_mut();
        dirty.extend(payload.dirty_elements.iter().map(|(k, _)| k.clone()));
        dirty.extend(payload.dirty_leaves.iter().cloned());
    });
    update(&editor, || {
        let p = get_root().get_first_child_or_throw()?;
        set_state_with(&p, &tags_state(), |_| json(r#"["b","a","b"]"#))?;
        // Returning the previous value as is.
        set_state_with(&p, &tags_state(), Value::clone)?;
        Ok(())
    });
    assert!(!dirty.borrow().contains(&key.borrow()));
}

/// `TagsParagraphNode`: a paragraph that declares the tags state.
#[derive(Clone, Debug, Default)]
struct TagsParagraphNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for TagsParagraphNode {
    fn get_type(&self) -> &'static str {
        "tags-paragraph"
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
    fn super_types(&self) -> Vec<&'static str> {
        vec!["paragraph"]
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![RequiredNodeStateConfig::new(&tags_state(), false)]
    }
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the JSON is canonical and stable across a round trip
#[test]
fn the_json_is_canonical_and_stable_across_a_round_trip() {
    let create_tags_editor = || {
        create_editor(CreateEditorArgs {
            nodes: vec![NodeClass::of::<TagsParagraphNode>()],
            ..CreateEditorArgs::default()
        })
    };
    let editor = create_tags_editor();
    update(&editor, || {
        get_root().clear()?;
        let p = set_state(
            &create::<TagsParagraphNode>(),
            &tags_state(),
            json(r#"["b","a"]"#),
        )?;
        get_root().append(&[p])
    });
    let serialized = editor.get_editor_state().to_json(false).to_string();
    assert!(
        serialized.contains(r#""$":{"tags":["a","b"]}"#),
        "{serialized}"
    );
    // Unordered, duplicated input with an entry out of the item's domain is
    // the same set, written back canonical without being read.
    let unordered = serialized.replacen(r#"["a","b"]"#, r#"["b","a","b",7]"#, 1);
    for input in [&serialized, &unordered] {
        let loaded = create_tags_editor();
        let state = loaded.parse_editor_state(input).unwrap();
        loaded.set_editor_state(&state, None).unwrap();
        assert_eq!(
            loaded.get_editor_state().to_json(false).to_string(),
            serialized
        );
        loaded
            .read(|| {
                let value = get_state(
                    &get_root().get_first_child_or_throw().unwrap(),
                    &tags_state(),
                );
                assert_eq!(value.unwrap(), json(r#"["a","b"]"#));
            })
            .unwrap();
    }
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > an unparse of its own still wins
#[test]
fn an_unparse_of_its_own_still_wins() {
    let mut config =
        StateValueConfig::<Value>::from_schema(array_set_value(string_value("")).unwrap());
    config.unparse = Rc::new(|v: &Value| {
        Value::String(
            v.as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap())
                .collect::<Vec<_>>()
                .join(","),
        )
    });
    let csv_state = create_state_with("csv", config);
    assert_eq!(
        csv_state.unparse(&csv_state.parse(&json(r#"["b","a"]"#))),
        Value::from("a,b")
    );
}

/// A node with a field of the schema's domain, for `withField`.
#[derive(Clone, Debug)]
struct TagsFieldNode {
    base: NodeBase,
    tags: Value,
}

impl Default for TagsFieldNode {
    fn default() -> Self {
        Self {
            base: NodeBase::default(),
            tags: Value::Array(Vec::new()),
        }
    }
}

impl LexicalNode for TagsFieldNode {
    fn get_type(&self) -> &'static str {
        "tags-field"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > an arraySetValue can only be the whole value of a createState
#[test]
fn an_array_set_value_can_only_be_the_whole_value_of_a_create_state() {
    let set = || array_set_value(string_value("")).unwrap();
    // `nodeSchema<N>()({tags: arraySetValue(item)})`: a Rust node schema
    // takes properties (`SchemaField`s), and the two ways to make one
    // (`with_field`, `with_accessors`) refuse the set, as below.
    let builds: Vec<Box<dyn Fn() -> Result<(), LexicalError>>> = vec![
        Box::new(|| array_value(set()).map(drop)),
        Box::new(|| nullable(set(), false).map(drop)),
        Box::new(|| array_set_value(set()).map(drop)),
        Box::new(|| object_value(vec![("tags", set())]).map(drop)),
        Box::new(|| {
            with_field(
                set(),
                |n: &TagsFieldNode| &n.tags,
                |n: &mut TagsFieldNode| &mut n.tags,
            )
            .map(drop)
        }),
        Box::new(|| with_accessors(set(), None, None).map(drop)),
        Box::new(|| optional(set(), false).map(drop)),
        Box::new(|| union_value(vec![set(), string_value("")], None).map(drop)),
        Box::new(|| aliased_value(set(), Vec::new()).map(drop)),
        Box::new(|| transform_value(set(), |v| v, None).map(drop)),
    ];
    for build in builds {
        let error = build().unwrap_err();
        assert!(
            error
                .message()
                .contains("arraySetValue can only be the whole value"),
            "{error}"
        );
    }
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the item schema must produce strings
#[test]
fn the_item_schema_must_produce_strings() {
    let error = array_set_value(number_value(0.0, Default::default()).unwrap()).unwrap_err();
    assert!(
        error
            .message()
            .contains("the item schema must produce strings")
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > an item schema that declines some strings drops them
#[test]
fn an_item_schema_that_declines_some_strings_drops_them() {
    // `/^\w+$/`.
    fn is_word(s: &str) -> bool {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
    let word = SerializationSchema::new(
        SerializationSchemaMeta::String,
        "".into(),
        None,
        Some(Rc::new(|v: &Value| v.as_str().is_some_and(is_word))),
        |v| match v {
            Value::String(_) => v.clone(),
            _ => "".into(),
        },
    );
    let parse = array_set_value(word).unwrap();
    let input = Value::Array(vec![
        "b".into(),
        "a b".into(),
        "".into(),
        "a".into(),
        7.0.into(),
    ]);
    assert_eq!(parse.parse(&input), json(r#"["a","b"]"#));
    assert_eq!(parse.parse(&json(r#"["a","b c"]"#)), json(r#"["a"]"#));
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > a frozen value that is not canonical is still canonicalized
#[test]
fn a_frozen_value_that_is_not_canonical_is_still_canonicalized() {
    for (input, expected) in [
        (r#"["a","a"]"#, r#"["a"]"#),
        (r#"["b","a"]"#, r#"["a","b"]"#),
        (r#"["a",1]"#, r#"["a"]"#),
        ("[]", "[]"),
    ] {
        assert_eq!(tags_state().parse(&json(input)), json(expected));
    }
    let canonical = json(r#"["a","b"]"#);
    assert_eq!(tags_state().parse(&canonical), canonical);
    let editor = create_set_editor();
    update(&editor, || {
        let p = paragraph_with_tags(json("[]"))?;
        set_state(&p, &tags_state(), json(r#"["b","a","a"]"#))?;
        assert_eq!(get_state(&p, &tags_state())?, json(r#"["a","b"]"#));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > a state that is not declared is canonical when it is read after loading JSON
#[test]
fn a_state_that_is_not_declared_is_canonical_when_it_is_read_after_loading_json() {
    let editor = create_set_editor();
    update(&editor, || paragraph_with_tags(json(r#"["a"]"#)).map(drop));
    let serialized = editor
        .get_editor_state()
        .to_json(false)
        .to_string()
        .replacen(r#"["a"]"#, r#"["b","a","b"]"#, 1);
    let loaded = create_set_editor();
    let state = loaded.parse_editor_state(&serialized).unwrap();
    loaded.set_editor_state(&state, None).unwrap();
    loaded
        .read(|| {
            let value = get_state(
                &get_root().get_first_child_or_throw().unwrap(),
                &tags_state(),
            );
            assert_eq!(value.unwrap(), json(r#"["a","b"]"#));
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalNodeState.test.ts::$setState stores the value as given for every state that is not an arraySetValue > arrayValue, a hand-written parse and an updater keep the exact reference
#[test]
fn array_value_a_hand_written_parse_and_an_updater_keep_the_exact_reference() {
    // A Rust value has no identity to keep; what holds is that it is stored
    // as given, neither sorted, deduplicated nor parsed.
    let editor = create_set_editor();
    update(&editor, || {
        let list_state = create_state_with(
            "list",
            StateValueConfig::<Value>::from_schema(array_value(string_value("")).unwrap()),
        );
        let hand_state = create_state_with(
            "hand",
            StateValueConfig {
                parse: Rc::new(|v: &Value| match v {
                    Value::Array(_) => v.clone(),
                    _ => Value::Array(Vec::new()),
                }),
                unparse: Rc::new(Value::clone),
                is_equal: Rc::new(|a, b| a == b),
                reset_on_copy_node: false,
                derived: false,
                schema: None,
            },
        );
        let object_state = create_state_with(
            "object",
            StateValueConfig::<Value>::from_schema(
                object_value(vec![("a", string_value(""))]).unwrap(),
            ),
        );
        let p = create_paragraph_node();
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&p))?;
        let list = json(r#"["b","a","a"]"#);
        set_state(&p, &list_state, list.clone())?;
        assert_eq!(get_state(&p, &list_state)?, list);
        let hand = json(r#"["b","a"]"#);
        set_state(&p, &hand_state, hand.clone())?;
        assert_eq!(get_state(&p, &hand_state)?, hand);
        let updated = json(r#"["z","y"]"#);
        let u = updated.clone();
        set_state_with(&p, &list_state, move |_| u)?;
        assert_eq!(get_state(&p, &list_state)?, updated);
        let object = json(r#"{"a":"x"}"#);
        set_state(&p, &object_state, object.clone())?;
        assert_eq!(get_state(&p, &object_state)?, object);
        // Out of the parse's domain, and still stored as given.
        set_state(&p, &hand_state, "not an array".into())?;
        assert_eq!(get_state(&p, &hand_state)?, Value::from("not an array"));
        Ok(())
    });
}
