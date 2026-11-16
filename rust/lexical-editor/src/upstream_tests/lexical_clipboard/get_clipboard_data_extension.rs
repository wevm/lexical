//! Ports of
//! `packages/lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts`.
//!
//! JS's `$getClipboardDataFromSelection()` and
//! `$exportMimeTypeFromSelection(type)` default their selection to
//! `$getSelection()`; the Rust functions take it.

#![cfg(feature = "html")]

use crate::clipboard::{
    ExportMimeTypeConfig, ExportMimeTypeFunction, GetClipboardDataConfigOverrides,
    LEXICAL_MIME_TYPE, LexicalClipboardData, export_mime_type_from_selection,
    get_clipboard_data_extension, get_clipboard_data_from_selection,
    set_lexical_clipboard_data_transfer,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::{
    DataTransfer, InitialEditorStateType, LexicalExtension, config_extension,
    create_paragraph_node, create_text_node, define_extension, get_root, get_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

const SEED_TEXT: &str = "hello world";

/// `$initialEditorState`.
fn initial_editor_state() -> InitialEditorStateType {
    InitialEditorStateType::Update(Rc::new(|_| {
        let text = create_text_node(SEED_TEXT);
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().append(&[paragraph])?;
        text.select(Some(0), Some(SEED_TEXT.len()))?;
        Ok(())
    }))
}

/// `buildSeededEditor({$exportMimeType})`.
fn build_seeded_editor(export_mime_type: Option<ExportMimeTypeConfig>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(initial_editor_state()),
        dependencies: vec![config_extension(
            &get_clipboard_data_extension(),
            [GetClipboardDataConfigOverrides { export_mime_type }],
        )],
        ..LexicalExtension::<(), (), ()>::new("[test-root]", ())
    })
    .into()])
    .unwrap()
}

fn stack(mime_type: &str, fns: Vec<ExportMimeTypeFunction>) -> Option<ExportMimeTypeConfig> {
    Some(vec![(mime_type.to_owned(), fns)])
}

/// `data[type]`.
fn get<'a>(data: &'a LexicalClipboardData, mime_type: &str) -> Option<&'a str> {
    data.iter()
        .find(|(k, _)| k == mime_type)
        .map(|(_, v)| v.as_str())
}

/// `$getClipboardDataFromSelection()`, in a read of `editor`.
fn clipboard_data(editor: &LexicalEditorWithDispose) -> LexicalClipboardData {
    editor
        .read(|| get_clipboard_data_from_selection(get_selection().as_ref()).unwrap())
        .unwrap()
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > produces default text/plain, text/html, and application/x-lexical-editor entries
#[test]
fn produces_default_entries() {
    let editor = build_seeded_editor(None);
    let data = clipboard_data(&editor);
    assert_eq!(get(&data, "text/plain"), Some(SEED_TEXT));
    assert!(get(&data, "text/html").unwrap().contains(SEED_TEXT));
    let lexical_payload = get(&data, LEXICAL_MIME_TYPE).expect("a string");
    let parsed = crate::json::parse(lexical_payload).unwrap();
    let parsed = parsed.as_object().unwrap();
    assert_eq!(
        parsed.get("namespace").and_then(|n| n.as_str()),
        Some(editor.get_namespace())
    );
    let nodes = parsed
        .get("nodes")
        .and_then(|n| n.as_array())
        .expect("an array");
    assert!(!nodes.is_empty());
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > lets an override replace the default output for an existing MIME type
#[test]
fn lets_an_override_replace_the_default_output() {
    let editor = build_seeded_editor(stack(
        "text/plain",
        vec![Rc::new(|_, _| Ok(Some("OVERRIDDEN".to_owned())))],
    ));
    let data = clipboard_data(&editor);
    assert_eq!(get(&data, "text/plain"), Some("OVERRIDDEN"));
    // Other MIME types still come from the defaults below the override.
    assert!(get(&data, "text/html").unwrap().contains(SEED_TEXT));
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > falls through to the default handler when the override calls next()
#[test]
fn falls_through_to_the_default_handler_when_the_override_calls_next() {
    let editor = build_seeded_editor(stack(
        "text/plain",
        vec![Rc::new(|_, next| {
            Ok(Some(format!("[wrapped] {}", next()?.unwrap_or_default())))
        })],
    ));
    let data = clipboard_data(&editor);
    assert_eq!(
        get(&data, "text/plain"),
        Some(format!("[wrapped] {SEED_TEXT}").as_str())
    );
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > omits a MIME type when its top handler returns null without calling next()
#[test]
fn omits_a_mime_type_when_its_top_handler_returns_null() {
    let editor = build_seeded_editor(stack("text/html", vec![Rc::new(|_, _| Ok(None))]));
    let data = clipboard_data(&editor);
    assert_eq!(get(&data, "text/html"), None);
    // Defaults for other MIME types are unaffected.
    assert_eq!(get(&data, "text/plain"), Some(SEED_TEXT));
    assert!(get(&data, LEXICAL_MIME_TYPE).is_some());
    editor.dispose();
}

/// `JSON.stringify({text, v: 1})`.
fn my_format(text: &str) -> String {
    let mut json = crate::json::Object::new();
    json.insert("text", text.into());
    json.insert("v", 1.0.into());
    crate::json::stringify(&json.into()).unwrap()
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > registers a brand-new custom MIME type
#[test]
fn registers_a_brand_new_custom_mime_type() {
    let editor = build_seeded_editor(stack(
        "application/x-myformat",
        vec![Rc::new(|selection, _| {
            Ok(match selection {
                Some(selection) => Some(my_format(&selection.get_text_content()?)),
                None => None,
            })
        })],
    ));
    let data = clipboard_data(&editor);
    assert_eq!(
        get(&data, "application/x-myformat"),
        Some(my_format(SEED_TEXT).as_str())
    );
    assert_eq!(my_format(SEED_TEXT), r#"{"text":"hello world","v":1}"#);
    // Defaults are still produced.
    assert_eq!(get(&data, "text/plain"), Some(SEED_TEXT));
    assert!(get(&data, "text/html").unwrap().contains(SEED_TEXT));
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > runs higher-indexed handlers first within a single MIME stack
#[test]
fn runs_higher_indexed_handlers_first_within_a_single_mime_stack() {
    let calls = Rc::new(RefCell::new(Vec::<&str>::new()));
    let (a, b) = (calls.clone(), calls.clone());
    let editor = build_seeded_editor(stack(
        "text/plain",
        vec![
            Rc::new(move |_, next| {
                a.borrow_mut().push("A");
                Ok(Some(format!("A({})", next()?.unwrap_or_default())))
            }),
            Rc::new(move |_, next| {
                b.borrow_mut().push("B");
                Ok(Some(format!("B({})", next()?.unwrap_or_default())))
            }),
        ],
    ));
    let data = clipboard_data(&editor);
    // Stack after merge: [defaultFn, A, B]; B runs first.
    assert_eq!(
        get(&data, "text/plain"),
        Some(format!("B(A({SEED_TEXT}))").as_str())
    );
    assert_eq!(*calls.borrow(), ["B", "A"]);
    editor.dispose();
}

fn custom_payload() -> Option<ExportMimeTypeConfig> {
    stack(
        "application/x-myformat",
        vec![Rc::new(|_, _| Ok(Some("custom-payload".to_owned())))],
    )
}

/// `$exportMimeTypeFromSelection(mimeType)`.
fn export_mime_type(mime_type: &str) -> Option<String> {
    export_mime_type_from_selection(mime_type, get_selection().as_ref()).unwrap()
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > $exportMimeTypeFromSelection > returns the configured stack output for known MIME types
#[test]
fn export_mime_type_returns_the_configured_stack_output() {
    let editor = build_seeded_editor(custom_payload());
    editor
        .read(|| {
            assert_eq!(export_mime_type("text/plain").as_deref(), Some(SEED_TEXT));
            assert_eq!(
                export_mime_type("application/x-myformat").as_deref(),
                Some("custom-payload")
            );
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > $exportMimeTypeFromSelection > returns null for MIME types that have no registered handler
#[test]
fn export_mime_type_returns_null_for_mime_types_with_no_handler() {
    let editor = build_seeded_editor(None);
    editor
        .read(|| assert_eq!(export_mime_type("application/x-unknown"), None))
        .unwrap();
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > $exportMimeTypeFromSelection > uses the default config when GetClipboardDataExtension is not built into the editor
#[test]
fn export_mime_type_uses_the_default_config_without_the_extension() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(initial_editor_state()),
        ..LexicalExtension::<(), (), ()>::new("[test-root]", ())
    })
    .into()])
    .unwrap();
    editor
        .read(|| {
            assert_eq!(export_mime_type("text/plain").as_deref(), Some(SEED_TEXT));
            assert!(export_mime_type("text/html").unwrap().contains(SEED_TEXT));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > setLexicalClipboardDataTransfer wires custom MIME types into the DataTransfer
#[test]
fn set_lexical_clipboard_data_transfer_wires_custom_mime_types() {
    let editor = build_seeded_editor(custom_payload());
    let dt = DataTransfer::new();
    let data = clipboard_data(&editor);
    set_lexical_clipboard_data_transfer(&dt, &data);
    assert_eq!(dt.get_data("text/plain"), SEED_TEXT);
    assert!(dt.get_data("text/html").contains(SEED_TEXT));
    assert_eq!(dt.get_data("application/x-myformat"), "custom-payload");
    editor.dispose();
}
