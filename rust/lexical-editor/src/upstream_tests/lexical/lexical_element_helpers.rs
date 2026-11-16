//! Ports of `packages/lexical/src/__tests__/unit/LexicalElementHelpers.test.ts`.

#![cfg(feature = "html")]

use crate::dom::get_document;
use crate::lexical_dom::{add_class_names_to_element, remove_class_names_from_element};

// upstream: lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > addClassNamesToElement() and removeClassNamesFromElement() > basic
#[test]
fn basic() {
    let element = get_document().create_element("div");
    add_class_names_to_element(&element, &["test-class"]);
    assert_eq!(element.class_name(), "test-class");
    remove_class_names_from_element(&element, &["test-class"]);
    assert_eq!(element.class_name(), "");
}

/// Upstream also passes `null`, `undefined`, `false` and `true`, which a
/// `&str` can't hold; JS adds nothing for them either.
// upstream: lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > addClassNamesToElement() and removeClassNamesFromElement() > empty
#[test]
fn empty() {
    let element = get_document().create_element("div");
    add_class_names_to_element(&element, &["", " ", "  \t\n"]);
    assert_eq!(element.class_name(), "");
}

// upstream: lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > addClassNamesToElement() and removeClassNamesFromElement() > multiple
#[test]
fn multiple() {
    let element = get_document().create_element("div");
    add_class_names_to_element(&element, &["a", "b", "c"]);
    assert_eq!(element.class_name(), "a b c");
    remove_class_names_from_element(&element, &["a", "b", "c"]);
    assert_eq!(element.class_name(), "");
}

// upstream: lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > addClassNamesToElement() and removeClassNamesFromElement() > space separated
#[test]
fn space_separated() {
    let element = get_document().create_element("div");
    add_class_names_to_element(&element, &["a b c"]);
    assert_eq!(element.class_name(), "a b c");
    remove_class_names_from_element(&element, &["a b c"]);
    assert_eq!(element.class_name(), "");
}

// upstream: lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > multiple spaces
#[test]
fn multiple_spaces() {
    let class_names = " a  b   c \t\n  ";
    let element = get_document().create_element("div");
    add_class_names_to_element(&element, &[class_names]);
    assert_eq!(element.class_name(), "a b c");
    remove_class_names_from_element(&element, &[class_names]);
    assert_eq!(element.class_name(), "");
}
