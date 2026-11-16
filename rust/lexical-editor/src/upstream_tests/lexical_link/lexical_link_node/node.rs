//! `LexicalLinkNode tests`. `formatUrl` is ported next to `format_url` in
//! `link/link_node.rs`.

use crate::json::Value;
use crate::link::{LinkAttributes, LinkNode, create_link_node, is_link_node, toggle_link};
use crate::nodes::line_break::create_line_break_node;
use crate::upstream_tests::support::{initialize_unit_test, panics, update};
use crate::{
    CreateEditorArgs, Editor, LexicalError, NodeRef, PointType, Which, create_paragraph_node,
    create_range_selection, create_text_node, get_node_by_key, get_root, get_selection, select_all,
    set_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

fn editor() -> Editor {
    #[allow(unused_mut)]
    let mut args = CreateEditorArgs::default();
    #[cfg(feature = "mark")]
    args.nodes
        .push(crate::NodeClass::of::<crate::mark::MarkNode>());
    initialize_unit_test(args)
}

fn some(s: &str) -> Option<Option<String>> {
    Some(Some(s.to_owned()))
}

fn plain(url: &str) -> NodeRef {
    create_link_node(url, &LinkAttributes::default())
}

fn with_rel_target_title(url: &str, rel: &str, target: &str, title: &str) -> NodeRef {
    create_link_node(
        url,
        &LinkAttributes {
            rel: some(rel),
            target: some(target),
            title: some(title),
        },
    )
}

/// `__url`, `__target`, `__rel` and `__title`.
fn fields(node: &NodeRef) -> (String, Option<String>, Option<String>, Option<String>) {
    let latest = node.get_latest();
    let l = latest.downcast_ref::<LinkNode>().expect("a LinkNode");
    (
        l.url().to_owned(),
        l.target().map(str::to_owned),
        l.rel().map(str::to_owned),
        l.title().map(str::to_owned),
    )
}

fn paragraph() -> NodeRef {
    let p = get_root().get_first_child().unwrap();
    assert_eq!(p.get_type(), "paragraph");
    p
}

fn read(editor: &Editor, f: impl FnOnce()) {
    editor.read(f).unwrap();
}

/// `editorConfig`.
#[cfg(feature = "html")]
fn editor_config() -> crate::EditorConfig {
    crate::EditorConfig {
        theme: crate::json::parse(
            r#"{"link":"my-link-class","text":{"bold":"my-bold-class","code":"my-code-class","hashtag":"my-hashtag-class","italic":"my-italic-class","strikethrough":"my-strikethrough-class","underline":"my-underline-class","underlineStrikethrough":"my-underline-strikethrough-class"}}"#,
        )
        .unwrap()
        .as_object()
        .unwrap()
        .clone(),
    }
}

#[cfg(feature = "html")]
fn outer_html(node: &NodeRef, config: &crate::EditorConfig, editor: &Editor) -> String {
    node.get_latest()
        .create_dom(node, config, editor)
        .unwrap()
        .outer_html()
}

/// `createDOM(editorConfig).outerHTML` of `$createLinkNode(url)` is `expected`.
#[cfg(feature = "html")]
fn assert_create_dom(url: &str, expected: &str) {
    let editor = editor();
    let e = editor.clone();
    let (url, expected) = (url.to_owned(), expected.to_owned());
    update(&editor, move || {
        assert_eq!(
            outer_html(&plain(&url), &editor_config(), &e),
            expected,
            "{url:?}"
        );
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.updateFromJSON restores null for empty or absent attributes
#[test]
fn link_node_update_from_json_restores_null_for_empty_or_absent_attributes() {
    let editor = editor();
    update(&editor, || {
        let link = with_rel_target_title("https://example.com/", "noopener", "_blank", "example");
        let json = |s: &str| crate::json::parse(s).unwrap().as_object().unwrap().clone();
        link.update_from_json(&json(
            r#"{"rel":"","target":"","title":"","url":"https://example.com/"}"#,
        ))?;
        assert_eq!(link.get_rel(), None);
        assert_eq!(link.get_target(), None);
        assert_eq!(link.get_title(), None);
        link.set_rel(Some("noopener"));
        link.update_from_json(&json(r#"{"url":"https://example.com/"}"#))?;
        assert_eq!(link.get_rel(), None);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.constructor
#[test]
fn link_node_constructor() {
    let editor = editor();
    update(&editor, || {
        let link_node = plain("/");
        assert_eq!(link_node.get_type(), "link");
        assert_eq!(fields(&link_node).0, "/");
        Ok(())
    });
    assert!(panics(|| plain("")));
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.clone()
#[test]
fn link_node_clone() {
    let editor = editor();
    update(&editor, || {
        let node = plain("/");
        let clone = crate::clone_with_properties(&node);
        let latest = node.get_latest();
        assert!(!std::ptr::addr_eq(Rc::as_ptr(&clone), Rc::as_ptr(&latest)));
        let c = clone.downcast_ref::<LinkNode>().unwrap();
        let o = latest.downcast_ref::<LinkNode>().unwrap();
        assert_eq!(clone.get_key(), latest.get_key());
        assert_eq!(
            (c.url(), c.target(), c.rel(), c.title()),
            (o.url(), o.target(), o.rel(), o.title())
        );
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.getURL()
#[test]
fn link_node_get_url() {
    let editor = editor();
    update(&editor, || {
        assert_eq!(
            plain("https://example.com/foo").get_url().as_deref(),
            Some("https://example.com/foo")
        );
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.setURL()
#[test]
fn link_node_set_url() {
    let editor = editor();
    update(&editor, || {
        let node = plain("https://example.com/foo");
        assert_eq!(node.get_url().as_deref(), Some("https://example.com/foo"));
        node.set_url("https://example.com/bar");
        assert_eq!(node.get_url().as_deref(), Some("https://example.com/bar"));
        Ok(())
    });
}

fn with(url: &str, attributes: LinkAttributes) -> NodeRef {
    create_link_node(url, &attributes)
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.getTarget()
#[test]
fn link_node_get_target() {
    let editor = editor();
    update(&editor, || {
        let node = with(
            "https://example.com/foo",
            LinkAttributes {
                target: some("_blank"),
                ..LinkAttributes::default()
            },
        );
        assert_eq!(node.get_target().as_deref(), Some("_blank"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.setTarget()
#[test]
fn link_node_set_target() {
    let editor = editor();
    update(&editor, || {
        let node = with(
            "https://example.com/foo",
            LinkAttributes {
                target: some("_blank"),
                ..LinkAttributes::default()
            },
        );
        assert_eq!(node.get_target().as_deref(), Some("_blank"));
        node.set_target(Some("_self"));
        assert_eq!(node.get_target().as_deref(), Some("_self"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.getRel()
#[test]
fn link_node_get_rel() {
    let editor = editor();
    update(&editor, || {
        let node = with(
            "https://example.com/foo",
            LinkAttributes {
                rel: some("noopener noreferrer"),
                target: some("_blank"),
                ..LinkAttributes::default()
            },
        );
        assert_eq!(node.get_rel().as_deref(), Some("noopener noreferrer"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.setRel()
#[test]
fn link_node_set_rel() {
    let editor = editor();
    update(&editor, || {
        let node = with(
            "https://example.com/foo",
            LinkAttributes {
                rel: some("noopener"),
                target: some("_blank"),
                ..LinkAttributes::default()
            },
        );
        assert_eq!(node.get_rel().as_deref(), Some("noopener"));
        node.set_rel(Some("noopener noreferrer"));
        assert_eq!(node.get_rel().as_deref(), Some("noopener noreferrer"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.getTitle()
#[test]
fn link_node_get_title() {
    let editor = editor();
    update(&editor, || {
        let node = with(
            "https://example.com/foo",
            LinkAttributes {
                title: some("Hello world"),
                ..LinkAttributes::default()
            },
        );
        assert_eq!(node.get_title().as_deref(), Some("Hello world"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.setTitle()
#[test]
fn link_node_set_title() {
    let editor = editor();
    update(&editor, || {
        let node = with(
            "https://example.com/foo",
            LinkAttributes {
                title: some("Hello world"),
                ..LinkAttributes::default()
            },
        );
        assert_eq!(node.get_title().as_deref(), Some("Hello world"));
        node.set_title(Some("World hello"));
        assert_eq!(node.get_title().as_deref(), Some("World hello"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM()
#[cfg(feature = "html")]
#[test]
fn link_node_create_dom() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let node = plain("https://example.com/foo");
        assert_eq!(
            outer_html(&node, &editor_config(), &e),
            r#"<a href="https://example.com/foo" class="my-link-class"></a>"#
        );
        assert_eq!(
            outer_html(&node, &crate::EditorConfig::default(), &e),
            r#"<a href="https://example.com/foo"></a>"#
        );
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() with target, rel and title
#[cfg(feature = "html")]
#[test]
fn link_node_create_dom_with_target_rel_and_title() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let node = with_rel_target_title(
            "https://example.com/foo",
            "noopener noreferrer",
            "_blank",
            "Hello world",
        );
        assert_eq!(
            outer_html(&node, &editor_config(), &e),
            r#"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world" class="my-link-class"></a>"#
        );
        assert_eq!(
            outer_html(&node, &crate::EditorConfig::default(), &e),
            r#"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world"></a>"#
        );
        Ok(())
    });
}

const BLANK: &str = r#"<a href="about:blank" class="my-link-class"></a>"#;

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() sanitizes javascript: URLs
#[cfg(feature = "html")]
#[test]
fn link_node_create_dom_sanitizes_javascript_urls() {
    assert_create_dom("javascript:alert(0)", BLANK);
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() fails closed on unparseable URLs
#[cfg(feature = "html")]
#[test]
fn link_node_create_dom_fails_closed_on_unparseable_urls() {
    assert_create_dom("javascript://:99999999999/%0aalert(document.domain)", BLANK);
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme "java\u0000script:alert(document.domain)"
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme "java\tscript:alert(document.domain)"
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme "\u0001javascript:alert(document.domain)"
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme "java\nscript:alert(document.domain)"
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme "data:text/html,<script>alert(1)</script>"
#[cfg(feature = "html")]
#[test]
fn link_node_create_dom_neutralizes_obfuscated_dangerous_schemes() {
    for input in [
        "java\u{0}script:alert(document.domain)",
        "java\tscript:alert(document.domain)",
        "\u{1}javascript:alert(document.domain)",
        "java\nscript:alert(document.domain)",
        "data:text/html,<script>alert(1)</script>",
    ] {
        assert_create_dom(input, BLANK);
    }
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > https://example.com -> https://example.com
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > https://www.example.com -> https://www.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > example.com -> https://example.com
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > www.example.com -> https://www.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > mailto:user@example.com -> mailto:user@example.com
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > user@example.com -> mailto:user@example.com
// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > +1234567890 -> tel:+1234567890
#[cfg(feature = "html")]
#[test]
fn link_node_create_dom_formats_urls() {
    for (input, output) in [
        ("https://example.com", "https://example.com"),
        ("https://www.example.com", "https://www.example.com"),
        ("example.com", "https://example.com"),
        ("www.example.com", "https://www.example.com"),
        ("mailto:user@example.com", "mailto:user@example.com"),
        ("user@example.com", "mailto:user@example.com"),
        ("+1234567890", "tel:+1234567890"),
    ] {
        assert_create_dom(
            input,
            &format!(r#"<a href="{output}" class="my-link-class"></a>"#),
        );
    }
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.canInsertTextBefore()
#[test]
fn link_node_can_insert_text_before() {
    let editor = editor();
    update(&editor, || {
        assert!(!plain("https://example.com/foo").can_insert_text_before());
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.canInsertTextAfter()
#[test]
fn link_node_can_insert_text_after() {
    let editor = editor();
    update(&editor, || {
        assert!(!plain("https://example.com/foo").can_insert_text_after());
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $createLinkNode()
#[test]
fn create_link_node_function() {
    let editor = editor();
    update(&editor, || {
        let link_node = plain("https://example.com/foo");
        let created = plain("https://example.com/foo");
        assert_eq!(link_node.get_type(), created.get_type());
        assert_eq!(link_node.get_parent(), created.get_parent());
        assert_eq!(fields(&link_node).0, fields(&created).0);
        assert_ne!(link_node.get_key(), created.get_key());
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $createLinkNode() with target, rel and title
#[test]
fn create_link_node_with_target_rel_and_title() {
    let editor = editor();
    update(&editor, || {
        let make = || {
            with_rel_target_title(
                "https://example.com/foo",
                "noopener noreferrer",
                "_blank",
                "Hello world",
            )
        };
        let (link_node, created) = (make(), make());
        assert_eq!(link_node.get_type(), created.get_type());
        assert_eq!(link_node.get_parent(), created.get_parent());
        assert_eq!(fields(&link_node), fields(&created));
        assert_ne!(link_node.get_key(), created.get_key());
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $isLinkNode()
#[test]
fn is_link_node_function() {
    let editor = editor();
    update(&editor, || {
        assert!(is_link_node(&plain("")));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink applies the title attribute when creating
#[test]
fn toggle_link_applies_the_title_attribute_when_creating() {
    let editor = editor();
    update(&editor, || {
        let p = create_paragraph_node();
        let text_node = create_text_node("Some text");
        p.append(std::slice::from_ref(&text_node))?;
        get_root().append(std::slice::from_ref(&p))?;
        select_all()?;
        toggle_link(
            Some("https://lexical.dev/"),
            &LinkAttributes {
                title: some("Lexical Website"),
                ..LinkAttributes::default()
            },
        )?;
        let link_node = p.get_first_child().unwrap();
        assert!(is_link_node(&link_node));
        assert_eq!(link_node.get_title().as_deref(), Some("Lexical Website"));
        let selection = get_selection().unwrap();
        assert!(selection.is_range());
        let anchor = selection.point(Which::Anchor).unwrap();
        assert_eq!(
            (anchor.key.as_str(), anchor.offset, anchor.point_type),
            (text_node.get_key(), 0, PointType::Text)
        );
        let focus = selection.point(Which::Focus).unwrap();
        assert_eq!(
            (focus.key.as_str(), focus.offset, focus.point_type),
            (
                text_node.get_key(),
                text_node.get_text_content_size(),
                PointType::Text
            )
        );
        Ok(())
    });
    let json = editor.get_editor_state().to_json(false);
    let title = json
        .get("root")
        .and_then(|r| r.get("children"))
        .and_then(|c| c.as_array().map(|a| a[0].clone()))
        .and_then(|p| {
            p.get("children")
                .and_then(|c| c.as_array().map(|a| a[0].clone()))
        })
        .and_then(|l| l.get("title").cloned());
    assert_eq!(
        title.as_ref().and_then(Value::as_str),
        Some("Lexical Website")
    );
}

fn remove_all_links(editor: &Editor) {
    update(editor, || {
        select_all()?;
        toggle_link(None, &LinkAttributes::default())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink correctly removes link when textnode has children(like marknode)
#[cfg(feature = "mark")]
#[test]
fn toggle_link_correctly_removes_link_when_textnode_has_children() {
    use crate::mark::{create_mark_node, is_mark_node};
    let editor = editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let preceding_text = create_text_node("some ");
        let text_node = create_text_node("text");
        paragraph.append(&[preceding_text, text_node.clone()])?;
        let link_node = with(
            "https://example.com/foo",
            LinkAttributes {
                rel: some("noreferrer"),
                ..LinkAttributes::default()
            },
        );
        text_node.insert_after(&link_node, true)?;
        link_node.append(std::slice::from_ref(&text_node))?;
        let mark_node = create_mark_node(&["knetk".to_owned()]);
        text_node.insert_before(&mark_node, true)?;
        mark_node.append(&[text_node])?;
        get_root().append(&[paragraph])
    });
    let check_mark = |mark_node: &NodeRef| {
        if is_mark_node(mark_node) {
            assert_eq!(mark_node.get_type(), "mark");
            assert_eq!(mark_node.get_ids(), Some(vec!["knetk".to_owned()]));
            assert_eq!(mark_node.get_text_content(), "text");
        }
    };
    read(&editor, || {
        let children = paragraph().get_children();
        assert_eq!(children[0].get_text_content(), "some ");
        let link_node = &children[1];
        assert!(is_link_node(link_node));
        assert_eq!(
            link_node.get_url().as_deref(),
            Some("https://example.com/foo")
        );
        assert_eq!(link_node.get_rel().as_deref(), Some("noreferrer"));
        check_mark(&link_node.get_first_child().unwrap());
    });
    remove_all_links(&editor);
    read(&editor, || {
        let children = paragraph().get_children();
        assert_eq!(children[0].get_text_content(), "some ");
        assert!(is_mark_node(&children[1]));
        check_mark(&children[1]);
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink correctly removes link when link contains heading
#[cfg(feature = "rich-text")]
#[test]
fn toggle_link_correctly_removes_link_when_link_contains_heading() {
    use crate::rich_text::{HeadingTagType, create_heading_node};
    let editor = editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let link_node = plain("https://example.com/foo");
        let heading_node = create_heading_node(HeadingTagType::H3);
        heading_node.append(&[create_text_node("Example Link")])?;
        link_node.append(&[heading_node])?;
        paragraph.append(&[link_node])?;
        get_root().append(&[paragraph])
    });
    read(&editor, || {
        let link_node = paragraph().get_first_child().unwrap();
        assert!(is_link_node(&link_node));
        assert_eq!(
            link_node.get_url().as_deref(),
            Some("https://example.com/foo")
        );
        let heading_node = link_node.get_first_child().unwrap();
        assert_eq!(heading_node.get_type(), "heading");
        assert_eq!(heading_node.get_text_content(), "Example Link");
    });
    remove_all_links(&editor);
    read(&editor, || {
        let children = paragraph().get_children();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].get_type(), "heading");
        assert_eq!(children[0].get_text_content(), "Example Link");
        assert!(!is_link_node(&children[0]));
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink adds link with embedded LineBreakNode
#[test]
fn toggle_link_adds_link_with_embedded_line_break_node() {
    let editor = editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[
            create_text_node("some "),
            create_text_node("text"),
            create_line_break_node(),
        ])?;
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.select(Some(1), None)?;
        toggle_link(
            Some("https://example.com/foo"),
            &LinkAttributes {
                rel: some("noreferrer"),
                ..LinkAttributes::default()
            },
        )
    });
    read(&editor, || {
        let children = paragraph().get_children();
        assert_eq!(children[0].get_text_content(), "some ");
        let link_node = &children[1];
        assert!(is_link_node(link_node));
        assert_eq!(
            link_node.get_url().as_deref(),
            Some("https://example.com/foo")
        );
        assert_eq!(link_node.get_rel().as_deref(), Some("noreferrer"));
        let texts: Vec<String> = link_node
            .get_children()
            .iter()
            .map(NodeRef::get_text_content)
            .collect();
        assert_eq!(texts, ["text", "\n"]);
        let selection = get_selection().unwrap();
        let anchor = selection.point(Which::Anchor).unwrap();
        assert_eq!(
            (anchor.key.as_str(), anchor.offset, anchor.point_type),
            (
                link_node.get_first_child_or_throw().unwrap().get_key(),
                0,
                PointType::Text
            )
        );
        let focus = selection.point(Which::Focus).unwrap();
        assert_eq!(
            (focus.key.as_str(), focus.offset, focus.point_type),
            (link_node.get_key(), 2, PointType::Element)
        );
    });
    remove_all_links(&editor);
    read(&editor, || {
        let children = paragraph().get_children();
        let texts: Vec<String> = children.iter().map(NodeRef::get_text_content).collect();
        assert_eq!(texts, ["some text", "\n"]);
        assert!(children[0].is_text());
        assert_eq!(children[1].get_type(), "linebreak");
    });
}

/// A paragraph holding a link to https://example.com of `text`; returns
/// the text node's key.
fn link_of_text(editor: &Editor, text: &'static str) -> String {
    let key = Rc::new(RefCell::new(String::new()));
    let out = key.clone();
    update(editor, move || {
        let paragraph = create_paragraph_node();
        let text_node = create_text_node(text);
        *out.borrow_mut() = text_node.get_key().to_owned();
        let link_node = plain("https://example.com");
        link_node.append(&[text_node])?;
        paragraph.append(&[link_node])?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    });
    read(editor, || {
        let link_node = paragraph().get_first_child().unwrap();
        assert!(is_link_node(&link_node));
        assert_eq!(link_node.get_text_content(), text);
    });
    key.borrow().clone()
}

fn unlink_range(editor: &Editor, key: String, start: usize, end: usize) {
    update(editor, move || {
        let text_node = get_node_by_key(&key).unwrap();
        if text_node.is_text() {
            text_node.select(Some(start), Some(end))?;
            toggle_link(None, &LinkAttributes::default())?;
        }
        Ok(())
    });
}

fn assert_link(node: &NodeRef, text: &str) {
    assert!(is_link_node(node));
    assert_eq!(node.get_text_content(), text);
    assert_eq!(node.get_url().as_deref(), Some("https://example.com"));
}

fn assert_text(node: &NodeRef, text: &str) {
    assert!(node.is_text());
    assert_eq!(node.get_text_content(), text);
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink removes link from trailing space only
#[test]
fn toggle_link_removes_link_from_trailing_space_only() {
    let editor = editor();
    let key = link_of_text(&editor, "hello ");
    unlink_range(&editor, key, 5, 6);
    read(&editor, || {
        let children = paragraph().get_children();
        assert_eq!(children.len(), 2);
        assert_link(&children[0], "hello");
        assert_text(&children[1], " ");
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink removes link from leading space only
#[test]
fn toggle_link_removes_link_from_leading_space_only() {
    let editor = editor();
    let key = link_of_text(&editor, " hello");
    unlink_range(&editor, key, 0, 1);
    read(&editor, || {
        let children = paragraph().get_children();
        assert_eq!(children.len(), 2);
        assert_text(&children[0], " ");
        assert_link(&children[1], "hello");
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink removes link from middle word only, preserving surrounding spaces
#[test]
fn toggle_link_removes_link_from_middle_word_only() {
    let editor = editor();
    let key = link_of_text(&editor, " hello ");
    unlink_range(&editor, key, 1, 6);
    read(&editor, || {
        let children = paragraph().get_children();
        assert_eq!(children.len(), 3);
        assert_link(&children[0], " ");
        assert_text(&children[1], "hello");
        assert_link(&children[2], " ");
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink removes link when selection is collapsed
#[test]
fn toggle_link_removes_link_when_selection_is_collapsed() {
    let editor = editor();
    update(&editor, || -> Result<(), LexicalError> {
        let p = create_paragraph_node();
        let text_node = create_text_node("textboldtext");
        p.append(std::slice::from_ref(&text_node))?;
        get_root().append(std::slice::from_ref(&p))?;
        select_all()?;
        toggle_link(
            Some("https://lexical.dev/"),
            &LinkAttributes {
                title: some("Lexical Website"),
                ..LinkAttributes::default()
            },
        )?;
        let link_node = p.get_first_child().unwrap();
        text_node
            .select(Some(4), Some(8))?
            .format_text("bold", None)?;
        let sel = create_range_selection();
        let key = link_node
            .get_child_at_index(1)
            .unwrap()
            .get_key()
            .to_owned();
        sel.point_set(Which::Anchor, &key, 4, PointType::Text)?;
        sel.point_set(Which::Focus, &key, 4, PointType::Text)?;
        set_selection(Some(sel));
        toggle_link(None, &LinkAttributes::default())?;
        let children = p.get_children();
        assert_eq!(children.len(), 3);
        assert!(children.iter().all(NodeRef::is_text));
        Ok(())
    });
}
