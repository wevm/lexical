//! Ports of `packages/lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts`,
//! less the three `updateDOM` tests that check the DOM writes of
//! `updateDOM` (rust/parity/skips.json).

use crate::link::{
    AutoLinkAttributes, AutoLinkNode, LinkAttributes, create_auto_link_node, is_auto_link_node,
    toggle_link,
};
use crate::node_state::StateConfig;
use crate::upstream_tests::support::{initialize_unit_test, panics, update};
use crate::{
    CreateEditorArgs, Editor, NodeRef, create_paragraph_node, create_range_selection, create_state,
    create_text_node, get_root, get_state, json, select_all, set_state,
};

fn editor() -> Editor {
    initialize_unit_test(CreateEditorArgs::default())
}

fn some(s: &str) -> Option<Option<String>> {
    Some(Some(s.to_owned()))
}

/// `new AutoLinkNode(url, attributes)`.
fn auto_link(url: &str, attributes: AutoLinkAttributes) -> NodeRef {
    create_auto_link_node(url, &attributes)
}

fn plain(url: &str) -> NodeRef {
    auto_link(url, AutoLinkAttributes::default())
}

fn with_rel_target_title(url: &str, rel: &str, target: &str, title: &str) -> NodeRef {
    auto_link(
        url,
        AutoLinkAttributes {
            rel: some(rel),
            target: some(target),
            title: some(title),
            ..AutoLinkAttributes::default()
        },
    )
}

/// The fields of an `AutoLinkNode` (`__url`, `__target`, `__rel`,
/// `__title`, `__isUnlinked`).
fn fields(node: &NodeRef) -> (String, Option<String>, Option<String>, Option<String>, bool) {
    let latest = node.get_latest();
    let a = latest
        .downcast_ref::<AutoLinkNode>()
        .expect("an AutoLinkNode");
    let l = a.link();
    (
        l.url().to_owned(),
        l.target().map(str::to_owned),
        l.rel().map(str::to_owned),
        l.title().map(str::to_owned),
        a.is_unlinked(),
    )
}

/// `editorConfig`.
#[cfg(feature = "html")]
fn editor_config() -> crate::EditorConfig {
    crate::EditorConfig {
        theme: json::parse(
            r#"{"link":"my-autolink-class","text":{"bold":"my-bold-class","code":"my-code-class","hashtag":"my-hashtag-class","italic":"my-italic-class","strikethrough":"my-strikethrough-class","underline":"my-underline-class","underlineStrikethrough":"my-underline-strikethrough-class"}}"#,
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

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.constructor
#[test]
fn auto_link_node_constructor() {
    let editor = editor();
    update(&editor, || {
        let node = plain("/");
        assert_eq!(node.get_type(), "autolink");
        let (url, .., is_unlinked) = fields(&node);
        assert_eq!(url, "/");
        assert!(!is_unlinked);
        Ok(())
    });
    assert!(panics(|| plain("")));
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.constructor with isUnlinked param set to true
#[test]
fn auto_link_node_constructor_with_is_unlinked() {
    let editor = editor();
    update(&editor, || {
        let node = auto_link(
            "/",
            AutoLinkAttributes {
                is_unlinked: Some(true),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(node.get_type(), "autolink");
        let (url, .., is_unlinked) = fields(&node);
        assert_eq!(url, "/");
        assert!(is_unlinked);
        Ok(())
    });
    assert!(panics(|| plain("")));
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.clone()
#[test]
fn auto_link_node_clone() {
    let editor = editor();
    update(&editor, || {
        let node = plain("/");
        let clone = crate::clone_with_properties(&node);
        let latest = node.get_latest();
        // Not the same object, and equal field by field.
        assert!(!std::ptr::eq(
            &*clone as *const dyn crate::LexicalNode as *const u8,
            &*latest as *const dyn crate::LexicalNode as *const u8
        ));
        let c = clone.downcast_ref::<AutoLinkNode>().unwrap();
        let o = latest.downcast_ref::<AutoLinkNode>().unwrap();
        assert_eq!(clone.get_key(), latest.get_key());
        assert_eq!(c.link().url(), o.link().url());
        assert_eq!(c.link().target(), o.link().target());
        assert_eq!(c.link().rel(), o.link().rel());
        assert_eq!(c.link().title(), o.link().title());
        assert_eq!(c.is_unlinked(), o.is_unlinked());
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getURL()
#[test]
fn auto_link_node_get_url() {
    let editor = editor();
    update(&editor, || {
        let node = plain("https://example.com/foo");
        assert_eq!(node.get_url().as_deref(), Some("https://example.com/foo"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setURL()
#[test]
fn auto_link_node_set_url() {
    let editor = editor();
    update(&editor, || {
        let node = plain("https://example.com/foo");
        assert_eq!(node.get_url().as_deref(), Some("https://example.com/foo"));
        node.set_url("https://example.com/bar");
        assert_eq!(node.get_url().as_deref(), Some("https://example.com/bar"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getTarget()
#[test]
fn auto_link_node_get_target() {
    let editor = editor();
    update(&editor, || {
        let node = auto_link(
            "https://example.com/foo",
            AutoLinkAttributes {
                target: some("_blank"),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(node.get_target().as_deref(), Some("_blank"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setTarget()
#[test]
fn auto_link_node_set_target() {
    let editor = editor();
    update(&editor, || {
        let node = auto_link(
            "https://example.com/foo",
            AutoLinkAttributes {
                target: some("_blank"),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(node.get_target().as_deref(), Some("_blank"));
        node.set_target(Some("_self"));
        assert_eq!(node.get_target().as_deref(), Some("_self"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getRel()
#[test]
fn auto_link_node_get_rel() {
    let editor = editor();
    update(&editor, || {
        let node = auto_link(
            "https://example.com/foo",
            AutoLinkAttributes {
                rel: some("noopener noreferrer"),
                target: some("_blank"),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(node.get_rel().as_deref(), Some("noopener noreferrer"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setRel()
#[test]
fn auto_link_node_set_rel() {
    let editor = editor();
    update(&editor, || {
        let node = auto_link(
            "https://example.com/foo",
            AutoLinkAttributes {
                rel: some("noopener"),
                target: some("_blank"),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(node.get_rel().as_deref(), Some("noopener"));
        node.set_rel(Some("noopener noreferrer"));
        assert_eq!(node.get_rel().as_deref(), Some("noopener noreferrer"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getTitle()
#[test]
fn auto_link_node_get_title() {
    let editor = editor();
    update(&editor, || {
        let node = auto_link(
            "https://example.com/foo",
            AutoLinkAttributes {
                title: some("Hello world"),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(node.get_title().as_deref(), Some("Hello world"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setTitle()
#[test]
fn auto_link_node_set_title() {
    let editor = editor();
    update(&editor, || {
        let node = auto_link(
            "https://example.com/foo",
            AutoLinkAttributes {
                title: some("Hello world"),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(node.get_title().as_deref(), Some("Hello world"));
        node.set_title(Some("World hello"));
        assert_eq!(node.get_title().as_deref(), Some("World hello"));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getIsUnlinked()
#[test]
fn auto_link_node_get_is_unlinked() {
    let editor = editor();
    update(&editor, || {
        let node = auto_link(
            "/",
            AutoLinkAttributes {
                is_unlinked: Some(true),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(node.get_is_unlinked(), Some(true));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setIsUnlinked()
#[test]
fn auto_link_node_set_is_unlinked() {
    let editor = editor();
    update(&editor, || {
        let node = plain("/");
        assert_eq!(node.get_is_unlinked(), Some(false));
        node.set_is_unlinked(true);
        assert_eq!(node.get_is_unlinked(), Some(true));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.createDOM()
#[cfg(feature = "html")]
#[test]
fn auto_link_node_create_dom() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let node = plain("https://example.com/foo");
        assert_eq!(
            outer_html(&node, &editor_config(), &e),
            r#"<a href="https://example.com/foo" class="my-autolink-class"></a>"#
        );
        assert_eq!(
            outer_html(&node, &crate::EditorConfig::default(), &e),
            r#"<a href="https://example.com/foo"></a>"#
        );
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.createDOM() for unlinked
#[cfg(feature = "html")]
#[test]
fn auto_link_node_create_dom_for_unlinked() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let node = auto_link(
            "https://example.com/foo",
            AutoLinkAttributes {
                is_unlinked: Some(true),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(
            outer_html(&node, &editor_config(), &e),
            format!("<span>{}</span>", node.get_text_content())
        );
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.createDOM() with target, rel and title
#[cfg(feature = "html")]
#[test]
fn auto_link_node_create_dom_with_target_rel_and_title() {
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
            r#"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world" class="my-autolink-class"></a>"#
        );
        assert_eq!(
            outer_html(&node, &crate::EditorConfig::default(), &e),
            r#"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world"></a>"#
        );
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.createDOM() sanitizes javascript: URLs
#[cfg(feature = "html")]
#[test]
fn auto_link_node_create_dom_sanitizes_javascript_urls() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let node = plain("javascript:alert(0)");
        assert_eq!(
            outer_html(&node, &editor_config(), &e),
            r#"<a href="about:blank" class="my-autolink-class"></a>"#
        );
        Ok(())
    });
}

/// `updateDOM` is its model half in Rust (rust/PARITY.md): whether the
/// element is made again, which a change of `isUnlinked` asks for.
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.updateDOM() with isUnlinked "true"
#[cfg(feature = "html")]
#[test]
fn auto_link_node_update_dom_with_is_unlinked_true() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let node = auto_link(
            "https://example.com/foo",
            AutoLinkAttributes {
                is_unlinked: Some(false),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(
            outer_html(&node, &editor_config(), &e),
            r#"<a href="https://example.com/foo" class="my-autolink-class"></a>"#
        );
        let new_node = auto_link(
            "https://example.com/bar",
            AutoLinkAttributes {
                is_unlinked: Some(true),
                ..AutoLinkAttributes::default()
            },
        );
        assert_eq!(
            outer_html(&new_node, &editor_config(), &e),
            format!("<span>{}</span>", new_node.get_text_content())
        );
        assert!(new_node.get_latest().update_dom(&*node.get_latest()));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.canInsertTextBefore()
#[test]
fn auto_link_node_can_insert_text_before() {
    let editor = editor();
    update(&editor, || {
        assert!(!plain("https://example.com/foo").can_insert_text_before());
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.canInsertTextAfter()
#[test]
fn auto_link_node_can_insert_text_after() {
    let editor = editor();
    update(&editor, || {
        assert!(!plain("https://example.com/foo").can_insert_text_after());
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > $createAutoLinkNode()
#[test]
fn create_auto_link_node_function() {
    let editor = editor();
    update(&editor, || {
        let node = plain("https://example.com/foo");
        let created =
            create_auto_link_node("https://example.com/foo", &AutoLinkAttributes::default());
        assert_eq!(node.get_type(), created.get_type());
        assert_eq!(node.get_parent(), created.get_parent());
        let (a, b) = (fields(&node), fields(&created));
        assert_eq!(a.0, b.0);
        assert_eq!(a.4, b.4);
        assert_ne!(node.get_key(), created.get_key());
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > $createAutoLinkNode() with target, rel, isUnlinked and title
#[test]
fn create_auto_link_node_with_target_rel_is_unlinked_and_title() {
    let editor = editor();
    update(&editor, || {
        let node = with_rel_target_title(
            "https://example.com/foo",
            "noopener noreferrer",
            "_blank",
            "Hello world",
        );
        let created = create_auto_link_node(
            "https://example.com/foo",
            &AutoLinkAttributes {
                is_unlinked: Some(true),
                rel: some("noopener noreferrer"),
                target: some("_blank"),
                title: some("Hello world"),
            },
        );
        assert_eq!(node.get_type(), created.get_type());
        assert_eq!(node.get_parent(), created.get_parent());
        let (a, b) = (fields(&node), fields(&created));
        assert_eq!((&a.0, &a.1, &a.2, &a.3), (&b.0, &b.1, &b.2, &b.3));
        assert_ne!(node.get_key(), created.get_key());
        assert_ne!(a.4, b.4);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > $isAutoLinkNode()
#[test]
fn is_auto_link_node_function() {
    let editor = editor();
    update(&editor, || {
        assert!(is_auto_link_node(&plain("")));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > $toggleLink applies the title attribute when creating
#[test]
fn toggle_link_applies_the_title_attribute_when_creating() {
    let editor = editor();
    update(&editor, || {
        let p = create_paragraph_node();
        p.append(&[create_text_node("Some text")])?;
        get_root().append(&[p])
    });
    update(&editor, || {
        select_all()?;
        toggle_link(
            Some("https://lexical.dev/"),
            &LinkAttributes {
                title: some("Lexical Website"),
                ..LinkAttributes::default()
            },
        )
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
        title.as_ref().and_then(|t| t.as_str()),
        Some("Lexical Website")
    );
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.insertNewAfter does not create new paragraph
#[test]
fn auto_link_node_insert_new_after_does_not_create_new_paragraph() {
    let editor = editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        let auto_link = plain("https://example.com");
        auto_link.append(&[create_text_node("https://example.com")])?;
        paragraph.append(std::slice::from_ref(&auto_link))?;
        root.append(std::slice::from_ref(&paragraph))?;
        let new_node = auto_link.insert_new_after(&create_range_selection(), false)?;
        assert!(new_node.as_ref().is_some_and(is_auto_link_node));
        assert_eq!(paragraph.get_children_size(), 2);
        assert_eq!(root.get_children_size(), 1);
        Ok(())
    });
}

// AutoLinkNode.insertNewAfter carries the node over

thread_local! {
    static AUTO_LINK_TEST_STATE: StateConfig<String> =
        create_state("autoLinkTestState", |v: &json::Value| {
            v.as_str().map(str::to_owned).unwrap_or_default()
        });
}

/// `$seed()`.
fn seed() -> Result<NodeRef, crate::LexicalError> {
    let root = get_root();
    root.clear()?;
    let paragraph = create_paragraph_node();
    let auto_link = create_auto_link_node(
        "https://example.com",
        &AutoLinkAttributes {
            is_unlinked: Some(true),
            rel: some("noopener"),
            target: some("_blank"),
            title: some("Example"),
        },
    );
    auto_link.set_element_style("color: red");
    auto_link.set_text_style("font-size: 20px");
    auto_link.set_text_format(1.0);
    AUTO_LINK_TEST_STATE.with(|s| set_state(&auto_link, s, "carried".to_owned()))?;
    auto_link.append(&[create_text_node("https://example.com")])?;
    paragraph.append(std::slice::from_ref(&auto_link))?;
    root.append(&[paragraph])?;
    Ok(auto_link)
}

fn seed_and_insert_new_after() -> Result<NodeRef, crate::LexicalError> {
    Ok(seed()?
        .insert_new_after(&create_range_selection(), false)?
        .expect("a node"))
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::AutoLinkNode.insertNewAfter carries the node over > keeps the element style, text style and text format
#[test]
fn insert_new_after_keeps_the_element_style_text_style_and_text_format() {
    let editor = editor();
    update(&editor, || {
        let next = seed_and_insert_new_after()?;
        assert!(is_auto_link_node(&next));
        let data = next.element_data();
        assert_eq!(data.get_style(), "color: red");
        assert_eq!(data.get_text_style(), "font-size: 20px");
        assert_eq!(data.get_text_format(), 1.0);
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::AutoLinkNode.insertNewAfter carries the node over > keeps the NodeState
#[test]
fn insert_new_after_keeps_the_node_state() {
    let editor = editor();
    update(&editor, || {
        let next = seed_and_insert_new_after()?;
        assert_eq!(
            AUTO_LINK_TEST_STATE.with(|s| get_state(&next, s))?,
            "carried"
        );
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::AutoLinkNode.insertNewAfter carries the node over > keeps the link attributes (control)
#[test]
fn insert_new_after_keeps_the_link_attributes() {
    let editor = editor();
    update(&editor, || {
        let next = seed_and_insert_new_after()?;
        assert!(is_auto_link_node(&next));
        assert_eq!(next.get_url().as_deref(), Some("https://example.com"));
        assert_eq!(next.get_rel().as_deref(), Some("noopener"));
        assert_eq!(next.get_target().as_deref(), Some("_blank"));
        assert_eq!(next.get_title().as_deref(), Some("Example"));
        assert_eq!(next.get_is_unlinked(), Some(true));
        Ok(())
    });
}
