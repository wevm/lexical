//! Ports of `packages/lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts`.
//!
//! JS mounts the editors on a `contenteditable` div; these have a
//! [`set_test_root_element`] root element.

#![cfg(feature = "rich-text")]

use super::super::support::{
    TestBlockableDecoratorNode, create_test_blockable_decorator_node, set_is_inline,
    set_test_root_element, update, update_with,
};
use super::support::paragraph;
use crate::extension::{
    GetParentEditor, LexicalEditorWithDispose, NestedEditorConfigOverrides,
    SelectBlockConfigOverrides, build_editor_from_extensions, get_extension_dependency_from_editor,
    nested_editor_extension, prevent_select_all_extension, select_block_extension,
};
use crate::{
    Editor, InitialEditorStateType, KeyboardEvent, LexicalError, LexicalExtension, NodeClass,
    NodeRef, PointType, SELECT_ALL_COMMAND, Which, config_extension, create_node_selection,
    create_paragraph_node, define_extension, get_root, get_selection, is_block_fully_selected,
    set_selection,
};
use std::rc::Rc;

/// `setUpEditor({cascadeSelection, disabled})`.
fn set_up_editor(cascade_selection: bool, disabled: bool) -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            get_root().append(&[paragraph("first"), paragraph("second")])?;
            Ok(())
        }))),
        dependencies: vec![config_extension(
            &select_block_extension(),
            [SelectBlockConfigOverrides {
                cascade_selection: Some(cascade_selection),
                disabled: Some(disabled),
            }],
        )],
        nodes: vec![NodeClass::of::<TestBlockableDecoratorNode>()],
        ..LexicalExtension::<(), (), ()>::new("select-block-test", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

fn default_editor() -> LexicalEditorWithDispose {
    set_up_editor(false, false)
}

/// `selectAllKeyboardEvent()`.
fn select_all_event() -> Rc<KeyboardEvent> {
    Rc::new(KeyboardEvent {
        key: "a".to_owned(),
        ctrl_key: true,
        ..KeyboardEvent::default()
    })
}

fn select_all(editor: &Editor) -> bool {
    editor
        .dispatch_command(&SELECT_ALL_COMMAND, select_all_event())
        .unwrap()
}

/// `$secondParagraph()`.
fn second_paragraph() -> NodeRef {
    let node = get_root().get_child_at_index(1).expect("second paragraph");
    assert!(node.is_element());
    node
}

/// Whether the selection is a range that fully selects `block`.
fn fully_selected(block: &NodeRef) -> bool {
    let range = get_selection()
        .and_then(|s| s.range())
        .expect("expecting a RangeSelection");
    is_block_fully_selected(block, &range).unwrap()
}

/// `$expectFullySelected($getRoot())`, read.
fn expect_root_fully_selected(editor: &Editor) {
    assert!(editor.read(|| fully_selected(&get_root())).unwrap());
}

/// A caret 3 characters into the second paragraph.
fn caret_in_second(editor: &Editor) {
    update(editor, || {
        let text = second_paragraph().get_first_descendant().unwrap();
        assert!(text.is_text());
        text.select(Some(3), Some(3))?;
        Ok(())
    });
}

fn points(editor: &Editor) -> (String, usize, String, usize) {
    editor
        .read(|| {
            let range = get_selection().and_then(|s| s.range()).unwrap();
            let (a, f) = (range.point(Which::Anchor), range.point(Which::Focus));
            (a.key.clone(), a.offset, f.key.clone(), f.offset)
        })
        .unwrap()
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > selects the block first and then the whole document
#[test]
fn selects_the_block_then_the_document() {
    let editor = default_editor();
    caret_in_second(&editor);
    assert!(select_all(&editor));
    editor
        .read(|| {
            assert!(fully_selected(&second_paragraph()));
            assert!(!fully_selected(&get_root()));
        })
        .unwrap();
    assert!(select_all(&editor));
    expect_root_fully_selected(&editor);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > repeated select all does not change the selection
#[test]
fn repeated_select_all_does_not_change_the_selection() {
    let editor = default_editor();
    caret_in_second(&editor);
    select_all(&editor);
    select_all(&editor);
    let before = points(&editor);
    select_all(&editor);
    assert_eq!(points(&editor), before);
    expect_root_fully_selected(&editor);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > a selection spanning multiple blocks expands to the whole document
#[test]
fn a_selection_spanning_blocks_expands_to_the_document() {
    let editor = default_editor();
    update(&editor, || {
        let first = get_root().get_first_descendant().unwrap();
        let second = second_paragraph().get_first_descendant().unwrap();
        let selection = first.select(Some(2), Some(2))?;
        selection.point_set(Which::Focus, second.get_key(), 3, PointType::Text)?;
        Ok(())
    });
    assert!(select_all(&editor));
    expect_root_fully_selected(&editor);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > a selection anchored on the root still selects all
#[test]
fn a_selection_on_the_root_selects_all() {
    let editor = default_editor();
    update(&editor, || {
        // A partial selection with points directly on the root.
        get_root().select(Some(0), Some(1))?;
        Ok(())
    });
    assert!(select_all(&editor));
    expect_root_fully_selected(&editor);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > select all works after the document changes while fully selected
#[test]
fn select_all_after_the_document_changes() {
    let editor = default_editor();
    caret_in_second(&editor);
    select_all(&editor);
    select_all(&editor);
    // A remote or programmatic change that does not touch the selection.
    update(&editor, || {
        get_root().append(&[paragraph("third")])?;
        Ok(())
    });
    assert!(select_all(&editor));
    expect_root_fully_selected(&editor);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > select all preserves a manually created full selection
#[test]
fn select_all_preserves_a_full_selection() {
    let editor = default_editor();
    update(&editor, || {
        let first = get_root().get_first_descendant().unwrap();
        let last = get_root().get_last_descendant().unwrap();
        // A backward selection of the whole document.
        let selection = last.select(Some(6), Some(6))?;
        selection.point_set(Which::Focus, first.get_key(), 0, PointType::Text)?;
        Ok(())
    });
    assert!(select_all(&editor));
    editor
        .read(|| {
            let selection = get_selection().unwrap();
            // Not rewritten by $selectAll.
            assert!(selection.is_backward().unwrap());
            assert!(fully_selected(&get_root()));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > select all in a single-block document
#[test]
fn select_all_in_a_single_block_document() {
    let editor = default_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph("only")])?;
        root.get_first_descendant()
            .unwrap()
            .select(Some(2), Some(2))?;
        Ok(())
    });
    assert!(select_all(&editor));
    expect_root_fully_selected(&editor);
    assert!(select_all(&editor));
    expect_root_fully_selected(&editor);
    editor.dispose();
}

fn select_node(node: &NodeRef) {
    let selection = create_node_selection();
    selection.add(node.get_key());
    set_selection(Some(selection));
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > a node selection of an inline decorator selects its block
#[test]
fn a_node_selection_of_an_inline_decorator_selects_its_block() {
    let editor = default_editor();
    update(&editor, || {
        let decorator = create_test_blockable_decorator_node();
        second_paragraph().append(std::slice::from_ref(&decorator))?;
        select_node(&decorator);
        Ok(())
    });
    assert!(select_all(&editor));
    assert!(editor.read(|| fully_selected(&second_paragraph())).unwrap());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > a node selection of a top-level decorator selects all
#[test]
fn a_node_selection_of_a_top_level_decorator_selects_all() {
    let editor = default_editor();
    update(&editor, || {
        let decorator = set_is_inline(&create_test_blockable_decorator_node(), false);
        get_root().append(std::slice::from_ref(&decorator))?;
        select_node(&decorator);
        Ok(())
    });
    assert!(select_all(&editor));
    expect_root_fully_selected(&editor);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > an empty node selection defers to other handlers
#[test]
fn an_empty_node_selection_defers() {
    let editor = default_editor();
    update(&editor, || {
        set_selection(Some(create_node_selection()));
        Ok(())
    });
    assert!(!select_all(&editor));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > respects disabled config
#[test]
fn respects_disabled_config() {
    let editor = set_up_editor(false, true);
    caret_in_second(&editor);
    assert!(!select_all(&editor));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > enables and disables PreventSelectAllExtension together
#[test]
fn enables_and_disables_prevent_select_all_together() {
    let editor = set_up_editor(false, true);
    let select_block = get_extension_dependency_from_editor(&editor, &select_block_extension())
        .unwrap()
        .output
        .unwrap();
    let prevent = get_extension_dependency_from_editor(&editor, &prevent_select_all_extension())
        .unwrap()
        .output
        .unwrap();
    assert!(prevent.disabled.value());
    select_block.disabled.set_value(false);
    assert!(!prevent.disabled.value());
    select_block.disabled.set_value(true);
    assert!(prevent.disabled.value());
    editor.dispose();
}

/// `setUpNestedEditor(parentEditor, $initialEditorState)`.
fn set_up_nested_editor(
    parent: &Editor,
    init: fn() -> Result<(), LexicalError>,
) -> LexicalEditorWithDispose {
    let parent = parent.clone();
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| init()))),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            config_extension(
                &nested_editor_extension(),
                [NestedEditorConfigOverrides {
                    get_parent_editor: Some(GetParentEditor(Rc::new(move || Ok(parent.clone())))),
                    ..Default::default()
                }],
            ),
        ],
        namespace: Some("nested".to_owned()),
        ..LexicalExtension::<(), (), ()>::new("nested-editor-test", ())
    })
    .into()])
    .unwrap()
}

fn caption() -> Result<(), LexicalError> {
    get_root().append(&[paragraph("caption")])?;
    Ok(())
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension with nested editors > ignores commands bubbled from a nested editor when cascadeSelection is off
#[test]
fn nested_ignores_bubbled_commands_without_cascade() {
    let editor = default_editor();
    let nested = set_up_nested_editor(&editor, caption);
    // A stale node selection in the parent editor must not hijack the
    // nested editor's select all.
    update(&editor, || {
        let decorator = create_test_blockable_decorator_node();
        second_paragraph().append(std::slice::from_ref(&decorator))?;
        select_node(&decorator);
        Ok(())
    });
    assert!(select_all(&nested));
    expect_root_fully_selected(&nested);
    assert!(
        editor
            .read(|| get_selection().is_some_and(|s| s.is_node()))
            .unwrap()
    );
    nested.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension with nested editors > cascadeSelection ignores the command until the nested editor is fully selected
#[test]
fn nested_cascade_waits_until_the_nested_editor_is_fully_selected() {
    let editor = set_up_editor(true, false);
    let nested = set_up_nested_editor(&editor, caption);
    update(&nested, || {
        get_root()
            .get_first_descendant()
            .unwrap()
            .select(Some(3), Some(3))?;
        Ok(())
    });
    assert!(select_all(&nested));
    expect_root_fully_selected(&nested);
    assert!(
        !editor
            .read(|| get_selection().is_some_and(|s| s.is_range()))
            .unwrap()
    );
    // The next press cascades to the parent editor.
    assert!(select_all(&nested));
    expect_root_fully_selected(&editor);
    nested.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension with nested editors > cascadeSelection works when dispatched from inside a nested editor update
#[test]
fn nested_cascade_from_inside_a_nested_update() {
    let editor = set_up_editor(true, false);
    let nested = set_up_nested_editor(&editor, caption);
    let inner = nested.editor().clone();
    let handled = update_with(&nested, move || {
        crate::select_all()?;
        // Dispatch while this update is still in progress.
        let handled = inner.dispatch_command(&SELECT_ALL_COMMAND, select_all_event())?;
        // The update continues after the dispatch returns.
        get_root().append(&[paragraph("after")])?;
        Ok(handled)
    });
    assert!(handled);
    let text = nested.read(|| get_root().get_text_content()).unwrap();
    assert!(text.contains("caption"));
    assert!(text.contains("after"));
    expect_root_fully_selected(&editor);
    nested.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension with nested editors > cascadeSelection handles the command from an empty nested editor
#[test]
fn nested_cascade_from_an_empty_nested_editor() {
    let editor = set_up_editor(true, false);
    let nested = set_up_nested_editor(&editor, || {
        let p = create_paragraph_node();
        get_root().append(std::slice::from_ref(&p))?;
        p.select_start()?;
        Ok(())
    });
    // An empty nested editor is fully selected by its caret.
    assert!(select_all(&nested));
    expect_root_fully_selected(&editor);
    nested.dispose();
    editor.dispose();
}
