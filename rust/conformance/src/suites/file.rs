use crate::case::{Case, Outcome, same};
use lexical::file::{
    SerializedDocumentConfig, editor_state_from_serialized_document,
    serialized_document_from_editor_state,
};
use lexical::json::Value;
use lexical::{CreateEditorArgs, Editor, create_editor};

/// `file.test.ts`'s `newEditor()`: a headless editor with the list nodes.
fn new_editor() -> Editor {
    create_editor(CreateEditorArgs {
        nodes: lexical::list::list_nodes(),
        ..CreateEditorArgs::default()
    })
}

/// `file/export`: `serializedDocumentFromEditorState` of the parsed editor
/// state and the case's config writes JS's document byte for byte, and
/// `editorStateFromSerializedDocument` reads the editor state back.
pub fn check_export(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let editor_state = case.str("editorState")?;
    let config = case.obj("config")?;
    let document = case.str("document")?;
    let state = new_editor()
        .parse_editor_state(editor_state)
        .map_err(|e| format!("parsing the editor state: {e}"))?;
    let config = SerializedDocumentConfig {
        source: config
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_owned),
        last_saved: config.get("lastSaved").and_then(Value::as_f64),
    };
    same(
        "document",
        serialized_document_from_editor_state(&state, &config)
            .to_json()
            .to_string()
            .as_str(),
        document,
    )?;
    let read = editor_state_from_serialized_document(&new_editor(), document)
        .map_err(|e| format!("reading the document: {e}"))?;
    same(
        "editor state read back",
        read.to_json(false).to_string().as_str(),
        editor_state,
    )?;
    Ok(Outcome::Pass)
}

/// `file/import`: `editorStateFromSerializedDocument` of a document's text
/// gives JS's editor state.
pub fn check_import(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let state = editor_state_from_serialized_document(&new_editor(), case.str("document")?)
        .map_err(|e| format!("reading the document: {e}"))?;
    same(
        "editor state",
        state.to_json(false).to_string().as_str(),
        case.str("editorState")?,
    )?;
    Ok(Outcome::Pass)
}
