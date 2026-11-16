//! Ports of `packages/lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::upstream_tests::support::{mount_in_jsdom, update};
use crate::{
    LexicalExtension, NodeRef, create_paragraph_node, create_text_node, define_extension, get_root,
    get_selection,
};

/// `createEditor()`: `RichTextExtension`, mounted.
fn create_editor() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[paragraph-collapse-at-start]", ())
    })
    .into()])
    .unwrap();
    mount_in_jsdom(&editor);
    editor
}

/// The work Backspace does: `KEY_BACKSPACE_COMMAND` ->
/// `DELETE_CHARACTER_COMMAND`.
fn backspace(editor: &crate::Editor) {
    update(editor, || {
        if let Some(selection) = get_selection().filter(|s| s.is_range()) {
            selection.delete_character(true)?;
        }
        Ok(())
    });
}

fn blocks(editor: &crate::Editor) -> Vec<String> {
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(NodeRef::get_text_content)
                .collect()
        })
        .unwrap()
}

/// Sets the root to `first` and a second paragraph `world`, and selects the
/// start of `first`.
fn seed(editor: &crate::Editor, first_children: impl FnOnce() -> Vec<NodeRef> + 'static) {
    update(editor, move || {
        let first = create_paragraph_node();
        first.append(&first_children())?;
        let second = create_paragraph_node();
        second.append(&[create_text_node("world")])?;
        get_root().clear()?;
        get_root().append(&[first.clone(), second])?;
        first.select_start()?;
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts::ParagraphNode.collapseAtStart > keeps a first paragraph whose content follows a blank text node
#[test]
fn keeps_a_first_paragraph_whose_content_follows_a_blank_text_node() {
    let editor = create_editor();
    seed(&editor, || {
        let bold = create_text_node("hello");
        bold.toggle_format("bold");
        vec![create_text_node("  "), bold]
    });
    backspace(&editor);
    assert_eq!(blocks(&editor), ["  hello", "world"]);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts::ParagraphNode.collapseAtStart > keeps a first paragraph whose only other child is a line break
#[test]
fn keeps_a_first_paragraph_whose_only_other_child_is_a_line_break() {
    let editor = create_editor();
    seed(&editor, || {
        vec![
            create_text_node(" "),
            crate::nodes::line_break::create_line_break_node(),
        ]
    });
    backspace(&editor);
    assert_eq!(editor.read(|| get_root().get_children_size()).unwrap(), 2);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts::ParagraphNode.collapseAtStart > control: a blank first paragraph is still collapsed away
#[test]
fn control_a_blank_first_paragraph_is_still_collapsed_away() {
    let editor = create_editor();
    seed(&editor, || vec![create_text_node("  ")]);
    backspace(&editor);
    assert_eq!(blocks(&editor), ["world"]);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts::ParagraphNode.collapseAtStart > control: an empty first paragraph is still collapsed away
#[test]
fn control_an_empty_first_paragraph_is_still_collapsed_away() {
    let editor = create_editor();
    seed(&editor, Vec::new);
    backspace(&editor);
    assert_eq!(blocks(&editor), ["world"]);
    editor.dispose();
}
