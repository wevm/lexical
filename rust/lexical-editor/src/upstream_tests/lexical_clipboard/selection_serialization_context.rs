//! Ports of
//! `packages/lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts`.

use crate::clipboard::generate_json_from_selected_nodes;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::json::{Object, Value};
use crate::rich_text::rich_text_extension;
use crate::serialized_export::with_compact_export;
use crate::upstream_tests::support::{json_to_equal, update_with};
use crate::{
    InitialEditorStateType, LexicalExtension, create_node_selection, create_paragraph_node,
    create_text_node, define_extension, get_root, get_selection, select_all,
};
use std::rc::Rc;

/// `{dependencies: [extension], name: '[root]', namespace: ''}`.
fn build() -> LexicalEditorWithDispose {
    let extension = define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let secret = create_text_node("secret");
            secret.set_format(1.0); // 'bold'
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("plain"), secret])?;
            let root = get_root();
            root.clear()?;
            root.append(&[paragraph])?;
            Ok(())
        }))),
        dependencies: vec![rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[selection-serialization]", ())
    });
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![extension.into()],
        namespace: Some(String::new()),
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

/// `selectionJSON(compact)`.
fn selection_json(compact: bool) -> Vec<Object> {
    let editor = build();
    let ed = (*editor).clone();
    let nodes = update_with(&editor, move || {
        select_all()?;
        with_compact_export(compact, || {
            Ok(generate_json_from_selected_nodes(&ed, get_selection().as_ref())?.nodes)
        })
    });
    editor.dispose();
    nodes
}

// upstream: lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts::selection export honors the serialization context > legacy is the default
#[test]
fn legacy_is_the_default() {
    let plain = selection_json(false).remove(0);
    // `toMatchObject`: these properties, whatever else it has.
    for (key, value) in [
        ("detail", Value::from(0.0)),
        ("format", Value::from(0.0)),
        ("mode", Value::from("normal")),
        ("style", Value::from("")),
        ("text", Value::from("plain")),
        ("type", Value::from("text")),
        ("version", Value::from(1.0)),
    ] {
        assert_eq!(plain.get(key), Some(&value), "{key}");
    }
}

// upstream: lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts::selection export honors the serialization context > compact applies to a selection export too
#[test]
fn compact_applies_to_a_selection_export_too() {
    let nodes = Value::Array(selection_json(true).into_iter().map(Value::from).collect());
    let expected = crate::json::parse(
        r#"[{"text": "plain", "type": "text"}, {"format": 1, "text": "secret", "type": "text"}]"#,
    )
    .unwrap();
    assert!(json_to_equal(&nodes, &expected), "{nodes:?}");
}

// upstream: lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts::selection export honors the serialization context > the legacy form can be forced at a call site
#[test]
fn the_legacy_form_can_be_forced_at_a_call_site() {
    let plain = selection_json(false).remove(0);
    assert_eq!(plain.get("version"), Some(&Value::from(1.0)));
}

// upstream: lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts::the serialized nodes parse back as a document without a cast
#[test]
fn the_serialized_nodes_parse_back_as_a_document_without_a_cast() {
    let editor = build();
    let ed = (*editor).clone();
    let nodes = update_with(&editor, move || {
        // The paragraph itself, so the payload is a subtree a root can hold.
        let selection = create_node_selection();
        selection.add(get_root().get_first_child_or_throw()?.get_key());
        Ok(generate_json_from_selected_nodes(&ed, Some(&selection))?.nodes)
    });
    let mut root = Object::new();
    root.insert(
        "children",
        Value::Array(nodes.into_iter().map(Value::from).collect()),
    );
    root.insert("direction", Value::Null);
    root.insert("format", "".into());
    root.insert("indent", 0.0.into());
    root.insert("type", "root".into());
    root.insert("version", 1.0.into());
    let mut json = Object::new();
    json.insert("root", root.into());
    let state = editor.parse_editor_state_json(&json.into()).unwrap();
    assert_eq!(state.read(|| get_root().get_text_content()), "plainsecret");
    editor.dispose();
}
