//! `packages/lexical-selection/src/utils.ts`: the model part. The DOM
//! ranges and rects (`createDOMRange`, `createRectsFromDOMRange`) and the
//! computed styles (`$getComputedStyleForElement`,
//! `$getComputedStyleForParent`, `$isParentRTL`) need a DOM.

use crate::set_dom_style::{StyleObject, get_style_object_from_css};

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
