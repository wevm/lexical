//! Ports of
//! `packages/lexical-a11y/src/__tests__/unit/EditorModeAnnounceExtension.test.ts`.

use super::support::mount_root;
use crate::a11y::{EditorModeAnnounceExtensionConfigOverrides, editor_mode_announce_extension};
use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::rich_text::rich_text_extension;
use crate::{AnyLexicalExtensionArgument, LexicalExtension, config_extension, define_extension};

fn build(announce: AnyLexicalExtensionArgument) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![announce, rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

fn build_default() -> LexicalEditorWithDispose {
    build(editor_mode_announce_extension().into())
}

fn output(editor: &LexicalEditorWithDispose) -> crate::a11y::EditorModeAnnounceOutput {
    (*get_extension_dependency_from_editor(editor, &editor_mode_announce_extension())
        .unwrap()
        .output
        .expect("EditorModeAnnounceExtension has a build"))
    .clone()
}

// upstream: lexical-a11y/src/__tests__/unit/EditorModeAnnounceExtension.test.ts::EditorModeAnnounceExtension > announces the default editable / read-only messages on the dependency sink
#[test]
fn announces_the_default_editable_read_only_messages() {
    let editor = build_default();
    let body = mount_root(&editor);
    editor.set_editable(false);
    assert_eq!(body.read_live_region(), "Editor is read-only");
    editor.set_editable(true);
    assert_eq!(body.read_live_region(), "Editor is editable");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/EditorModeAnnounceExtension.test.ts::EditorModeAnnounceExtension > respects message overrides from configExtension
#[test]
fn respects_message_overrides_from_config_extension() {
    let editor = build(config_extension(
        &editor_mode_announce_extension(),
        [EditorModeAnnounceExtensionConfigOverrides {
            editable: Some("Now editing".to_owned()),
            read_only: Some("Locked".to_owned()),
            ..Default::default()
        }],
    ));
    let body = mount_root(&editor);
    editor.set_editable(false);
    assert_eq!(body.read_live_region(), "Locked");
    editor.set_editable(true);
    assert_eq!(body.read_live_region(), "Now editing");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/EditorModeAnnounceExtension.test.ts::EditorModeAnnounceExtension > reflects message signal changes at runtime
#[test]
fn reflects_message_signal_changes_at_runtime() {
    let editor = build_default();
    let body = mount_root(&editor);
    let output = output(&editor);
    output.editable.set_value("On".to_owned());
    output.read_only.set_value("Off".to_owned());
    editor.set_editable(false);
    assert_eq!(body.read_live_region(), "Off");
    editor.set_editable(true);
    assert_eq!(body.read_live_region(), "On");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/EditorModeAnnounceExtension.test.ts::EditorModeAnnounceExtension > does not announce while disabled, and resumes when re-enabled
#[test]
fn does_not_announce_while_disabled_and_resumes_when_re_enabled() {
    let editor = build_default();
    let body = mount_root(&editor);
    let disabled = output(&editor).disabled;
    disabled.set_value(true);
    editor.set_editable(false);
    assert_eq!(body.read_live_region(), "");
    disabled.set_value(false);
    editor.set_editable(true);
    assert_eq!(body.read_live_region(), "Editor is editable");
    editor.dispose();
}
