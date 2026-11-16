//! Ports of `packages/lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts`.
//!
//! JS arms on a triple click's `mousedown` and trims the `selectionchange`
//! that follows. The Rust extension follows a native view, which has synced
//! its selection by the time it reports the click as `CLICK_COMMAND` with
//! the click count as its `detail` (see the extension's docs). So each port
//! makes the overselection, then reports the click; a click outside the
//! editor is a click reported to another editor, as Swift's port has it.
//! The cases about arming, cancelling and timing are skipped
//! (rust/parity/skips.json).

use super::super::support::update;
use super::support::paragraph;
use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions,
    normalize_triple_click_selection_extension,
};
use crate::{
    CLICK_COMMAND, Editor, InitialEditorStateType, LexicalExtension, MouseEvent, PointType, Which,
    define_extension, get_root, get_selection,
};
use std::rc::Rc;

/// `setUpEditor()`.
fn set_up_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            get_root().append(&[paragraph("Paragraph 1"), paragraph("Paragraph 2")])?;
            Ok(())
        }))),
        dependencies: vec![normalize_triple_click_selection_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("normalize-triple-click-selection-test", ())
    })
    .into()])
    .unwrap()
}

/// What a triple click selects: all of paragraph 1 up to the start of
/// paragraph 2 (`overselect`, as the view syncs it).
fn overselect(editor: &Editor) {
    update(editor, || {
        let children = get_root().get_children();
        let text = children[0].get_first_child_or_throw()?;
        let selection = text.select(Some(0), Some(0))?;
        selection.point_set(Which::Focus, children[1].get_key(), 0, PointType::Element)?;
        Ok(())
    });
}

/// The view reports a click of `detail` clicks.
fn click(editor: &Editor, detail: i32) {
    editor
        .dispatch_command(
            &CLICK_COMMAND,
            Rc::new(MouseEvent {
                detail,
                ..MouseEvent::default()
            }),
        )
        .unwrap();
}

/// `readFocus(editor)`: the focus's offset and its node's text.
fn read_focus(editor: &Editor) -> Option<(usize, String)> {
    editor
        .read(|| {
            let range = get_selection()?.range()?;
            let focus = range.point(Which::Focus);
            Some((focus.offset, focus.get_node().unwrap().get_text_content()))
        })
        .unwrap()
}

fn fixed() -> Option<(usize, String)> {
    Some(("Paragraph 1".len(), "Paragraph 1".to_owned()))
}

fn overselected() -> Option<(usize, String)> {
    Some((0, "Paragraph 2".to_owned()))
}

// upstream: lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > trims the selection change that follows a triple click
#[test]
fn trims_the_triple_click() {
    let editor = set_up_editor();
    overselect(&editor);
    click(&editor, 3);
    assert_eq!(read_focus(&editor), fixed());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > trims a triple click on formatted text
#[test]
fn trims_a_triple_click_on_formatted_text() {
    let editor = set_up_editor();
    update(&editor, || {
        for paragraph in get_root().get_children() {
            let text = paragraph.get_first_child_or_throw()?;
            text.set_format_type("bold");
            text.set_style("color: red");
        }
        Ok(())
    });
    overselect(&editor);
    click(&editor, 3);
    assert_eq!(read_focus(&editor), fixed());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > trims after a fourth click
#[test]
fn trims_after_a_fourth_click() {
    let editor = set_up_editor();
    overselect(&editor);
    click(&editor, 4);
    assert_eq!(read_focus(&editor), fixed());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > only trims the first selection change
#[test]
fn only_trims_the_first_selection_change() {
    let editor = set_up_editor();
    overselect(&editor);
    click(&editor, 3);
    update(&editor, || {
        get_root().select_start()?;
        Ok(())
    });
    overselect(&editor);
    assert_eq!(read_focus(&editor), overselected());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > a single or double click cancels it
#[test]
fn a_single_or_double_click_does_not_trim() {
    for detail in [1, 2] {
        let editor = set_up_editor();
        overselect(&editor);
        click(&editor, detail);
        assert_eq!(read_focus(&editor), overselected(), "detail {detail}");
        editor.dispose();
    }
}

// upstream: lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > a triple click outside the editor does not arm it
#[test]
fn a_triple_click_outside_the_editor_does_not_trim() {
    let editor = set_up_editor();
    let other = set_up_editor();
    overselect(&editor);
    click(&other, 3);
    assert_eq!(read_focus(&editor), overselected());
    other.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > does nothing without a triple click
#[test]
fn does_nothing_without_a_triple_click() {
    let editor = set_up_editor();
    overselect(&editor);
    assert_eq!(read_focus(&editor), overselected());
    editor.dispose();
}
