//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts`.
//!
//! The editors are mounted with [`mount_in_jsdom`], where JS
//! mounts them on a `contenteditable` div. The crate reconciles no DOM into
//! such a root, so the one check of the rendered `textContent` reads the
//! committed document's text, which is what it is reconciled from.

#![cfg(all(
    feature = "rich-text",
    feature = "history",
    feature = "clipboard",
    feature = "html"
))]

use super::super::support::{mount_in_jsdom, update};
use super::support::{as_edited, caret, counter, nth, paragraph, seed, strings};
use crate::clipboard::{get_clipboard_data_from_selection, insert_data_transfer_for_rich_text};
use crate::extension::{
    BlockIdConfigOverrides, LexicalEditorWithDispose, block_id_extension,
    build_editor_from_extensions, get_block_by_id, get_block_id,
};
use crate::rich_text::{HeadingTagType, create_heading_node};
use crate::{
    CONTROLLED_TEXT_INSERTION_COMMAND, DELETE_CHARACTER_COMMAND, DataTransfer, Editor,
    HISTORY_PUSH_TAG, INSERT_PARAGRAPH_COMMAND, InitialEditorStateType, LexicalExtension,
    PointType, UNDO_COMMAND, UpdateOptions, Which, config_extension, create_text_node,
    define_extension, get_root, get_selection,
};

/// `mount(prefix)`.
fn mount(prefix: &str) -> LexicalEditorWithDispose {
    let editor = as_edited(
        build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Null),
            dependencies: vec![
                crate::rich_text::rich_text_extension().into(),
                config_extension(
                    &crate::history::history_extension(),
                    [crate::history::HistoryConfigOverrides {
                        delay: Some(0.0),
                        // JS's editors have a window, where history is on.
                        disabled: Some(false),
                        ..Default::default()
                    }],
                ),
                config_extension(
                    &block_id_extension(),
                    [BlockIdConfigOverrides {
                        generate_id: Some(counter(prefix)),
                        ..Default::default()
                    }],
                ),
            ],
            // Editors that share a namespace paste each other's Lexical JSON.
            namespace: Some("block-id-dom".to_owned()),
            ..LexicalExtension::<(), (), ()>::new("[block-id-dom]", ())
        })
        .into()])
        .unwrap(),
    );
    mount_in_jsdom(&editor);
    editor
}

/// `blocks(editor)`: `type:text:id` of each child of the root.
fn blocks(editor: &Editor) -> Vec<String> {
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|node| {
                    format!(
                        "{}:{}:{}",
                        node.get_type(),
                        node.get_text_content(),
                        get_block_id(node)
                            .unwrap()
                            .unwrap_or_else(|| "null".to_owned())
                    )
                })
                .collect()
        })
        .unwrap()
}

/// `editor.update(fn, {discrete: true, tag})`.
fn update_tag(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), crate::LexicalError> + 'static,
    tag: &str,
) {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                tags: vec![tag.to_owned()],
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > typing, Enter and Backspace in a mounted editor
#[test]
fn typing_enter_and_backspace() {
    let editor = mount("b");
    seed(&editor, &["hello", "world"]);
    let e = editor.editor().clone();
    update(&editor, move || {
        caret(0, 5)?;
        e.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, "!".into())?;
        Ok(())
    });
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:hello!:b1", "paragraph:world:b2"])
    );
    let e = editor.editor().clone();
    update(&editor, move || {
        caret(0, 2)?;
        e.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
        Ok(())
    });
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:he:b1", "paragraph:llo!:b3", "paragraph:world:b2"])
    );
    let e = editor.editor().clone();
    update(&editor, move || {
        caret(2, 0)?;
        e.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
        Ok(())
    });
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:he:b1", "paragraph:llo!world:b3"])
    );
    // The rendered text: the committed document's.
    assert_eq!(
        editor
            .read(|| get_root()
                .get_children()
                .iter()
                .map(|n| n.get_text_content())
                .collect::<String>())
            .unwrap(),
        "hello!world"
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > Backspace at the start of the first heading
#[test]
fn backspace_at_the_start_of_the_first_heading() {
    let editor = mount("b");
    update(&editor, || {
        let title = create_heading_node(HeadingTagType::H2);
        title.append(&[create_text_node("Title")])?;
        get_root().append(&[
            title,
            create_heading_node(HeadingTagType::H3),
            paragraph("after"),
        ])?;
        Ok(())
    });
    let e = editor.editor().clone();
    update(&editor, move || {
        caret(0, 0)?;
        e.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
        Ok(())
    });
    // Nothing to delete before it: unchanged.
    assert_eq!(
        blocks(&editor),
        strings(&["heading:Title:b1", "heading::b2", "paragraph:after:b3"])
    );
    // An empty heading is removed; the other blocks keep their ids.
    let e = editor.editor().clone();
    update(&editor, move || {
        nth(1).select(Some(0), Some(0))?;
        e.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
        Ok(())
    });
    assert_eq!(
        blocks(&editor),
        strings(&["heading:Title:b1", "paragraph:after:b3"])
    );
    editor.dispose();
}

/// `copy(editor, from, to)`.
fn copy(editor: &Editor, from: usize, to: usize) -> Vec<(String, String)> {
    super::super::support::update_with(editor, move || {
        let first = nth(from).get_first_descendant().unwrap();
        let last = nth(to).get_last_descendant().unwrap();
        assert!(first.is_text() && last.is_text());
        let selection = first.select(Some(0), Some(0))?;
        selection.point_set(
            Which::Focus,
            last.get_key(),
            last.get_text_content_size(),
            PointType::Text,
        )?;
        get_clipboard_data_from_selection(get_selection().as_ref())
    })
}

/// `paste(editor, data, $place)`.
fn paste(
    editor: &Editor,
    data: &[(String, String)],
    place: fn() -> Result<(), crate::LexicalError>,
) {
    let transfer = DataTransfer::new();
    for (format, value) in data {
        transfer.set_data(format, value);
    }
    let e = editor.clone();
    update(editor, move || {
        place()?;
        let selection = get_selection().expect("a selection");
        insert_data_transfer_for_rich_text(&transfer, &selection, &e)
    });
}

fn select_end() -> Result<(), crate::LexicalError> {
    get_root().select_end()?;
    Ok(())
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > copy and paste between editors and within one editor
#[test]
fn copy_and_paste_between_editors_and_within_one() {
    let source = mount("s");
    let target = mount("t");
    seed(&source, &["alpha", "beta", "gamma"]);
    seed(&target, &["one", ""]);
    let data = copy(&source, 1, 2);
    let lexical = data
        .iter()
        .find(|(t, _)| t == "application/x-lexical-editor")
        .map(|(_, v)| v.clone())
        .unwrap();
    assert!(lexical.contains(r#""blockId":"s2""#));
    // Into another editor, replacing its empty paragraph: kept.
    paste(&target, &data, || {
        nth(1).select(None, None)?;
        Ok(())
    });
    assert_eq!(
        blocks(&target),
        strings(&[
            "paragraph:one:t1",
            "paragraph:beta:s2",
            "paragraph:gamma:s3"
        ])
    );
    // Again into that editor: now they clash, and the earlier blocks keep
    // the ids.
    paste(&target, &data, select_end);
    assert_eq!(
        blocks(&target),
        strings(&[
            "paragraph:one:t1",
            "paragraph:beta:s2",
            "paragraph:gammabeta:s3",
            "paragraph:gamma:t3"
        ])
    );
    // Into the source editor, below the originals: fresh ids.
    paste(&source, &data, select_end);
    assert_eq!(
        blocks(&source),
        strings(&[
            "paragraph:alpha:s1",
            "paragraph:beta:s2",
            "paragraph:gammabeta:s3",
            "paragraph:gamma:s4"
        ])
    );
    target.dispose();
    source.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > HTML pasted through the legacy importer gets fresh ids
#[test]
fn html_pasted_through_the_legacy_importer_gets_fresh_ids() {
    let editor = mount("b");
    seed(&editor, &["one"]);
    paste(
        &editor,
        &[(
            "text/html".to_owned(),
            r#"<p data-block-id="b1">pasted a</p><p data-block-id="b1">pasted b</p>"#.to_owned(),
        )],
        select_end,
    );
    let result = blocks(&editor);
    assert_eq!(result[0], "paragraph:onepasted a:b1");
    assert_eq!(result[1..], strings(&["paragraph:pasted b:b2"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > undo restores ids and the index
#[test]
fn undo_restores_ids_and_the_index() {
    let editor = mount("b");
    seed(&editor, &["one", "two"]);
    let before = blocks(&editor);
    let e = editor.editor().clone();
    update_tag(
        &editor,
        move || {
            caret(0, 1)?;
            e.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
            Ok(())
        },
        HISTORY_PUSH_TAG,
    );
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:o:b1", "paragraph:ne:b3", "paragraph:two:b2"])
    );
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(blocks(&editor), before);
    editor
        .read(|| {
            assert!(get_block_by_id("b3").unwrap().is_none());
            assert_eq!(
                get_block_by_id("b1").unwrap().unwrap().get_text_content(),
                "one"
            );
        })
        .unwrap();
    editor.dispose();
}
