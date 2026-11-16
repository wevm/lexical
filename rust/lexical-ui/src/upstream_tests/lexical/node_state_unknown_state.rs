//! Ports of `packages/lexical/src/__tests__/unit/NodeStateUnknownState.test.ts`.
//! Neither state is known to `ParagraphNode` in a fresh editor until it is
//! first read or written, so a paragraph made from JSON keeps their values
//! in `unknownState` until then.

use crate::json::{Object, Value, parse};
use crate::node_state::get_writable_node_state;
use crate::nodes::paragraph::ParagraphNode;
use crate::upstream_tests::support::json_to_equal;
use crate::{
    CreateEditorArgs, Editor, NodeClass, NodeRef, StateConfig, UpdateOptions, create_editor,
    create_paragraph_node, create_state, get_root, get_state, parse_serialized_node, set_state,
    set_state_with,
};
use std::rc::Rc;

thread_local! {
    static ID_STATE: StateConfig<Option<String>> =
        create_state("nsuId", |v| v.as_str().map(str::to_owned));
    static COUNT_STATE: StateConfig<f64> = create_state("nsuCount", |v| v.as_f64().unwrap_or(0.0));
}

fn id_state() -> StateConfig<Option<String>> {
    ID_STATE.with(Clone::clone)
}

fn count_state() -> StateConfig<f64> {
    COUNT_STATE.with(Clone::clone)
}

const JSON_STATE: &str = r#"{"nsuCount":2,"nsuId":"stored","nsuOther":true}"#;

fn paragraph_json() -> Value {
    parse(&format!(
        r#"{{"$":{JSON_STATE},"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}}"#
    ))
    .unwrap()
}

fn append_from_json() -> NodeRef {
    let node = parse_serialized_node(&paragraph_json()).unwrap();
    get_root().clear().unwrap();
    get_root().append(std::slice::from_ref(&node)).unwrap();
    node
}

fn export_state(editor: &Editor) -> Value {
    editor.get_editor_state().read(|| {
        get_root()
            .get_first_child_or_throw()
            .unwrap()
            .export_json(false)
            .get(crate::constants::NODE_STATE_KEY)
            .cloned()
            .unwrap_or(Value::Undefined)
    })
}

fn make_editor() -> Editor {
    create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<ParagraphNode>()],
        ..CreateEditorArgs::default()
    })
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn count_and_other() -> Value {
    parse(r#"{"nsuCount":2,"nsuOther":true}"#).unwrap()
}

// upstream: lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > $setState to the default removes it, in the same update and after
#[test]
fn set_state_to_the_default_removes_it_in_the_same_update_and_after() {
    let editor = make_editor();
    editor
        .update(
            || {
                let node = append_from_json();
                set_state(&node, &id_state(), None)?;
                assert_eq!(get_state(&node, &id_state())?, None);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| {
            let node = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(get_state(&node, &id_state()).unwrap(), None);
        })
        .unwrap();
    assert!(json_to_equal(&export_state(&editor), &count_and_other()));
}

// upstream: lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > $setState to the default after a read removes it
#[test]
fn set_state_to_the_default_after_a_read_removes_it() {
    let editor = make_editor();
    editor
        .update(
            || {
                let node = append_from_json();
                assert_eq!(get_state(&node, &id_state())?.as_deref(), Some("stored"));
                set_state_with(&node, &id_state(), |_| None)?;
                assert_eq!(get_state(&node, &id_state())?, None);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| {
            let node = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(get_state(&node, &id_state()).unwrap(), None);
        })
        .unwrap();
    assert!(json_to_equal(&export_state(&editor), &count_and_other()));
}

// upstream: lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > updateFromUnknown with a known key removes it
#[test]
fn update_from_unknown_with_a_known_key_removes_it() {
    let editor = make_editor();
    editor
        .update(
            || {
                let node = append_from_json();
                // Make the key known to the type after the node was made
                // from JSON.
                set_state(
                    &create_paragraph_node(),
                    &id_state(),
                    Some("other".to_owned()),
                )?;
                get_writable_node_state(&node)?
                    .borrow_mut()
                    .update_from_unknown("nsuId", Rc::new(Value::Undefined));
                assert_eq!(get_state(&node, &id_state())?, None);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    assert!(json_to_equal(&export_state(&editor), &count_and_other()));
}

// upstream: lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > a value that is set is the only copy of the key
#[test]
fn a_value_that_is_set_is_the_only_copy_of_the_key() {
    let editor = make_editor();
    editor
        .update(
            || {
                let node = append_from_json();
                set_state(&node, &id_state(), Some("new".to_owned()))?;
                set_state(&node, &count_state(), 0.0)?;
                let state = get_writable_node_state(&node)?;
                let state = state.borrow();
                // `getInternalState()`: `[unknownState, knownState]`.
                let unknown: Object = state
                    .unknown_state
                    .iter()
                    .flat_map(|u| u.iter())
                    .map(|(k, v)| (k, (**v).clone()))
                    .collect();
                assert!(json_to_equal(
                    &Value::Object(unknown),
                    &parse(r#"{"nsuOther":true}"#).unwrap()
                ));
                let known = state.known_entries();
                let value_of = |key: &str| {
                    known
                        .iter()
                        .find(|(c, _)| c.key() == key)
                        .map(|(_, v)| v.clone())
                        .expect("a known value")
                };
                assert_eq!(
                    value_of("nsuId").downcast_ref::<Option<String>>(),
                    Some(&Some("new".to_owned()))
                );
                assert_eq!(value_of("nsuCount").downcast_ref::<f64>(), Some(&0.0));
                drop(state);
                assert_eq!(get_state(&node, &id_state())?.as_deref(), Some("new"));
                assert_eq!(get_state(&node, &count_state())?, 0.0);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    assert!(json_to_equal(
        &export_state(&editor),
        &parse(r#"{"nsuId":"new","nsuOther":true}"#).unwrap()
    ));
}

// upstream: lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > reading it does not change the JSON
#[test]
fn reading_it_does_not_change_the_json() {
    let editor = make_editor();
    editor
        .update(
            || {
                let node = append_from_json();
                assert_eq!(get_state(&node, &id_state())?.as_deref(), Some("stored"));
                assert_eq!(get_state(&node, &count_state())?, 2.0);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    // Byte for byte, in the order it came.
    assert_eq!(export_state(&editor).to_string(), JSON_STATE);
}
