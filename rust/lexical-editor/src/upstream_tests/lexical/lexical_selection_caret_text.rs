//! Ports of `packages/lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts`.

use crate::{
    CreateEditorArgs, LexicalError, PointType, UpdateOptions, Which, create_editor,
    create_paragraph_node, create_range_selection, create_text_node, get_root, set_slot,
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

/// The `[start, end]` points: `[focus, anchor]` when backward.
fn ends(backward: bool) -> (Which, Which) {
    if backward {
        (Which::Focus, Which::Anchor)
    } else {
        (Which::Anchor, Which::Focus)
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::selected text ends at the same boundary for an element or text point (backward=false)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::selected text ends at the same boundary for an element or text point (backward=true)
#[test]
fn selected_text_ends_at_the_same_boundary_for_an_element_or_text_point() {
    for backward in [false, true] {
        run(move || {
            let first = create_text_node("abcd");
            let second = create_text_node("efgh");
            second.set_format_type("italic");
            let third = create_text_node("ijkl");
            third.set_format_type("bold");
            let paragraph = create_paragraph_node();
            paragraph.append(&[first.clone(), second.clone(), third])?;
            get_root().clear()?;
            get_root().append(std::slice::from_ref(&paragraph))?;
            let selection = create_range_selection();
            let (start, end) = ends(backward);
            selection.point_set(start, first.get_key(), 1, PointType::Text)?;
            selection.point_set(end, second.get_key(), 4, PointType::Text)?;
            assert_eq!(selection.get_text_content()?, "bcdefgh");
            selection.point_set(end, paragraph.get_key(), 2, PointType::Element)?;
            assert_eq!(selection.get_text_content()?, "bcdefgh");
            assert_eq!(paragraph.get_text_content(), "abcdefghijkl");
            Ok(())
        });
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::equivalent mixed text and element points contain no selected text (backward=false)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::equivalent mixed text and element points contain no selected text (backward=true)
#[test]
fn equivalent_mixed_text_and_element_points_contain_no_selected_text() {
    for backward in [false, true] {
        run(move || {
            let first = create_text_node("abcd");
            let second = create_text_node("efgh");
            second.set_format_type("italic");
            let paragraph = create_paragraph_node();
            paragraph.append(&[first.clone(), second])?;
            get_root().clear()?;
            get_root().append(std::slice::from_ref(&paragraph))?;
            let selection = create_range_selection();
            let (start, end) = ends(backward);
            selection.point_set(start, first.get_key(), 4, PointType::Text)?;
            selection.point_set(end, paragraph.get_key(), 1, PointType::Element)?;
            assert_eq!(selection.get_text_content()?, "");
            assert_eq!(paragraph.get_text_content(), "abcdefgh");
            Ok(())
        });
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::collapsed element point beside a slot host contains no text (offset=0)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::collapsed element point beside a slot host contains no text (offset=1)
#[test]
fn collapsed_element_point_beside_a_slot_host_contains_no_text() {
    for offset in [0, 1] {
        run(move || {
            let root = get_root();
            let host = create_paragraph_node();
            root.clear()?;
            root.append(std::slice::from_ref(&host))?;
            let title = create_paragraph_node();
            title.append(&[create_text_node("Title")])?;
            set_slot(&host, "title", &title)?;
            let selection = create_range_selection();
            selection.point_set(Which::Anchor, root.get_key(), offset, PointType::Element)?;
            selection.point_set(Which::Focus, root.get_key(), offset, PointType::Element)?;
            assert_eq!(
                selection.get_nodes()?,
                std::slice::from_ref(&host),
                "offset={offset}"
            );
            assert_eq!(selection.get_text_content()?, "");
            assert_eq!(host.get_text_content(), "Title");
            Ok(())
        });
    }
}
