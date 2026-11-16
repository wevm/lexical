//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts`.
//!
//! Where upstream reads the mounted editor's `outerHTML`, the committed
//! document is rendered as the reconciler does ([`rendered_html`]). JS's
//! `insertNewAfter()` and `collapseAtStart()` without a selection take an
//! empty node selection here (it has no anchor, as `undefined` has none).

#![cfg(all(feature = "html", feature = "list", feature = "link"))]

use crate::link::{LinkAttributes, create_link_node};
use crate::list::{
    ListType, create_list_item_node, create_list_node, is_list_item_node, is_list_node,
};
use crate::nodes::line_break::create_line_break_node;
use crate::rich_text::{
    HeadingTagType, create_heading_node, create_quote_node, is_heading_node, is_quote_node,
};
use crate::upstream_tests::lexical_selection::support::rendered_html;
use crate::upstream_tests::support::{
    create_test_decorator_node, create_test_editor, create_test_element_node, mount_in_jsdom,
    panics, update, update_with,
};
use crate::{
    CreateEditorArgs, Editor, EditorConfig, NodeRef, PointType, SelectionRef, Which,
    create_node_selection, create_paragraph_node, create_range_selection, create_text_node,
    get_root, get_selection, set_selection,
};

use HeadingTagType::{H1, H2};

const ROOT_OPEN: &str = "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\">";

/// `initializeUnitTest`'s editor, mounted in jsdom.
fn test_editor() -> Editor {
    crate::reset_random_key();
    let editor = create_test_editor(CreateEditorArgs::default());
    mount_in_jsdom(&editor);
    editor
}

/// `testEnv.outerHTML`.
fn outer_html(editor: &Editor) -> String {
    format!("{ROOT_OPEN}{}</div>", rendered_html(editor))
}

/// `editorConfig`.
fn editor_config(theme: &str) -> EditorConfig {
    EditorConfig {
        theme: crate::json::parse(theme)
            .unwrap()
            .as_object()
            .unwrap()
            .clone(),
    }
}

const THEME: &str = r#"{"heading":{"h1":"my-h1-class","h2":"my-h2-class","h3":"my-h3-class","h4":"my-h4-class","h5":"my-h5-class","h6":"my-h6-class"}}"#;

fn create_dom(node: &NodeRef, config: &EditorConfig, editor: &Editor) -> crate::dom::DomNode {
    node.get_latest().create_dom(node, config, editor).unwrap()
}

fn range() -> SelectionRef {
    get_selection()
        .filter(SelectionRef::is_range)
        .expect("a RangeSelection")
}

fn heading_of(tag: HeadingTagType, children: &[NodeRef]) -> NodeRef {
    let heading = create_heading_node(tag);
    heading.append(children).unwrap();
    heading
}

fn paragraph_of(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

fn set_root(children: &[NodeRef]) {
    let root = get_root();
    root.clear().unwrap();
    root.append(children).unwrap();
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.constructor
#[test]
fn heading_node_constructor() {
    let editor = test_editor();
    update(&editor, || {
        let heading_node = create_heading_node(H1);
        assert_eq!(heading_node.get_type(), "heading");
        assert_eq!(heading_node.get_heading_tag(), Some(H1));
        assert_eq!(heading_node.get_text_content(), "");
        Ok(())
    });
    assert!(panics(|| create_heading_node(H1)));
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.createDOM()
#[test]
fn heading_node_create_dom() {
    let editor = test_editor();
    let ed = editor.clone();
    update(&editor, move || {
        let heading_node = create_heading_node(H1);
        assert_eq!(
            create_dom(&heading_node, &editor_config(THEME), &ed).outer_html(),
            "<h1 class=\"my-h1-class\"></h1>"
        );
        assert_eq!(
            create_dom(&heading_node, &editor_config(r#"{"heading":{}}"#), &ed).outer_html(),
            "<h1></h1>"
        );
        assert_eq!(
            create_dom(&heading_node, &editor_config("{}"), &ed).outer_html(),
            "<h1></h1>"
        );
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.updateDOM()
#[test]
fn heading_node_update_dom() {
    let editor = test_editor();
    let ed = editor.clone();
    update(&editor, move || {
        let heading_node = create_heading_node(H1);
        let dom_element = create_dom(&heading_node, &editor_config(THEME), &ed);
        assert_eq!(dom_element.outer_html(), "<h1 class=\"my-h1-class\"></h1>");
        let new_heading_node = create_heading_node(H1);
        let result = new_heading_node
            .get_latest()
            .update_dom(heading_node.get_latest().as_ref());
        assert!(!result);
        assert_eq!(dom_element.outer_html(), "<h1 class=\"my-h1-class\"></h1>");
        // A new tag returns true and leaves the DOM to createDOM.
        let new_tag = create_heading_node(H2);
        assert!(
            new_tag
                .get_latest()
                .update_dom(heading_node.get_latest().as_ref())
        );
        assert_eq!(dom_element.outer_html(), "<h1 class=\"my-h1-class\"></h1>");
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.insertNewAfter() empty
#[test]
fn heading_node_insert_new_after_empty() {
    let editor = test_editor();
    let heading = update_with(&editor, || {
        let heading_node = create_heading_node(H1);
        get_root().append(std::slice::from_ref(&heading_node))?;
        Ok(heading_node)
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h1 dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></h1></div>"
        )
    );
    update(&editor, move || {
        let selection = get_selection().unwrap_or_else(create_node_selection);
        let result = heading.insert_new_after(&selection, true)?.unwrap();
        assert_eq!(result.get_type(), "paragraph");
        assert_eq!(
            result.element_data().get_direction(),
            heading.element_data().get_direction()
        );
        Ok(())
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h1 dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></h1><p dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p></div>"
        )
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.insertNewAfter() middle
#[test]
fn heading_node_insert_new_after_middle() {
    let editor = test_editor();
    let heading = update_with(&editor, || {
        let text = create_text_node("hello world");
        let heading_node = heading_of(H1, std::slice::from_ref(&text));
        get_root().append(std::slice::from_ref(&heading_node))?;
        text.select(Some(5), Some(5))?;
        Ok(heading_node)
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h1 dir=\"auto\"><span data-lexical-text=\"true\">hello world</span></h1></div>"
        )
    );
    update(&editor, move || {
        let result = heading.insert_new_after(&range(), true)?.unwrap();
        assert!(is_heading_node(&result));
        assert_eq!(
            result.element_data().get_direction(),
            heading.element_data().get_direction()
        );
        Ok(())
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h1 dir=\"auto\"><span data-lexical-text=\"true\">hello world</span></h1><h1 dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></h1></div>"
        )
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.insertNewAfter() middle keeps format and style
#[test]
fn heading_node_insert_new_after_middle_keeps_format_and_style() {
    let editor = test_editor();
    let heading = update_with(&editor, || {
        let text = create_text_node("hello world");
        let heading_node = create_heading_node(H1);
        heading_node.set_element_format("center");
        heading_node.set_element_style("color: red;");
        heading_node.append(std::slice::from_ref(&text))?;
        get_root().append(std::slice::from_ref(&heading_node))?;
        text.select(Some(5), Some(5))?;
        Ok(heading_node)
    });
    update(&editor, move || {
        let result = heading.insert_new_after(&range(), true)?.unwrap();
        assert!(is_heading_node(&result));
        assert_eq!(result.element_data().get_format_type(), "center");
        assert_eq!(result.element_data().get_style(), "color: red;");
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.insertNewAfter() end
#[test]
fn heading_node_insert_new_after_end() {
    let editor = test_editor();
    let heading = update_with(&editor, || {
        let text1 = create_text_node("hello");
        let text2 = create_text_node(" world");
        text2.set_format_type("bold");
        let heading_node = heading_of(H1, &[text1, text2.clone()]);
        get_root().append(std::slice::from_ref(&heading_node))?;
        text2.select_end()?;
        Ok(heading_node)
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h1 dir=\"auto\"><span data-lexical-text=\"true\">hello</span><strong data-lexical-text=\"true\"> world</strong></h1></div>"
        )
    );
    update(&editor, move || {
        let result = heading.insert_new_after(&range(), true)?.unwrap();
        assert_eq!(result.get_type(), "paragraph");
        assert_eq!(
            result.element_data().get_direction(),
            heading.element_data().get_direction()
        );
        Ok(())
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h1 dir=\"auto\"><span data-lexical-text=\"true\">hello</span><strong data-lexical-text=\"true\"> world</strong></h1><p dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p></div>"
        )
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.setTag()
#[test]
fn heading_node_set_tag() {
    let editor = test_editor();
    update(&editor, || {
        get_root().append(&[create_heading_node(H1)])?;
        Ok(())
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h1 dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></h1></div>"
        )
    );
    update(&editor, || {
        let heading = get_root().get_first_child().unwrap();
        assert!(is_heading_node(&heading));
        assert_eq!(heading.get_heading_tag(), Some(H1));
        heading.set_heading_tag(H2);
        assert_eq!(heading.get_heading_tag(), Some(H2));
        Ok(())
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h2 dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></h2></div>"
        )
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > $createHeadingNode()
#[test]
fn create_heading_node_test() {
    let editor = test_editor();
    update(&editor, || {
        let heading_node = create_heading_node(H1);
        let created_heading_node = create_heading_node(H1);
        assert_eq!(heading_node.get_type(), created_heading_node.get_type());
        assert_eq!(heading_node.get_parent(), created_heading_node.get_parent());
        assert_ne!(heading_node.get_key(), created_heading_node.get_key());
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > $isHeadingNode()
#[test]
fn is_heading_node_test() {
    let editor = test_editor();
    update(&editor, || {
        assert!(is_heading_node(&create_heading_node(H1)));
        Ok(())
    });
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > creates a h2 with text and can insert a new paragraph after
#[test]
fn creates_a_h2_with_text_and_can_insert_a_new_paragraph_after() {
    let editor = test_editor();
    let heading = update_with(&editor, || {
        let heading_node = create_heading_node(H2);
        get_root().append(std::slice::from_ref(&heading_node))?;
        heading_node.append(&[create_text_node("hello world")])?;
        Ok(heading_node)
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h2 dir=\"auto\"><span data-lexical-text=\"true\">hello world</span></h2></div>"
        )
    );
    update(&editor, move || {
        let result = heading
            .insert_new_after(&create_node_selection(), true)?
            .unwrap();
        assert_eq!(result.get_type(), "paragraph");
        assert_eq!(
            result.element_data().get_direction(),
            heading.element_data().get_direction()
        );
        Ok(())
    });
    assert_eq!(
        outer_html(&editor),
        format!(
            "{ROOT_OPEN}<h2 dir=\"auto\"><span data-lexical-text=\"true\">hello world</span></h2><p dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p></div>"
        )
    );
}

/// One case of `Backspace at start of heading (#4359)`'s `test.for`.
fn backspace_at_start(build: fn() -> NodeRef, check: fn()) {
    let editor = test_editor();
    update(&editor, move || {
        build().select(Some(0), Some(0))?.delete_character(true)
    });
    editor.read(check).unwrap();
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > preserves heading when previous block i…
#[test]
fn backspace_preserves_heading_when_previous_block_is_an_empty_paragraph() {
    backspace_at_start(
        || {
            let text = create_text_node("Heading");
            set_root(&[
                create_paragraph_node(),
                heading_of(H2, std::slice::from_ref(&text)),
            ]);
            text
        },
        || {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].get_heading_tag(), Some(H2));
            assert_eq!(children[0].get_text_content(), "Heading");
        },
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > preserves quote when previous block is …
#[test]
fn backspace_preserves_quote_when_previous_block_is_an_empty_paragraph() {
    backspace_at_start(
        || {
            let text = create_text_node("Quote");
            let quote = create_quote_node(false);
            quote.append(std::slice::from_ref(&text)).unwrap();
            set_root(&[create_paragraph_node(), quote]);
            text
        },
        || {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert!(is_quote_node(&children[0]));
            assert_eq!(children[0].get_text_content(), "Quote");
        },
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > removes only the empty paragraph betwee…
#[test]
fn backspace_removes_only_the_empty_paragraph_between_two_non_empty_siblings() {
    backspace_at_start(
        || {
            let text = create_text_node("Heading");
            set_root(&[
                paragraph_of(&[create_text_node("Before")]),
                create_paragraph_node(),
                heading_of(H1, std::slice::from_ref(&text)),
                paragraph_of(&[create_text_node("After")]),
            ]);
            text
        },
        || {
            let children = get_root().get_children();
            assert_eq!(children.len(), 3);
            assert_eq!(children[0].get_text_content(), "Before");
            assert_eq!(children[1].get_heading_tag(), Some(H1));
            assert_eq!(children[1].get_text_content(), "Heading");
            assert_eq!(children[2].get_text_content(), "After");
        },
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > legacy merge still applies when previou…
#[test]
fn backspace_legacy_merge_still_applies_when_previous_block_is_non_empty() {
    backspace_at_start(
        || {
            let text = create_text_node("Heading");
            set_root(&[
                paragraph_of(&[create_text_node("Before")]),
                heading_of(H2, std::slice::from_ref(&text)),
            ]);
            text
        },
        || {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert!(!is_heading_node(&children[0]));
            assert_eq!(children[0].get_text_content(), "BeforeHeading");
        },
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > list with an empty item: cross-block me…
#[test]
fn backspace_list_with_an_empty_item_pulls_the_heading_text_into_the_item() {
    backspace_at_start(
        || {
            let list = create_list_node(ListType::Bullet, 1.0);
            list.append(&[create_list_item_node(None)]).unwrap();
            let text = create_text_node("Heading");
            set_root(&[list, heading_of(H1, std::slice::from_ref(&text))]);
            text
        },
        || {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert!(is_list_node(&children[0]));
            let item = children[0].get_first_child().unwrap();
            assert!(is_list_item_node(&item));
            assert_eq!(item.get_text_content(), "Heading");
        },
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > HeadingNode.collapseAtStart preserves a non-empty heading
#[test]
fn heading_node_collapse_at_start_preserves_a_non_empty_heading() {
    let editor = test_editor();
    let original_key = update_with(&editor, || {
        let heading = heading_of(H1, &[create_text_node("Heading")]);
        set_root(std::slice::from_ref(&heading));
        let selection = heading
            .get_first_child()
            .unwrap()
            .select(Some(0), Some(0))?;
        heading.collapse_at_start(&selection)?;
        Ok(heading.get_key().to_owned())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].get_heading_tag(), Some(H1));
            assert_eq!(children[0].get_text_content(), "Heading");
            assert_eq!(children[0].get_key(), original_key);
        })
        .unwrap();
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > preserves heading wrapped inside another ElementNode
#[test]
fn preserves_heading_wrapped_inside_another_element_node() {
    let editor = test_editor();
    let original_key = update_with(&editor, || {
        let wrapper = create_test_element_node();
        let heading = heading_of(H1, &[create_text_node("Heading")]);
        wrapper.append(std::slice::from_ref(&heading))?;
        set_root(&[wrapper]);
        heading.collapse_at_start(&create_node_selection())?;
        Ok(heading.get_key().to_owned())
    });
    editor
        .read(|| {
            let wrap = get_root().get_first_child().unwrap();
            assert!(wrap.is_element());
            assert_eq!(wrap.get_type(), "test_block");
            let inner = wrap.get_first_child().unwrap();
            assert_eq!(inner.get_key(), original_key);
            assert_eq!(inner.get_heading_tag(), Some(H1));
            assert_eq!(inner.get_text_content(), "Heading");
        })
        .unwrap();
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > preserves heading when followed by a non-paragraph sibling
#[test]
fn preserves_heading_when_followed_by_a_non_paragraph_sibling() {
    let editor = test_editor();
    let original_key = update_with(&editor, || {
        let heading = heading_of(H1, &[create_text_node("Heading")]);
        let item = create_list_item_node(None);
        item.append(&[create_text_node("Item")])?;
        let list = create_list_node(ListType::Bullet, 1.0);
        list.append(&[item])?;
        set_root(&[heading.clone(), list]);
        heading.collapse_at_start(&create_node_selection())?;
        Ok(heading.get_key().to_owned())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert!(is_heading_node(&children[0]));
            assert_eq!(children[0].get_key(), original_key);
            assert_eq!(children[0].get_text_content(), "Heading");
            assert_eq!(children[1].get_type(), "list");
        })
        .unwrap();
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > HeadingNode.collapseAtStart converts an empty heading to a paragraph
#[test]
fn heading_node_collapse_at_start_converts_an_empty_heading_to_a_paragraph() {
    let editor = test_editor();
    update(&editor, || {
        let heading = create_heading_node(H2);
        set_root(std::slice::from_ref(&heading));
        let selection = heading.select(Some(0), Some(0))?;
        heading.collapse_at_start(&selection)?;
        Ok(())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert!(!is_heading_node(&children[0]));
            assert_eq!(children[0].get_type(), "paragraph");
        })
        .unwrap();
}

/// `$replaceRootWithHeading(...children)`.
fn replace_root_with_heading(children: &[NodeRef]) -> NodeRef {
    let heading = heading_of(H1, children);
    set_root(std::slice::from_ref(&heading));
    heading
}

/// `$describeBlocks()`.
fn describe_blocks() -> Vec<Vec<String>> {
    get_root()
        .get_children()
        .iter()
        .map(|block| {
            assert!(block.is_element());
            let head = match block.get_heading_tag() {
                Some(tag) => tag.as_str().to_owned(),
                None => block.get_type().to_owned(),
            };
            std::iter::once(head)
                .chain(block.get_children().iter().map(|child| {
                    if child.is_text() {
                        child.get_text_content()
                    } else {
                        child.get_type().to_owned()
                    }
                }))
                .collect()
        })
        .collect()
}

/// One case of `Enter in a heading ... (#9293)`'s `test.for`.
fn enter(build: fn() -> Result<SelectionRef, crate::LexicalError>, expected: &[&[&str]]) {
    let editor = test_editor();
    update(&editor, move || build()?.insert_paragraph().map(drop));
    let blocks = editor.read(describe_blocks).unwrap();
    assert_eq!(blocks, expected);
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > start of the text after a line break sp…
#[test]
fn enter_at_the_start_of_the_text_after_a_line_break_splits_the_heading() {
    enter(
        || {
            let second = create_text_node("second");
            replace_root_with_heading(&[
                create_text_node("first"),
                create_line_break_node(),
                second.clone(),
            ]);
            second.select(Some(0), Some(0))
        },
        &[&["h1", "first", "linebreak"], &["h1", "second"]],
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > just after an inline decorator splits t…
#[test]
fn enter_just_after_an_inline_decorator_splits_the_heading() {
    enter(
        || {
            let text = create_text_node(" b");
            replace_root_with_heading(&[
                create_text_node("a "),
                create_test_decorator_node(),
                text.clone(),
            ]);
            text.select(Some(0), Some(0))
        },
        &[&["h1", "a ", "test_decorator"], &["h1", " b"]],
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > end of a heading that ends with a line …
#[test]
fn enter_at_the_end_of_a_heading_that_ends_with_a_line_break_adds_a_paragraph() {
    enter(
        || {
            replace_root_with_heading(&[create_text_node("first"), create_line_break_node()])
                .select(Some(2), Some(2))
        },
        &[&["h1", "first", "linebreak"], &["paragraph"]],
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > end of a heading that ends with an inli…
#[test]
fn enter_at_the_end_of_a_heading_that_ends_with_an_inline_decorator_adds_a_paragraph() {
    enter(
        || {
            replace_root_with_heading(&[create_text_node("a "), create_test_decorator_node()])
                .select(Some(2), Some(2))
        },
        &[&["h1", "a ", "test_decorator"], &["paragraph"]],
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > start of the heading adds a paragraph a…
#[test]
fn enter_at_the_start_of_the_heading_adds_a_paragraph_above() {
    enter(
        || {
            let first = create_text_node("first");
            replace_root_with_heading(&[
                first.clone(),
                create_line_break_node(),
                create_text_node("second"),
            ]);
            first.select(Some(0), Some(0))
        },
        &[&["paragraph"], &["h1", "first", "linebreak", "second"]],
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > (link, 0) in a leading link adds a para…
#[test]
fn enter_at_link_0_in_a_leading_link_adds_a_paragraph_above() {
    enter(
        || {
            let link = create_link_node("https://lexical.dev", &LinkAttributes::default());
            link.append(&[create_text_node("a")])?;
            replace_root_with_heading(&[link.clone(), create_text_node(" b")]);
            // link.select(0, 0) would move into the text: a link can't be empty.
            let selection = create_range_selection();
            selection.point_set(Which::Anchor, link.get_key(), 0, PointType::Element)?;
            selection.point_set(Which::Focus, link.get_key(), 0, PointType::Element)?;
            set_selection(Some(selection.clone()));
            Ok(selection)
        },
        &[&["paragraph"], &["h1", "link", " b"]],
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > start of the heading before an inline d…
#[test]
fn enter_at_the_start_of_the_heading_before_an_inline_decorator_adds_a_paragraph_above() {
    enter(
        || {
            replace_root_with_heading(&[create_test_decorator_node(), create_text_node(" b")])
                .select(Some(0), Some(0))
        },
        &[&["paragraph"], &["h1", "test_decorator", " b"]],
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > middle of a word splits the heading
#[test]
fn enter_in_the_middle_of_a_word_splits_the_heading() {
    enter(
        || {
            let second = create_text_node("second");
            replace_root_with_heading(&[
                create_text_node("first"),
                create_line_break_node(),
                second.clone(),
            ]);
            second.select(Some(3), Some(3))
        },
        &[&["h1", "first", "linebreak", "sec"], &["h1", "ond"]],
    );
}

// upstream: lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > end of the text adds a paragraph
#[test]
fn enter_at_the_end_of_the_text_adds_a_paragraph() {
    enter(
        || {
            let second = create_text_node("second");
            replace_root_with_heading(&[
                create_text_node("first"),
                create_line_break_node(),
                second.clone(),
            ]);
            second.select(Some(6), Some(6))
        },
        &[&["h1", "first", "linebreak", "second"], &["paragraph"]],
    );
}
