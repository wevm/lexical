//! Ports of `packages/lexical/src/__tests__/unit/Issue8745Repro.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::{
    TestBlockableDecoratorNode, TestShadowRootNode, create_test_decorator_node_inline,
    create_test_shadow_root_node,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::rich_text::{HeadingTagType, create_heading_node};
use crate::{
    LexicalError, LexicalExtension, NodeClass, NodeRef, SelectionRef, UpdateOptions,
    create_node_selection, create_paragraph_node, create_text_node, define_extension, get_root,
    select_all, set_selection,
};

fn ext() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        nodes: vec![
            NodeClass::of::<TestBlockableDecoratorNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[8745]", ())
    })
    .into()])
    .unwrap()
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn with(node: NodeRef, children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    node.append(children)?;
    Ok(node)
}

fn p(text: &str) -> Result<NodeRef, LexicalError> {
    with(create_paragraph_node(), &[create_text_node(text)])
}

fn shadow(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    with(create_test_shadow_root_node(), children)
}

fn set_root(children: &[NodeRef]) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(children)
}

/// Builds the editor with `document`, then runs `edit` in an update.
fn run(
    document: impl FnOnce() -> Result<Vec<NodeRef>, LexicalError> + 'static,
    edit: impl FnOnce() -> Result<(), LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    let editor = ext();
    editor
        .update(move || set_root(&document()?), discrete())
        .unwrap();
    editor.update(edit, discrete()).unwrap();
    editor
}

fn select_all_range() -> Result<SelectionRef, LexicalError> {
    let selection = select_all()?;
    assert!(selection.is_range(), "Expected RangeSelection");
    Ok(selection)
}

fn remove_text_all() -> Result<(), LexicalError> {
    select_all()?.remove_text()
}

fn delete_all(is_backward: bool) -> impl FnOnce() -> Result<(), LexicalError> {
    move || select_all_range()?.delete_character(is_backward)
}

/// One child, an empty one.
fn expect_one_empty(editor: &LexicalEditorWithDispose) {
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 1);
            assert_eq!(root.get_first_child().unwrap().get_text_content(), "");
        })
        .unwrap();
}

fn children_size(editor: &LexicalEditorWithDispose) -> usize {
    editor.read(|| get_root().get_children_size()).unwrap()
}

/// `hello`, then a shadow root holding a paragraph of `inner`.
fn hello_then_shadow(inner: &'static str) -> impl FnOnce() -> Result<Vec<NodeRef>, LexicalError> {
    move || Ok(vec![p("hello")?, shadow(&[p(inner)?])?])
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > leaves one empty paragraph when document ends with a shadow root
#[test]
fn leaves_one_empty_paragraph_when_document_ends_with_a_shadow_root() {
    let editor = run(hello_then_shadow("shadow content"), remove_text_all);
    expect_one_empty(&editor);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > leaves one empty paragraph with nested shadow roots
#[test]
fn leaves_one_empty_paragraph_with_nested_shadow_roots() {
    let editor = run(
        || {
            Ok(vec![
                p("hello")?,
                shadow(&[shadow(&[p("nested content")?])?])?,
            ])
        },
        remove_text_all,
    );
    expect_one_empty(&editor);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > leaves one empty paragraph with multi-child shadow root (columns)
#[test]
fn leaves_one_empty_paragraph_with_multi_child_shadow_root() {
    let editor = run(
        || {
            let columns = shadow(&[shadow(&[p("column 1")?])?, shadow(&[p("column 2")?])?])?;
            Ok(vec![p("hello")?, columns])
        },
        remove_text_all,
    );
    expect_one_empty(&editor);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > preserves content after shadow root when shadow root is in the middle
#[test]
fn preserves_content_after_shadow_root_when_shadow_root_is_in_the_middle() {
    let editor = run(
        || Ok(vec![p("before")?, shadow(&[p("shadow")?])?, p("after")?]),
        remove_text_all,
    );
    assert_eq!(children_size(&editor), 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > deleteCharacter with select-all cleans up trailing shadow root
#[test]
fn delete_character_with_select_all_cleans_up_trailing_shadow_root() {
    let editor = run(hello_then_shadow("shadow content"), delete_all(false));
    expect_one_empty(&editor);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > heading first child + backward deleteCharacter: reproduces issue author scenario
#[test]
fn heading_first_child_backward_delete_character() {
    let editor = run(
        || {
            let h1 = with(
                create_heading_node(HeadingTagType::H1),
                &[create_text_node("Welcome")],
            )?;
            Ok(vec![h1, p("some text")?, shadow(&[p("shadow content")?])?])
        },
        delete_all(true),
    );
    expect_one_empty(&editor);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > paragraph first child + backward deleteCharacter: simple case
#[test]
fn paragraph_first_child_backward_delete_character() {
    let editor = run(hello_then_shadow("shadow content"), delete_all(true));
    expect_one_empty(&editor);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > HR + collapsible only: select-all delete leaves at least one child
#[test]
fn hr_and_collapsible_only_select_all_delete_leaves_at_least_one_child() {
    let editor = run(
        || {
            let hr = create_test_decorator_node_inline(false);
            let container = shadow(&[shadow(&[p("Title")?])?, shadow(&[p("Content")?])?])?;
            Ok(vec![hr, container])
        },
        || {
            let root = get_root();
            let selection = root.select(Some(0), Some(root.get_children_size()))?;
            assert!(selection.is_range(), "Expected RangeSelection");
            selection.delete_character(true)
        },
    );
    assert!(children_size(&editor) >= 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > select-all + forward deleteCharacter leaves at least one child
#[test]
fn select_all_forward_delete_character_leaves_at_least_one_child() {
    let editor = run(hello_then_shadow("shadow"), delete_all(false));
    assert!(children_size(&editor) >= 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > select-all + removeText leaves at least one child
#[test]
fn select_all_remove_text_leaves_at_least_one_child() {
    let editor = run(hello_then_shadow("shadow"), || {
        select_all_range()?.remove_text()
    });
    assert!(children_size(&editor) >= 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > NodeSelection.deleteNodes leaves at least one root child
#[test]
fn node_selection_delete_nodes_leaves_at_least_one_root_child() {
    let editor = run(
        || {
            Ok(vec![
                create_test_decorator_node_inline(false),
                shadow(&[p("shadow")?])?,
            ])
        },
        || {
            let selection = create_node_selection();
            for child in get_root().get_children() {
                selection.add(child.get_key());
            }
            set_selection(Some(selection.clone()));
            selection.delete_nodes()
        },
    );
    assert!(children_size(&editor) >= 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > partial deletion within shadow root preserves structure
#[test]
fn partial_deletion_within_shadow_root_preserves_structure() {
    let editor = run(
        || {
            let container = shadow(&[p("title text")?, shadow(&[p("content text")?])?])?;
            Ok(vec![p("before")?, container])
        },
        || {
            let root = get_root();
            let container = root.get_last_child().unwrap();
            assert!(container.is_element());
            let title_p = container.get_first_child().unwrap();
            let content = container.get_last_child().unwrap();
            let content_p = content.get_first_child().unwrap();
            let title_text = title_p.get_first_child().unwrap();
            let content_text = content_p.get_first_child().unwrap();
            assert!(title_text.is_text() && content_text.is_text());
            let selection = title_text.select(Some(5), Some(5))?;
            assert!(selection.is_range(), "Expected RangeSelection");
            selection.set_text_node_range(&title_text, 5, &content_text, 7)?;
            selection.remove_text()
        },
    );
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 2);
            let container = root.get_last_child().unwrap();
            assert!(container.is_element());
            assert_eq!(container.get_children_size(), 2);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > shadow root only (no paragraph): select-all + removeText leaves at least one child
#[test]
fn shadow_root_only_select_all_remove_text_leaves_at_least_one_child() {
    let editor = run(|| Ok(vec![shadow(&[p("only shadow")?])?]), remove_text_all);
    assert!(children_size(&editor) >= 1);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > shadow root only (no paragraph): select-all + backward deleteCharacter leaves at least one child
#[test]
fn shadow_root_only_select_all_backward_delete_character_leaves_at_least_one_child() {
    let editor = run(|| Ok(vec![shadow(&[p("only shadow")?])?]), delete_all(true));
    assert!(children_size(&editor) >= 1);
    editor.dispose();
}
