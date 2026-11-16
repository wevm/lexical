//! `LinkNode transform (Regression #8083)`.

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{LinkAttributes, create_link_node, is_link_node, link_extension};
use crate::nodes::line_break::create_line_break_node;
use crate::rich_text::{HeadingTagType, create_heading_node, is_heading_node, rich_text_extension};
use crate::{
    LexicalError, LexicalExtension, NodeRef, PointType, UpdateOptions, Which,
    create_paragraph_node, create_text_node, define_extension, get_root, get_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `transformExtension`.
fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![link_extension().into(), rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[test-link-transform]", ())
    })
    .into()])
    .unwrap()
}

/// Runs `f` in a discrete update; it returns a key the test reads back.
fn setup(
    editor: &LexicalEditorWithDispose,
    f: impl FnOnce() -> Result<String, LexicalError> + 'static,
) -> String {
    let key = Rc::new(RefCell::new(String::new()));
    let out = key.clone();
    editor
        .update(
            move || {
                *out.borrow_mut() = f()?;
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    key.borrow().clone()
}

fn link(children: &[NodeRef]) -> NodeRef {
    let link = create_link_node("https://lexical.dev", &LinkAttributes::default());
    link.append(children).unwrap();
    link
}

fn heading(tag: HeadingTagType, text: &NodeRef) -> NodeRef {
    let heading = create_heading_node(tag);
    heading.append(std::slice::from_ref(text)).unwrap();
    heading
}

fn set_root_paragraph(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(children)?;
    let root = get_root();
    root.clear()?;
    root.append(std::slice::from_ref(&paragraph))?;
    Ok(paragraph)
}

/// The selection's point `which` is a text point at `offset` in `key`
/// (any key when `None`).
fn assert_point(which: Which, key: Option<&str>, offset: usize) {
    let selection = get_selection().expect("Selection must be a RangeSelection");
    assert!(selection.is_range(), "Selection must be a RangeSelection");
    let point = selection.point(which).unwrap();
    assert_eq!(point.point_type, PointType::Text);
    if let Some(key) = key {
        assert_eq!(point.key.as_str(), key);
    }
    assert_eq!(point.offset, offset);
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > extracts block child (HeadingNode) from link
#[test]
fn extracts_block_child_heading_node_from_link() {
    let editor = build_editor();
    let text_key = setup(&editor, || {
        let text = create_text_node("Lexical");
        set_root_paragraph(&[link(&[heading(HeadingTagType::H1, &text)])])?;
        text.select(Some(3), Some(3))?;
        Ok(text.get_key().to_owned())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert!(
                is_heading_node(&children[0]),
                "First child must be a HeadingNode"
            );
            assert_eq!(children[0].get_children_size(), 1);
            let heading_child = children[0].get_first_child().unwrap();
            assert!(
                is_link_node(&heading_child),
                "Heading child must be a LinkNode"
            );
            assert_eq!(heading_child.get_text_content(), "Lexical");
            assert_point(Which::Anchor, Some(&text_key), 3);
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > extracts block child (ParagraphNode) from link
#[test]
fn extracts_block_child_paragraph_node_from_link() {
    let editor = build_editor();
    let text_key = setup(&editor, || {
        let text = create_text_node("Lexical");
        let inner_paragraph = create_paragraph_node();
        inner_paragraph.append(std::slice::from_ref(&text))?;
        set_root_paragraph(&[link(&[inner_paragraph])])?;
        text.select(Some(0), Some(7))?;
        Ok(text.get_key().to_owned())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].get_type(), "paragraph");
            assert_eq!(children[0].get_children_size(), 1);
            let para_child = children[0].get_first_child().unwrap();
            assert!(
                is_link_node(&para_child),
                "Paragraph child must be a LinkNode"
            );
            assert_eq!(para_child.get_text_content(), "Lexical");
            assert_point(Which::Anchor, Some(&text_key), 0);
            assert_point(Which::Focus, Some(&text_key), 7);
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > handles siblings after block child
#[test]
fn handles_siblings_after_block_child() {
    let editor = build_editor();
    let after_key = setup(&editor, || {
        let after_text = create_text_node(" after");
        set_root_paragraph(&[link(&[
            heading(HeadingTagType::H1, &create_text_node("Heading")),
            after_text.clone(),
        ])])?;
        after_text.select(Some(2), Some(5))?;
        Ok(after_text.get_key().to_owned())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert!(
                is_heading_node(&children[0]),
                "First child must be a HeadingNode"
            );
            let heading_link = children[0].get_first_child().unwrap();
            assert!(
                is_link_node(&heading_link),
                "Heading child must be a LinkNode"
            );
            assert_eq!(heading_link.get_text_content(), "Heading");
            assert_eq!(children[1].get_type(), "paragraph");
            let trailing_link = children[1].get_first_child().unwrap();
            assert!(is_link_node(&trailing_link));
            assert_eq!(trailing_link.get_text_content(), " after");
            assert_point(Which::Anchor, Some(&after_key), 2);
            assert_point(Which::Focus, Some(&after_key), 5);
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > fixes element selection in paragraph split to the right of LinkNode
#[test]
fn fixes_element_selection_in_paragraph_split_to_the_right_of_link_node() {
    let editor = build_editor();
    let after_key = setup(&editor, || {
        let text_after = create_text_node("after");
        let paragraph = set_root_paragraph(&[
            create_text_node("before"),
            link(&[heading(HeadingTagType::H1, &create_text_node("Heading"))]),
            text_after.clone(),
        ])?;
        paragraph.select(Some(2), Some(2))?;
        Ok(text_after.get_key().to_owned())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 3);
            assert_eq!(children[2].get_text_content(), "after");
            assert_point(Which::Anchor, Some(&after_key), 0);
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > fixes element selection in LinkNode to the right of non-inline node
#[test]
fn fixes_element_selection_in_link_node_to_the_right_of_non_inline_node() {
    let editor = build_editor();
    let after_key = setup(&editor, || {
        let after_text = create_text_node(" after");
        let l = link(&[
            heading(HeadingTagType::H1, &create_text_node("Heading")),
            after_text.clone(),
        ]);
        set_root_paragraph(std::slice::from_ref(&l))?;
        l.select(Some(1), Some(1))?;
        Ok(after_text.get_key().to_owned())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[1].get_type(), "paragraph");
            assert!(is_link_node(&children[1].get_first_child().unwrap()));
            assert_point(Which::Anchor, Some(&after_key), 0);
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > fixes element selection with multiple non-inline siblings
#[test]
fn fixes_element_selection_with_multiple_non_inline_siblings() {
    let editor = build_editor();
    let heading2_key = setup(&editor, || {
        let heading2_text = create_text_node("Second");
        let l = link(&[
            heading(HeadingTagType::H1, &create_text_node("First")),
            heading(HeadingTagType::H2, &heading2_text),
        ]);
        set_root_paragraph(std::slice::from_ref(&l))?;
        l.select(Some(1), Some(1))?;
        Ok(heading2_text.get_key().to_owned())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert!(is_heading_node(&children[0]));
            assert_eq!(children[0].get_text_content(), "First");
            assert!(is_heading_node(&children[1]));
            assert_eq!(children[1].get_text_content(), "Second");
            assert_point(Which::Anchor, Some(&heading2_key), 0);
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > fixes element selection when no siblings to the right of LinkNode
#[test]
fn fixes_element_selection_when_no_siblings_to_the_right_of_link_node() {
    let editor = build_editor();
    setup(&editor, || {
        let paragraph = set_root_paragraph(&[link(&[heading(
            HeadingTagType::H1,
            &create_text_node("Heading"),
        )])])?;
        paragraph.select(Some(1), Some(1))?;
        Ok(String::new())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert!(is_heading_node(&children[0]));
            let heading_child = children[0].get_first_child().unwrap();
            assert!(is_link_node(&heading_child));
            assert_eq!(heading_child.get_text_content(), "Heading");
            assert!(get_selection().is_some_and(|s| s.is_range()));
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > selection in paragraph right of link with trailing text in link
#[test]
fn selection_in_paragraph_right_of_link_with_trailing_text_in_link() {
    let editor = build_editor();
    let after_key = setup(&editor, || {
        let after = create_text_node("after");
        let paragraph = set_root_paragraph(&[
            create_text_node("before"),
            link(&[
                heading(HeadingTagType::H1, &create_text_node("Heading")),
                create_text_node(" trailing"),
            ]),
            after.clone(),
        ])?;
        paragraph.select(Some(2), Some(2))?;
        Ok(after.get_key().to_owned())
    });
    editor
        .read(|| {
            assert_eq!(get_root().get_children_size(), 3);
            assert_point(Which::Anchor, Some(&after_key), 0);
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > selection at end of link with heading only child
#[test]
fn selection_at_end_of_link_with_heading_only_child() {
    let editor = build_editor();
    let heading_key = setup(&editor, || {
        let heading_text = create_text_node("Heading");
        let l = link(&[heading(HeadingTagType::H1, &heading_text)]);
        set_root_paragraph(std::slice::from_ref(&l))?;
        l.select(Some(1), Some(1))?;
        Ok(heading_text.get_key().to_owned())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert!(is_heading_node(&children[0]));
            assert_point(Which::Anchor, Some(&heading_key), 7);
        })
        .unwrap();
}

/// The nodes of a merge-boundary scenario: `[linkA, linkB, paragraph,
/// textA, textB]`.
type Boundary = fn(&[NodeRef; 5]) -> Result<(), LexicalError>;

/// After the two links merge, the collapsed selection is at the end of
/// `textA` (or, without `text_a`, at offset 0 of a text point).
fn assert_merged(editor: &LexicalEditorWithDispose, text_a: Option<&str>, offset: usize) {
    editor
        .read(|| {
            let paragraph = get_root().get_first_child().unwrap();
            assert_eq!(paragraph.get_type(), "paragraph");
            assert_eq!(paragraph.get_children_size(), 1);
            assert!(is_link_node(&paragraph.get_first_child().unwrap()));
            let selection = get_selection().unwrap();
            assert!(
                selection
                    .range()
                    .expect("Expected RangeSelection")
                    .is_collapsed()
            );
            assert_point(Which::Anchor, text_a, offset);
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: text point at end of first link text
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: text point at start of second link text
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: element point at last child of first link
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: element point between the two links
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: element point at first child of second link
#[test]
fn cursor_at_merge_boundary() {
    let scenarios: [(&str, Boundary); 5] = [
        ("text point at end of first link text", |[_, _, _, a, _]| {
            a.select(Some(5), Some(5)).map(|_| ())
        }),
        (
            "text point at start of second link text",
            |[_, _, _, _, b]| b.select(Some(0), Some(0)).map(|_| ()),
        ),
        ("element point at last child of first link", |[a, ..]| {
            a.select(Some(1), Some(1)).map(|_| ())
        }),
        ("element point between the two links", |[_, _, p, _, _]| {
            p.select(Some(1), Some(1)).map(|_| ())
        }),
        (
            "element point at first child of second link",
            |[_, b, ..]| b.select(Some(0), Some(0)).map(|_| ()),
        ),
    ];
    for (desc, set_selection_at_boundary) in scenarios {
        let editor = build_editor();
        let text_a_key = setup(&editor, move || {
            let text_a = create_text_node("link1");
            let text_b = create_text_node("link2");
            let link_a = link(std::slice::from_ref(&text_a));
            let link_b = link(std::slice::from_ref(&text_b));
            let paragraph = set_root_paragraph(&[link_a.clone(), link_b.clone()])?;
            set_selection_at_boundary(&[link_a, link_b, paragraph, text_a.clone(), text_b])?;
            Ok(text_a.get_key().to_owned())
        });
        editor
            .read(|| {
                assert_eq!(
                    get_root().get_first_child().unwrap().get_text_content(),
                    "link1link2",
                    "{desc}"
                );
            })
            .unwrap();
        assert_merged(&editor, Some(&text_a_key), 5);
    }
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 edge case: second link starts with LineBreakNode
#[test]
fn edge_case_second_link_starts_with_line_break_node() {
    let editor = build_editor();
    let text_a_key = setup(&editor, || {
        let text_a = create_text_node("link1");
        let paragraph = set_root_paragraph(&[
            link(std::slice::from_ref(&text_a)),
            link(&[create_line_break_node(), create_text_node("link2")]),
        ])?;
        paragraph.select(Some(1), Some(1))?;
        Ok(text_a.get_key().to_owned())
    });
    assert_merged(&editor, Some(&text_a_key), 5);
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 edge case: first link ends with LineBreakNode
#[test]
fn edge_case_first_link_ends_with_line_break_node() {
    let editor = build_editor();
    setup(&editor, || {
        let paragraph = set_root_paragraph(&[
            link(&[create_text_node("link1"), create_line_break_node()]),
            link(&[create_text_node("link2")]),
        ])?;
        paragraph.select(Some(1), Some(1))?;
        Ok(String::new())
    });
    assert_merged(&editor, None, 0);
}
