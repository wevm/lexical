//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts`.

#![cfg(feature = "extension")]

use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::rich_text::{
    DRAG_DROP_PASTE, RichTextConfigOverrides, ShouldHandlePasteAsFiles, rich_text_extension,
};
use crate::{
    AnyLexicalExtensionArgument, COMMAND_PRIORITY_CRITICAL, ClipboardEvent, DataTransfer,
    DataTransferFile, InitialEditorStateType, LexicalExtension, PASTE_COMMAND, PasteCommandType,
    config_extension, define_extension,
};
use std::cell::RefCell;
use std::rc::Rc;

fn build(
    name: &'static str,
    rich_text: AnyLexicalExtensionArgument,
    initial: Option<InitialEditorStateType>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: initial,
        dependencies: vec![rich_text],
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `(files) => files.length > 0`.
fn files_win() -> ShouldHandlePasteAsFiles {
    ShouldHandlePasteAsFiles(Rc::new(|files, _| !files.is_empty()))
}

fn overridden() -> AnyLexicalExtensionArgument {
    config_extension(
        &rich_text_extension(),
        [RichTextConfigOverrides {
            should_handle_paste_as_files: Some(files_win()),
            ..RichTextConfigOverrides::default()
        }],
    )
}

fn fake_image() -> DataTransferFile {
    DataTransferFile::new("photo.png", "image/png", b"fake-bytes".to_vec())
}

/// `createDataTransferWithFiles(textEntries, files)`.
fn create_data_transfer_with_files(
    text_entries: &[(&str, &str)],
    files: Vec<DataTransferFile>,
) -> Rc<DataTransfer> {
    let data_transfer = DataTransfer::new();
    for (type_, value) in text_entries {
        data_transfer.set_data(type_, value);
    }
    data_transfer.set_files(files);
    Rc::new(data_transfer)
}

fn paste(editor: &LexicalEditorWithDispose, data_transfer: Rc<DataTransfer>) {
    editor
        .dispatch_command(
            &PASTE_COMMAND,
            PasteCommandType::Clipboard(Rc::new(ClipboardEvent::new("paste", Some(data_transfer)))),
        )
        .unwrap();
}

/// `dispatchPasteAndCaptureFiles(editor, dataTransfer)`: the names of the
/// files `DRAG_DROP_PASTE` was dispatched with.
fn dispatch_paste_and_capture_files(
    editor: &LexicalEditorWithDispose,
    data_transfer: Rc<DataTransfer>,
) -> Option<Vec<String>> {
    let dispatched: Rc<RefCell<Option<Vec<String>>>> = Rc::default();
    let captured = dispatched.clone();
    let dispose = editor.register_command(
        &DRAG_DROP_PASTE,
        move |files, _| {
            *captured.borrow_mut() = Some(files.iter().map(|f| f.name.clone()).collect());
            Ok(true)
        },
        COMMAND_PRIORITY_CRITICAL,
    );
    paste(editor, data_transfer);
    dispose();
    dispatched.take()
}

// upstream: lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > default: a file pasted alongside text/html (browser copy-image) does NOT dispatch DRAG_DROP_PASTE
#[test]
fn default_a_file_pasted_alongside_text_html_does_not_dispatch_drag_drop_paste() {
    let editor = build("test-default", rich_text_extension().into(), None);
    let data_transfer = create_data_transfer_with_files(
        &[("text/html", "<img src=\"blob:fake\">")],
        vec![fake_image()],
    );
    assert_eq!(
        dispatch_paste_and_capture_files(&editor, data_transfer),
        None
    );
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > override: treating only text/plain as content lets the file win over an incidental text/html entry
#[test]
fn override_lets_the_file_win_over_an_incidental_text_html_entry() {
    let editor = build("test-override", overridden(), None);
    let data_transfer = create_data_transfer_with_files(
        &[("text/html", "<img src=\"blob:fake\">")],
        vec![fake_image()],
    );
    assert_eq!(
        dispatch_paste_and_capture_files(&editor, data_transfer),
        Some(vec!["photo.png".to_owned()])
    );
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > override falls back to HTML when DRAG_DROP_PASTE is not handled
#[cfg(feature = "html")]
#[test]
fn override_falls_back_to_html_when_drag_drop_paste_is_not_handled() {
    use crate::{create_paragraph_node, get_root};
    let editor = build(
        "test-unhandled-file",
        overridden(),
        Some(InitialEditorStateType::Update(Rc::new(|_| {
            let paragraph = create_paragraph_node();
            get_root().append(std::slice::from_ref(&paragraph))?;
            paragraph.select_end()?;
            Ok(())
        }))),
    );
    let data_transfer = create_data_transfer_with_files(
        &[("text/html", "<p>HTML fallback</p>")],
        vec![fake_image()],
    );
    crate::upstream_tests::support::update(&editor, || {
        get_root().get_first_child().unwrap().select_end()?;
        Ok(())
    });
    paste(&editor, data_transfer);
    assert_eq!(
        editor.read(|| get_root().get_text_content()).unwrap(),
        "HTML fallback"
    );
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > override does not hijack a plain text/html paste with no file
#[test]
fn override_does_not_hijack_a_plain_text_html_paste_with_no_file() {
    let editor = build(
        "test-no-file",
        overridden(),
        Some(InitialEditorStateType::Update(Rc::new(|_| Ok(())))),
    );
    let data_transfer = create_data_transfer_with_files(&[("text/html", "<b>hello</b>")], vec![]);
    assert_eq!(
        dispatch_paste_and_capture_files(&editor, data_transfer),
        None
    );
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > a file with no text content at all still dispatches DRAG_DROP_PASTE under the default config
#[test]
fn a_file_with_no_text_content_still_dispatches_drag_drop_paste_by_default() {
    let editor = build("test-file-only", rich_text_extension().into(), None);
    let data_transfer = create_data_transfer_with_files(&[], vec![fake_image()]);
    assert_eq!(
        dispatch_paste_and_capture_files(&editor, data_transfer),
        Some(vec!["photo.png".to_owned()])
    );
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > shouldHandlePasteAsFiles can be modified as a signal after the editor is built
#[test]
fn should_handle_paste_as_files_can_be_modified_as_a_signal_after_the_editor_is_built() {
    let editor = build("test-signal", rich_text_extension().into(), None);
    get_extension_dependency_from_editor(&editor, &rich_text_extension())
        .unwrap()
        .output
        .unwrap()
        .should_handle_paste_as_files
        .set_value(files_win());
    let data_transfer = create_data_transfer_with_files(
        &[("text/html", "<img src=\"blob:fake\">")],
        vec![fake_image()],
    );
    assert!(dispatch_paste_and_capture_files(&editor, data_transfer).is_some());
    editor.dispose();
}
