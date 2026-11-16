//! Ports of
//! `packages/lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts`.

#![cfg(feature = "extension")]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::selection::{StylePatchValue, patch_style_text};
use crate::upstream_tests::support::update;
use crate::{
    LexicalExtension, NodeRef, PointType, SelectionRef, Which, create_node_selection,
    create_paragraph_node, create_range_selection, create_text_node, define_extension, get_root,
    get_selection, set_selection,
};

#[derive(Clone, Copy, Debug)]
enum Active {
    None,
    Collapsed,
    Range,
}

fn build(name: &str) -> LexicalEditorWithDispose {
    build_editor_from_extensions([
        define_extension(LexicalExtension::<(), (), ()>::new(name, ())).into(),
    ])
    .unwrap()
}

fn style_of(node: &NodeRef) -> String {
    node.text_data().get_style().to_owned()
}

fn patch(property: &str, value: &str) -> [(String, StylePatchValue); 1] {
    [(property.to_owned(), value.into())]
}

/// `$createFixture(active)`'s nodes and selections.
struct Fixture {
    text: NodeRef,
    paragraph: NodeRef,
    other: NodeRef,
    active_selection: Option<SelectionRef>,
    original_active: Option<SelectionRef>,
}

/// `$createFixture(active)`.
fn create_fixture(active: Active) -> Fixture {
    let text = create_text_node("abcdef");
    let paragraph = create_paragraph_node();
    paragraph.append(std::slice::from_ref(&text)).unwrap();
    let other = create_text_node("xyz");
    let other_paragraph = create_paragraph_node();
    other_paragraph
        .append(std::slice::from_ref(&other))
        .unwrap();
    get_root()
        .append(&[paragraph.clone(), other_paragraph])
        .unwrap();
    let active_selection = match active {
        Active::None => None,
        Active::Collapsed => Some(other.select(Some(0), Some(0)).unwrap()),
        Active::Range => Some(other.select(Some(0), Some(2)).unwrap()),
    };
    set_selection(active_selection.clone());
    let original_active = active_selection.as_ref().map(SelectionRef::clone_selection);
    Fixture {
        text,
        paragraph,
        other,
        active_selection,
        original_active,
    }
}

impl Fixture {
    /// `$expectActiveUnchanged()`.
    fn expect_active_unchanged(&self) {
        assert_eq!(
            self.other.get_parent_or_throw().unwrap().get_children(),
            std::slice::from_ref(&self.other)
        );
        assert_eq!(style_of(&self.other), "");
        let current = get_selection();
        match (&current, &self.active_selection) {
            (None, None) => {}
            (Some(current), Some(active)) => assert!(current.ptr_eq(active)),
            _ => panic!("the active selection changed"),
        }
        if let (Some(active), Some(original)) = (&self.active_selection, &self.original_active) {
            assert!(active.get().is(&original.get()));
        }
    }
}

/// `repeatedly styles a detached text range (active=$active,
/// backward=$backward, partial=$partial)`.
fn repeatedly_styles_a_detached_text_range(active: Active, backward: bool, partial: bool) {
    let editor = build("detached-style-range");
    update(&editor, move || {
        let fixture = create_fixture(active);
        let (text, paragraph) = (&fixture.text, &fixture.paragraph);
        let selection = create_range_selection();
        let (start, end) = if backward {
            (Which::Focus, Which::Anchor)
        } else {
            (Which::Anchor, Which::Focus)
        };
        selection.point_set(
            start,
            text.get_key(),
            if partial { 2 } else { 0 },
            PointType::Text,
        )?;
        selection.point_set(
            end,
            text.get_key(),
            if partial { 4 } else { 6 },
            PointType::Text,
        )?;
        let expected_text = if partial { "cd" } else { "abcdef" };
        patch_style_text(&selection, &patch("color", "red"))?;
        let styled = paragraph
            .get_child_at_index(if partial { 1 } else { 0 })
            .unwrap();
        assert!(styled.is_text());
        // The iterator preserves its existing forward single-text convention.
        let range = selection.range().unwrap();
        assert_eq!(
            (
                range.anchor.key.as_str(),
                range.anchor.offset,
                range.anchor.point_type
            ),
            (styled.get_key(), 0, PointType::Text)
        );
        assert_eq!(
            (
                range.focus.key.as_str(),
                range.focus.offset,
                range.focus.point_type
            ),
            (styled.get_key(), expected_text.len(), PointType::Text)
        );
        assert_eq!(selection.get_text_content()?, expected_text);
        assert_eq!(selection.get_nodes()?, std::slice::from_ref(&styled));
        patch_style_text(&selection, &patch("font-size", "20px"))?;
        assert_eq!(style_of(&styled), "color: red;font-size: 20px;");
        assert_eq!(selection.get_text_content()?, expected_text);
        assert_eq!(paragraph.get_text_content(), "abcdef");
        if partial {
            let first = paragraph.get_first_child_or_throw()?;
            assert!(first.is_text());
            assert_eq!(style_of(&first), "");
            let last = paragraph.get_last_child_or_throw()?;
            assert!(last.is_text());
            assert_eq!(style_of(&last), "");
        }
        fixture.expect_active_unchanged();
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=none, backward=false, partial=false)
#[test]
fn repeatedly_styles_a_detached_text_range_active_none_backward_false_partial_false() {
    repeatedly_styles_a_detached_text_range(Active::None, false, false);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=none, backward=false, partial=true)
#[test]
fn repeatedly_styles_a_detached_text_range_active_none_backward_false_partial_true() {
    repeatedly_styles_a_detached_text_range(Active::None, false, true);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=none, backward=true, partial=false)
#[test]
fn repeatedly_styles_a_detached_text_range_active_none_backward_true_partial_false() {
    repeatedly_styles_a_detached_text_range(Active::None, true, false);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=none, backward=true, partial=true)
#[test]
fn repeatedly_styles_a_detached_text_range_active_none_backward_true_partial_true() {
    repeatedly_styles_a_detached_text_range(Active::None, true, true);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=collapsed, backward=false, partial=false)
#[test]
fn repeatedly_styles_a_detached_text_range_active_collapsed_backward_false_partial_false() {
    repeatedly_styles_a_detached_text_range(Active::Collapsed, false, false);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=collapsed, backward=false, partial=true)
#[test]
fn repeatedly_styles_a_detached_text_range_active_collapsed_backward_false_partial_true() {
    repeatedly_styles_a_detached_text_range(Active::Collapsed, false, true);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=collapsed, backward=true, partial=false)
#[test]
fn repeatedly_styles_a_detached_text_range_active_collapsed_backward_true_partial_false() {
    repeatedly_styles_a_detached_text_range(Active::Collapsed, true, false);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=collapsed, backward=true, partial=true)
#[test]
fn repeatedly_styles_a_detached_text_range_active_collapsed_backward_true_partial_true() {
    repeatedly_styles_a_detached_text_range(Active::Collapsed, true, true);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=range, backward=false, partial=false)
#[test]
fn repeatedly_styles_a_detached_text_range_active_range_backward_false_partial_false() {
    repeatedly_styles_a_detached_text_range(Active::Range, false, false);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=range, backward=false, partial=true)
#[test]
fn repeatedly_styles_a_detached_text_range_active_range_backward_false_partial_true() {
    repeatedly_styles_a_detached_text_range(Active::Range, false, true);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=range, backward=true, partial=false)
#[test]
fn repeatedly_styles_a_detached_text_range_active_range_backward_true_partial_false() {
    repeatedly_styles_a_detached_text_range(Active::Range, true, false);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=range, backward=true, partial=true)
#[test]
fn repeatedly_styles_a_detached_text_range_active_range_backward_true_partial_true() {
    repeatedly_styles_a_detached_text_range(Active::Range, true, true);
}

/// `styles a detached caret without styling active text (%s)`.
fn styles_a_detached_caret_without_styling_active_text(active: Active) {
    let editor = build("detached-style-caret");
    update(&editor, move || {
        let fixture = create_fixture(active);
        let text = &fixture.text;
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, text.get_key(), 2, PointType::Text)?;
        selection.point_set(Which::Focus, text.get_key(), 2, PointType::Text)?;
        patch_style_text(&selection, &patch("color", "red"))?;
        assert_eq!(selection.range().unwrap().style, "color: red;");
        assert_eq!(style_of(text), "");
        fixture.expect_active_unchanged();
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached caret without styling active text (none)
#[test]
fn styles_a_detached_caret_without_styling_active_text_none() {
    styles_a_detached_caret_without_styling_active_text(Active::None);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached caret without styling active text (collapsed)
#[test]
fn styles_a_detached_caret_without_styling_active_text_collapsed() {
    styles_a_detached_caret_without_styling_active_text(Active::Collapsed);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached caret without styling active text (range)
#[test]
fn styles_a_detached_caret_without_styling_active_text_range() {
    styles_a_detached_caret_without_styling_active_text(Active::Range);
}

/// `styles a detached node selection (%s)`.
fn styles_a_detached_node_selection(active: Active) {
    let editor = build("detached-style-node");
    update(&editor, move || {
        let fixture = create_fixture(active);
        let selection = create_node_selection();
        selection.add(fixture.text.get_key());
        patch_style_text(&selection, &patch("color", "red"))?;
        assert_eq!(style_of(&fixture.text), "color: red;");
        fixture.expect_active_unchanged();
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached node selection (none)
#[test]
fn styles_a_detached_node_selection_none() {
    styles_a_detached_node_selection(Active::None);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached node selection (collapsed)
#[test]
fn styles_a_detached_node_selection_collapsed() {
    styles_a_detached_node_selection(Active::Collapsed);
}

// upstream: lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached node selection (range)
#[test]
fn styles_a_detached_node_selection_range() {
    styles_a_detached_node_selection(Active::Range);
}
