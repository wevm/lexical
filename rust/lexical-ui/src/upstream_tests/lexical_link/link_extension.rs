//! Ports of `packages/lexical-link/src/__tests__/unit/LinkExtension.test.ts`.

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{
    AutoLinkAttributes, LinkAttributes, LinkConfigOverrides, TOGGLE_LINK_COMMAND,
    ToggleLinkPayload, create_auto_link_node, create_link_node, is_link_node, link_extension,
    toggle_link,
};
use crate::rich_text::rich_text_extension;
use crate::{
    AnyLexicalExtensionArgument, InitialEditorStateType, LexicalError, LexicalExtension, NodeClass,
    NodeRef, UpdateOptions, Which, config_extension, create_paragraph_node, create_text_node,
    define_extension, get_root, get_selection,
};
use std::rc::Rc;

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn build(
    name: &str,
    initial: impl Fn() -> Result<(), LexicalError> + 'static,
    dependencies: Vec<AnyLexicalExtensionArgument>,
    nodes: Vec<NodeClass>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        dependencies,
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn paragraph_with(text: &'static str) -> impl Fn() -> Result<(), LexicalError> {
    move || {
        let p = create_paragraph_node();
        p.append(&[create_text_node(text)])?;
        get_root().append(&[p])
    }
}

/// `extension`: a paragraph of "Hello", with `LinkExtension` and
/// `RichTextExtension`.
fn build_editor() -> LexicalEditorWithDispose {
    build(
        "[root]",
        paragraph_with("Hello"),
        vec![link_extension().into(), rich_text_extension().into()],
        vec![],
    )
}

fn some(s: &str) -> Option<Option<String>> {
    Some(Some(s.to_owned()))
}

fn link(url: &str, attributes: LinkAttributes, text: &str) -> NodeRef {
    let link = create_link_node(url, &attributes);
    link.append(&[create_text_node(text)]).unwrap();
    link
}

fn plain_link(url: &str, text: &str) -> NodeRef {
    link(url, LinkAttributes::default(), text)
}

fn paragraph() -> NodeRef {
    let p = get_root().get_first_child().unwrap();
    assert_eq!(
        p.get_type(),
        "paragraph",
        "Expecting a ParagraphNode in the root"
    );
    p
}

/// Clears the paragraph and appends `children` to it.
fn fill_paragraph(children: &[NodeRef]) -> Result<(), LexicalError> {
    let p = paragraph();
    p.clear()?;
    p.append(children)
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > can convert a text node to a link with $toggleLink
#[test]
fn can_convert_a_text_node_to_a_link_with_toggle_link() {
    let editor = build_editor();
    editor
        .update(
            || {
                let text_node = get_root().get_last_descendant().unwrap();
                assert!(text_node.is_text(), "Expected a TextNode");
                text_node.select(Some(0), None)?;
                let parent_is_link = || text_node.get_parent().is_some_and(|p| is_link_node(&p));
                assert!(!parent_is_link());
                toggle_link(Some("https://lexical.dev/"), &LinkAttributes::default())?;
                assert!(parent_is_link());
                let link_node = text_node.get_parent().unwrap();
                assert_eq!(link_node.get_url().as_deref(), Some("https://lexical.dev/"));
                assert_eq!(link_node.get_target(), None);
                assert_eq!(get_root().get_text_content(), "Hello");
                toggle_link(None, &LinkAttributes::default())?;
                assert_eq!(get_root().get_text_content(), "Hello");
                assert!(!parent_is_link());
                toggle_link(
                    Some("https://lexical.dev/"),
                    &LinkAttributes {
                        rel: some("noopener"),
                        target: some("_blank"),
                        title: some("title"),
                    },
                )?;
                let link_node = text_node.get_parent().unwrap();
                assert!(is_link_node(&link_node), "Expected a LinkNode");
                assert_eq!(link_node.get_url().as_deref(), Some("https://lexical.dev/"));
                assert_eq!(link_node.get_target().as_deref(), Some("_blank"));
                assert_eq!(link_node.get_rel().as_deref(), Some("noopener"));
                assert_eq!(link_node.get_title().as_deref(), Some("title"));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// merge adjacent LinkNodes

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > merges adjacent LinkNodes with identical attributes
#[test]
fn merges_adjacent_link_nodes_with_identical_attributes() {
    let editor = build_editor();
    let attributes = || LinkAttributes {
        rel: some("noreferrer"),
        target: some("_blank"),
        title: some("Lexical"),
    };
    editor
        .update(
            move || {
                fill_paragraph(&[
                    link("https://lexical.dev/", attributes(), "Hello "),
                    link("https://lexical.dev/", attributes(), "World"),
                ])
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| {
            let children = paragraph().get_children();
            assert_eq!(children.len(), 1);
            let link = &children[0];
            assert!(is_link_node(link), "Expected a LinkNode");
            assert_eq!(link.get_url().as_deref(), Some("https://lexical.dev/"));
            assert_eq!(link.get_target().as_deref(), Some("_blank"));
            assert_eq!(link.get_rel().as_deref(), Some("noreferrer"));
            assert_eq!(link.get_title().as_deref(), Some("Lexical"));
            assert_eq!(link.get_text_content(), "Hello World");
        })
        .unwrap();
}

fn paragraph_children_count(editor: &LexicalEditorWithDispose) -> usize {
    editor.read(|| paragraph().get_children_size()).unwrap()
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > does not merge LinkNodes with different URLs
#[test]
fn does_not_merge_link_nodes_with_different_urls() {
    let editor = build_editor();
    editor
        .update(
            || {
                fill_paragraph(&[
                    plain_link("https://lexical.dev/", "Hello "),
                    plain_link("https://other.dev/", "World"),
                ])
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(paragraph_children_count(&editor), 2);
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > does not merge LinkNode with AutoLinkNode
#[test]
fn does_not_merge_link_node_with_auto_link_node() {
    let editor = build(
        "[root-autolink]",
        paragraph_with("Hello"),
        vec![link_extension().into(), rich_text_extension().into()],
        vec![NodeClass::of::<crate::link::AutoLinkNode>()],
    );
    editor
        .update(
            || {
                let auto_link =
                    create_auto_link_node("https://lexical.dev/", &AutoLinkAttributes::default());
                auto_link.append(&[create_text_node("World")])?;
                fill_paragraph(&[plain_link("https://lexical.dev/", "Hello "), auto_link])
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(paragraph_children_count(&editor), 2);
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > does not merge LinkNodes with different rel
#[test]
fn does_not_merge_link_nodes_with_different_rel() {
    let editor = build_editor();
    let rel = |r: &str| LinkAttributes {
        rel: some(r),
        ..LinkAttributes::default()
    };
    editor
        .update(
            move || {
                fill_paragraph(&[
                    link("https://lexical.dev/", rel("noreferrer"), "Hello "),
                    link("https://lexical.dev/", rel("noopener"), "World"),
                ])
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(paragraph_children_count(&editor), 2);
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > merges three adjacent identical links into one
#[test]
fn merges_three_adjacent_identical_links_into_one() {
    let editor = build_editor();
    editor
        .update(
            || {
                fill_paragraph(&[
                    plain_link("https://lexical.dev/", "A"),
                    plain_link("https://lexical.dev/", "B"),
                    plain_link("https://lexical.dev/", "C"),
                ])
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| {
            let children = paragraph().get_children();
            assert_eq!(children.len(), 1);
            assert!(is_link_node(&children[0]));
            assert_eq!(children[0].get_text_content(), "ABC");
        })
        .unwrap();
}

/// The selection after a merge: at 8 in "Hello World", inside a link.
fn assert_selection_after_merge() {
    let children = paragraph().get_children();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].get_text_content(), "Hello World");
    let selection = get_selection().unwrap();
    assert!(selection.is_range());
    let anchor = selection.point(Which::Anchor).unwrap();
    assert_eq!(anchor.offset, 8);
    let node = anchor.get_node().unwrap();
    assert_eq!(node.get_text_content(), "Hello World");
    assert!(node.get_parent().is_some_and(|p| is_link_node(&p)));
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > preserves selection when cursor is in the second link during merge
#[test]
fn preserves_selection_when_cursor_is_in_the_second_link_during_merge() {
    let editor = build_editor();
    editor
        .update(
            || {
                let link2 = create_link_node("https://lexical.dev/", &LinkAttributes::default());
                let text2 = create_text_node("World");
                link2.append(std::slice::from_ref(&text2))?;
                fill_paragraph(&[plain_link("https://lexical.dev/", "Hello "), link2])?;
                text2.select(Some(2), Some(2)).map(|_| ())
            },
            discrete(),
        )
        .unwrap();
    editor.read(assert_selection_after_merge).unwrap();
}

fn set_second_link_url(editor: &LexicalEditorWithDispose) {
    editor
        .update(
            || {
                let link2 = paragraph().get_children()[1].clone();
                assert!(is_link_node(&link2), "Expected LinkNode");
                link2.set_url("https://lexical.dev/");
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > merges when only the second link is changed to match the first
#[test]
fn merges_when_only_the_second_link_is_changed_to_match_the_first() {
    let editor = build_editor();
    editor
        .update(
            || {
                fill_paragraph(&[
                    plain_link("https://lexical.dev/", "Hello "),
                    plain_link("https://other.dev/", "World"),
                ])
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(paragraph_children_count(&editor), 2);
    set_second_link_url(&editor);
    editor
        .read(|| {
            let children = paragraph().get_children();
            assert_eq!(children.len(), 1);
            assert!(is_link_node(&children[0]));
            assert_eq!(children[0].get_text_content(), "Hello World");
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > preserves selection when cursor is in a link that merges into its left neighbor
#[test]
fn preserves_selection_when_cursor_is_in_a_link_that_merges_into_its_left_neighbor() {
    let editor = build_editor();
    editor
        .update(
            || {
                let link2 = create_link_node("https://other.dev/", &LinkAttributes::default());
                let text2 = create_text_node("World");
                link2.append(std::slice::from_ref(&text2))?;
                fill_paragraph(&[plain_link("https://lexical.dev/", "Hello "), link2])?;
                text2.select(Some(2), Some(2)).map(|_| ())
            },
            discrete(),
        )
        .unwrap();
    set_second_link_url(&editor);
    editor.read(assert_selection_after_merge).unwrap();
}

// PASTE_COMMAND with URLs

const PASTE_URL: &str = "https://lexical.dev/";

fn dispatch_paste(editor: &crate::Editor) {
    let clipboard_data = crate::DataTransfer::new();
    clipboard_data.set_data("text/plain", PASTE_URL);
    clipboard_data.set_data("text/html", PASTE_URL);
    let event = crate::ClipboardEvent::new("paste", Some(Rc::new(clipboard_data)));
    editor
        .dispatch_command(
            &crate::PASTE_COMMAND,
            crate::PasteCommandType::Clipboard(Rc::new(event)),
        )
        .unwrap();
}

fn validate_any() -> AnyLexicalExtensionArgument {
    config_extension(
        &link_extension(),
        [LinkConfigOverrides {
            validate_url: Some(Some(Rc::new(|_: &str| true))),
            ..LinkConfigOverrides::default()
        }],
    )
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > PASTE_COMMAND with URLs > does not convert pasted URL to link inside a code block
#[cfg(feature = "code")]
#[test]
fn does_not_convert_pasted_url_to_link_inside_a_code_block() {
    use crate::code::{code_extension, create_code_highlight_node, create_code_node, is_code_node};
    let editor = build(
        "[root-code]",
        || {
            let code = create_code_node(None, None);
            code.append(&[create_code_highlight_node("const x = 5;", None)])?;
            get_root().append(&[code])
        },
        vec![
            validate_any(),
            rich_text_extension().into(),
            code_extension().into(),
        ],
        vec![],
    );
    let inner = editor.clone();
    editor
        .update(
            move || {
                let code_highlight = get_root().get_first_descendant().unwrap();
                assert!(code_highlight.is_text(), "Expected a TextNode");
                code_highlight.select(Some(0), Some(5))?;
                dispatch_paste(&inner);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| {
            let code = get_root().get_first_child().unwrap();
            assert!(is_code_node(&code), "Expected a CodeNode");
            assert!(!code.get_children().iter().any(is_link_node));
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > PASTE_COMMAND with URLs > wraps selected plain text in a link when a URL is pasted
#[test]
fn wraps_selected_plain_text_in_a_link_when_a_url_is_pasted() {
    let editor = build(
        "[root-paste]",
        paragraph_with("click here"),
        vec![validate_any(), rich_text_extension().into()],
        vec![],
    );
    let inner = editor.clone();
    editor
        .update(
            move || {
                let text = get_root().get_first_descendant().unwrap();
                assert!(text.is_text(), "Expected a TextNode");
                text.select(Some(0), Some(text.get_text_content_size()))?;
                dispatch_paste(&inner);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| {
            let link = paragraph().get_first_child().unwrap();
            assert!(is_link_node(&link), "Expected a LinkNode");
            assert_eq!(link.get_url().as_deref(), Some(PASTE_URL));
            assert_eq!(link.get_text_content(), "click here");
        })
        .unwrap();
}

// TOGGLE_LINK_COMMAND with configured attributes

fn build_configured_editor() -> LexicalEditorWithDispose {
    build(
        "[root-attributes]",
        paragraph_with("Hello"),
        vec![
            config_extension(
                &link_extension(),
                [LinkConfigOverrides {
                    attributes: Some(Some(LinkAttributes {
                        rel: some("noopener"),
                        target: some("_blank"),
                        title: some("Configured title"),
                    })),
                    ..LinkConfigOverrides::default()
                }],
            ),
            rich_text_extension().into(),
        ],
        vec![],
    )
}

/// Selects "Hello", dispatches `payload` and returns the link's
/// `(rel, target, title)`.
fn toggle_configured(
    payload: ToggleLinkPayload,
) -> (Option<String>, Option<String>, Option<String>) {
    let editor = build_configured_editor();
    let inner = editor.clone();
    editor
        .update(
            move || {
                let text_node = get_root().get_last_descendant().unwrap();
                assert!(text_node.is_text(), "Expected a TextNode");
                text_node.select(Some(0), Some(text_node.get_text_content_size()))?;
                inner.dispatch_command(&TOGGLE_LINK_COMMAND, Some(payload))?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| {
            let link = paragraph().get_first_child().unwrap();
            assert!(is_link_node(&link), "Expected a LinkNode");
            (link.get_rel(), link.get_target(), link.get_title())
        })
        .unwrap()
}

fn configured() -> (Option<String>, Option<String>, Option<String>) {
    (
        Some("noopener".to_owned()),
        Some("_blank".to_owned()),
        Some("Configured title".to_owned()),
    )
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > TOGGLE_LINK_COMMAND with configured attributes > applies the configured attributes for a string payload
#[test]
fn applies_the_configured_attributes_for_a_string_payload() {
    assert_eq!(
        toggle_configured(ToggleLinkPayload::Url("https://lexical.dev/".to_owned())),
        configured()
    );
}

// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > TOGGLE_LINK_COMMAND with configured attributes > applies the configured attributes for an object payload
#[test]
fn applies_the_configured_attributes_for_an_object_payload() {
    assert_eq!(
        toggle_configured(ToggleLinkPayload::Attributes {
            url: "https://lexical.dev/".to_owned(),
            attributes: LinkAttributes::default(),
        }),
        configured()
    );
}

/// `{target: null}` is an attribute set to null, which overrides the
/// configured one (`Some(None)`).
// upstream: lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > TOGGLE_LINK_COMMAND with configured attributes > lets the object payload override the configured attributes
#[test]
fn lets_the_object_payload_override_the_configured_attributes() {
    let (rel, target, title) = toggle_configured(ToggleLinkPayload::Attributes {
        url: "https://lexical.dev/".to_owned(),
        attributes: LinkAttributes {
            target: Some(None),
            ..LinkAttributes::default()
        },
    });
    assert_eq!(rel.as_deref(), Some("noopener"));
    assert_eq!(target, None);
    assert_eq!(title.as_deref(), Some("Configured title"));
}
