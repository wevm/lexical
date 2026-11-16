//! Ports of `packages/lexical/src/__tests__/unit/LexicalInsertParagraphEndpointRepair.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text", feature = "link"))]

use crate::extension::build_editor_from_extensions;
use crate::link::{LinkAttributes, LinkNode, create_link_node, is_link_node};
use crate::upstream_tests::support::update;
use crate::{
    EditorReadMode, LexicalExtension, NodeClass, PointType, Which, create_paragraph_node,
    create_range_selection, create_text_node, define_extension, get_root, set_selection,
};

fn insert_paragraph_over_a_range_from_an_empty_paragraph_into_styled_text(backward: bool) {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        nodes: vec![NodeClass::of::<LinkNode>()],
        ..LexicalExtension::<(), (), ()>::new("insert-paragraph-endpoint-repair", ())
    })
    .into()])
    .unwrap();
    update(&editor, move || {
        let empty = create_paragraph_node();
        let text = create_text_node("ead");
        text.set_style("color: red");
        let link = create_link_node("https://example.com", &LinkAttributes::default());
        link.append(&[create_text_node("caa")])?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[text.clone(), link])?;
        get_root().clear()?;
        get_root().append(&[empty.clone(), paragraph])?;
        let selection = create_range_selection();
        let (start, end) = if backward {
            (Which::Focus, Which::Anchor)
        } else {
            (Which::Anchor, Which::Focus)
        };
        selection.point_set(start, empty.get_key(), 0, PointType::Element)?;
        selection.point_set(end, text.get_key(), 2, PointType::Text)?;
        set_selection(Some(selection.clone()));
        selection.insert_paragraph()?;
        Ok(())
    });
    editor
        .read_with_mode(EditorReadMode::Latest, || {
            let root = get_root();
            assert_eq!(root.get_text_content(), "\n\ndcaa");
            assert_eq!(root.get_children_size(), 2);
            assert_eq!(root.get_first_child_or_throw()?.get_text_content(), "");
            let paragraph = root.get_last_child_or_throw()?;
            assert_eq!(paragraph.get_type(), "paragraph");
            assert_eq!(paragraph.get_children_size(), 2);
            let remaining_text = paragraph.get_first_child_or_throw()?;
            assert!(remaining_text.is_text());
            assert_eq!(remaining_text.get_text_content(), "d");
            assert_eq!(remaining_text.text_data().get_style(), "color: red");
            let link = paragraph.get_last_child_or_throw()?;
            assert!(is_link_node(&link));
            assert_eq!(link.get_url().as_deref(), Some("https://example.com"));
            assert_eq!(link.get_text_content(), "caa");
            Ok::<_, crate::LexicalError>(())
        })
        .unwrap()
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalInsertParagraphEndpointRepair.test.ts::insertParagraph over a range from an empty paragraph into styled text (backward=false)
#[test]
fn insert_paragraph_over_a_range_from_an_empty_paragraph_into_styled_text_forward() {
    insert_paragraph_over_a_range_from_an_empty_paragraph_into_styled_text(false);
}

// upstream: lexical/src/__tests__/unit/LexicalInsertParagraphEndpointRepair.test.ts::insertParagraph over a range from an empty paragraph into styled text (backward=true)
#[test]
fn insert_paragraph_over_a_range_from_an_empty_paragraph_into_styled_text_backward() {
    insert_paragraph_over_a_range_from_an_empty_paragraph_into_styled_text(true);
}
