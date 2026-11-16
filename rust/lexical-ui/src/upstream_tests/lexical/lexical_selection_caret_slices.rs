//! Ports of `packages/lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts`.

use crate::constants::{IS_BOLD, IS_ITALIC};
use crate::{
    CreateEditorArgs, LexicalError, NodeRef, PointType, SelectionRef, UpdateOptions, Which,
    create_editor, create_paragraph_node, create_range_selection, create_text_node, get_root,
    get_selection, set_selection,
};

/// `buildEditorFromExtensions({name})`, then a discrete update.
fn run(name: &str, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    let editor = create_editor(CreateEditorArgs {
        namespace: name.into(),
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

/// `$createTextNode(text).setFormat(format)`.
fn text_with_format(text: &str, format: &str) -> NodeRef {
    let node = create_text_node(text);
    node.set_format_type(format);
    node
}

/// `$createParagraphNode().append(...children)`.
fn paragraph(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(children)?;
    Ok(paragraph)
}

/// `$getRoot().clear().append(...children)`.
fn set_root(children: &[NodeRef]) -> Result<(), LexicalError> {
    get_root().clear()?;
    get_root().append(children)
}

/// The `[start, end]` points: `[focus, anchor]` when backward.
fn ends(backward: bool) -> (Which, Which) {
    if backward {
        (Which::Focus, Which::Anchor)
    } else {
        (Which::Anchor, Which::Focus)
    }
}

fn point(selection: &SelectionRef, which: Which) -> crate::Point {
    selection.point(which).expect("a range selection")
}

/// `expect($getSelection()).toBe(selection)`.
fn assert_active(selection: &SelectionRef) {
    assert!(get_selection().is_some_and(|s| s.ptr_eq(selection)));
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > format of a detached text range repairs the active element point
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > extract of a detached text range repairs the active element point
#[test]
fn detached_text_range_repairs_the_active_element_point() {
    for operation in ["format", "extract"] {
        run("slice-active-element", move || {
            let text = create_text_node("abcde");
            let following = text_with_format("after", "italic");
            let paragraph = paragraph(&[text.clone(), following.clone()])?;
            set_root(std::slice::from_ref(&paragraph))?;
            let active = paragraph.select(Some(2), Some(2))?;
            let selection = create_range_selection();
            selection.point_set(Which::Anchor, text.get_key(), 1, PointType::Text)?;
            selection.point_set(Which::Focus, text.get_key(), 4, PointType::Text)?;

            if operation == "format" {
                selection.format_text("bold", None)?;
            } else {
                selection.extract()?;
            }

            assert_eq!(selection.get_text_content()?, "bcd", "{operation}");
            assert_active(&active);
            assert!(active.range().unwrap().is_collapsed());
            let anchor = point(&active, Which::Anchor);
            assert_eq!(anchor.key, following.get_key(), "{operation}");
            assert_eq!(anchor.point_type, PointType::Text);
            assert_eq!(anchor.offset, 5);
            Ok(())
        });
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > styling retains the selected prefix at an element start (backward=false)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > styling retains the selected prefix at an element start (backward=true)
#[cfg(feature = "selection")]
#[test]
fn styling_retains_the_selected_prefix_at_an_element_start() {
    use crate::selection::{StylePatchValue, patch_style_text};
    for backward in [false, true] {
        run("slice-style", move || {
            let text = create_text_node("abc");
            let paragraph = paragraph(std::slice::from_ref(&text))?;
            set_root(std::slice::from_ref(&paragraph))?;
            let selection = create_range_selection();
            let (start, end) = ends(backward);
            selection.point_set(start, paragraph.get_key(), 0, PointType::Element)?;
            selection.point_set(end, text.get_key(), 1, PointType::Text)?;
            set_selection(Some(selection.clone()));

            patch_style_text(&selection, &[("color", StylePatchValue::from("red"))])?;
            assert_eq!(selection.get_text_content()?, "a", "backward={backward}");
            patch_style_text(&selection, &[("color", StylePatchValue::from("blue"))])?;
            assert_eq!(
                paragraph.get_first_child_or_throw()?.get_text_content(),
                "a"
            );
            assert_eq!(
                point(&selection, Which::Anchor)
                    .get_node()?
                    .text_data()
                    .style,
                "color: blue;"
            );
            assert_eq!(
                paragraph.get_last_child_or_throw()?.get_text_content(),
                "bc"
            );
            Ok(())
        });
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > format retains detached element endpoints after a split
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > extract retains detached element endpoints after a split
#[test]
fn retains_detached_element_endpoints_after_a_split() {
    for operation in ["format", "extract"] {
        run("slice-detached-end", move || {
            let first = create_text_node("abc");
            let second = text_with_format("def", "italic");
            let paragraph = paragraph(&[first.clone(), second])?;
            let other = create_text_node("other");
            set_root(&[
                paragraph.clone(),
                self::paragraph(std::slice::from_ref(&other))?,
            ])?;
            let active = other.select(Some(2), Some(2))?;
            let selection = create_range_selection();
            selection.point_set(Which::Anchor, first.get_key(), 1, PointType::Text)?;
            selection.point_set(Which::Focus, paragraph.get_key(), 2, PointType::Element)?;

            if operation == "format" {
                selection.format_text("bold", None)?;
            } else {
                selection.extract()?;
            }
            assert_eq!(selection.get_text_content()?, "bcdef", "{operation}");
            let focus = point(&selection, Which::Focus);
            let focus_node = focus.get_node()?;
            let limit = if focus.point_type == PointType::Element {
                focus_node.get_children_size()
            } else {
                focus_node.get_text_content_size()
            };
            assert!(focus.offset <= limit, "{operation}");
            assert_active(&active);
            let anchor = point(&active, Which::Anchor);
            assert_eq!(anchor.key, other.get_key());
            assert_eq!(anchor.offset, 2);
            Ok(())
        });
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > extracts through an element endpoint between children (backward=false)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > extracts through an element endpoint between children (backward=true)
#[test]
fn extracts_through_an_element_endpoint_between_children() {
    for backward in [false, true] {
        run("selection-text-slices", move || {
            let first = create_text_node("abcd");
            let second = text_with_format("efgh", "italic");
            let third = text_with_format("ijkl", "bold");
            let paragraph = paragraph(&[first.clone(), second, third])?;
            set_root(std::slice::from_ref(&paragraph))?;
            let selection = create_range_selection();
            let (start, end) = ends(backward);
            selection.point_set(start, first.get_key(), 1, PointType::Text)?;
            selection.point_set(end, paragraph.get_key(), 2, PointType::Element)?;
            set_selection(Some(selection.clone()));

            let extracted: Vec<String> = selection
                .extract()?
                .iter()
                .filter(|n| n.is_text())
                .map(NodeRef::get_text_content)
                .collect();
            assert_eq!(extracted, ["bcd", "efgh"], "backward={backward}");
            assert_eq!(paragraph.get_text_content(), "abcdefghijkl");
            assert_eq!(selection.is_backward()?, backward);
            Ok(())
        });
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > does not extract text from an empty mixed-point range (backward=false)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > does not extract text from an empty mixed-point range (backward=true)
#[test]
fn does_not_extract_text_from_an_empty_mixed_point_range() {
    for backward in [false, true] {
        run("selection-text-slices", move || {
            let text = create_text_node("abcd");
            let following = text_with_format("efgh", "italic");
            let paragraph = paragraph(&[text.clone(), following.clone()])?;
            set_root(std::slice::from_ref(&paragraph))?;
            let selection = create_range_selection();
            let (element_point, text_point) = ends(backward);
            selection.point_set(element_point, paragraph.get_key(), 1, PointType::Element)?;
            selection.point_set(text_point, text.get_key(), 4, PointType::Text)?;
            set_selection(Some(selection.clone()));

            assert_eq!(selection.extract()?, [], "backward={backward}");
            assert_eq!(paragraph.get_children(), [text.clone(), following]);
            assert_eq!(text.get_text_content(), "abcd");
            Ok(())
        });
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > pending format excludes a final node selected at offset zero
#[test]
fn pending_format_excludes_a_final_node_selected_at_offset_zero() {
    run("selection-text-slices", || {
        let first = create_text_node("abcd");
        first.set_format(f64::from(IS_BOLD));
        let second = create_text_node("efgh");
        second.set_format(f64::from(IS_ITALIC));
        let paragraph = paragraph(&[first.clone(), second.clone()])?;
        set_root(std::slice::from_ref(&paragraph))?;
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, first.get_key(), 0, PointType::Text)?;
        selection.point_set(Which::Focus, second.get_key(), 0, PointType::Text)?;
        selection.set_format(f64::from(IS_BOLD));
        set_selection(Some(selection.clone()));

        selection.format_text("bold", None)?;
        assert_eq!(first.text_data().format, 0.0);
        assert_eq!(second.text_data().format, f64::from(IS_ITALIC));
        assert_eq!(selection.range().unwrap().format, 0.0);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting a detached selection retains its partial text range
#[test]
fn formatting_a_detached_selection_retains_its_partial_text_range() {
    run("selection-text-slices", || {
        let text = create_text_node("abcde");
        let other = create_text_node("other");
        let paragraph = paragraph(std::slice::from_ref(&text))?;
        set_root(&[
            paragraph.clone(),
            self::paragraph(std::slice::from_ref(&other))?,
        ])?;
        let active = other.select(Some(2), Some(2))?;
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, text.get_key(), 4, PointType::Text)?;
        selection.point_set(Which::Focus, text.get_key(), 1, PointType::Text)?;

        selection.format_text("bold", None)?;
        let children: Vec<(String, Option<f64>)> = paragraph
            .get_children()
            .iter()
            .map(|n| {
                (
                    n.get_text_content(),
                    n.is_text().then(|| n.text_data().format),
                )
            })
            .collect();
        assert_eq!(
            children,
            [
                ("a".to_owned(), Some(0.0)),
                ("bcd".to_owned(), Some(f64::from(IS_BOLD))),
                ("e".to_owned(), Some(0.0)),
            ]
        );
        let anchor = point(&selection, Which::Anchor);
        assert_eq!(anchor.get_node()?.get_text_content(), "bcd");
        assert_eq!(anchor.offset, 3);
        assert_eq!(point(&selection, Which::Focus).offset, 0);
        assert_active(&active);
        let active_anchor = point(&active, Which::Anchor);
        assert_eq!(active_anchor.key, other.get_key());
        assert_eq!(active_anchor.offset, 2);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=false, preceding=false, end=1)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=false, preceding=false, end=3)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=false, preceding=true, end=1)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=false, preceding=true, end=3)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=true, preceding=false, end=1)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=true, preceding=false, end=3)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=true, preceding=true, end=1)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=true, preceding=true, end=3)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=false, preceding=false, end=1)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=false, preceding=false, end=3)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=false, preceding=true, end=1)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=false, preceding=true, end=3)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=true, preceding=false, end=1)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=true, preceding=false, end=3)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=true, preceding=true, end=1)
// upstream: lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=true, preceding=true, end=3)
#[test]
fn formatting_retains_an_element_start() {
    for backward in [false, true] {
        for active in [false, true] {
            for preceding in [false, true] {
                for end_offset in [1, 3] {
                    formatting_retains_an_element_start_case(
                        backward, active, preceding, end_offset,
                    );
                }
            }
        }
    }
}

fn formatting_retains_an_element_start_case(
    backward: bool,
    active: bool,
    preceding: bool,
    end_offset: usize,
) {
    let case =
        format!("backward={backward}, active={active}, preceding={preceding}, end={end_offset}");
    run("selection-text-slices", move || {
        let text = create_text_node("abc");
        let paragraph = create_paragraph_node();
        if preceding {
            paragraph.append(&[text_with_format("before", "italic")])?;
        }
        paragraph.append(&[text.clone(), create_text_node("def")])?;
        let other = create_text_node("other");
        set_root(&[
            paragraph.clone(),
            self::paragraph(std::slice::from_ref(&other))?,
        ])?;
        let unrelated = other.select(Some(2), Some(2))?;
        let selection = create_range_selection();
        let (start, end) = ends(backward);
        selection.point_set(
            start,
            paragraph.get_key(),
            usize::from(preceding),
            PointType::Element,
        )?;
        selection.point_set(end, text.get_key(), end_offset, PointType::Text)?;
        if active {
            set_selection(Some(selection.clone()));
        }
        let expected_text = &"abc"[..end_offset];
        for expected_format in [IS_BOLD, 0] {
            selection.format_text("bold", None)?;
            let formatted = paragraph
                .get_child_at_index(usize::from(preceding))
                .expect("a child");
            assert!(formatted.is_text(), "{case}");
            assert_eq!(
                formatted.text_data().format,
                f64::from(expected_format),
                "{case}"
            );
            assert_eq!(formatted.get_text_content(), expected_text, "{case}");
            assert_eq!(selection.get_text_content()?, expected_text, "{case}");
            let start_point = point(&selection, start);
            let expected_start_key = if end_offset == 3 {
                paragraph.get_key()
            } else {
                formatted.get_key()
            };
            assert_eq!(start_point.key, expected_start_key, "{case}");
            assert_eq!(
                start_point.point_type,
                if end_offset == 3 {
                    PointType::Element
                } else {
                    PointType::Text
                },
                "{case}"
            );
            assert_eq!(
                start_point.offset,
                usize::from(end_offset == 3 && preceding),
                "{case}"
            );
            let end_point = point(&selection, end);
            assert_eq!(end_point.key, formatted.get_key(), "{case}");
            assert_eq!(end_point.offset, end_offset, "{case}");
            assert_eq!(selection.is_backward()?, backward, "{case}");
            if !active {
                assert_active(&unrelated);
                let anchor = point(&unrelated, Which::Anchor);
                assert_eq!(anchor.key, other.get_key(), "{case}");
                assert_eq!(anchor.offset, 2, "{case}");
            }
        }
        Ok(())
    });
}
