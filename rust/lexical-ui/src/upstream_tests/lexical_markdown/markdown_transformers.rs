//! Ports of `packages/lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts`.

#![cfg(feature = "history")]

use super::support::{Using, build};
use crate::link::{LinkAttributes, create_link_node, is_link_node};
use crate::markdown::{register_markdown_shortcuts, transformers};
use crate::upstream_tests::support::update;
use crate::{
    Editor, LexicalExtension, NodeRef, UNDO_COMMAND, UpdateOptions, create_paragraph_node,
    create_text_node, define_extension, get_root, get_selection,
};
use std::rc::Rc;

const FILE: &str = "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts";

/// `buildEditorFromExtensions([MarkdownShortcutTestExtension])`: history,
/// link, rich text, list and code, with the shortcuts registered.
fn shortcut_editor() -> Using {
    build(vec![
        define_extension(LexicalExtension {
            dependencies: vec![
                crate::history::history_extension().into(),
                crate::link::link_extension().into(),
                crate::rich_text::rich_text_extension().into(),
                crate::list::list_extension().into(),
                crate::code::code_extension().into(),
            ],
            register: Some(Rc::new(|editor: &Editor, _, _| {
                register_markdown_shortcuts(editor, &transformers())
            })),
            ..LexicalExtension::<(), (), ()>::new("MarkdownShortcutTest", ())
        })
        .into(),
    ])
}

/// `typeMarkdown(editor, text)`.
fn type_markdown(editor: &Editor, text: &str) {
    editor
        .update(
            || {
                let selection = get_selection();
                let collapsed = selection
                    .as_ref()
                    .and_then(|s| s.range())
                    .is_some_and(|r| r.is_collapsed());
                if !collapsed {
                    get_root().select_end()?;
                }
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    for ch in text.chars() {
        update(editor, move || {
            if let Some(selection) = get_selection() {
                selection.insert_text(&ch.to_string())?;
            }
            Ok(())
        });
    }
    // Markdown shortcuts issues a cascading update that is not discrete, so
    // force sync reconciliation with a read.
    editor.read(|| {}).unwrap();
}

/// The root's first child, which must be a paragraph.
fn first_paragraph() -> NodeRef {
    let paragraph = get_root().get_first_child_or_throw().unwrap();
    assert_eq!(
        paragraph.get_type(),
        "paragraph",
        "Root child must be a paragraph"
    );
    paragraph
}

fn texts(nodes: &[NodeRef]) -> Vec<String> {
    nodes.iter().map(NodeRef::get_text_content).collect()
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > text before a markdown link is preserved
#[test]
fn text_before_a_markdown_link_is_preserved() {
    let editor = shortcut_editor();
    type_markdown(&editor, "Start [test](url)");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert_eq!(texts(&children), ["Start ", "test"]);
            assert!(
                is_link_node(&children[1]),
                "Second child must be a LinkNode"
            );
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > formatted text before a markdown link is preserved
#[test]
fn formatted_text_before_a_markdown_link_is_preserved() {
    let editor = shortcut_editor();
    type_markdown(&editor, "**Bold** [Link](url)");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert_eq!(texts(&children), ["Bold", " ", "Link"]);
            let link_node = &children[2];
            assert!(is_link_node(link_node), "Third child must be a LinkNode");
            assert_eq!(link_node.get_text_content(), "Link");
            assert_eq!(link_node.get_url().as_deref(), Some("url"));
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > LINK is not too greedy if there is a preceding match that was not processed
#[test]
fn link_is_not_too_greedy_if_there_is_a_preceding_match_that_was_not_processed() {
    // https://github.com/meta/lexical/issues/8129
    let editor = shortcut_editor();
    update(&editor, || {
        get_root()
            .select_end()?
            .insert_raw_text("[a](https://a.example.com) [b](https://b.example.com")
    });
    type_markdown(&editor, ")");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert_eq!(texts(&children), ["[a](https://a.example.com) ", "b"]);
            let link_node = &children[1];
            assert!(is_link_node(link_node), "Second child must be a LinkNode");
            assert_eq!(link_node.get_text_content(), "b");
            assert_eq!(
                link_node.get_url().as_deref(),
                Some("https://b.example.com")
            );
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > a destination between angle brackets keeps the whitespace in the URL
#[test]
fn a_destination_between_angle_brackets_keeps_the_whitespace_in_the_url() {
    let editor = shortcut_editor();
    type_markdown(&editor, "[test](<https://example.com/a b>)");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert_eq!(children.len(), 1);
            let link_node = &children[0];
            assert!(is_link_node(link_node), "Expected a link");
            assert_eq!(link_node.get_text_content(), "test");
            assert_eq!(
                link_node.get_url().as_deref(),
                Some("https://example.com/a b")
            );
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > a title ending in an escaped backslash closes at the next quote
#[test]
fn a_title_ending_in_an_escaped_backslash_closes_at_the_next_quote() {
    let editor = shortcut_editor();
    type_markdown(&editor, "[test](url \"a\\\\\")");
    editor
        .read(|| {
            let link_node = get_root().get_all_text_nodes()[0].get_parent().unwrap();
            assert!(is_link_node(&link_node), "Expected a link");
            assert_eq!(
                (link_node.get_url(), link_node.get_title()),
                (Some("url".to_owned()), Some("a\\".to_owned()))
            );
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > a title whose closing quote is escaped makes no link
#[test]
fn a_title_whose_closing_quote_is_escaped_makes_no_link() {
    let editor = shortcut_editor();
    type_markdown(&editor, "[test](url \"a\\\")");
    editor
        .read(|| {
            assert_eq!(get_root().get_text_content(), "[test](url \"a\\\")");
            assert!(
                !get_root()
                    .get_all_text_nodes()
                    .iter()
                    .any(|node| node.get_parent().is_some_and(|p| is_link_node(&p)))
            );
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > markdown link should not be created inside another link.
#[test]
fn markdown_link_should_not_be_created_inside_another_link() {
    let editor = shortcut_editor();
    update(&editor, || {
        let link = create_link_node("link", &LinkAttributes::default());
        link.append(&[create_text_node("hello")])?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[link])?;
        get_root().select_end()?.insert_nodes(vec![paragraph])
    });
    // moves the cursor inside the link text: hell‸o
    update(&editor, || {
        get_root().get_all_text_nodes()[0].select(Some(4), Some(4))?;
        Ok(())
    });
    type_markdown(&editor, "[world](www)");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert_eq!(children.len(), 1);
            let link_node = &children[0];
            assert!(is_link_node(link_node), "Expected a link");
            assert_eq!(link_node.get_text_content(), "hell[world](www)o");
            assert_eq!(link_node.get_url().as_deref(), Some("link"));
        })
        .unwrap();
}

/// A paragraph holding one text node, `text`, formatted `format` and not
/// `not`.
fn assert_single_code_span(typed: &str, text: &str, format: &str, not: &str) {
    let editor = shortcut_editor();
    type_markdown(&editor, typed);
    let (text, format, not) = (text.to_owned(), format.to_owned(), not.to_owned());
    editor
        .read(move || {
            let children = first_paragraph().get_children();
            assert_eq!(children.len(), 1);
            let text_node = &children[0];
            assert!(text_node.is_text(), "Child must be a TextNode");
            assert_eq!(text_node.get_text_content(), text);
            assert!(text_node.has_format(&format));
            assert!(!text_node.has_format(&not));
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > __bold__ inside backticks is not formatted as bold
#[test]
fn underscore_bold_inside_backticks_is_not_formatted_as_bold() {
    assert_single_code_span("`__bold__`", "__bold__", "code", "bold");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > **bold** inside backticks is not formatted as bold
#[test]
fn star_bold_inside_backticks_is_not_formatted_as_bold() {
    assert_single_code_span("`**bold**`", "**bold**", "code", "bold");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > *italic* inside backticks is not formatted as italic
#[test]
fn italic_inside_backticks_is_not_formatted_as_italic() {
    assert_single_code_span("`*italic*`", "*italic*", "code", "italic");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > __bold__ without backticks still formats as bold
#[test]
fn underscore_bold_without_backticks_still_formats_as_bold() {
    let editor = shortcut_editor();
    type_markdown(&editor, "__bold__");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert_eq!(children.len(), 1);
            let text_node = &children[0];
            assert!(text_node.is_text(), "Child must be a TextNode");
            assert_eq!(text_node.get_text_content(), "bold");
            assert!(text_node.has_format("bold"));
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > __bold__ after a completed code span still formats as bold
#[test]
fn underscore_bold_after_a_completed_code_span_still_formats_as_bold() {
    let editor = shortcut_editor();
    type_markdown(&editor, "`code` __bold__");
    editor
        .read(|| {
            let bold_node = first_paragraph()
                .get_children()
                .into_iter()
                .filter(NodeRef::is_text)
                .find(|node| node.has_format("bold"))
                .expect("a bold text node");
            assert_eq!(bold_node.get_text_content(), "bold");
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::UNDERSCORE_AFTER_UNICODE_PUNCTUATION > typing "See –_here_" formats the word between the underscores
// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::UNDERSCORE_AFTER_UNICODE_PUNCTUATION > typing "See “_here_" formats the word between the underscores
// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::UNDERSCORE_AFTER_UNICODE_PUNCTUATION > typing "See 😀_here_" formats the word between the underscores
#[test]
fn typing_formats_the_word_between_the_underscores() {
    const EN_DASH: &str = "\u{2013}";
    const LEFT_DOUBLE_QUOTE: &str = "\u{201C}";
    let cases = [
        (format!("See {EN_DASH}_here_"), format!("See {EN_DASH}")),
        (
            format!("See {LEFT_DOUBLE_QUOTE}_here_"),
            format!("See {LEFT_DOUBLE_QUOTE}"),
        ),
        // An emoji (So) outside the Basic Multilingual Plane, read whole.
        ("See \u{1F600}_here_".to_owned(), "See \u{1F600}".to_owned()),
    ];
    let cases: super::support::Cases = cases
        .into_iter()
        .map(|(typed, before)| {
            let name = format!(
                "UNDERSCORE_AFTER_UNICODE_PUNCTUATION > typing {} formats the word between the underscores",
                super::support::j(&typed)
            );
            let case: Box<dyn FnOnce()> = Box::new(move || {
                let editor = shortcut_editor();
                type_markdown(&editor, &typed);
                editor
                    .read(move || {
                        let text_nodes: Vec<NodeRef> = first_paragraph()
                            .get_children()
                            .into_iter()
                            .filter(NodeRef::is_text)
                            .collect();
                        assert_eq!(texts(&text_nodes), [before.as_str(), "here"]);
                        assert_eq!(
                            text_nodes
                                .iter()
                                .map(|n| n.has_format("italic"))
                                .collect::<Vec<_>>(),
                            [false, true]
                        );
                    })
                    .unwrap();
            });
            (name, case)
        })
        .collect();
    super::support::run_cases(
        include_str!("markdown_transformers.rs"),
        FILE,
        "typing_formats_the_word_between_the_underscores",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::UNDERSCORE_AFTER_UNICODE_PUNCTUATION > typing an underscore format right after a letter still does not format it
#[test]
fn typing_an_underscore_format_right_after_a_letter_still_does_not_format_it() {
    let editor = shortcut_editor();
    type_markdown(&editor, "See \u{00E9}_here_");
    editor
        .read(|| {
            let text_nodes: Vec<NodeRef> = first_paragraph()
                .get_children()
                .into_iter()
                .filter(NodeRef::is_text)
                .collect();
            assert_eq!(texts(&text_nodes), ["See \u{00E9}_here_"]);
            assert!(!text_nodes[0].has_format("italic"));
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::WRAPPING_PRESERVES_FORMAT > **...** around already-bold text preserves bold
#[test]
fn wrapping_around_already_bold_text_preserves_bold() {
    // https://github.com/meta/lexical/issues/8727
    let editor = shortcut_editor();
    update(&editor, || {
        let text_node = create_text_node("**bold*");
        text_node.toggle_format("bold");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text_node))?;
        get_root().select_end()?.insert_nodes(vec![paragraph])?;
        text_node
            .select_end()?
            .set_format(text_node.text_data().get_format());
        Ok(())
    });
    type_markdown(&editor, "*");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert_eq!(children.len(), 1);
            let text_node = &children[0];
            assert!(text_node.is_text(), "Child must be a TextNode");
            assert_eq!(text_node.get_text_content(), "bold");
            assert!(text_node.has_format("bold"));
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::WRAPPING_PRESERVES_FORMAT > **...** around mixed-format text formats every wrapped node bold
#[test]
fn wrapping_around_mixed_format_text_formats_every_wrapped_node_bold() {
    // https://github.com/meta/lexical/issues/8727
    let editor = shortcut_editor();
    update(&editor, || {
        let plain_node = create_text_node("**foo");
        let bold_node = create_text_node("bar*");
        bold_node.toggle_format("bold");
        let paragraph = create_paragraph_node();
        paragraph.append(&[plain_node])?;
        paragraph.append(std::slice::from_ref(&bold_node))?;
        get_root().select_end()?.insert_nodes(vec![paragraph])?;
        bold_node
            .select_end()?
            .set_format(bold_node.text_data().get_format());
        Ok(())
    });
    type_markdown(&editor, "*");
    editor
        .read(|| {
            let paragraph = first_paragraph();
            assert_eq!(paragraph.get_text_content(), "foobar");
            assert!(
                paragraph
                    .get_children()
                    .iter()
                    .filter(|n| n.is_text())
                    .all(|node| node.has_format("bold"))
            );
        })
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::HISTORY > undo after markdown format transform preserves typed markdown text
#[test]
fn undo_after_markdown_format_transform_preserves_typed_markdown_text() {
    let editor = shortcut_editor();
    type_markdown(&editor, "lorem *ipsum*");
    let e = editor.0.editor().clone();
    update(&editor, move || {
        e.dispatch_command(&UNDO_COMMAND, ())?;
        Ok(())
    });
    editor
        .read(|| {
            assert_eq!(first_paragraph().get_text_content(), "lorem *ipsum*");
        })
        .unwrap();
}
