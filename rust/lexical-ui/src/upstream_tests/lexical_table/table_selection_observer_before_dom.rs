//! Ports of `packages/lexical-table/src/__tests__/unit/TableSelectionObserverBeforeDOM.test.ts`.

use super::support_c::{build_table_editor, table_with_dimensions};
use crate::extension::build_editor_from_extensions;
use crate::table::is_table_node;
use crate::upstream_tests::support::update;
use crate::{Editor, LexicalExtension, NodeKey, create_paragraph_node, define_extension, get_root};
use std::rc::Rc;

/// `$appendTable()`.
fn append_table() -> Result<(), crate::LexicalError> {
    get_root().append(&[table_with_dimensions(2, 2, false), create_paragraph_node()])
}

/// `$getTableKey()`.
fn get_table_key() -> Option<NodeKey> {
    get_root()
        .get_children()
        .into_iter()
        .find(is_table_node)
        .map(|t| t.get_key().to_owned())
}

/// `tableStateJSON()`: the serialized state of one 2x2 table, made by a
/// separate editor.
fn table_state_json() -> String {
    let source = build_table_editor("table-source", Vec::new());
    update(&source, append_table);
    let json = source.get_editor_state().to_json(false).to_string();
    source.dispose();
    json
}

// upstream: lexical-table/src/__tests__/unit/TableSelectionObserverBeforeDOM.test.ts::table selection observer for tables that exist before their DOM > builds when an earlier extension sets a state holding a table during register
#[test]
fn builds_when_an_earlier_extension_sets_a_state_holding_a_table_during_register() {
    let json = table_state_json();
    // An extension listed before TableExtension that replaces the editor
    // state from its register, as one that loads stored content does.
    let set_state_on_register = define_extension(LexicalExtension {
        register: Some(Rc::new(
            move |editor: &Editor, _: &(), _: &crate::ExtensionRegisterState<(), ()>| {
                let state = editor.parse_editor_state(&json)?;
                editor.set_editor_state(&state, None)?;
                Ok(Box::new(|| {}) as crate::Unregister)
            },
        )),
        ..LexicalExtension::<(), (), ()>::new("set-state-on-register", ())
    });
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            set_state_on_register.into(),
            crate::table::table_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("state-before-table", ())
    })
    .into()])
    .unwrap();
    assert!(editor.read(get_table_key).unwrap().is_some());
    editor.dispose();
}
