//! `packages/lexical-selection/src/utils.ts`: the model part, with the
//! computed styles read from the root element
//! ([`crate::RootElement::computed_style`]). The DOM ranges and rects
//! (`createDOMRange`, `createRectsFromDOMRange`) need a DOM.

use crate::node::NodeRef;
use crate::set_dom_style::{StyleObject, get_style_object_from_css};

/// The properties of a computed style (`CSSStyleDeclaration`) that
/// `@lexical/selection` reads: `direction` and `writingMode`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputedStyle {
    /// `direction`: `ltr` or `rtl`.
    pub direction: String,
    /// `writingMode`: `horizontal-tb`, `vertical-rl`...
    pub writing_mode: String,
}

impl Default for ComputedStyle {
    /// `ltr` and `horizontal-tb`, CSS's initial values.
    fn default() -> Self {
        Self {
            direction: "ltr".to_owned(),
            writing_mode: "horizontal-tb".to_owned(),
        }
    }
}

/// `$getComputedStyleForElement(element)`: the computed style of the
/// element's rendered DOM, which the editor's root element reports
/// ([`crate::RootElement::computed_style`], as JS reads
/// `getComputedStyle(editor.getElementByKey(key))`). `None` without a root
/// element, an active editor, or a style for the node.
pub fn get_computed_style_for_element(element: &NodeRef) -> Option<ComputedStyle> {
    let editor = crate::updates::get_active_editor()?;
    editor
        .get_root_element()?
        .0
        .computed_style(element.get_key())
}

/// `$getComputedStyleForParent(node)`: the computed style of the node's
/// parent, or of the root itself. A named slot's value and a detached
/// node have no parent, so `None`.
pub fn get_computed_style_for_parent(node: &NodeRef) -> Option<ComputedStyle> {
    let parent = if node.is_root() {
        Some(node.clone())
    } else {
        node.get_parent()
    };
    parent.and_then(|parent| get_computed_style_for_element(&parent))
}

/// `$isParentRTL(node)`: whether the computed style of the node's parent
/// has `direction: rtl`.
pub fn is_parent_rtl(node: &NodeRef) -> bool {
    get_computed_style_for_parent(node).is_some_and(|styles| styles.direction == "rtl")
}

/// `getStyleObjectFromRawCSS(css)`: deprecated upstream, an alias of
/// `getStyleObjectFromCSS`.
pub fn get_style_object_from_raw_css(css: &str) -> StyleObject {
    get_style_object_from_css(css)
}

/// `getCSSFromStyleObject(styles)`: `prop: value;` for each property in
/// order, the inverse of `getStyleObjectFromCSS`. An empty property name is
/// left out.
pub fn get_css_from_style_object(styles: &StyleObject) -> String {
    let mut css = String::new();
    for (style, value) in styles.iter() {
        if !style.is_empty() {
            css.push_str(style);
            css.push_str(": ");
            css.push_str(value);
            css.push(';');
        }
    }
    css
}

/// A rectangle as a `DOMRect` describes it (`left`, `top`, `right`,
/// `bottom`, `width`, `height`): what [`filter_selection_rects`] and
/// `@lexical/utils`' `dedupeSelectionRects` read of a selection's rects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionRect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub width: f64,
    pub height: f64,
}

impl SelectionRect {
    /// A rect at `left`, `top` of the size `width` by `height`. A measured
    /// rect whose edges and size disagree by rounding sets the fields
    /// directly.
    pub fn new(left: f64, top: f64, width: f64, height: f64) -> Self {
        Self {
            left,
            top,
            right: left + width,
            bottom: top + height,
            width,
            height,
        }
    }
}

/// The filtering half of `createRectsFromDOMRange(editor, range)`: sorts a
/// range's client rects (`range.getClientRects()`) from top left to bottom
/// right, as rows when their tops are within 3 of each other, then drops
/// each rect contained in the one kept before it and each that spans the
/// root (its width plus `root_padding`, the root's horizontal padding,
/// equals `root_width`, the width of the root's bounding rect). The DOM
/// half, measuring the range and the root, is the platform's.
pub fn filter_selection_rects(
    rects: &[SelectionRect],
    root_width: f64,
    root_padding: f64,
) -> Vec<SelectionRect> {
    // `Array.prototype.sort` is stable; an insertion sort is too, and takes
    // the 3-point tolerance, which is not a total order, without panicking.
    let mut selection_rects: Vec<SelectionRect> = Vec::with_capacity(rects.len());
    for rect in rects {
        let mut i = selection_rects.len();
        while i > 0 && sorts_before(rect, &selection_rects[i - 1]) {
            i -= 1;
        }
        selection_rects.insert(i, *rect);
    }
    let mut prev_rect: Option<SelectionRect> = None;
    let mut i = 0;
    while i < selection_rects.len() {
        let selection_rect = selection_rects[i];
        // Only discard a rect whose entire area is already covered.
        let is_contained_rect = prev_rect.is_some_and(|prev| {
            prev.top <= selection_rect.top
                && prev.bottom >= selection_rect.bottom
                && prev.left <= selection_rect.left
                && prev.right >= selection_rect.right
        });
        // Exclude selections that span the entire element
        let selection_spans_element = selection_rect.width + root_padding == root_width;
        if is_contained_rect || selection_spans_element {
            selection_rects.remove(i);
            continue;
        }
        prev_rect = Some(selection_rect);
        i += 1;
    }
    selection_rects
}

/// Whether `createRectsFromDOMRange`'s comparator puts `a` before `b`:
/// by `left` when the tops are within 3, by `top` otherwise.
fn sorts_before(a: &SelectionRect, b: &SelectionRect) -> bool {
    let top = a.top - b.top;
    // Some rects match position closely, but not perfectly,
    // so we give a 3px tolerance.
    if top.abs() <= 3.0 {
        return a.left - b.left < 0.0;
    }
    top < 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sides(rects: &[SelectionRect]) -> Vec<[f64; 4]> {
        rects
            .iter()
            .map(|r| [r.left, r.top, r.width, r.height])
            .collect()
    }

    /// The rects JS's `createRectsFromDOMRange` returns for these client
    /// rects (a root 500 wide with a padding of 8 and 12), recorded by
    /// running it with a stubbed range and root.
    #[test]
    fn filter_selection_rects_matches_create_rects_from_dom_range() {
        let r = SelectionRect::new;
        let rects = [
            r(50.0, 22.0, 30.0, 18.0),
            r(10.0, 0.0, 40.0, 18.0),
            r(5.0, 2.0, 20.0, 18.0),
            r(0.0, 20.0, 480.0, 18.0),
            r(12.0, 4.0, 10.0, 10.0),
            r(60.0, 40.0, 10.0, 18.0),
            r(12.0, 1.0, 10.0, 10.0),
        ];
        assert_eq!(
            sides(&filter_selection_rects(&rects, 500.0, 20.0)),
            [
                [5.0, 2.0, 20.0, 18.0],
                [10.0, 0.0, 40.0, 18.0],
                [50.0, 22.0, 30.0, 18.0],
                [60.0, 40.0, 10.0, 18.0],
            ]
        );
        let rects = [
            r(10.0, 0.0, 100.0, 20.0),
            r(20.0, 5.0, 50.0, 10.0),
            r(200.0, 1.0, 10.0, 10.0),
            r(100.0, 0.0, 30.0, 20.0),
        ];
        assert_eq!(
            sides(&filter_selection_rects(&rects, 500.0, 20.0)),
            [
                [10.0, 0.0, 100.0, 20.0],
                [100.0, 0.0, 30.0, 20.0],
                [200.0, 1.0, 10.0, 10.0],
                [20.0, 5.0, 50.0, 10.0],
            ]
        );
    }
}
