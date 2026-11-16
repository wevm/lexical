//! Ports of
//! `packages/lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts`.

use super::support::mount_root;
use crate::a11y::{HistoryAnnounceExtensionConfigOverrides, history_announce_extension};
use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::rich_text::rich_text_extension;
use crate::{
    AnyLexicalExtensionArgument, LexicalExtension, REDO_COMMAND, UNDO_COMMAND, config_extension,
    define_extension,
};
use std::cell::Cell;
use std::rc::Rc;

fn build(announce: AnyLexicalExtensionArgument) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![announce, rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

fn build_default() -> LexicalEditorWithDispose {
    build(history_announce_extension().into())
}

fn output(editor: &LexicalEditorWithDispose) -> crate::a11y::HistoryAnnounceOutput {
    (*get_extension_dependency_from_editor(editor, &history_announce_extension())
        .unwrap()
        .output
        .expect("HistoryAnnounceExtension has a build"))
    .clone()
}

// upstream: lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > announces the default Undone / Redone messages on the dependency sink
#[test]
fn announces_the_default_undone_redone_messages() {
    let editor = build_default();
    let body = mount_root(&editor);
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(body.read_live_region(), "Undone");
    editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
    assert_eq!(body.read_live_region(), "Redone");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > respects message overrides from configExtension
#[test]
fn respects_message_overrides_from_config_extension() {
    let editor = build(config_extension(
        &history_announce_extension(),
        [HistoryAnnounceExtensionConfigOverrides {
            redone: Some("Custom redo".to_owned()),
            undone: Some("Custom undo".to_owned()),
            ..Default::default()
        }],
    ));
    let body = mount_root(&editor);
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(body.read_live_region(), "Custom undo");
    editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
    assert_eq!(body.read_live_region(), "Custom redo");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > reflects message signal changes at runtime
#[test]
fn reflects_message_signal_changes_at_runtime() {
    let editor = build_default();
    let body = mount_root(&editor);
    let output = output(&editor);
    output.undone.set_value("Reverted".to_owned());
    output.redone.set_value("Restored".to_owned());
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(body.read_live_region(), "Reverted");
    editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
    assert_eq!(body.read_live_region(), "Restored");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > does not announce while disabled, and resumes when re-enabled
#[test]
fn does_not_announce_while_disabled_and_resumes_when_re_enabled() {
    let editor = build_default();
    let body = mount_root(&editor);
    let disabled = output(&editor).disabled;
    disabled.set_value(true);
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(body.read_live_region(), "");
    disabled.set_value(false);
    editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
    assert_eq!(body.read_live_region(), "Redone");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > keeps the history command chain intact (returns false)
#[test]
fn keeps_the_history_command_chain_intact() {
    let editor = build_default();
    mount_root(&editor);
    // A handler registered after HistoryAnnounce at the same priority still
    // receives UNDO_COMMAND, because HistoryAnnounce returns false.
    let reached = Rc::new(Cell::new(false));
    let flag = reached.clone();
    let _unregister = editor.register_command(
        &UNDO_COMMAND,
        move |_, _| {
            flag.set(true);
            Ok(false)
        },
        1,
    );
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert!(reached.get());
    editor.dispose();
}
