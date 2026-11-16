//! Ports of `packages/lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::update;
use crate::code::{
    CodeIndentConfigOverrides, code_indent_extension, create_code_highlight_node, create_code_node,
    is_code_node,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::nodes::tab::create_tab_node;
use crate::{
    AnyLexicalExtensionArgument, Editor, InitialEditorStateType, LexicalExtension, NodeRef,
    OUTDENT_CONTENT_COMMAND, config_extension, define_extension, get_root,
};
use std::rc::Rc;

/// `CodeIndentExtension`, or `configExtension(CodeIndentExtension,
/// {tabSize})`.
fn code_indent(tab_size: Option<usize>) -> AnyLexicalExtensionArgument {
    match tab_size {
        None => code_indent_extension().into(),
        Some(tab_size) => config_extension(
            &code_indent_extension(),
            [CodeIndentConfigOverrides {
                tab_size: Some(Some(tab_size)),
                ..Default::default()
            }],
        ),
    }
}

/// An editor with a code block holding `children()`, built with
/// `[code_indent, RichTextExtension]`.
fn build(
    name: &str,
    tab_size: Option<usize>,
    children: fn() -> Vec<NodeRef>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
            let code = create_code_node(Some("javascript"), None);
            code.append(&children())?;
            get_root().append(&[code])
        }))),
        dependencies: vec![
            code_indent(tab_size),
            crate::rich_text::rich_text_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `buildEditor(tabSize)`.
fn build_editor(tab_size: Option<usize>) -> LexicalEditorWithDispose {
    build("[root-outdent]", tab_size, || {
        vec![create_tab_node(), create_code_highlight_node("hello", None)]
    })
}

/// `editor.read($codeText)`.
fn code_text(editor: &Editor) -> String {
    editor
        .read(|| {
            let code = get_root().get_first_child_or_throw().unwrap();
            assert!(is_code_node(&code), "expected a CodeNode");
            code.get_text_content()
        })
        .unwrap()
}

fn outdent(editor: &Editor) {
    editor
        .dispatch_command(&OUTDENT_CONTENT_COMMAND, ())
        .unwrap();
}

// upstream: lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > outdents when the caret is collapsed at column 0 (on the TabNode)
#[test]
fn outdents_when_the_caret_is_collapsed_at_column_0_on_the_tab_node() {
    let editor = build_editor(None);
    update(&editor, || {
        let tab = get_root().get_first_descendant().unwrap();
        assert_eq!(tab.get_type(), "tab", "expected a TabNode");
        tab.select(Some(0), Some(0))?;
        Ok(())
    });
    outdent(&editor);
    assert_eq!(code_text(&editor), "hello");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > outdents when the caret is collapsed at column 0 of the code text
#[test]
fn outdents_when_the_caret_is_collapsed_at_column_0_of_the_code_text() {
    let editor = build_editor(None);
    update(&editor, || {
        get_root().get_last_descendant().unwrap().select_start()?;
        Ok(())
    });
    outdent(&editor);
    assert_eq!(code_text(&editor), "hello");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > still outdents from a non-zero column (unchanged behaviour)
#[test]
fn still_outdents_from_a_non_zero_column() {
    let editor = build_editor(None);
    update(&editor, || {
        let text = get_root().get_last_descendant().unwrap();
        assert!(text.is_text(), "expected a text node");
        text.select(Some(1), Some(1))?;
        Ok(())
    });
    outdent(&editor);
    assert_eq!(code_text(&editor), "hello");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > strips a space indent from column 0 when tabSize is configured
#[test]
fn strips_a_space_indent_from_column_0_when_tab_size_is_configured() {
    let editor = build("[root-outdent-spaces]", Some(2), || {
        vec![create_code_highlight_node("  hello", None)]
    });
    update(&editor, || {
        get_root().get_last_descendant().unwrap().select_start()?;
        Ok(())
    });
    outdent(&editor);
    assert_eq!(code_text(&editor), "hello");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > is still a no-op on an unindented line
#[test]
fn is_still_a_no_op_on_an_unindented_line() {
    let editor = build("[root-outdent-flat]", None, || {
        vec![create_code_highlight_node("hello", None)]
    });
    update(&editor, || {
        get_root().get_last_descendant().unwrap().select_start()?;
        Ok(())
    });
    outdent(&editor);
    assert_eq!(code_text(&editor), "hello");
}
