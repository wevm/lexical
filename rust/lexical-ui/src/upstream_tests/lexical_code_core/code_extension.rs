//! Ports of `packages/lexical-code-core/src/__tests__/unit/CodeExtension.test.ts`.

#![cfg(feature = "rich-text")]

use super::super::support::update;
use crate::code::{code_extension, create_code_highlight_node, create_code_node, is_code_node};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::nodes::line_break::create_line_break_node;
use crate::nodes::tab::create_tab_node;
use crate::{
    InitialEditorStateType, KEY_ENTER_COMMAND, KeyboardEvent, LexicalError, LexicalExtension,
    NodeRef, define_extension, get_root,
};
use std::rc::Rc;

/// `buildEditorFromExtensions(defineExtension({$initialEditorState,
/// dependencies: [CodeExtension, RichTextExtension], name}))`, the code
/// block holding `children()`.
fn build(name: &str, children: fn() -> Vec<NodeRef>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
            let code_node = create_code_node(Some("javascript"), None);
            code_node.append(&children())?;
            get_root().append(&[code_node])?;
            Ok(())
        }))),
        dependencies: vec![
            code_extension().into(),
            crate::rich_text::rich_text_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `$assertNodeType($getRoot().getFirstChild(), $isCodeNode)`.
fn first_code_node() -> NodeRef {
    let node = get_root().get_first_child().unwrap();
    assert!(is_code_node(&node));
    node
}

/// `editor.dispatchCommand(KEY_ENTER_COMMAND, new KeyboardEvent('keydown',
/// {key: 'Enter'}))`.
fn press_enter(editor: &crate::Editor) {
    editor
        .dispatch_command(
            &KEY_ENTER_COMMAND,
            Some(Rc::new(KeyboardEvent {
                key: "Enter".to_owned(),
                ..KeyboardEvent::default()
            })),
        )
        .unwrap();
}

fn root_children_size_and_first_is_code(editor: &crate::Editor, size: usize) {
    update(editor, move || {
        let root = get_root();
        assert_eq!(root.get_children_size(), size);
        assert!(is_code_node(&root.get_first_child_or_throw()?));
        Ok::<(), LexicalError>(())
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should not escape code block when content has consecutive blank lines (paste scenario)
#[test]
fn should_not_escape_code_block_when_content_has_consecutive_blank_lines() {
    let editor = build("[root]", || {
        vec![
            create_code_highlight_node("line1", None),
            create_line_break_node(),
            create_line_break_node(),
            create_line_break_node(),
            create_code_highlight_node("line2", None),
        ]
    });
    root_children_size_and_first_is_code(&editor, 1);
}

// upstream: lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should escape code block on Enter when cursor is after two trailing blank lines
#[test]
fn should_escape_code_block_on_enter_after_two_trailing_blank_lines() {
    let editor = build("[root-escape]", || {
        vec![
            create_code_highlight_node("hello", None),
            create_line_break_node(),
            create_line_break_node(),
        ]
    });
    update(&editor, || {
        let code_node = first_code_node();
        let size = code_node.get_children_size();
        code_node.select(Some(size), Some(size))?;
        Ok(())
    });
    press_enter(&editor);
    update(&editor, || {
        let root = get_root();
        assert_eq!(root.get_children_size(), 2);
        assert!(is_code_node(&root.get_first_child_or_throw()?));
        let second_child = root.get_last_child_or_throw()?;
        assert!(second_child.is_element());
        assert_eq!(second_child.get_type(), "paragraph");
        Ok(())
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should not escape code block on Enter with only one trailing blank line
#[test]
fn should_not_escape_code_block_on_enter_with_one_trailing_blank_line() {
    let editor = build("[root-one-blank]", || {
        vec![
            create_code_highlight_node("hello", None),
            create_line_break_node(),
        ]
    });
    update(&editor, || {
        let code_node = first_code_node();
        let size = code_node.get_children_size();
        code_node.select(Some(size), Some(size))?;
        Ok(())
    });
    press_enter(&editor);
    root_children_size_and_first_is_code(&editor, 1);
}

// upstream: lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should strip format from TabNode inside CodeNode
#[test]
fn should_strip_format_from_tab_node_inside_code_node() {
    let editor = build("[tab-format]", || {
        vec![create_code_highlight_node("x", None), create_tab_node()]
    });
    update(&editor, || {
        let tab = first_code_node().get_last_child().unwrap();
        assert_eq!(tab.get_type(), "tab");
        tab.set_format(1.0);
        Ok(())
    });
    editor
        .read(|| {
            let tab = first_code_node().get_last_child().unwrap();
            assert_eq!(tab.get_type(), "tab");
            assert_eq!(tab.text_data().get_format(), 0.0);
        })
        .unwrap();
}

// upstream: lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should not escape code block on Enter when cursor is not at the end
#[test]
fn should_not_escape_code_block_on_enter_when_cursor_is_not_at_the_end() {
    let editor = build("[root-middle]", || {
        vec![
            create_code_highlight_node("hello", None),
            create_line_break_node(),
            create_line_break_node(),
            create_code_highlight_node("world", None),
        ]
    });
    update(&editor, || {
        first_code_node().select(Some(1), Some(1))?;
        Ok(())
    });
    press_enter(&editor);
    root_children_size_and_first_is_code(&editor, 1);
}
