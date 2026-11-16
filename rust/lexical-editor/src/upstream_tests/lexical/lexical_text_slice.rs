//! Ports of `packages/lexical/src/caret/__tests__/unit/LexicalTextSlice.test.ts`.

use crate::constants::IS_BOLD;
use crate::selection_format::split_text_point_caret_slice;
use crate::{
    CaretDirection, CreateEditorArgs, LexicalError, UpdateOptions, create_editor,
    create_paragraph_node, create_text_node, get_root, get_text_point_caret,
    get_text_point_caret_slice,
};

/// `buildEditorFromExtensions({name: 'caret-selection-test'})`, then a
/// discrete update.
fn run(f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    let editor = create_editor(CreateEditorArgs {
        namespace: "caret-selection-test".into(),
        ..CreateEditorArgs::default()
    });
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

// upstream: lexical/src/caret/__tests__/unit/LexicalTextSlice.test.ts::split a next text slice
// upstream: lexical/src/caret/__tests__/unit/LexicalTextSlice.test.ts::split a previous text slice
#[test]
fn split_a_text_slice() {
    for direction in [CaretDirection::Next, CaretDirection::Previous] {
        run(move || {
            let text = create_text_node("abcde");
            text.set_format_type("bold");
            text.set_style("color: red;");
            let paragraph = create_paragraph_node();
            paragraph.append(std::slice::from_ref(&text))?;
            get_root().clear()?;
            get_root().append(std::slice::from_ref(&paragraph))?;
            let next = direction == CaretDirection::Next;
            let caret = get_text_point_caret(&text, direction, if next { 1 } else { 4 })?;
            let slice = get_text_point_caret_slice(caret, if next { 3 } else { -3 });
            let extracted = split_text_point_caret_slice(&slice, None)?.unwrap();
            let texts: Vec<String> = paragraph
                .get_children()
                .iter()
                .map(|n| n.get_text_content())
                .collect();
            assert_eq!(texts, ["a", "bcd", "e"], "{direction:?}");
            assert_eq!(extracted.get_text_content(), "bcd");
            assert_eq!(extracted.text_data().format, f64::from(IS_BOLD));
            assert_eq!(extracted.text_data().style, "color: red;");
            Ok(())
        });
    }
}

// upstream: lexical/src/caret/__tests__/unit/LexicalTextSlice.test.ts::empty and full text slices leave the node intact
#[test]
fn empty_and_full_text_slices_leave_the_node_intact() {
    run(|| {
        let text = create_text_node("abcde");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        let empty =
            get_text_point_caret_slice(get_text_point_caret(&text, CaretDirection::Next, 2)?, 0);
        assert_eq!(split_text_point_caret_slice(&empty, None)?, None);
        let full = get_text_point_caret_slice(
            get_text_point_caret(&text, CaretDirection::Previous, 5)?,
            -5,
        );
        assert_eq!(
            split_text_point_caret_slice(&full, None)?,
            Some(text.clone())
        );
        assert_eq!(paragraph.get_children(), [text]);
        Ok(())
    });
}
