//! Ports of `packages/lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts`.

#![cfg(feature = "hashtag")]

use super::support::{build, markdown_test_extension};
use crate::markdown::{register_markdown_shortcuts, transformers};
use crate::rich_text::{HeadingTagType, is_heading_node};
use crate::upstream_tests::support::update;
use crate::{
    Editor, LexicalExtension, create_paragraph_node, create_text_node, define_extension, get_root,
    get_selection,
};
use std::rc::Rc;

const FILE: &str = "lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts";

/// Types each character in its own discrete update, firing the shortcut
/// listener.
fn type_text(editor: &Editor, text: &str) {
    for character in text.chars() {
        update(editor, move || {
            if let Some(selection) = get_selection().filter(|s| s.is_range()) {
                selection.insert_text(&character.to_string())?;
            }
            Ok(())
        });
    }
}

// upstream: lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the "# " shortcut typed before existing text
// upstream: lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the "## " shortcut typed before existing text
// upstream: lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the "### " shortcut typed before existing text
// upstream: lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the "#### " shortcut typed before existing text
// upstream: lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the "##### " shortcut typed before existing text
// upstream: lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the "###### " shortcut typed before existing text
#[test]
fn transforms_with_the_shortcut_typed_before_existing_text() {
    let cases = [
        ("# ", HeadingTagType::H1),
        ("## ", HeadingTagType::H2),
        ("### ", HeadingTagType::H3),
        ("#### ", HeadingTagType::H4),
        ("##### ", HeadingTagType::H5),
        ("###### ", HeadingTagType::H6),
    ];
    let cases: super::support::Cases = cases
        .into_iter()
        .map(|(shortcut, tag)| {
            let name = format!(
                "Issue #5366: hashtags block the heading shortcut > transforms with the \"{shortcut}\" shortcut typed before existing text"
            );
            let case: Box<dyn FnOnce()> = Box::new(move || {
                // HashtagExtension is a dependency so that its transform is
                // registered ahead of the shortcut listener.
                let editor = build(vec![
                    define_extension(LexicalExtension {
                        dependencies: vec![
                            markdown_test_extension(),
                            crate::hashtag::hashtag_extension().into(),
                        ],
                        register: Some(Rc::new(|editor: &Editor, _, _| {
                            register_markdown_shortcuts(editor, &transformers())
                        })),
                        ..LexicalExtension::<(), (), ()>::new("Issue5366Test", ())
                    })
                    .into(),
                ]);
                update(&editor, || {
                    let root = get_root();
                    root.clear()?;
                    let paragraph = create_paragraph_node();
                    let text = create_text_node("Welcome to the playground");
                    paragraph.append(std::slice::from_ref(&text))?;
                    root.append(&[paragraph])?;
                    text.select(Some(0), Some(0))?;
                    Ok(())
                });
                type_text(&editor, shortcut);
                editor
                    .read(|| {
                        let heading = get_root().get_first_child().expect("a first child");
                        assert!(is_heading_node(&heading));
                        assert_eq!(heading.get_heading_tag(), Some(tag));
                        assert_eq!(heading.get_text_content(), "Welcome to the playground");
                    })
                    .unwrap();
            });
            (name, case)
        })
        .collect();
    super::support::run_cases(
        include_str!("issue5366_repro.rs"),
        FILE,
        "transforms_with_the_shortcut_typed_before_existing_text",
        cases,
    );
}
