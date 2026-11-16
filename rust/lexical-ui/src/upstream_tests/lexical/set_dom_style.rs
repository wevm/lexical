//! Ports of `packages/lexical/src/__tests__/unit/setDOMStyle.test.ts`.

use crate::{StyleObject, get_style_object_from_css};

/// `toEqual` on a style object: the same properties and values.
fn style(entries: &[(&str, &str)]) -> StyleObject {
    let mut object = StyleObject::new();
    for (k, v) in entries {
        object.insert(*k, *v);
    }
    object
}

fn same(a: &StyleObject, b: &StyleObject) -> bool {
    a.len() == b.len() && a.iter().all(|(k, v)| b.get(k) == Some(v))
}

// upstream: lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > parses CSS text into a style object
#[test]
fn parses_css_text_into_a_style_object() {
    assert!(same(
        &get_style_object_from_css("color: red; margin: 0 !important; --custom: example;"),
        &style(&[
            ("--custom", "example"),
            ("color", "red"),
            ("margin", "0 !important")
        ]),
    ));
}

// upstream: lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > parses values with comments, semicolons and colons inside quotes and parentheses
#[test]
fn parses_values_with_comments_semicolons_and_colons_inside_quotes_and_parentheses() {
    assert!(same(
        &get_style_object_from_css(
            r#"background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'></svg>"); /* ignored */ content: "semi;colon:value"; color: red;"#,
        ),
        &style(&[
            (
                "background-image",
                r#"url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'></svg>")"#
            ),
            ("color", "red"),
            ("content", r#""semi;colon:value""#),
        ]),
    ));
}

// upstream: lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > returns a fresh style object for each parse
#[test]
fn returns_a_fresh_style_object_for_each_parse() {
    let mut style_object = get_style_object_from_css("color: red;");
    style_object.insert("color", "blue");
    assert!(same(
        &get_style_object_from_css("color: red;"),
        &style(&[("color", "red")])
    ));
}

#[cfg(feature = "html")]
mod dom {
    use crate::lexical_dom::get_document;
    use crate::{set_dom_style_from_css, set_dom_style_object};

    // upstream: lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > applies CSS text without cssText
    #[test]
    fn applies_css_text_without_css_text() {
        let element = get_document().create_element("div");
        // `prevCSSText` defaults to `''`.
        set_dom_style_from_css(
            &element.style(),
            "color: red; margin: 0 !IMPORTANT; --custom: example;",
            "",
        );
        assert_eq!(element.style().get_property_value("color"), "red");
        assert_eq!(element.style().get_property_priority("margin"), "important");
        assert_eq!(element.style().get_property_value("--custom"), "example");
    }

    // upstream: lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > replaces previous inline styles when CSS text changes
    #[test]
    fn replaces_previous_inline_styles_when_css_text_changes() {
        let element = get_document().create_element("div");
        set_dom_style_from_css(
            &element.style(),
            "color: red; margin: 0 !important; --custom: example;",
            "",
        );
        set_dom_style_from_css(
            &element.style(),
            "padding: 1px;",
            "color: red; margin: 0 !important; --custom: example;",
        );
        assert_eq!(element.style().get_property_value("color"), "");
        assert_eq!(element.style().get_property_value("margin"), "");
        assert_eq!(element.style().get_property_value("--custom"), "");
        assert_eq!(element.style().get_property_value("padding"), "1px");
    }

    // upstream: lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > applies direct style objects
    #[test]
    fn applies_direct_style_objects() {
        let element = get_document().create_element("div");
        set_dom_style_object(
            &element.style(),
            [
                ("color", Some("red")),
                ("margin", Some("0 !IMPORTANT")),
                ("padding", None),
            ],
        );
        assert_eq!(element.style().get_property_value("color"), "red");
        assert_eq!(element.style().get_property_priority("margin"), "important");
        assert_eq!(element.style().get_property_value("padding"), "");
    }
}

// upstream: lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > drops comments and respects quotes and parentheses
#[test]
fn drops_comments_and_respects_quotes_and_parentheses() {
    assert!(same(
        &get_style_object_from_css("color: re/* x */d; margin: 0"),
        &style(&[("color", "red"), ("margin", "0")]),
    ));
    assert!(same(
        &get_style_object_from_css("a/* c1 *//* c2 */b: c"),
        &style(&[("ab", "c")]),
    ));
    assert!(same(
        &get_style_object_from_css(r#"content: "a\"b;c:d""#),
        &style(&[("content", r#""a\"b;c:d""#)]),
    ));
    assert!(same(
        &get_style_object_from_css("background: url(http://x/a;b);"),
        &style(&[("background", "url(http://x/a;b)")]),
    ));
}
