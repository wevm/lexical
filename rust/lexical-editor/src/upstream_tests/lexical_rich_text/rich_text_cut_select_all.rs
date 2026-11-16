//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts`.
//! JS's cut removes the selection after the copy's promise settles; Rust
//! queues that update with the others, so `editor.read` (which flushes
//! pending updates) stands for `await flush()`.

#![cfg(all(feature = "extension", feature = "list"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::list::{ListType, create_list_item_node, create_list_node, list_extension};
use crate::rich_text::{HeadingTagType, create_heading_node, rich_text_extension};
use crate::selection_insert_nodes::select_all;
use crate::upstream_tests::support::{set_test_root_element, update};
use crate::{
    CUT_COMMAND, ClipboardEvent, ClipboardOrKeyboardEvent, DataTransfer, LexicalError,
    LexicalExtension, NodeRef, PASTE_COMMAND, PasteCommandType, create_text_node, define_extension,
    get_root,
};
use std::rc::Rc;

fn create_editor() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![rich_text_extension().into(), list_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[5835-cut]", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

/// The document as nested `[type, children]` pairs, with text spelled out.
fn outline(node: &NodeRef) -> String {
    if node.is_element() {
        let children: Vec<String> = node.get_children().iter().map(outline).collect();
        format!("[{}, [{}]]", node.get_type(), children.join(", "))
    } else {
        format!("{}:{}", node.get_type(), node.get_text_content())
    }
}

fn read_outline(editor: &LexicalEditorWithDispose) -> String {
    editor
        .read(|| {
            let children: Vec<String> = get_root().get_children().iter().map(outline).collect();
            format!("[{}]", children.join(", "))
        })
        .unwrap()
}

fn cut(editor: &LexicalEditorWithDispose, data: &Rc<DataTransfer>) {
    editor
        .dispatch_command(
            &CUT_COMMAND,
            Some(ClipboardOrKeyboardEvent::Clipboard(Rc::new(
                ClipboardEvent::new("cut", Some(data.clone())),
            ))),
        )
        .unwrap();
}

fn heading_hi() -> Result<NodeRef, LexicalError> {
    let heading = create_heading_node(HeadingTagType::H1);
    heading.append(&[create_text_node("hi")])?;
    Ok(heading)
}

fn list_of(items: &[&str]) -> Result<NodeRef, LexicalError> {
    let list = create_list_node(ListType::Bullet, 1.0);
    for text in items {
        let item = create_list_item_node(None);
        if !text.is_empty() {
            item.append(&[create_text_node(text)])?;
        }
        list.append(&[item])?;
    }
    Ok(list)
}

fn lone_block_leaves_an_empty_paragraph(block: fn() -> Result<NodeRef, LexicalError>) {
    let editor = create_editor();
    update(&editor, move || {
        let root = get_root();
        root.clear()?;
        root.append(&[block()?])
    });
    update(&editor, || select_all().map(drop));
    cut(&editor, &Rc::new(DataTransfer::new()));
    assert_eq!(read_outline(&editor), "[[paragraph, []]]");
    editor.dispose();
}

fn cutting_and_pasting_restores(block: fn() -> Result<NodeRef, LexicalError>) {
    let editor = create_editor();
    update(&editor, move || {
        let root = get_root();
        root.clear()?;
        root.append(&[block()?])
    });
    let before = read_outline(&editor);
    update(&editor, || select_all().map(drop));
    let cut_data = Rc::new(DataTransfer::new());
    cut(&editor, &cut_data);
    assert_eq!(read_outline(&editor), "[[paragraph, []]]");
    editor
        .dispatch_command(
            &PASTE_COMMAND,
            PasteCommandType::Clipboard(Rc::new(ClipboardEvent::new("paste", Some(cut_data)))),
        )
        .unwrap();
    assert_eq!(read_outline(&editor), before);
    editor.dispose();
}

fn collapsed_caret_cuts_nothing(block: fn() -> Result<NodeRef, LexicalError>, expected: &str) {
    let editor = create_editor();
    update(&editor, move || {
        let block = block()?;
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&block))?;
        block.select_start()?;
        Ok(())
    });
    cut(&editor, &Rc::new(DataTransfer::new()));
    assert_eq!(read_outline(&editor), expected);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > a lone heading leaves an empty paragraph
#[test]
fn a_lone_heading_leaves_an_empty_paragraph() {
    lone_block_leaves_an_empty_paragraph(heading_hi);
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > a lone list leaves an empty paragraph
#[test]
fn a_lone_list_leaves_an_empty_paragraph() {
    lone_block_leaves_an_empty_paragraph(|| list_of(&["one"]));
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > cutting a lone heading and pasting restores it
#[test]
fn cutting_a_lone_heading_and_pasting_restores_it() {
    cutting_and_pasting_restores(heading_hi);
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > cutting a lone list and pasting restores it
#[test]
fn cutting_a_lone_list_and_pasting_restores_it() {
    cutting_and_pasting_restores(|| list_of(&["one", "two"]));
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > a collapsed caret in an empty heading cuts nothing
#[test]
fn a_collapsed_caret_in_an_empty_heading_cuts_nothing() {
    collapsed_caret_cuts_nothing(
        || Ok(create_heading_node(HeadingTagType::H1)),
        "[[heading, []]]",
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > a collapsed caret in an empty list cuts nothing
#[test]
fn a_collapsed_caret_in_an_empty_list_cuts_nothing() {
    collapsed_caret_cuts_nothing(|| list_of(&[""]), "[[list, [[listitem, []]]]]");
}
