//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts`.
//!
//! JS tells a drag from a paste by the event's class; [`event_files`] takes
//! a [`FilesEvent`] (rust/PARITY.md). A `DataTransfer` reports `"Files"`
//! among its types when it has files, as browsers do, so the upstream
//! types are its data formats plus its files.

use crate::events::{
    ClipboardEvent, DataTransfer, DataTransferFile, DragEvent, KeyboardEvent, PasteCommandType,
};
use crate::utils::{FilesEvent, event_files};
use std::rc::Rc;

fn data_transfer(types: &[&str], files: &[DataTransferFile]) -> Rc<DataTransfer> {
    let data = DataTransfer::new();
    for type_ in types.iter().filter(|t| **t != "Files") {
        data.set_data(type_, "");
    }
    data.set_files(files.to_vec());
    assert_eq!(
        data.types().contains(&"Files".to_owned()),
        types.contains(&"Files")
    );
    Rc::new(data)
}

/// `makeClipboardEvent(types, files)`, as the paste command's payload.
fn make_clipboard_event(types: &[&str], files: &[DataTransferFile]) -> PasteCommandType {
    PasteCommandType::Clipboard(Rc::new(ClipboardEvent::new(
        "paste",
        Some(data_transfer(types, files)),
    )))
}

/// `makeDragEvent(types, files)`.
fn make_drag_event(types: &[&str], files: &[DataTransferFile]) -> DragEvent {
    DragEvent::new("drop", Some(data_transfer(types, files)))
}

fn png_file() -> DataTransferFile {
    DataTransferFile::new("image.png", "image/png", b"img".to_vec())
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > returns [false, [], false] when event has no DataTransfer
#[test]
fn returns_false_empty_false_when_event_has_no_data_transfer() {
    let event = PasteCommandType::Keyboard(Rc::new(KeyboardEvent::default()));
    let (has_files, files, has_content) = event_files(FilesEvent::Paste(&event));
    assert!(!has_files);
    assert!(files.is_empty());
    assert!(!has_content);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > returns hasContent=true for text/html only clipboard (no files)
#[test]
fn returns_has_content_for_text_html_only_clipboard() {
    let event = make_clipboard_event(&["text/html"], &[]);
    let (has_files, files, has_content) = event_files(FilesEvent::Paste(&event));
    assert!(!has_files);
    assert_eq!(files.len(), 0);
    assert!(has_content);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > returns hasContent=true for text/plain only clipboard (no files)
#[test]
fn returns_has_content_for_text_plain_only_clipboard() {
    let event = make_clipboard_event(&["text/plain"], &[]);
    let (has_files, files, has_content) = event_files(FilesEvent::Paste(&event));
    assert!(!has_files);
    assert_eq!(files.len(), 0);
    assert!(has_content);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > returns hasFiles=true and hasContent=false when only Files present
#[test]
fn returns_has_files_and_no_content_when_only_files_present() {
    let event = make_clipboard_event(&["Files"], &[png_file()]);
    let (has_files, files, has_content) = event_files(FilesEvent::Paste(&event));
    assert!(has_files);
    assert_eq!(files.len(), 1);
    assert!(!has_content);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > browser-copied image: Files + text/html, no text/plain > returns hasFiles=true and hasContent=true (text/html present)
#[test]
fn browser_copied_image_returns_has_files_and_has_content() {
    let event = make_clipboard_event(&["Files", "text/html"], &[png_file()]);
    let (has_files, files, has_content) = event_files(FilesEvent::Paste(&event));
    assert!(has_files);
    assert_eq!(files.len(), 1);
    assert!(has_content);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > browser-copied image: Files + text/html, no text/plain > DragEvent with files and text/html behaves the same way
#[test]
fn browser_copied_image_drag_event_behaves_the_same_way() {
    let event = make_drag_event(&["Files", "text/html"], &[png_file()]);
    let (has_files, files, has_content) = event_files(FilesEvent::Drag(&event));
    assert!(has_files);
    assert_eq!(files.len(), 1);
    assert!(has_content);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > Word-copied text: Files + text/html + text/plain > returns hasFiles=true and hasContent=true (all types present)
#[test]
fn word_copied_text_returns_has_files_and_has_content() {
    let event = make_clipboard_event(&["text/plain", "text/html", "Files"], &[png_file()]);
    let (has_files, files, has_content) = event_files(FilesEvent::Paste(&event));
    assert!(has_files);
    assert_eq!(files.len(), 1);
    assert!(has_content);
}
