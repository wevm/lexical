//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextDropFiles.test.ts`.
//! JS mocks `document.caretRangeFromPoint`; a Rust editor asks its root
//! element for the caret at the drop point
//! ([`RootElement::caret_from_point`](crate::RootElement::caret_from_point)),
//! so the root element here returns the caret that range stands for. With
//! no root element there is no caret, as with no `caretRangeFromPoint`.

#![cfg(feature = "extension")]

use crate::caret::{CaretDirection, PointCaret, get_text_point_caret};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::rich_text::{DRAG_DROP_PASTE, rich_text_extension};
use crate::upstream_tests::support::update;
use crate::{
    COMMAND_PRIORITY_CRITICAL, DROP_COMMAND, DataTransfer, DataTransferFile, DragEvent,
    InitialEditorStateType, LexicalExtension, MouseEvent, RootElement, RootElementRef,
    create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

fn build(name: &'static str, initial: Option<InitialEditorStateType>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: initial,
        dependencies: vec![rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn fake_image() -> DataTransferFile {
    DataTransferFile::new("photo.png", "image/png", b"fake-bytes".to_vec())
}

/// `createDropEventWithFiles(files)`: a drop at (10, 10).
fn create_drop_event_with_files(files: Vec<DataTransferFile>) -> Rc<DragEvent> {
    let data_transfer = DataTransfer::new();
    data_transfer.set_files(files);
    Rc::new(DragEvent {
        mouse: MouseEvent {
            client_x: 10.0,
            client_y: 10.0,
            ..MouseEvent::new("drop")
        },
        data_transfer: Some(Rc::new(data_transfer)),
    })
}

/// `dispatchDropAndCaptureFiles(editor, event)`: the names of the files
/// `DRAG_DROP_PASTE` was dispatched with.
fn dispatch_drop_and_capture_files(
    editor: &LexicalEditorWithDispose,
    event: Rc<DragEvent>,
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
    editor.dispatch_command(&DROP_COMMAND, event).unwrap();
    dispose();
    dispatched.take()
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextDropFiles.test.ts::RichTextExtension DROP_COMMAND file handling > unhandled desktop text files are still consumed
#[test]
fn unhandled_desktop_text_files_are_still_consumed() {
    let editor = build("test-desktop-text-file-drop", None);
    let event = create_drop_event_with_files(vec![DataTransferFile::new(
        "document.txt",
        "text/plain",
        b"text".to_vec(),
    )]);
    assert!(
        editor
            .dispatch_command(&DROP_COMMAND, event.clone())
            .unwrap()
    );
    assert!(event.default_prevented());
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextDropFiles.test.ts::RichTextExtension DROP_COMMAND file handling > dropped files are forwarded even when the drop point resolves to no caret
#[test]
fn dropped_files_are_forwarded_even_when_the_drop_point_resolves_to_no_caret() {
    let editor = build("test-drop-no-caret", None);
    let files =
        dispatch_drop_and_capture_files(&editor, create_drop_event_with_files(vec![fake_image()]));
    assert_eq!(files, Some(vec!["photo.png".to_owned()]));
    editor.dispose();
}

/// The mounted root element, whose `caretFromPoint` is offset 1 of the
/// first text node.
struct CaretRootElement;

impl RootElement for CaretRootElement {
    fn caret_from_point(&self, _x: f64, _y: f64) -> Option<PointCaret> {
        let text = get_root().get_first_descendant()?;
        get_text_point_caret(&text, CaretDirection::Next, 1).ok()
    }
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextDropFiles.test.ts::RichTextExtension DROP_COMMAND file handling > dropped files are forwarded when the drop point does resolve to a caret
#[test]
fn dropped_files_are_forwarded_when_the_drop_point_does_resolve_to_a_caret() {
    let editor = build(
        "test-drop-with-caret",
        Some(InitialEditorStateType::Update(Rc::new(|_| {
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("hello")])?;
            get_root().append(&[paragraph])
        }))),
    );
    editor
        .set_root_element(Some(RootElementRef(Rc::new(CaretRootElement))))
        .unwrap();
    update(&editor, || Ok(()));
    let files =
        dispatch_drop_and_capture_files(&editor, create_drop_event_with_files(vec![fake_image()]));
    assert_eq!(files, Some(vec!["photo.png".to_owned()]));
    editor.set_root_element(None).unwrap();
    editor.dispose();
}
