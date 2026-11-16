//! Ports of `packages/lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts`.
//!
//! JS regex literals become `regex` patterns with the same meaning: `\s`
//! is [`JS_WHITESPACE`] in a class, and every pattern here is ASCII.

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{
    AutoLinkAttributes, AutoLinkConfigOverrides, ChangeHandler, ExcludeParent, JS_WHITESPACE,
    LinkMatcher, LinkMatcherResult, auto_link_extension, create_link_matcher_with_reg_exp,
    is_auto_link_node,
};
use crate::{
    AnyLexicalExtensionArgument, ElementData, InitialEditorStateType, LexicalExtension,
    LexicalNode, NodeBase, NodeClass, NodeRef, UpdateOptions, config_extension,
    create_paragraph_node, create_text_node, define_extension, get_root,
};
use regex::Regex;
use std::cell::RefCell;
use std::rc::Rc;

/// `ExcludedParentNode`: a plain element node (`excluded-parent`).
#[derive(Clone, Debug, Default)]
struct ExcludedParentNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for ExcludedParentNode {
    fn get_type(&self) -> &'static str {
        "excluded-parent"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
}

fn is_excluded_parent_node(node: &NodeRef) -> bool {
    node.get_latest().is::<ExcludedParentNode>()
}

/// `TestLexicalAutoLinkExtension`.
fn test_lexical_auto_link_extension() -> AnyLexicalExtensionArgument {
    let theme = crate::json::parse(
        r#"{"link":"my-autolink-class","text":{"bold":"my-bold-class","code":"my-code-class","hashtag":"my-hashtag-class","italic":"my-italic-class","strikethrough":"my-strikethrough-class","underline":"my-underline-class","underlineStrikethrough":"my-underline-strikethrough-class"}}"#,
    )
    .unwrap()
    .as_object()
    .unwrap()
    .clone();
    define_extension(LexicalExtension {
        dependencies: vec![auto_link_extension().into()],
        nodes: vec![NodeClass::of::<ExcludedParentNode>()],
        theme: Some(theme),
        ..LexicalExtension::<(), (), ()>::new("[test root]", ())
    })
    .into()
}

/// `buildEditorFromExtensions({$initialEditorState, dependencies:
/// [TestLexicalAutoLinkExtension, configExtension(AutoLinkExtension,
/// overrides)], name})`.
fn build(
    name: &str,
    initial: impl Fn() -> Result<(), crate::LexicalError> + 'static,
    overrides: AutoLinkConfigOverrides,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        dependencies: vec![
            test_lexical_auto_link_extension(),
            config_extension(&auto_link_extension(), [overrides]),
        ],
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn paragraph_with(text: &'static str) -> impl Fn() -> Result<(), crate::LexicalError> {
    move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node(text)])?;
        get_root().append(&[paragraph])
    }
}

fn matcher(pattern: &str) -> LinkMatcher {
    create_link_matcher_with_reg_exp(Regex::new(pattern).unwrap(), None)
}

fn url_matcher() -> LinkMatcher {
    matcher(r"https?://[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
}

fn port_url_matcher() -> LinkMatcher {
    matcher(r"https?://[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}:\d+")
}

fn gh_tag_matcher() -> LinkMatcher {
    matcher(r"GH-\d+")
}

/// `/[.,;:\s]/`.
fn colon_separator() -> Regex {
    Regex::new(&format!("[.,;:{JS_WHITESPACE}]")).unwrap()
}

fn first_paragraph() -> NodeRef {
    let paragraph = get_root().get_first_child().unwrap();
    assert_eq!(
        paragraph.get_type(),
        "paragraph",
        "first root child must be a ParagraphNode"
    );
    paragraph
}

/// The paragraph's first child, an AutoLinkNode holding `link`, and the
/// text after it, `rest`.
fn assert_link_then(link: &str, rest: &str) {
    let auto_link_node = first_paragraph().get_first_child().unwrap();
    assert!(
        is_auto_link_node(&auto_link_node),
        "first child must be an AutoLinkNode"
    );
    assert_eq!(auto_link_node.get_text_content(), link);
    let next_sibling = auto_link_node.get_next_sibling().unwrap();
    assert!(next_sibling.is_text(), "next sibling must be a TextNode");
    assert_eq!(next_sibling.get_text_content(), rest);
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > registerAutoLink does not cause infinite transform loop with #1234.Another
#[test]
fn register_auto_link_does_not_cause_infinite_transform_loop() {
    let editor = build(
        "[test override]",
        paragraph_with("#1234.Another"),
        AutoLinkConfigOverrides {
            matchers: Some(vec![matcher(r"#\d+")]),
            ..AutoLinkConfigOverrides::default()
        },
    );
    editor
        .read(|| {
            assert_eq!(first_paragraph().get_text_content(), "#1234.Another");
            assert_link_then("#1234", ".Another");
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > excludeParents prevents auto-linking inside excluded parent nodes
#[test]
fn exclude_parents_prevents_auto_linking_inside_excluded_parent_nodes() {
    let exclude: ExcludeParent = Rc::new(is_excluded_parent_node);
    let editor = build(
        "[test override]",
        || {
            let parent = crate::create::<ExcludedParentNode>();
            parent.append(&[create_text_node("https://example.com")])?;
            get_root().append(&[parent])
        },
        AutoLinkConfigOverrides {
            exclude_parents: Some(vec![exclude]),
            matchers: Some(vec![url_matcher()]),
            ..AutoLinkConfigOverrides::default()
        },
    );
    editor
        .read(|| {
            let excluded_parent = get_root().get_first_child().unwrap();
            assert!(
                is_excluded_parent_node(&excluded_parent),
                "first root child must be an ExcludedParentNode"
            );
            let child = excluded_parent.get_first_child().unwrap();
            assert!(child.is_text(), "child must be a TextNode");
            assert_eq!(child.get_text_content(), "https://example.com");
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > excludeParents does not prevent auto-linking in non-excluded parents
#[test]
fn exclude_parents_does_not_prevent_auto_linking_in_non_excluded_parents() {
    let exclude: ExcludeParent = Rc::new(is_excluded_parent_node);
    let editor = build(
        "[test override]",
        paragraph_with("https://example.com"),
        AutoLinkConfigOverrides {
            exclude_parents: Some(vec![exclude]),
            matchers: Some(vec![url_matcher()]),
            ..AutoLinkConfigOverrides::default()
        },
    );
    editor
        .read(|| {
            let auto_link_node = first_paragraph().get_first_child().unwrap();
            assert!(
                is_auto_link_node(&auto_link_node),
                "first child must be an AutoLinkNode"
            );
            assert_eq!(auto_link_node.get_text_content(), "https://example.com");
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > default separator regex does not treat colon as an auto-link boundary
#[test]
fn default_separator_regex_does_not_treat_colon_as_an_auto_link_boundary() {
    let editor = build(
        "[test override]",
        paragraph_with("GH-123: investigate"),
        AutoLinkConfigOverrides {
            matchers: Some(vec![gh_tag_matcher()]),
            ..AutoLinkConfigOverrides::default()
        },
    );
    editor
        .read(|| {
            let first_child = first_paragraph().get_first_child().unwrap();
            assert!(first_child.is_text(), "first child must be a TextNode");
            assert_eq!(first_child.get_text_content(), "GH-123: investigate");
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > custom separator regex can treat colon as an auto-link boundary
#[test]
fn custom_separator_regex_can_treat_colon_as_an_auto_link_boundary() {
    let editor = build(
        "[test override]",
        paragraph_with("GH-123: investigate"),
        AutoLinkConfigOverrides {
            matchers: Some(vec![gh_tag_matcher()]),
            separator_regex: Some(colon_separator()),
            ..AutoLinkConfigOverrides::default()
        },
    );
    editor
        .read(|| assert_link_then("GH-123", ": investigate"))
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > custom separator regex does not break auto-linking URLs with embedded colons
#[test]
fn custom_separator_regex_does_not_break_auto_linking_urls_with_embedded_colons() {
    let editor = build(
        "[test override]",
        paragraph_with("https://example.com:3000 "),
        AutoLinkConfigOverrides {
            matchers: Some(vec![port_url_matcher()]),
            separator_regex: Some(colon_separator()),
            ..AutoLinkConfigOverrides::default()
        },
    );
    editor
        .read(|| assert_link_then("https://example.com:3000", " "))
        .unwrap();
}

// handleLinkEdit keeps the matcher attributes in sync

/// `ATTRIBUTE_MATCHER`: every attribute derived from the matched url.
fn attribute_matcher() -> LinkMatcher {
    let re = Regex::new(r"https?://[a-z]+\.com").unwrap();
    Rc::new(move |text: &str| {
        let m = re.find(text)?;
        let host = m.as_str().replacen("https://", "", 1);
        let some = |prefix: &str| Some(Some(format!("{prefix}-{host}")));
        Some(LinkMatcherResult {
            attributes: Some(AutoLinkAttributes {
                rel: some("rel"),
                target: some("target"),
                title: some("title"),
                ..AutoLinkAttributes::default()
            }),
            index: m.start(),
            length: m.as_str().len(),
            text: m.as_str().to_owned(),
            url: m.as_str().to_owned(),
        })
    })
}

fn build_attribute_editor(
    name: &str,
    change_handlers: Option<Vec<ChangeHandler>>,
) -> LexicalEditorWithDispose {
    build(
        name,
        paragraph_with("https://a.com"),
        AutoLinkConfigOverrides {
            change_handlers,
            matchers: Some(vec![attribute_matcher()]),
            ..AutoLinkConfigOverrides::default()
        },
    )
}

/// `$getAutoLink()`.
fn get_auto_link() -> NodeRef {
    let auto_link = first_paragraph().get_first_child().unwrap();
    assert!(is_auto_link_node(&auto_link));
    auto_link
}

fn retype_as_b(editor: &LexicalEditorWithDispose) {
    editor
        .update(
            || {
                let text_node = get_auto_link().get_first_child().unwrap();
                assert!(text_node.is_text());
                text_node.set_text_content("https://b.com");
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > handleLinkEdit keeps the matcher attributes in sync > the creation path applies every attribute
#[test]
fn the_creation_path_applies_every_attribute() {
    let editor = build_attribute_editor("[test attributes]", None);
    editor
        .read(|| {
            let auto_link = get_auto_link();
            assert_eq!(auto_link.get_url().as_deref(), Some("https://a.com"));
            assert_eq!(auto_link.get_rel().as_deref(), Some("rel-a.com"));
            assert_eq!(auto_link.get_target().as_deref(), Some("target-a.com"));
            assert_eq!(auto_link.get_title().as_deref(), Some("title-a.com"));
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > handleLinkEdit keeps the matcher attributes in sync > editing the link text refreshes rel and target
#[test]
fn editing_the_link_text_refreshes_rel_and_target() {
    let editor = build_attribute_editor("[test attributes]", None);
    retype_as_b(&editor);
    editor
        .read(|| {
            let auto_link = get_auto_link();
            assert_eq!(auto_link.get_url().as_deref(), Some("https://b.com"));
            assert_eq!(auto_link.get_rel().as_deref(), Some("rel-b.com"));
            assert_eq!(auto_link.get_target().as_deref(), Some("target-b.com"));
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > handleLinkEdit keeps the matcher attributes in sync > editing the link text refreshes the title
#[test]
fn editing_the_link_text_refreshes_the_title() {
    let editor = build_attribute_editor("[test attributes]", None);
    retype_as_b(&editor);
    editor
        .read(|| {
            assert_eq!(get_auto_link().get_title().as_deref(), Some("title-b.com"));
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > handleLinkEdit keeps the matcher attributes in sync > an attribute refresh is not reported as a url change
#[test]
fn an_attribute_refresh_is_not_reported_as_a_url_change() {
    type Changes = Vec<(Option<String>, Option<String>)>;
    let changes: Rc<RefCell<Changes>> = Rc::default();
    let sink = changes.clone();
    let handler: ChangeHandler = Rc::new(move |url, prev_url| {
        sink.borrow_mut()
            .push((url.map(str::to_owned), prev_url.map(str::to_owned)));
    });
    let editor = build_attribute_editor("[test change handler]", Some(vec![handler]));
    changes.borrow_mut().clear();
    retype_as_b(&editor);
    assert_eq!(
        *changes.borrow(),
        [(
            Some("https://b.com".to_owned()),
            Some("https://a.com".to_owned())
        )]
    );
}
