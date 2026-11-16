//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts`.
//! The live region is the `LiveRegion` the root element's body hosts
//! (`lexical_a11y::support`).

#![cfg(feature = "a11y")]

use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::rich_text::{
    HeadingAnnounceExtensionConfigOverrides, HeadingAnnounceOutput, HeadingTagType,
    create_heading_node, heading_announce_extension, rich_text_extension,
};
use crate::upstream_tests::lexical_a11y::support::{Body, mount_root};
use crate::upstream_tests::support::update;
use crate::{
    AnyLexicalExtensionArgument, LexicalExtension, config_extension, create_text_node,
    define_extension, get_root,
};

fn build(dependencies: Vec<AnyLexicalExtensionArgument>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

fn build_rich_text() -> LexicalEditorWithDispose {
    build(vec![rich_text_extension().into()])
}

/// `readLiveRegion()`, with the repeat marker U+200B stripped.
fn read_live_region(body: &Body) -> String {
    body.read_live_region().replace('\u{200B}', "")
}

/// `addHeading(editor, tag, text)`: appends a heading.
fn add_heading(editor: &LexicalEditorWithDispose, tag: HeadingTagType, text: &'static str) {
    update(editor, move || {
        let heading = create_heading_node(tag);
        heading.append(&[create_text_node(text)])?;
        get_root().append(&[heading])
    });
}

/// `removeLastBlock(editor)`.
fn remove_last_block(editor: &LexicalEditorWithDispose) {
    update(editor, || {
        if let Some(last) = get_root().get_last_child() {
            last.remove(false)?;
        }
        Ok(())
    });
}

fn output(editor: &LexicalEditorWithDispose) -> HeadingAnnounceOutput {
    (*get_extension_dependency_from_editor(editor, &heading_announce_extension())
        .unwrap()
        .output
        .expect("HeadingAnnounceExtension has a build"))
    .clone()
}

// upstream: lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > announces every heading level as it is created
#[test]
fn announces_every_heading_level_as_it_is_created() {
    let editor = build_rich_text();
    let body = mount_root(&editor);
    for (tag, level) in [
        (HeadingTagType::H1, 1),
        (HeadingTagType::H2, 2),
        (HeadingTagType::H3, 3),
        (HeadingTagType::H4, 4),
        (HeadingTagType::H5, 5),
        (HeadingTagType::H6, 6),
    ] {
        add_heading(&editor, tag, "Title");
        assert_eq!(read_live_region(&body), format!("Heading level {level}"));
    }
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > announces the level a heading had when it is removed
#[test]
fn announces_the_level_a_heading_had_when_it_is_removed() {
    let editor = build_rich_text();
    let body = mount_root(&editor);
    add_heading(&editor, HeadingTagType::H2, "Title");
    assert_eq!(read_live_region(&body), "Heading level 2");
    remove_last_block(&editor);
    assert_eq!(read_live_region(&body), "Heading level 2 removed");
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > stays silent while editing inside a heading
#[test]
fn stays_silent_while_editing_inside_a_heading() {
    let editor = build_rich_text();
    let body = mount_root(&editor);
    add_heading(&editor, HeadingTagType::H3, "Before");
    assert_eq!(read_live_region(&body), "Heading level 3");
    // JS clears the region's text first; a test cannot write the region
    // here, so it checks that the raw text (where a repeat would add
    // U+200B) is unchanged.
    let before = body.read_live_region();
    update(&editor, || {
        let heading = get_root().get_first_child().unwrap();
        heading.select_end()?.insert_text(" and after")
    });
    assert_eq!(body.read_live_region(), before);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > announces the new level when a heading changes level
#[test]
fn announces_the_new_level_when_a_heading_changes_level() {
    let editor = build_rich_text();
    let body = mount_root(&editor);
    for (tag, expected) in [
        (HeadingTagType::H1, "Heading level 1"),
        (HeadingTagType::H2, "Heading level 2"),
    ] {
        update(&editor, move || {
            let root = get_root();
            root.clear()?;
            root.append(&[create_heading_node(tag)])
        });
        assert_eq!(read_live_region(&body), expected);
    }
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > respects message overrides from configExtension
#[test]
fn respects_message_overrides_from_config_extension() {
    let editor = build(vec![
        config_extension(
            &heading_announce_extension(),
            [HeadingAnnounceExtensionConfigOverrides {
                created: Some("Now an H%s".to_owned()),
                destroyed: Some("H%s gone".to_owned()),
                ..Default::default()
            }],
        ),
        rich_text_extension().into(),
    ]);
    let body = mount_root(&editor);
    add_heading(&editor, HeadingTagType::H4, "Title");
    assert_eq!(read_live_region(&body), "Now an H4");
    remove_last_block(&editor);
    assert_eq!(read_live_region(&body), "H4 gone");
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > reflects message signal changes at runtime
#[test]
fn reflects_message_signal_changes_at_runtime() {
    let editor = build_rich_text();
    let body = mount_root(&editor);
    output(&editor)
        .created
        .set_value("Section, depth %s".to_owned());
    add_heading(&editor, HeadingTagType::H2, "Title");
    assert_eq!(read_live_region(&body), "Section, depth 2");
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > does not announce while disabled, and resumes when re-enabled
#[test]
fn does_not_announce_while_disabled_and_resumes_when_re_enabled() {
    let editor = build_rich_text();
    let body = mount_root(&editor);
    let disabled = output(&editor).disabled;
    disabled.set_value(true);
    add_heading(&editor, HeadingTagType::H1, "Title");
    assert_eq!(read_live_region(&body), "");
    disabled.set_value(false);
    add_heading(&editor, HeadingTagType::H2, "Title");
    assert_eq!(read_live_region(&body), "Heading level 2");
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > leaves a plain text editor alone
#[cfg(feature = "plain-text")]
#[test]
fn leaves_a_plain_text_editor_alone() {
    // The announcer ships with rich text, so a plain text editor never gets
    // it, and @lexical/a11y builds without @lexical/rich-text.
    let editor = build(vec![
        crate::a11y::aria_live_region_extension().into(),
        crate::plain_text::plain_text_extension().into(),
    ]);
    let body = mount_root(&editor);
    update(&editor, || {
        get_root().append(&[crate::create_paragraph_node()])
    });
    assert_eq!(read_live_region(&body), "");
    editor.dispose();
}
