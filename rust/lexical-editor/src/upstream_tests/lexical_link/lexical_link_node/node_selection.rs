//! `$toggleLink with a NodeSelection`.

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{LinkAttributes, is_link_node, link_extension, toggle_link};
use crate::rich_text::rich_text_extension;
use crate::{
    InitialEditorStateType, LexicalError, LexicalExtension, NodeRef, UpdateOptions,
    create_node_selection, create_paragraph_node, create_text_node, define_extension, get_root,
    set_selection,
};
use std::rc::Rc;

fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let root = get_root();
            root.clear()?;
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("hello")])?;
            root.append(&[paragraph])
        }))),
        dependencies: vec![link_extension().into(), rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[root-node-selection]", ())
    })
    .into()])
    .unwrap()
}

/// `$selectTheTextNode()`, then `$toggleLink(url, attributes)`.
fn toggle_on_node_selection(
    editor: &LexicalEditorWithDispose,
    url: &'static str,
    attributes: LinkAttributes,
) {
    editor
        .update(
            move || -> Result<(), LexicalError> {
                let text_node = get_root().get_last_descendant().unwrap();
                assert!(text_node.is_text(), "Expected a TextNode");
                let selection = create_node_selection();
                selection.add(text_node.get_key());
                set_selection(Some(selection));
                toggle_link(Some(url), &attributes)
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

/// `$getLink()`.
fn get_link() -> NodeRef {
    let paragraph = get_root().get_first_child().unwrap();
    assert_eq!(paragraph.get_type(), "paragraph");
    let link = paragraph.get_first_child().unwrap();
    assert!(is_link_node(&link), "Expected a LinkNode");
    link
}

fn title(t: &str) -> LinkAttributes {
    LinkAttributes {
        title: Some(Some(t.to_owned())),
        ..LinkAttributes::default()
    }
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink with a NodeSelection > applies the title when creating a link
#[test]
fn applies_the_title_when_creating_a_link() {
    let editor = build_editor();
    toggle_on_node_selection(
        &editor,
        "https://lexical.dev/",
        LinkAttributes {
            rel: Some(Some("noopener".to_owned())),
            target: Some(Some("_blank".to_owned())),
            title: Some(Some("Lexical".to_owned())),
        },
    );
    editor
        .read(|| {
            let link = get_link();
            assert_eq!(link.get_url().as_deref(), Some("https://lexical.dev/"));
            assert_eq!(link.get_rel().as_deref(), Some("noopener"));
            assert_eq!(link.get_target().as_deref(), Some("_blank"));
            assert_eq!(link.get_title().as_deref(), Some("Lexical"));
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink with a NodeSelection > applies the title when updating an existing link
#[test]
fn applies_the_title_when_updating_an_existing_link() {
    let editor = build_editor();
    toggle_on_node_selection(&editor, "https://lexical.dev/", title("First"));
    toggle_on_node_selection(&editor, "https://lexical.dev/docs", title("Second"));
    editor
        .read(|| {
            let link = get_link();
            assert_eq!(link.get_url().as_deref(), Some("https://lexical.dev/docs"));
            assert_eq!(link.get_title().as_deref(), Some("Second"));
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink with a NodeSelection > leaves an existing title alone when none is supplied
#[test]
fn leaves_an_existing_title_alone_when_none_is_supplied() {
    let editor = build_editor();
    toggle_on_node_selection(&editor, "https://lexical.dev/", title("Kept"));
    toggle_on_node_selection(
        &editor,
        "https://lexical.dev/docs",
        LinkAttributes::default(),
    );
    editor
        .read(|| {
            let link = get_link();
            assert_eq!(link.get_url().as_deref(), Some("https://lexical.dev/docs"));
            assert_eq!(link.get_title().as_deref(), Some("Kept"));
        })
        .unwrap();
}
