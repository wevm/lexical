//! Ports of `packages/lexical-extension/src/__tests__/unit/ClearEditorExtension.test.ts`
//! and `DecoratorTextExtension.test.ts`.

use super::super::support::update;
use super::support::paragraph;
use crate::extension::{
    ClearEditorConfigOverrides, DecoratorTextNode, LexicalEditorWithDispose, OnClear,
    build_editor_from_extensions, clear_editor_extension, create_decorator_text_node,
};
use crate::{
    AnyLexicalExtensionArgument, CLEAR_EDITOR_COMMAND, InitialEditorStateType, LexicalExtension,
    config_extension, define_extension, get_root,
};
use std::rc::Rc;

/// `setUpEditor(dependency)`.
fn set_up_editor(dependency: AnyLexicalExtensionArgument) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            get_root().append(&[paragraph("content")])?;
            Ok(())
        }))),
        dependencies: vec![dependency],
        ..LexicalExtension::<(), (), ()>::new("clear-editor-test", ())
    })
    .into()])
    .unwrap()
}

fn text_content(editor: &crate::Editor) -> String {
    editor.read(|| get_root().get_text_content()).unwrap()
}

// upstream: lexical-extension/src/__tests__/unit/ClearEditorExtension.test.ts::ClearEditorExtension > CLEAR_EDITOR_COMMAND empties the document
#[test]
fn clear_editor_command_empties_the_document() {
    let editor = set_up_editor(clear_editor_extension().into());
    editor.dispatch_command(&CLEAR_EDITOR_COMMAND, ()).unwrap();
    assert_eq!(text_content(&editor), "");
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/ClearEditorExtension.test.ts::ClearEditorExtension > a configured $onClear replaces the default behavior
#[test]
fn a_configured_on_clear_replaces_the_default() {
    let editor = set_up_editor(config_extension(
        &clear_editor_extension(),
        [ClearEditorConfigOverrides {
            on_clear: Some(OnClear(Rc::new(|| {
                let root = get_root();
                root.clear()?;
                root.append(&[paragraph("cleared")])?;
                Ok(())
            }))),
        }],
    ));
    editor.dispatch_command(&CLEAR_EDITOR_COMMAND, ()).unwrap();
    assert_eq!(text_content(&editor), "cleared");
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/ClearEditorExtension.test.ts::ClearEditorExtension > $onClear runs inline when dispatched from inside an update
#[test]
fn on_clear_runs_inline_inside_an_update() {
    let editor = set_up_editor(clear_editor_extension().into());
    // The document-replacement idiom: clear, then build the new document,
    // atomically. A nested update would be queued and clear the editor
    // after the content below was appended.
    let inner = editor.editor().clone();
    update(&editor, move || {
        inner.dispatch_command(&CLEAR_EDITOR_COMMAND, ())?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph("replacement")])?;
        Ok(())
    });
    assert_eq!(text_content(&editor), "replacement");
    editor.dispose();
}

#[cfg(feature = "rich-text")]
mod decorator_text {
    use super::*;
    use crate::extension::decorator_text_extension;
    use crate::{FORMAT_TEXT_COMMAND, create_paragraph_node, create_text_node, get_selection};

    const BOLD: f64 = 1.0;

    /// The editor of each case: a decorator then a text node, all
    /// selected, with the formats given; `selection_format` is the format
    /// the browser would compute (onSelectionChange doesn't run here).
    fn build(
        decorator_format: f64,
        text_format: f64,
        selection_format: Option<f64>,
    ) -> LexicalEditorWithDispose {
        build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
                let paragraph = create_paragraph_node();
                get_root().append(std::slice::from_ref(&paragraph))?;
                let decorator = create_decorator_text_node();
                if decorator_format != 0.0 {
                    DecoratorTextNode::set_format(&decorator, decorator_format)?;
                }
                let text = create_text_node("hello");
                if text_format != 0.0 {
                    text.set_format(text_format);
                }
                paragraph.append(&[decorator, text])?;
                crate::select_all()?;
                if let Some(format) = selection_format
                    && let Some(selection) = get_selection()
                {
                    selection.set_format(format);
                }
                Ok(())
            }))),
            dependencies: vec![
                decorator_text_extension().into(),
                crate::rich_text::rich_text_extension().into(),
            ],
            ..LexicalExtension::<(), (), ()>::new("test", ())
        })
        .into()])
        .unwrap()
    }

    /// Dispatches bold and returns `(decorator format, text format)`.
    fn bold_formats(editor: &LexicalEditorWithDispose) -> (f64, f64) {
        editor
            .dispatch_command(&FORMAT_TEXT_COMMAND, "bold".to_owned())
            .unwrap();
        editor
            .read(|| {
                let children = get_root()
                    .get_first_child_or_throw()
                    .unwrap()
                    .get_children();
                (
                    DecoratorTextNode::get_format(&children[0]).unwrap(),
                    // `getFormat()`, as the node's JSON has it.
                    children
                        .last()
                        .unwrap()
                        .export_json(false)
                        .get("format")
                        .unwrap()
                        .as_f64()
                        .unwrap(),
                )
            })
            .unwrap()
    }

    // upstream: lexical-extension/src/__tests__/unit/DecoratorTextExtension.test.ts::DecoratorTextExtension FORMAT_TEXT_COMMAND > aligns DecoratorTextNode to not-bold when TextNode is bold
    #[test]
    fn aligns_the_decorator_to_not_bold() {
        let editor = build(0.0, BOLD, Some(BOLD));
        // selection.format = BOLD → toggle OFF; the decorator aligns to 0.
        assert_eq!(bold_formats(&editor), (0.0, 0.0));
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/DecoratorTextExtension.test.ts::DecoratorTextExtension FORMAT_TEXT_COMMAND > aligns DecoratorTextNode to bold when TextNode is not-bold
    #[test]
    fn aligns_the_decorator_to_bold() {
        let editor = build(BOLD, 0.0, None);
        // The text node (bold=0) drives the next format → bold=1.
        assert_eq!(bold_formats(&editor), (BOLD, BOLD));
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/DecoratorTextExtension.test.ts::DecoratorTextExtension FORMAT_TEXT_COMMAND > removes bold from all nodes when every node in selection is already bold
    #[test]
    fn removes_bold_from_all_nodes() {
        let editor = build(BOLD, BOLD, Some(BOLD));
        assert_eq!(bold_formats(&editor), (0.0, 0.0));
        editor.dispose();
    }
}
