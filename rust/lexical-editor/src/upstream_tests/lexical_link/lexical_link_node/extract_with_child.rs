//! `LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers)`.

use crate::link::{AutoLinkAttributes, LinkAttributes, create_auto_link_node, create_link_node};
use crate::upstream_tests::support::{initialize_unit_test, update};
use crate::{
    CreateEditorArgs, LexicalError, NodeRef, PointType, Which, create_paragraph_node,
    create_text_node, get_root,
};

fn run(f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, f);
}

/// `$setupLinkInParagraph()`: `(linkNode, textNode)`.
fn setup_link_in_paragraph() -> Result<(NodeRef, NodeRef), LexicalError> {
    let paragraph = create_paragraph_node();
    let link_node = create_link_node("https://example.com", &LinkAttributes::default());
    let text_node = create_text_node("hello link");
    link_node.append(std::slice::from_ref(&text_node))?;
    paragraph.append(std::slice::from_ref(&link_node))?;
    get_root().append(&[paragraph])?;
    Ok((link_node, text_node))
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > Chrome-like — anchor/focus both on inner TextNode
#[test]
fn chrome_like_anchor_and_focus_both_on_inner_text_node() {
    run(|| {
        let (link_node, text_node) = setup_link_in_paragraph()?;
        let selection = text_node.select(Some(0), Some(10))?;
        assert!(link_node.extract_with_child(&text_node, Some(&selection), "clone")?);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > Firefox-like — anchor/focus both on LinkNode itself (element point)
#[test]
fn firefox_like_anchor_and_focus_both_on_link_node() {
    run(|| {
        let (link_node, text_node) = setup_link_in_paragraph()?;
        let selection = link_node.select(Some(0), Some(1))?;
        assert!(link_node.extract_with_child(&text_node, Some(&selection), "clone")?);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > Safari-like — asymmetric (anchor on LinkNode, focus on inner TextNode)
#[test]
fn safari_like_asymmetric() {
    run(|| {
        let (link_node, text_node) = setup_link_in_paragraph()?;
        let selection = link_node.select(Some(0), Some(1))?;
        selection.point_set(Which::Focus, text_node.get_key(), 10, PointType::Text)?;
        assert!(link_node.extract_with_child(&text_node, Some(&selection), "clone")?);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > selection entirely outside the link returns false
#[test]
fn selection_entirely_outside_the_link_returns_false() {
    run(|| {
        let paragraph = create_paragraph_node();
        let link_node = create_link_node("https://example.com", &LinkAttributes::default());
        let text_node = create_text_node("hello link");
        link_node.append(std::slice::from_ref(&text_node))?;
        let outside_text = create_text_node("outside");
        paragraph.append(std::slice::from_ref(&link_node))?;
        paragraph.append(std::slice::from_ref(&outside_text))?;
        get_root().append(&[paragraph])?;
        let selection = outside_text.select(Some(0), Some(7))?;
        assert!(!link_node.extract_with_child(&text_node, Some(&selection), "clone")?);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > collapsed (zero-length) selection inside the link returns false
#[test]
fn collapsed_selection_inside_the_link_returns_false() {
    run(|| {
        let (link_node, text_node) = setup_link_in_paragraph()?;
        let selection = text_node.select(Some(5), Some(5))?;
        assert!(!link_node.extract_with_child(&text_node, Some(&selection), "clone")?);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > AutoLinkNode (LinkNode subclass) inherits the Firefox-like fix
#[test]
fn auto_link_node_inherits_the_firefox_like_fix() {
    run(|| {
        let paragraph = create_paragraph_node();
        let auto_link =
            create_auto_link_node("https://example.com", &AutoLinkAttributes::default());
        let text_node = create_text_node("https://example.com");
        auto_link.append(std::slice::from_ref(&text_node))?;
        paragraph.append(std::slice::from_ref(&auto_link))?;
        get_root().append(&[paragraph])?;
        let selection = auto_link.select(Some(0), Some(1))?;
        assert!(auto_link.extract_with_child(&text_node, Some(&selection), "clone")?);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > partial text selection inside link with Firefox-like element anchor still preserves wrap
#[test]
fn partial_text_selection_with_firefox_like_element_anchor_still_preserves_wrap() {
    run(|| {
        let (link_node, text_node) = setup_link_in_paragraph()?;
        let selection = link_node.select(Some(0), Some(1))?;
        selection.point_set(Which::Focus, text_node.get_key(), 4, PointType::Text)?;
        assert!(link_node.extract_with_child(&text_node, Some(&selection), "clone")?);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > selecting one of two adjacent links does not pull in the other
#[test]
fn selecting_one_of_two_adjacent_links_does_not_pull_in_the_other() {
    run(|| {
        let paragraph = create_paragraph_node();
        let link1 = create_link_node("https://first.example", &LinkAttributes::default());
        let text1 = create_text_node("first");
        link1.append(std::slice::from_ref(&text1))?;
        let between = create_text_node(" and ");
        let link2 = create_link_node("https://second.example", &LinkAttributes::default());
        let text2 = create_text_node("second");
        link2.append(std::slice::from_ref(&text2))?;
        paragraph.append(std::slice::from_ref(&link1))?;
        paragraph.append(&[between])?;
        paragraph.append(std::slice::from_ref(&link2))?;
        get_root().append(&[paragraph])?;
        let selection = link1.select(Some(0), Some(1))?;
        assert!(link1.extract_with_child(&text1, Some(&selection), "clone")?);
        assert!(!link2.extract_with_child(&text2, Some(&selection), "clone")?);
        Ok(())
    });
}
