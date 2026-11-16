//! Ports of `packages/lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts`.
//!
//! Upstream's `SlotContainerNode` (a shadow-root element) is the shared
//! [`TestShadowRootNode`]; only its type name differs.

#![cfg(feature = "rich-text")]

use super::super::support::{
    TestShadowRootNode, create_test_shadow_root_node, set_test_root_element, update, update_with,
};
use super::support::paragraph;
use crate::extension::{
    LexicalEditorWithDispose, SelectBlockConfigOverrides, build_editor_from_extensions,
    select_block_extension,
};
use crate::{
    Editor, InitialEditorStateType, KeyboardEvent, LexicalExtension, NodeClass, NodeRef,
    SELECT_ALL_COMMAND, config_extension, define_extension, get_root, get_selection, get_slot,
    get_slot_frame, is_block_fully_selected, set_slot,
};
use std::rc::Rc;

/// `$slotContainer(...texts)`.
fn slot_container(texts: &[String]) -> NodeRef {
    let container = create_test_shadow_root_node();
    for text in texts {
        container.append(&[paragraph(text)]).unwrap();
    }
    container
}

fn build(
    name: &str,
    disabled: bool,
    init: impl Fn() -> Result<(), crate::LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| init()))),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            config_extension(
                &select_block_extension(),
                [SelectBlockConfigOverrides {
                    cascade_selection: Some(false),
                    disabled: Some(disabled),
                }],
            ),
        ],
        nodes: vec![NodeClass::of::<TestShadowRootNode>()],
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

/// `setUpSlotEditor({disabled, slotTexts})`.
fn set_up_slot_editor(disabled: bool, slot_texts: &[&str]) -> LexicalEditorWithDispose {
    let texts: Vec<String> = slot_texts.iter().map(|t| (*t).to_owned()).collect();
    build("select-block-slots-test", disabled, move || {
        let host = paragraph("host body");
        set_slot(&host, "title", &slot_container(&texts))?;
        get_root().append(&[paragraph("before"), host, paragraph("after")])?;
        Ok(())
    })
}

fn default_slot_editor() -> LexicalEditorWithDispose {
    set_up_slot_editor(false, &["slot one", "slot two"])
}

fn select_all(editor: &Editor) -> bool {
    editor
        .dispatch_command(
            &SELECT_ALL_COMMAND,
            Rc::new(KeyboardEvent {
                key: "a".to_owned(),
                ctrl_key: true,
                ..KeyboardEvent::default()
            }),
        )
        .unwrap()
}

fn host_paragraph() -> NodeRef {
    let node = get_root().get_child_at_index(1).expect("host paragraph");
    assert!(node.is_element());
    node
}

fn slot_value() -> NodeRef {
    let slot = get_slot(&host_paragraph(), "title").expect("slot value");
    assert_eq!(slot.get_type(), "test_shadow_root");
    slot
}

fn slot_paragraph(index: usize) -> NodeRef {
    let node = slot_value().get_child_at_index(index).unwrap();
    assert!(node.is_element());
    node
}

fn slot_text(index: usize) -> NodeRef {
    let text = slot_paragraph(index).get_first_child().unwrap();
    assert!(text.is_text());
    text
}

fn fully_selected(block: &NodeRef) -> bool {
    let range = get_selection()
        .and_then(|s| s.range())
        .expect("expecting a RangeSelection");
    is_block_fully_selected(block, &range).unwrap()
}

/// `$expectSelectionInSlotFrame()`.
fn expect_selection_in_slot_frame() {
    let range = get_selection().and_then(|s| s.range()).unwrap();
    let value = slot_value();
    for point in [&range.anchor, &range.focus] {
        let frame = get_slot_frame(&point.get_node().unwrap()).expect("inside the slot frame");
        assert_eq!(frame.get_key(), value.get_key());
    }
}

fn caret_in_slot(editor: &Editor) {
    update(editor, || {
        slot_text(0).select(Some(2), Some(2))?;
        Ok(())
    });
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > select all inside a slot selects exactly the in-slot block
#[test]
fn select_all_inside_a_slot_selects_the_in_slot_block() {
    let editor = default_slot_editor();
    caret_in_slot(&editor);
    assert!(select_all(&editor));
    editor
        .read(|| {
            assert!(fully_selected(&slot_paragraph(0)));
            assert!(!fully_selected(&slot_value()));
            expect_selection_in_slot_frame();
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > repeated select all expands block, then slot, then document
#[test]
fn repeated_select_all_expands_block_slot_document() {
    let editor = default_slot_editor();
    caret_in_slot(&editor);
    select_all(&editor);
    editor
        .read(|| {
            assert!(fully_selected(&slot_paragraph(0)));
            assert!(!fully_selected(&slot_value()));
        })
        .unwrap();
    assert!(select_all(&editor));
    editor
        .read(|| {
            assert!(fully_selected(&slot_value()));
            assert!(fully_selected(&slot_paragraph(0)));
            assert!(fully_selected(&slot_paragraph(1)));
            assert!(!fully_selected(&get_root()));
            expect_selection_in_slot_frame();
        })
        .unwrap();
    // The cross-frame comparison returns false rather than failing.
    assert!(select_all(&editor));
    editor
        .read(|| {
            assert!(fully_selected(&get_root()));
            let range = get_selection().and_then(|s| s.range()).unwrap();
            assert!(get_slot_frame(&range.anchor.get_node().unwrap()).is_none());
            assert!(get_slot_frame(&range.focus.get_node().unwrap()).is_none());
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > a one-block slot escalates block, then document, with no dead press
#[test]
fn a_one_block_slot_escalates_block_then_document() {
    let editor = set_up_slot_editor(false, &["only block"]);
    caret_in_slot(&editor);
    select_all(&editor);
    assert!(editor.read(|| fully_selected(&slot_paragraph(0))).unwrap());
    select_all(&editor);
    assert!(editor.read(|| fully_selected(&get_root())).unwrap());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > $isBlockFullySelected is slot-frame aware instead of throwing
#[test]
fn is_block_fully_selected_is_slot_frame_aware() {
    let editor = default_slot_editor();
    let results = update_with(&editor, || {
        // A selection inside a slot never fully selects a block outside
        // its frame (and does not fail on the comparison).
        let in_slot = slot_text(0).select(Some(2), Some(2))?.range().unwrap();
        let a = is_block_fully_selected(&get_root(), &in_slot)?;
        let size = slot_text(0).get_text_content_size();
        let spanning = slot_text(0).select(Some(0), Some(size))?.range().unwrap();
        let b = is_block_fully_selected(&slot_paragraph(0), &spanning)?;
        let c = is_block_fully_selected(&slot_paragraph(1), &spanning)?;
        // And the other direction.
        let outside_text = get_root().get_first_descendant().unwrap();
        assert!(outside_text.is_text());
        let size = outside_text.get_text_content_size();
        let outside = outside_text.select(Some(0), Some(size))?.range().unwrap();
        let d = is_block_fully_selected(&slot_paragraph(0), &outside)?;
        Ok([a, b, c, d])
    });
    assert_eq!(results, [false, true, false, false]);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > a block-shaped slot value escalates value, then document
#[test]
fn a_block_shaped_slot_value_escalates_value_then_document() {
    let editor = build("select-block-line-slot-test", false, || {
        let host = paragraph("host body");
        set_slot(&host, "title", &paragraph("the line"))?;
        get_root().append(&[paragraph("before"), host])?;
        Ok(())
    });
    update(&editor, || {
        let host = get_root().get_child_at_index(1).unwrap();
        let line = get_slot(&host, "title").unwrap();
        assert!(line.is_element());
        let text = line.get_first_child().unwrap();
        assert!(text.is_text());
        text.select(Some(3), Some(3))?;
        Ok(())
    });
    select_all(&editor);
    editor
        .read(|| {
            let selection = get_selection().unwrap();
            assert_eq!(selection.get_text_content().unwrap(), "the line");
            let range = selection.range().unwrap();
            assert!(get_slot_frame(&range.anchor.get_node().unwrap()).is_some());
            assert!(!is_block_fully_selected(&get_root(), &range).unwrap());
        })
        .unwrap();
    select_all(&editor);
    assert!(editor.read(|| fully_selected(&get_root())).unwrap());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > with SelectBlockExtension disabled, the rich-text default scopes select all to the slot
#[test]
fn disabled_rich_text_default_scopes_select_all_to_the_slot() {
    let editor = set_up_slot_editor(true, &["slot one", "slot two"]);
    caret_in_slot(&editor);
    assert!(select_all(&editor));
    editor
        .read(|| {
            assert!(fully_selected(&slot_value()));
            assert!(fully_selected(&slot_paragraph(0)));
            assert!(fully_selected(&slot_paragraph(1)));
            assert!(!fully_selected(&get_root()));
            expect_selection_in_slot_frame();
        })
        .unwrap();
    // Outside a slot the default selects the whole document.
    update(&editor, || {
        get_root()
            .get_first_descendant()
            .unwrap()
            .select(Some(3), Some(3))?;
        Ok(())
    });
    assert!(select_all(&editor));
    assert!(editor.read(|| fully_selected(&get_root())).unwrap());
    editor.dispose();
}
