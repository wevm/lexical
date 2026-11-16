//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts`.
//!
//! Upstream runs the real `createRectsFromDOMRange` on a faked root (1200
//! wide, no padding) and a faked range; its filtering, which is what those
//! tests characterize, is [`filter_selection_rects`], given the same root
//! width and padding.

use crate::selection::{SelectionRect, filter_selection_rects};
use crate::utils::dedupe_selection_rects;

fn rect(left: f64, top: f64, width: f64, height: f64) -> SelectionRect {
    SelectionRect::new(left, top, width, height)
}

fn sorted(mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(f64::total_cmp);
    values
}

fn widths(rects: &[SelectionRect]) -> Vec<f64> {
    sorted(rects.iter().map(|r| r.width).collect())
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps one of two identical rects (the doubled-opacity duplicate)
#[test]
fn keeps_one_of_two_identical_rects() {
    let kept = dedupe_selection_rects(&[
        rect(128.0, 166.0, 394.0, 18.0),
        rect(128.0, 166.0, 394.0, 18.0),
    ]);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].width, 394.0);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps the smaller text rect when a wider rect contains it (#7106 extra area)
#[test]
fn keeps_the_smaller_text_rect_when_a_wider_rect_contains_it() {
    let text = rect(128.0, 90.0, 368.0, 40.0);
    let wider = rect(128.0, 90.0, 1024.0, 40.0);
    let kept = dedupe_selection_rects(&[text, wider]);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].width, 368.0);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps the smaller rect regardless of input order
#[test]
fn keeps_the_smaller_rect_regardless_of_input_order() {
    let text = rect(128.0, 90.0, 368.0, 40.0);
    let wider = rect(128.0, 90.0, 1024.0, 40.0);
    assert_eq!(dedupe_selection_rects(&[wider, text])[0].width, 368.0);
    assert_eq!(dedupe_selection_rects(&[text, wider])[0].width, 368.0);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > preserves genuine multi-line rects (different rows are not contained)
#[test]
fn preserves_genuine_multi_line_rects() {
    let row1 = rect(128.0, 90.0, 368.0, 18.0);
    let row2 = rect(128.0, 112.0, 200.0, 18.0);
    let kept = dedupe_selection_rects(&[row1, row1, row2, row2]);
    assert_eq!(kept.len(), 2);
    assert_eq!(sorted(kept.iter().map(|r| r.top).collect()), [90.0, 112.0]);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > drops zero-area rects
#[test]
fn drops_zero_area_rects() {
    let kept = dedupe_selection_rects(&[
        rect(0.0, 0.0, 0.0, 18.0),
        rect(0.0, 0.0, 100.0, 0.0),
        rect(0.0, 0.0, 100.0, 18.0),
    ]);
    assert_eq!(kept.len(), 1);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > returns empty for an empty list
#[test]
fn returns_empty_for_an_empty_list() {
    assert_eq!(dedupe_selection_rects(&[]).len(), 0);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > treats near-identical rects within 1px as duplicates
#[test]
fn treats_near_identical_rects_within_1px_as_duplicates() {
    let kept = dedupe_selection_rects(&[
        rect(128.0, 166.0, 394.0, 18.0),
        SelectionRect {
            bottom: 184.5,
            height: 18.2,
            left: 128.4,
            right: 522.1,
            top: 166.3,
            width: 393.7,
        },
    ]);
    assert_eq!(kept.len(), 1);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps horizontally disjoint rects on the same row (no false containment)
#[test]
fn keeps_horizontally_disjoint_rects_on_the_same_row() {
    // Two inline boxes tiled on one line: neither contains the other.
    let kept = dedupe_selection_rects(&[rect(0.0, 0.0, 50.0, 18.0), rect(100.0, 0.0, 50.0, 18.0)]);
    assert_eq!(kept.len(), 2);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > does not clip a wide rect (e.g. trailing whitespace) against a disjoint narrower row
#[test]
fn does_not_clip_a_wide_rect_against_a_disjoint_narrower_row() {
    let wide_row = rect(0.0, 0.0, 300.0, 18.0);
    let narrow_row = rect(0.0, 22.0, 80.0, 18.0);
    let kept = dedupe_selection_rects(&[wide_row, narrow_row]);
    assert_eq!(kept.len(), 2);
    assert_eq!(widths(&kept), [80.0, 300.0]);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps every row of a ragged multi-row selection (varying offsets, e.g. RTL)
#[test]
fn keeps_every_row_of_a_ragged_multi_row_selection() {
    let rows = [
        rect(200.0, 0.0, 100.0, 18.0),
        rect(150.0, 22.0, 150.0, 18.0),
        rect(180.0, 44.0, 120.0, 18.0),
    ];
    assert_eq!(dedupe_selection_rects(&rows).len(), 3);
}

/// `createRects(fakeEditorWithRoot(1200), rects)`.
fn create_rects(rects: &[SelectionRect]) -> Vec<SelectionRect> {
    filter_selection_rects(rects, 1200.0, 0.0)
}

fn text() -> SelectionRect {
    rect(128.0, 90.5, 368.0, 18.0)
}

/// Not the editor's width (1200): not dropped as full-width.
fn wider() -> SelectionRect {
    rect(128.0, 90.0, 900.0, 18.0)
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects + createRectsFromDOMRange > createRectsFromDOMRange leaves the contained pair (its adjacent-only pass does not catch it)
#[test]
fn create_rects_from_dom_range_leaves_the_contained_pair() {
    assert_eq!(widths(&create_rects(&[text(), wider()])), [368.0, 900.0]);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects + createRectsFromDOMRange > dedupeSelectionRects collapses that survivor pair to the text-hugging rect
#[test]
fn dedupe_selection_rects_collapses_that_survivor_pair() {
    assert_eq!(
        widths(&dedupe_selection_rects(&create_rects(&[text(), wider()]))),
        [368.0]
    );
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects + createRectsFromDOMRange > preserves the partially overlapping pair in either order before tolerant deduplication
#[test]
fn preserves_the_partially_overlapping_pair_in_either_order() {
    assert_eq!(widths(&create_rects(&[text(), wider()])), [368.0, 900.0]);
    assert_eq!(widths(&create_rects(&[wider(), text()])), [368.0, 900.0]);
    assert_eq!(widths(&dedupe_selection_rects(&[text(), wider()])), [368.0]);
    assert_eq!(widths(&dedupe_selection_rects(&[wider(), text()])), [368.0]);
}

// upstream: lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects + createRectsFromDOMRange > keeps the smaller (text) rect by design — keeping the wider one re-introduces the #7106 extra-area paint
#[test]
fn keeps_the_smaller_text_rect_by_design() {
    let kept = dedupe_selection_rects(&create_rects(&[text(), wider()]));
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].width, 368.0);
}
