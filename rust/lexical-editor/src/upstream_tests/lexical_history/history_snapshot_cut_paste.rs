//! Ports of `packages/lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts`.
//!
//! Upstream awaits a timer before reading what its update listener heard,
//! for the microtask that commits the handlers' updates and the rich-text
//! cut handler's await of `copyToClipboard`: [`flush`].

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::history::{HistoryConfigOverrides, HistoryNow, history_extension};
use crate::{
    AnyLexicalExtensionArgument, CUT_COMMAND, CUT_TAG, ClipboardEvent, ClipboardOrKeyboardEvent,
    DataTransfer, Editor, InitialEditorStateType, LexicalError, LexicalExtension, PASTE_COMMAND,
    PASTE_TAG, PasteCommandType, UNDO_COMMAND, UpdateOptions, config_extension,
    create_paragraph_node, define_extension, get_root, get_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

// Use a constant clock so that every change falls inside the merge window
// (delay). Without the fix, a clipboard operation the heuristic treats as
// mergeable (an in-place single-character insert) would collapse into the
// surrounding run of typing; @lexical/history now classifies PASTE_TAG/CUT_TAG
// updates as their own undo boundary instead (see #8609).
fn create_editor(text_extension: Option<AnyLexicalExtensionArgument>) -> LexicalEditorWithDispose {
    let mut dependencies: Vec<AnyLexicalExtensionArgument> = text_extension.into_iter().collect();
    dependencies.push(config_extension(
        &history_extension(),
        [HistoryConfigOverrides {
            delay: Some(1000.0),
            now: Some(HistoryNow(Rc::new(|| 0.0))),
            ..Default::default()
        }],
    ));
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let paragraph = create_paragraph_node();
            get_root().append(std::slice::from_ref(&paragraph))?;
            paragraph.select_end()?;
            Ok(())
        }))),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new("test", ())
    })
    .into()])
    .unwrap()
}

/// `$type(text)`.
fn type_text(text: &str) -> Result<(), LexicalError> {
    if let Some(selection) = get_selection().filter(|s| s.is_range()) {
        selection.insert_text(text)?;
    }
    Ok(())
}

fn update_tagged(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
    tag: Option<&str>,
) {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                tags: tag.into_iter().map(str::to_owned).collect(),
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

fn text_content(editor: &Editor) -> String {
    editor.read(|| get_root().get_text_content()).unwrap()
}

/// `await flush()`: commits what is pending.
fn flush(editor: &Editor) {
    crate::updates::commit_pending_updates(editor).unwrap();
}

/// `clipboardEvent(type, text)`.
fn clipboard_event(type_: &str, text: &str) -> Rc<ClipboardEvent> {
    let clipboard_data = DataTransfer::new();
    clipboard_data.set_data("text/plain", text);
    Rc::new(ClipboardEvent::new(type_, Some(Rc::new(clipboard_data))))
}

// Recognition lives in @lexical/history: any update tagged PASTE_TAG or CUT_TAG
// is classified as `OTHER`, which both keeps it from merging into the preceding
// keystrokes and (by becoming `prevChangeType`) keeps the following keystrokes
// from merging into it. These drive editor.update directly with the tag so they
// exercise the history logic independent of the clipboard handlers.
// upstream: lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::@lexical/history treats PASTE_TAG as an undo boundary (#8609) > a tagged single-character change is isolated between keystrokes
// upstream: lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::@lexical/history treats CUT_TAG as an undo boundary (#8609) > a tagged single-character change is isolated between keystrokes
#[test]
fn a_tagged_single_character_change_is_isolated_between_keystrokes() {
    for tag in [PASTE_TAG, CUT_TAG] {
        let editor = create_editor(None);

        // Type three characters that merge into a single history entry.
        update_tagged(&editor, || type_text("a"), None);
        update_tagged(&editor, || type_text("b"), None);
        update_tagged(&editor, || type_text("c"), None);

        // An in-place single-character insert would normally merge into the run of
        // typing; the tag forces it into its own entry.
        update_tagged(&editor, || type_text("X"), Some(tag));

        // Following keystrokes do not merge into the tagged entry either.
        update_tagged(&editor, || type_text("d"), None);

        // Undo removes only the trailing keystroke...
        editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
        assert_eq!(text_content(&editor), "abcX", "{tag}");

        // ...then the tagged change, leaving the earlier typing intact.
        editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
        assert_eq!(text_content(&editor), "abc", "{tag}");
    }
}

/// The tags of each update the editor commits while `f` runs, one entry per
/// update (upstream's `vi.fn()` update listener).
fn tags_of_updates(editor: &Editor, f: impl FnOnce()) -> Vec<Vec<String>> {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let heard = calls.clone();
    let unregister = editor
        .register_update_listener(move |payload| heard.borrow_mut().push(payload.tags.clone()));
    f();
    flush(editor);
    unregister();
    calls.take()
}

/// `paste tags the update with PASTE_TAG`, for a text extension.
#[cfg(feature = "clipboard")]
fn paste_tags_the_update_with_paste_tag(text_extension: AnyLexicalExtensionArgument) {
    let editor = create_editor(Some(text_extension));
    let calls = tags_of_updates(&editor, || {
        editor
            .dispatch_command(
                &PASTE_COMMAND,
                PasteCommandType::Clipboard(clipboard_event("paste", "X")),
            )
            .unwrap();
    });
    let paste_tagged = calls.iter().any(|tags| tags.iter().any(|t| t == PASTE_TAG));
    assert!(paste_tagged);
}

/// `cut tags the update with CUT_TAG`, for a text extension.
#[cfg(feature = "clipboard")]
fn cut_tags_the_update_with_cut_tag(text_extension: AnyLexicalExtensionArgument) {
    let editor = create_editor(Some(text_extension));
    let calls = tags_of_updates(&editor, || {
        update_tagged(&editor, || type_text("abc"), None);
        update_tagged(
            &editor,
            || {
                get_root().get_all_text_nodes()[0].select(Some(1), Some(3))?;
                Ok(())
            },
            None,
        );
        editor
            .dispatch_command(
                &CUT_COMMAND,
                Some(ClipboardOrKeyboardEvent::Clipboard(clipboard_event(
                    "cut", "",
                ))),
            )
            .unwrap();
    });
    let cut_tagged = calls.iter().any(|tags| tags.iter().any(|t| t == CUT_TAG));
    assert!(cut_tagged);
}

// upstream: lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::rich text clipboard handlers tag their updates (#8609) > paste tags the update with PASTE_TAG
#[cfg(feature = "rich-text")]
#[test]
fn rich_text_paste_tags_the_update_with_paste_tag() {
    paste_tags_the_update_with_paste_tag(crate::rich_text::rich_text_extension().into());
}

// upstream: lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::rich text clipboard handlers tag their updates (#8609) > cut tags the update with CUT_TAG
#[cfg(feature = "rich-text")]
#[test]
fn rich_text_cut_tags_the_update_with_cut_tag() {
    cut_tags_the_update_with_cut_tag(crate::rich_text::rich_text_extension().into());
}

// upstream: lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::plain text clipboard handlers tag their updates (#8609) > paste tags the update with PASTE_TAG
#[cfg(feature = "plain-text")]
#[test]
fn plain_text_paste_tags_the_update_with_paste_tag() {
    paste_tags_the_update_with_paste_tag(crate::plain_text::plain_text_extension().into());
}

// upstream: lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::plain text clipboard handlers tag their updates (#8609) > cut tags the update with CUT_TAG
#[cfg(feature = "plain-text")]
#[test]
fn plain_text_cut_tags_the_update_with_cut_tag() {
    cut_tags_the_update_with_cut_tag(crate::plain_text::plain_text_extension().into());
}

// End-to-end coverage of the user-facing bug from #8609: undoing a paste should
// not also undo the typing before it. This is only *observable* for a short
// rich-text paste, which inserts in place (RangeSelection.insertText) and would
// otherwise merge into the preceding keystrokes; cut and plain-text paste
// produce an `OTHER` change the heuristic already isolates. Removing the
// PASTE_TAG recognition in @lexical/history makes this test fail.
// upstream: lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::rich-text paste is isolated from the preceding typing (#8609) > a single undo after a paste keeps the text typed before it
#[cfg(feature = "rich-text")]
#[test]
fn a_single_undo_after_a_paste_keeps_the_text_typed_before_it() {
    let editor = create_editor(Some(crate::rich_text::rich_text_extension().into()));

    update_tagged(&editor, || type_text("a"), None);
    update_tagged(&editor, || type_text("b"), None);
    update_tagged(&editor, || type_text("c"), None);

    editor
        .dispatch_command(
            &PASTE_COMMAND,
            PasteCommandType::Clipboard(clipboard_event("paste", "X")),
        )
        .unwrap();
    assert_eq!(text_content(&editor), "abcX");

    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(text_content(&editor), "abc");
}
