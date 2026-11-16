//! `import/inlineStylesFromStyleSheets.ts`: the declarations of a
//! document's `<style>` rules copied onto the elements they match, as
//! inline styles that don't override the element's own.
//!
//! JS reads the CSSOM (`document.styleSheets`, `querySelectorAll`). Here
//! each `<style>` element's text is parsed into rules
//! ([`parse_style_sheet`]) and their selectors matched with
//! [`DomNode::query_selector_all`]. A selector it rejects is skipped, as
//! JS skips one `querySelectorAll` throws on; a valid selector outside its
//! subset (pseudo-classes, namespaces, escapes) is skipped too, where JS
//! would apply it. Shorthand properties (`font`, `margin`...) are copied
//! as they are, without the longhands a browser's declaration expands them
//! into (rust/PARITY.md).

#[cfg(feature = "extension")]
use super::dom_import_extension::{DomPreprocessContext, PreprocessNext};
use crate::dom::{CssRule, DomNode, parse_style_sheet};
use std::collections::{HashMap, HashSet};

/// `$inlineStylesFromStyleSheets`: [`inline_styles_from_style_sheets_dom`],
/// then the next preprocessor.
#[cfg(feature = "extension")]
pub fn inline_styles_from_style_sheets(
    dom: &DomNode,
    _ctx: &DomPreprocessContext,
    next: PreprocessNext<'_>,
) -> Result<(), crate::LexicalError> {
    inline_styles_from_style_sheets_dom(dom);
    next()
}

/// `$inlineStylesFromStyleSheetsDOM(dom)`: only for a document.
pub fn inline_styles_from_style_sheets_dom(dom: &DomNode) {
    if !dom.is_document() {
        return;
    }
    let styles: Vec<DomNode> = dom
        .descendant_elements()
        .into_iter()
        .filter(|e| e.local_name() == "style")
        .collect();
    if styles.is_empty() {
        return;
    }
    let mut original_inline_styles: HashMap<usize, HashSet<String>> = HashMap::new();
    for style in styles {
        for rule in parse_style_sheet(&style.text_content()) {
            let CssRule::Style {
                selector_text,
                style: declaration,
            } = rule
            else {
                continue;
            };
            let Ok(elements) = dom.query_selector_all(&selector_text) else {
                continue;
            };
            for el in elements {
                let original = original_inline_styles
                    .entry(el.identity())
                    .or_insert_with(|| el.style().property_names().into_iter().collect());
                let el_style = el.style();
                for prop in declaration.property_names() {
                    if !original.contains(&prop) {
                        el_style.set_property(
                            &prop,
                            &declaration.get_property_value(&prop),
                            &declaration.get_property_priority(&prop),
                        );
                    }
                }
            }
        }
    }
}
