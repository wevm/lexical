//! Ports of `packages/lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts`.
//!
//! The `it.each` cases are one Rust test each, calling the case's body with
//! its key.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::update;
use crate::code::{
    CodeIndentConfigOverrides, code_indent_extension, create_code_highlight_node, create_code_node,
    is_code_node,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::nodes::line_break::create_line_break_node;
use crate::{
    Editor, InitialEditorStateType, KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND,
    KEY_ARROW_RIGHT_COMMAND, KEY_ARROW_UP_COMMAND, KeyboardEvent, LexicalCommand, LexicalError,
    LexicalExtension, NodeRef, config_extension, create_paragraph_node, define_extension, get_root,
};
use std::rc::Rc;

/// `buildEditorFromExtensions(defineExtension({$initialEditorState,
/// dependencies: [configExtension(CodeIndentExtension, {escapeWithArrows:
/// true}), RichTextExtension], name: '[root-middle]'}))`.
fn build_escaping(
    initial: impl Fn() -> Result<(), LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        dependencies: vec![
            config_extension(
                &code_indent_extension(),
                [CodeIndentConfigOverrides {
                    escape_with_arrows: Some(true),
                    ..Default::default()
                }],
            ),
            crate::rich_text::rich_text_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[root-middle]", ())
    })
    .into()])
    .unwrap()
}

/// The command of an arrow key.
fn command(key: &str) -> LexicalCommand<Rc<KeyboardEvent>> {
    match key {
        "ArrowDown" => KEY_ARROW_DOWN_COMMAND,
        "ArrowRight" => KEY_ARROW_RIGHT_COMMAND,
        "ArrowUp" => KEY_ARROW_UP_COMMAND,
        "ArrowLeft" => KEY_ARROW_LEFT_COMMAND,
        _ => unreachable!(),
    }
}

/// `editor.dispatchCommand(command, new KeyboardEvent('keydown', {altKey,
/// key}))`.
fn press(editor: &Editor, key: &str, alt_key: bool) {
    editor
        .dispatch_command(
            &command(key),
            Rc::new(KeyboardEvent {
                key: key.to_owned(),
                alt_key,
                ..KeyboardEvent::default()
            }),
        )
        .unwrap();
}

/// A code block holding `hello world`, appended to the root; returns the
/// text node.
fn code_with_text() -> Result<(NodeRef, NodeRef), LexicalError> {
    let code_node = create_code_node(Some("javascript"), None);
    let text = create_code_highlight_node("hello world", None);
    code_node.append(std::slice::from_ref(&text))?;
    Ok((code_node, text))
}

/// The root's children are `types` (`code` or `paragraph`).
fn expect_root(editor: &Editor, types: &'static [&'static str]) {
    update(editor, move || {
        let root = get_root();
        assert_eq!(root.get_children_size(), types.len());
        for (child, ty) in root.get_children().iter().zip(types) {
            match *ty {
                "code" => assert!(is_code_node(child)),
                _ => assert_eq!(child.get_type(), *ty),
            }
        }
        Ok(())
    });
}

/// Expects two children, the one at `index` a paragraph (the case only
/// checks that one).
fn expect_two_with_paragraph(editor: &Editor, last: bool) {
    update(editor, move || {
        let root = get_root();
        assert_eq!(root.get_children_size(), 2);
        let paragraph = if last {
            root.get_last_child_or_throw()?
        } else {
            root.get_first_child_or_throw()?
        };
        assert_eq!(paragraph.get_type(), "paragraph");
        Ok(())
    });
}

fn escapes_at_the_end(key: &str) {
    let editor = build_escaping(|| {
        let (code_node, text) = code_with_text()?;
        get_root().append(&[code_node])?;
        text.select_end()?;
        Ok(())
    });
    press(&editor, key, false);
    expect_two_with_paragraph(&editor, true);
}

fn does_not_escape_not_at_the_end(key: &str) {
    let editor = build_escaping(|| {
        let (code_node, text) = code_with_text()?;
        get_root().append(&[code_node])?;
        let length = text.get_text_content_size();
        text.select(Some(length - 1), Some(length - 1))?;
        Ok(())
    });
    press(&editor, key, false);
    expect_root(&editor, &["code"]);
}

fn does_not_escape_at_the_end_with_next_sibling(key: &str) {
    let editor = build_escaping(|| {
        let (code_node, text) = code_with_text()?;
        get_root().append(&[code_node, create_paragraph_node()])?;
        text.select_end()?;
        Ok(())
    });
    press(&editor, key, false);
    expect_root(&editor, &["code", "paragraph"]);
}

fn escapes_at_the_beginning(key: &str) {
    let editor = build_escaping(|| {
        let (code_node, text) = code_with_text()?;
        get_root().append(&[code_node])?;
        text.select_start()?;
        Ok(())
    });
    press(&editor, key, false);
    expect_two_with_paragraph(&editor, false);
}

fn does_not_escape_not_at_the_beginning(key: &str) {
    let editor = build_escaping(|| {
        let (code_node, text) = code_with_text()?;
        get_root().append(&[code_node])?;
        text.select(Some(1), Some(1))?;
        Ok(())
    });
    press(&editor, key, false);
    expect_root(&editor, &["code"]);
}

fn does_not_escape_at_the_beginning_with_previous_sibling(key: &str) {
    let editor = build_escaping(|| {
        let (code_node, text) = code_with_text()?;
        get_root().append(&[create_paragraph_node(), code_node])?;
        text.select_start()?;
        Ok(())
    });
    press(&editor, key, false);
    expect_root(&editor, &["paragraph", "code"]);
}

/// An empty code block with the caret at its start.
fn empty_block_editor() -> LexicalEditorWithDispose {
    build_escaping(|| {
        let code_node = create_code_node(Some("javascript"), None);
        get_root().append(std::slice::from_ref(&code_node))?;
        code_node.select_start()?;
        Ok(())
    })
}

fn escapes_backward_inside_an_empty_block(key: &str) {
    let editor = empty_block_editor();
    press(&editor, key, false);
    expect_two_with_paragraph(&editor, false);
}

fn escapes_forward_inside_an_empty_block(key: &str) {
    let editor = empty_block_editor();
    press(&editor, key, false);
    expect_two_with_paragraph(&editor, true);
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowDown when cursor is at the end
#[test]
fn escapes_at_the_end_by_arrow_down() {
    escapes_at_the_end("ArrowDown");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowRight when cursor is at the end
#[test]
fn escapes_at_the_end_by_arrow_right() {
    escapes_at_the_end("ArrowRight");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowDown when cursor is not at the end
#[test]
fn does_not_escape_not_at_the_end_by_arrow_down() {
    does_not_escape_not_at_the_end("ArrowDown");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowRight when cursor is not at the end
#[test]
fn does_not_escape_not_at_the_end_by_arrow_right() {
    does_not_escape_not_at_the_end("ArrowRight");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowDown when cursor is at the end and code block has next sibling
#[test]
fn does_not_escape_at_the_end_with_next_sibling_by_arrow_down() {
    does_not_escape_at_the_end_with_next_sibling("ArrowDown");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowRight when cursor is at the end and code block has next sibling
#[test]
fn does_not_escape_at_the_end_with_next_sibling_by_arrow_right() {
    does_not_escape_at_the_end_with_next_sibling("ArrowRight");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowUp when cursor is at the beginning
#[test]
fn escapes_at_the_beginning_by_arrow_up() {
    escapes_at_the_beginning("ArrowUp");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowLeft when cursor is at the beginning
#[test]
fn escapes_at_the_beginning_by_arrow_left() {
    escapes_at_the_beginning("ArrowLeft");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowUp when cursor is not at the beginning
#[test]
fn does_not_escape_not_at_the_beginning_by_arrow_up() {
    does_not_escape_not_at_the_beginning("ArrowUp");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowLeft when cursor is not at the beginning
#[test]
fn does_not_escape_not_at_the_beginning_by_arrow_left() {
    does_not_escape_not_at_the_beginning("ArrowLeft");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowUp when cursor is at the beginning and code block has previous sibling
#[test]
fn does_not_escape_at_the_beginning_with_previous_sibling_by_arrow_up() {
    does_not_escape_at_the_beginning_with_previous_sibling("ArrowUp");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowLeft when cursor is at the beginning and code block has previous sibling
#[test]
fn does_not_escape_at_the_beginning_with_previous_sibling_by_arrow_left() {
    does_not_escape_at_the_beginning_with_previous_sibling("ArrowLeft");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowUp when cursor is inside an empty block
#[test]
fn escapes_backward_inside_an_empty_block_by_arrow_up() {
    escapes_backward_inside_an_empty_block("ArrowUp");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowLeft when cursor is inside an empty block
#[test]
fn escapes_backward_inside_an_empty_block_by_arrow_left() {
    escapes_backward_inside_an_empty_block("ArrowLeft");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowDown when cursor is inside an empty block
#[test]
fn escapes_forward_inside_an_empty_block_by_arrow_down() {
    escapes_forward_inside_an_empty_block("ArrowDown");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowRight when cursor is inside an empty block
#[test]
fn escapes_forward_inside_an_empty_block_by_arrow_right() {
    escapes_forward_inside_an_empty_block("ArrowRight");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by Alt/Option + ArrowDown when cursor is at the end
#[test]
fn does_not_escape_by_alt_arrow_down_at_the_end() {
    let editor = build_escaping(|| {
        let (code_node, text) = code_with_text()?;
        get_root().append(&[code_node])?;
        text.select_end()?;
        Ok(())
    });
    press(&editor, "ArrowDown", true);
    expect_root(&editor, &["code"]);
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by Alt/Option + ArrowUp when cursor is at the beginning
#[test]
fn does_not_escape_by_alt_arrow_up_at_the_beginning() {
    let editor = build_escaping(|| {
        let (code_node, text) = code_with_text()?;
        get_root().append(&[code_node])?;
        text.select_start()?;
        Ok(())
    });
    press(&editor, "ArrowUp", true);
    expect_root(&editor, &["code"]);
}

/// `buildBlankLineEditor(caretOnLastLine)`: lines `A`, `` and `B`.
fn build_blank_line_editor(caret_on_last_line: bool) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
            let code_node = create_code_node(Some("javascript"), None);
            let first = create_code_highlight_node("A", None);
            let last = create_code_highlight_node("B", None);
            code_node.append(&[
                first.clone(),
                create_line_break_node(),
                create_line_break_node(),
                last.clone(),
            ])?;
            get_root().append(&[code_node])?;
            (if caret_on_last_line { last } else { first }).select(Some(0), Some(0))?;
            Ok(())
        }))),
        dependencies: vec![
            code_indent_extension().into(),
            crate::rich_text::rich_text_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[root-shift-lines]", ())
    })
    .into()])
    .unwrap()
}

fn code_text(editor: &Editor) -> String {
    editor
        .read(|| {
            let code_node = get_root().get_first_child_or_throw().unwrap();
            assert!(is_code_node(&code_node));
            code_node.get_text_content()
        })
        .unwrap()
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > shiftLines > moves a line up past a blank line without merging it into the line above
#[test]
fn moves_a_line_up_past_a_blank_line() {
    let editor = build_blank_line_editor(true);
    press(&editor, "ArrowUp", true);
    assert_eq!(code_text(&editor), "A\nB\n");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > shiftLines > moves a line down past a blank line without merging it into the line below
#[test]
fn moves_a_line_down_past_a_blank_line() {
    let editor = build_blank_line_editor(false);
    press(&editor, "ArrowDown", true);
    assert_eq!(code_text(&editor), "\nA\nB");
}
