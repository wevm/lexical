//! Ports of
//! `packages/lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts`.
//!
//! JS mounts the region in the root element's `ownerDocument.body`; a Rust
//! root element names that host ([`RootElement::live_region_host`]), so a
//! [`Body`] stands for a document's body and a root mounted in it. The
//! iframe test is skipped in `rust/parity/skips.json`.
//! `using` becomes an explicit `dispose()`.
//!
//! [`RootElement::live_region_host`]: crate::RootElement::live_region_host

use super::support::{Body, mount_root, mount_root_in};
use crate::a11y::{
    AriaLiveRegionExtensionConfigOverrides, AriaPoliteness, LiveRegionHostRef,
    aria_live_region_extension,
};
use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::rich_text::rich_text_extension;
use crate::{AnyLexicalExtensionArgument, LexicalExtension, config_extension, define_extension};
use std::rc::Rc;

fn build(live_region: AnyLexicalExtensionArgument) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![live_region, rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

fn build_default() -> LexicalEditorWithDispose {
    build(aria_live_region_extension().into())
}

fn output(editor: &LexicalEditorWithDispose) -> crate::a11y::AriaLiveRegion {
    (*get_extension_dependency_from_editor(editor, &aria_live_region_extension())
        .unwrap()
        .output
        .expect("AriaLiveRegionExtension has a build"))
    .clone()
}

// upstream: lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > mounts a polite live region on the root document by default
#[test]
fn mounts_a_polite_live_region_on_the_root_document_by_default() {
    let editor = build_default();
    let body = mount_root(&editor);
    let regions = body.regions();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].politeness(), Some(AriaPoliteness::Polite));
    assert!(regions[0].atomic());
    assert_eq!(regions[0].role(), "status");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > disposes the live region when the editor is disposed
#[test]
fn disposes_the_live_region_when_the_editor_is_disposed() {
    let editor = build_default();
    let body = mount_root(&editor);
    assert_eq!(body.regions().len(), 1);
    editor.dispose();
    assert_eq!(body.regions().len(), 0);
}

// upstream: lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > respects assertive politeness from config
#[test]
fn respects_assertive_politeness_from_config() {
    let editor = build(config_extension(
        &aria_live_region_extension(),
        [AriaLiveRegionExtensionConfigOverrides {
            politeness: Some(AriaPoliteness::Assertive),
            ..Default::default()
        }],
    ));
    let body = mount_root(&editor);
    assert_eq!(
        body.region().unwrap().politeness(),
        Some(AriaPoliteness::Assertive)
    );
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > reflects politeness changes from the output signal at runtime
#[test]
fn reflects_politeness_changes_from_the_output_signal_at_runtime() {
    let editor = build_default();
    let body = mount_root(&editor);
    let region = body.region().unwrap();
    assert_eq!(region.politeness(), Some(AriaPoliteness::Polite));
    output(&editor)
        .politeness
        .set_value(AriaPoliteness::Assertive);
    assert_eq!(region.politeness(), Some(AriaPoliteness::Assertive));
    editor.dispose();
}

fn with_owner(owner: &Rc<Body>) -> LexicalEditorWithDispose {
    build(config_extension(
        &aria_live_region_extension(),
        [AriaLiveRegionExtensionConfigOverrides {
            owner: Some(Some(LiveRegionHostRef(owner.clone()))),
            ..Default::default()
        }],
    ))
}

// upstream: lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > mounts onto a custom owner element regardless of the root
#[test]
fn mounts_onto_a_custom_owner_element_regardless_of_the_root() {
    let owner = Body::new();
    let editor = with_owner(&owner);
    assert!(owner.region().is_some());
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > exposes a stable announce sink via dependency output
#[test]
fn exposes_a_stable_announce_sink_via_dependency_output() {
    let editor = build_default();
    let body = mount_root(&editor);
    let announce = output(&editor);
    let region = body.region().unwrap();
    announce.announce("hello");
    assert_eq!(region.text_content(), "hello");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > does not replay the last message into a region re-created on remount
#[test]
fn does_not_replay_the_last_message_into_a_region_re_created_on_remount() {
    let editor = build_default();
    let announce = output(&editor);
    let body = Body::new();
    mount_root_in(&editor, &body);
    announce.announce("Saved");
    assert_eq!(body.read_live_region(), "Saved");
    // Remount the editor into a fresh root (unmount, then re-mount).
    editor.set_root_element(None).unwrap();
    mount_root_in(&editor, &body);
    // The freshly created region must start empty.
    let region = body.region().unwrap();
    assert_eq!(region.text_content(), "");
    // A genuinely new announcement still lands on the remounted region.
    announce.announce("Loaded");
    assert_eq!(region.text_content(), "Loaded");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > announcing before a region is mounted is a no-op, not buffered for mount
#[test]
fn announcing_before_a_region_is_mounted_is_a_no_op() {
    let editor = build_default();
    let announce = output(&editor);
    let body = Body::new();
    // No root yet, so no region: the announcement is dropped.
    announce.announce("Early");
    assert!(body.region().is_none());
    mount_root_in(&editor, &body);
    let region = body.region().unwrap();
    assert_eq!(region.text_content(), "");
    announce.announce("Ready");
    assert_eq!(region.text_content(), "Ready");
    editor.dispose();
}

// upstream: lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > keeps a custom-owner region across editor root changes (no churn)
#[test]
fn keeps_a_custom_owner_region_across_editor_root_changes() {
    let owner = Body::new();
    let editor = with_owner(&owner);
    let region = owner.region().expect("a region");
    // Mount then unmount a root; the owner-hosted region must not be torn
    // down and rebuilt.
    mount_root(&editor);
    editor.set_root_element(None).unwrap();
    assert!(Rc::ptr_eq(&owner.region().unwrap(), &region));
    assert_eq!(owner.regions().len(), 1);
    editor.dispose();
}
