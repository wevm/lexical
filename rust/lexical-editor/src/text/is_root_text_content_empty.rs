//! `packages/lexical-text/src/isRootTextContentEmpty.ts`.

use super::root_text_content;
use crate::selection_deletion::is_js_whitespace;

/// `$isRootTextContentEmpty(isEditorComposing, trim)`: whether the root has
/// no text, or only whitespace when `trim` (JS's default; `String#trim`'s
/// whitespace). `false` while composing.
pub fn is_root_text_content_empty(is_editor_composing: bool, trim: bool) -> bool {
    if is_editor_composing {
        return false;
    }
    let text = root_text_content();
    let text = if trim {
        text.trim_matches(is_js_whitespace)
    } else {
        &text
    };
    text.is_empty()
}

/// `$isRootTextContentEmptyCurry(isEditorComposing, trim)`: a function that
/// runs [`is_root_text_content_empty`] with these arguments (JS's `trim`
/// left `undefined` is `true`).
pub fn is_root_text_content_empty_curry(
    is_editor_composing: bool,
    trim: bool,
) -> impl Fn() -> bool {
    move || is_root_text_content_empty(is_editor_composing, trim)
}
