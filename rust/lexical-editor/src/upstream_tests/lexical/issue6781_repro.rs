//! Ports of `packages/lexical/src/__tests__/unit/Issue6781Repro.test.ts`.

use super::super::support::{initialize_unit_test, update};
use crate::constants::{IS_BOLD, IS_ITALIC};
use crate::{
    CreateEditorArgs, SelectionRef, create_paragraph_node, create_text_node, get_root,
    get_selection,
};

/// jsdom's window as the upstream unit tests install it: after Lexical
/// loaded, so it gives no DOM selection to measure a deletion with
/// (`CAN_USE_DOM` is false).
struct JsdomWindow;

impl crate::SelectionMovement for JsdomWindow {
    fn landed_point(
        &self,
        _from: &crate::Point,
        _is_backward: bool,
        _granularity: crate::SelectionGranularity,
    ) -> Result<Option<crate::Point>, crate::LexicalError> {
        Ok(None)
    }
    fn has_dom_selection(&self) -> bool {
        false
    }
}

fn range_selection() -> SelectionRef {
    let selection = get_selection().filter(|s| s.is_range());
    selection.expect("Expected RangeSelection")
}

// upstream: lexical/src/__tests__/unit/Issue6781Repro.test.ts::Backspacing an empty block updates the pending format (#6781) > merging into the previous block adopts that block text format and style
#[test]
fn merging_into_the_previous_block_adopts_that_block_text_format_and_style() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    editor.set_selection_movement(Some(std::rc::Rc::new(JsdomWindow)));
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let text = create_text_node("hello");
        text.set_format(f64::from(IS_BOLD));
        text.set_style("color: red");
        paragraph.append(&[text])?;
        let empty_paragraph = create_paragraph_node();
        empty_paragraph.set_text_format(f64::from(IS_ITALIC));
        empty_paragraph.set_text_style("color: blue");
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph, empty_paragraph.clone()])?;
        let selection = empty_paragraph.select(Some(0), Some(0))?;
        selection.set_format(f64::from(IS_ITALIC));
        selection.set_style("color: blue");
        Ok(())
    });
    update(&editor, || range_selection().delete_character(true));
    editor
        .read(|| {
            let range = range_selection().range().unwrap();
            assert_eq!(range.format, f64::from(IS_BOLD));
            assert_eq!(range.style, "color: red");
        })
        .unwrap();
    update(&editor, || range_selection().insert_text("X"));
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_text_content(), "helloX");
            let text = root.get_last_descendant().unwrap();
            assert!(text.is_text());
            assert_eq!(text.text_data().get_format(), f64::from(IS_BOLD));
            assert_eq!(text.text_data().get_style(), "color: red");
            let paragraph = root.get_first_child().unwrap();
            assert!(paragraph.is_element());
            assert_eq!(paragraph.get_children_size(), 1);
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/Issue6781Repro.test.ts::Backspacing an empty block updates the pending format (#6781) > backspacing inside a block leaves the pending format alone
#[test]
fn backspacing_inside_a_block_leaves_the_pending_format_alone() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    editor.set_selection_movement(Some(std::rc::Rc::new(JsdomWindow)));
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let text = create_text_node("hello");
        text.set_format(f64::from(IS_BOLD));
        text.set_style("color: red");
        paragraph.append(std::slice::from_ref(&text))?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])?;
        let selection = text.select(Some(5), Some(5))?;
        selection.set_format(f64::from(IS_ITALIC));
        selection.set_style("color: blue");
        Ok(())
    });
    update(&editor, || range_selection().delete_character(true));
    editor
        .read(|| {
            let range = range_selection().range().unwrap();
            assert_eq!(range.format, f64::from(IS_ITALIC));
            assert_eq!(range.style, "color: blue");
        })
        .unwrap();
}
