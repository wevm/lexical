//! `$toggleLink(null) with a selection touching a link boundary`: the
//! `test.each` table, one table-driven test. Vitest cuts a name past 40
//! characters to 39 and `…`, and the markers carry the ids as it reports
//! them.

use crate::extension::build_editor_from_extensions;
use crate::link::{LinkAttributes, create_link_node, is_link_node, link_extension, toggle_link};
use crate::{
    LexicalExtension, NodeRef, PointType, UpdateOptions, Which, create_paragraph_node,
    create_range_selection, create_text_node, define_extension, get_root, get_selection,
    set_selection,
};

/// A paragraph's children: a text, or a link of texts.
#[derive(Clone, Debug, PartialEq)]
enum Part {
    Text(String),
    Link(Vec<String>),
}

fn t(s: &str) -> Part {
    Part::Text(s.to_owned())
}

fn l(texts: &[&str]) -> Part {
    Part::Link(texts.iter().map(|s| (*s).to_owned()).collect())
}

/// `$buildParagraph(layout)`.
fn build_paragraph(layout: &[Part]) -> Result<(), crate::LexicalError> {
    let paragraph = create_paragraph_node();
    for (i, part) in layout.iter().enumerate() {
        match part {
            Part::Text(text) => paragraph.append(&[create_text_node(text)])?,
            Part::Link(texts) => {
                let link = create_link_node(
                    &format!("https://example.com/{i}"),
                    &LinkAttributes::default(),
                );
                // Alternate formats so adjacent text nodes are not merged.
                for (j, text) in texts.iter().enumerate() {
                    let node = create_text_node(text);
                    node.set_format(if j % 2 == 1 { 1.0 } else { 0.0 });
                    link.append(&[node])?;
                }
                paragraph.append(&[link])?;
            }
        }
    }
    let root = get_root();
    root.clear()?;
    root.append(&[paragraph])
}

/// `$getTextNode(text)`.
fn get_text_node(text: &str) -> NodeRef {
    get_root()
        .get_all_text_nodes()
        .into_iter()
        .find(|n| n.get_text_content() == text)
        .unwrap_or_else(|| panic!("Expected a TextNode with text {text}"))
}

/// `$describeParagraph()`.
fn describe_paragraph() -> Vec<Part> {
    let paragraph = get_root().get_first_child_or_throw().unwrap();
    assert_eq!(paragraph.get_type(), "paragraph");
    paragraph
        .get_children()
        .iter()
        .map(|child| {
            if is_link_node(child) {
                Part::Link(
                    child
                        .get_children()
                        .iter()
                        .map(NodeRef::get_text_content)
                        .collect(),
                )
            } else {
                Part::Text(child.get_text_content())
            }
        })
        .collect()
}

struct Case {
    layout: Vec<Part>,
    anchor: (&'static str, usize),
    focus: (&'static str, usize),
    expected: Vec<Part>,
    selected_text: &'static str,
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > forward selection ending at offset 0 of…
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > backward selection ending at offset 0 o…
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > forward selection starting at the end o…
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > backward selection starting at the end …
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > selection ending at offset 0 of a multi…
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > selection starting at the end of a mult…
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > unlinks a selected link while leaving a…
#[test]
fn toggle_link_null_with_a_selection_touching_a_link_boundary() {
    let ab_cd_ef = || vec![t("ab"), l(&["cd"]), t("ef")];
    let cases = [
        Case {
            layout: ab_cd_ef(),
            anchor: ("ab", 1),
            focus: ("cd", 0),
            expected: ab_cd_ef(),
            selected_text: "b",
        },
        Case {
            layout: ab_cd_ef(),
            anchor: ("cd", 0),
            focus: ("ab", 1),
            expected: ab_cd_ef(),
            selected_text: "b",
        },
        Case {
            layout: ab_cd_ef(),
            anchor: ("cd", 2),
            focus: ("ef", 1),
            expected: ab_cd_ef(),
            selected_text: "e",
        },
        Case {
            layout: ab_cd_ef(),
            anchor: ("ef", 1),
            focus: ("cd", 2),
            expected: ab_cd_ef(),
            selected_text: "e",
        },
        Case {
            layout: vec![t("ab"), l(&["cd", "ef"])],
            anchor: ("ab", 1),
            focus: ("cd", 0),
            expected: vec![t("ab"), l(&["cd", "ef"])],
            selected_text: "b",
        },
        Case {
            layout: vec![l(&["ab", "cd"]), t("ef")],
            anchor: ("cd", 2),
            focus: ("ef", 1),
            expected: vec![l(&["ab", "cd"]), t("ef")],
            selected_text: "e",
        },
        Case {
            layout: vec![l(&["ab"]), t("cd"), l(&["ef"])],
            anchor: ("ab", 1),
            focus: ("ef", 0),
            expected: vec![l(&["a"]), t("bcd"), l(&["ef"])],
            selected_text: "bcd",
        },
    ];
    let discrete = || UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    };
    for case in cases {
        let editor = build_editor_from_extensions([define_extension(LexicalExtension {
            dependencies: vec![link_extension().into()],
            ..LexicalExtension::<(), (), ()>::new("[test-unlink-link-boundary]", ())
        })
        .into()])
        .unwrap();
        let layout = case.layout.clone();
        editor
            .update(move || build_paragraph(&layout), discrete())
            .unwrap();
        let (anchor, focus) = (case.anchor, case.focus);
        editor
            .update(
                move || {
                    let selection = create_range_selection();
                    selection.point_set(
                        Which::Anchor,
                        get_text_node(anchor.0).get_key(),
                        anchor.1,
                        PointType::Text,
                    )?;
                    selection.point_set(
                        Which::Focus,
                        get_text_node(focus.0).get_key(),
                        focus.1,
                        PointType::Text,
                    )?;
                    set_selection(Some(selection));
                    toggle_link(None, &LinkAttributes::default())
                },
                discrete(),
            )
            .unwrap();
        editor
            .read(|| {
                assert_eq!(describe_paragraph(), case.expected);
                let selection = get_selection().unwrap();
                assert!(selection.is_range(), "Expected a RangeSelection");
                assert_eq!(selection.get_text_content().unwrap(), case.selected_text);
            })
            .unwrap();
    }
}
