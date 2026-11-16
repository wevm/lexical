//! Ports of `packages/lexical/src/__tests__/unit/Issue4815Repro.test.ts`.

use super::super::support::{TestInlineElementNode, create_test_inline_element_node};
use crate::json::{Value, stringify};
use crate::nodes::element::ElementData;
use crate::nodes::line_break::create_line_break_node;
use crate::{
    CreateEditorArgs, Editor, LexicalError, LexicalNode, NodeBase, NodeClass, NodeRef,
    UpdateOptions, create_editor, create_paragraph_node, create_text_node, get_root, get_selection,
};

/// `TestHeadingNode` (`test-heading`): a block element that renders an `h3`.
#[derive(Clone, Debug, Default)]
struct TestHeadingNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for TestHeadingNode {
    fn get_type(&self) -> &'static str {
        "test-heading"
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
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("h3"))
    }
}

fn create_test_heading_node() -> NodeRef {
    crate::lexical_utils::set_node_key(Box::new(TestHeadingNode::default()))
}

fn create_test_editor() -> Editor {
    create_editor(CreateEditorArgs {
        namespace: "test".to_owned(),
        nodes: vec![
            NodeClass::of::<TestHeadingNode>(),
            NodeClass::of::<TestInlineElementNode>(),
        ],
        ..CreateEditorArgs::default()
    })
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn with_children(node: NodeRef, children: Vec<NodeRef>) -> Result<NodeRef, LexicalError> {
    node.append(&children)?;
    Ok(node)
}

fn paragraph_of(children: Vec<NodeRef>) -> Result<NodeRef, LexicalError> {
    with_children(create_paragraph_node(), children)
}

/// `$pasteBlocks(editor, $buildDocument, $placeSelection)`: `place` gets
/// the nodes `build` returned and the node `build` kept for it.
fn paste_blocks(
    editor: &Editor,
    build: impl FnOnce() -> Result<(Vec<NodeRef>, NodeRef), LexicalError> + 'static,
    place: impl FnOnce(&NodeRef) -> Result<(), LexicalError> + 'static,
) {
    editor
        .update(
            move || {
                let (document, target) = build()?;
                get_root().append(&document)?;
                place(&target)?;
                let selection = get_selection().expect("Expected RangeSelection");
                assert!(selection.is_range(), "Expected RangeSelection");
                selection.insert_nodes(vec![
                    with_children(
                        create_test_heading_node(),
                        vec![create_text_node("Heading 3")],
                    )?,
                    paragraph_of(vec![create_text_node("Some paragraph")])?,
                ])
            },
            discrete(),
        )
        .unwrap();
}

fn json_string(s: &str) -> String {
    stringify(&Value::String(s.to_owned())).unwrap()
}

/// `$describeRoot()`.
fn describe_root() -> Vec<String> {
    get_root()
        .get_children()
        .iter()
        .map(|node| {
            format!(
                "{}({})",
                node.get_type(),
                json_string(&node.get_text_content())
            )
        })
        .collect()
}

/// `$describeSelection()`.
fn describe_selection() -> String {
    let selection = get_selection().expect("Expected RangeSelection");
    let range = selection.range().expect("Expected RangeSelection");
    assert!(range.is_collapsed());
    let anchor = range.anchor.get_node().unwrap();
    format!(
        "{}@{}",
        json_string(&anchor.get_text_content()),
        range.anchor.offset
    )
}

fn read<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read(f).unwrap()
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block at the end of a paragraph after consecutive line breaks
#[test]
fn preserves_a_pasted_block_at_the_end_of_a_paragraph_after_consecutive_line_breaks() {
    let editor = create_test_editor();
    paste_blocks(
        &editor,
        || {
            let paragraph = paragraph_of(vec![
                create_text_node("Line of text"),
                create_line_break_node(),
                create_line_break_node(),
            ])?;
            Ok((vec![paragraph.clone()], paragraph))
        },
        |paragraph| paragraph.select(Some(3), Some(3)).map(|_| ()),
    );
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Line of text\n")"#,
                r#"test-heading("Heading 3")"#,
                r#"paragraph("Some paragraph")"#,
            ]
        );
        assert_eq!(describe_selection(), r#""Some paragraph"@14"#);
    });
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block before text that follows consecutive line breaks
#[test]
fn preserves_a_pasted_block_before_text_that_follows_consecutive_line_breaks() {
    let editor = create_test_editor();
    paste_blocks(
        &editor,
        || {
            let paragraph = paragraph_of(vec![
                create_text_node("Line of text"),
                create_line_break_node(),
                create_line_break_node(),
                create_text_node("Paragraph 1"),
            ])?;
            Ok((vec![paragraph.clone()], paragraph))
        },
        |paragraph| paragraph.select(Some(3), Some(3)).map(|_| ()),
    );
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Line of text\n")"#,
                r#"test-heading("Heading 3")"#,
                r#"paragraph("Some paragraphParagraph 1")"#,
            ]
        );
        assert_eq!(describe_selection(), r#""Some paragraphParagraph 1"@14"#);
    });
}

/// The document of the text-point cases: `Line of text`, two line breaks,
/// then `Paragraph 1` (in an inline element when `inline`).
fn line_breaks_then_text(inline: bool) -> Result<(Vec<NodeRef>, NodeRef), LexicalError> {
    let text = create_text_node("Paragraph 1");
    let last = if inline {
        with_children(create_test_inline_element_node(), vec![text.clone()])?
    } else {
        text.clone()
    };
    let paragraph = paragraph_of(vec![
        create_text_node("Line of text"),
        create_line_break_node(),
        create_line_break_node(),
        last,
    ])?;
    Ok((vec![paragraph], text))
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block for a text point at the start of the line
#[test]
fn preserves_a_pasted_block_for_a_text_point_at_the_start_of_the_line() {
    let editor = create_test_editor();
    paste_blocks(
        &editor,
        || line_breaks_then_text(false),
        |text| text.select(Some(0), Some(0)).map(|_| ()),
    );
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Line of text\n")"#,
                r#"test-heading("Heading 3")"#,
                r#"paragraph("Some paragraphParagraph 1")"#,
            ]
        );
    });
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block inside an inline element at the start of the line
#[test]
fn preserves_a_pasted_block_inside_an_inline_element_at_the_start_of_the_line() {
    let editor = create_test_editor();
    paste_blocks(
        &editor,
        || line_breaks_then_text(true),
        |text| text.select(Some(0), Some(0)).map(|_| ()),
    );
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Line of text\n")"#,
                r#"test-heading("Heading 3")"#,
                r#"paragraph("Some paragraphParagraph 1")"#,
            ]
        );
    });
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block when replacing a selection that starts after the empty line
#[test]
fn preserves_a_pasted_block_when_replacing_a_selection_that_starts_after_the_empty_line() {
    let editor = create_test_editor();
    paste_blocks(
        &editor,
        || line_breaks_then_text(false),
        |text| text.select(Some(0), Some("Paragraph".len())).map(|_| ()),
    );
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Line of text\n")"#,
                r#"test-heading("Heading 3")"#,
                r#"paragraph("Some paragraph 1")"#,
            ]
        );
    });
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves every line break of a longer run
#[test]
fn preserves_every_line_break_of_a_longer_run() {
    let editor = create_test_editor();
    paste_blocks(
        &editor,
        || {
            let paragraph = paragraph_of(vec![
                create_text_node("Line of text"),
                create_line_break_node(),
                create_line_break_node(),
                create_line_break_node(),
                create_text_node("Paragraph 1"),
            ])?;
            Ok((vec![paragraph.clone()], paragraph))
        },
        |paragraph| paragraph.select(Some(4), Some(4)).map(|_| ()),
    );
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Line of text\n\n")"#,
                r#"test-heading("Heading 3")"#,
                r#"paragraph("Some paragraphParagraph 1")"#,
            ]
        );
    });
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > keeps the existing merge behavior after a single line break
#[test]
fn keeps_the_existing_merge_behavior_after_a_single_line_break() {
    let editor = create_test_editor();
    paste_blocks(
        &editor,
        || {
            let paragraph = paragraph_of(vec![
                create_text_node("Line of text"),
                create_line_break_node(),
                create_text_node("Paragraph 1"),
            ])?;
            Ok((vec![paragraph.clone()], paragraph))
        },
        |paragraph| paragraph.select(Some(2), Some(2)).map(|_| ()),
    );
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Line of text\nHeading 3")"#,
                r#"paragraph("Some paragraphParagraph 1")"#,
            ]
        );
    });
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > gives a lone pasted paragraph its own block after an empty line
#[test]
fn gives_a_lone_pasted_paragraph_its_own_block_after_an_empty_line() {
    let editor = create_test_editor();
    editor
        .update(
            || {
                let paragraph = paragraph_of(vec![
                    create_text_node("Line of text"),
                    create_line_break_node(),
                    create_line_break_node(),
                    create_text_node("Paragraph 1"),
                ])?;
                get_root().append(std::slice::from_ref(&paragraph))?;
                paragraph.select(Some(3), Some(3))?;
                let selection = get_selection().expect("Expected RangeSelection");
                assert!(selection.is_range(), "Expected RangeSelection");
                selection.insert_nodes(vec![paragraph_of(vec![create_text_node(
                    "Some paragraph",
                )])?])
            },
            discrete(),
        )
        .unwrap();
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Line of text\n")"#,
                r#"paragraph("Some paragraphParagraph 1")"#,
            ]
        );
    });
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > keeps the existing merge behavior at the start of a block
#[test]
fn keeps_the_existing_merge_behavior_at_the_start_of_a_block() {
    let editor = create_test_editor();
    paste_blocks(
        &editor,
        || {
            let paragraph = paragraph_of(vec![create_text_node("Paragraph 1")])?;
            Ok((
                vec![
                    paragraph_of(vec![create_text_node("Above")])?,
                    paragraph.clone(),
                ],
                paragraph,
            ))
        },
        |paragraph| paragraph.select(Some(0), Some(0)).map(|_| ()),
    );
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Above")"#,
                r#"test-heading("Heading 3")"#,
                r#"paragraph("Some paragraphParagraph 1")"#,
            ]
        );
    });
}

// upstream: lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > keeps the existing merge behavior in the middle of a line
#[test]
fn keeps_the_existing_merge_behavior_in_the_middle_of_a_line() {
    let editor = create_test_editor();
    paste_blocks(
        &editor,
        || line_breaks_then_text(false),
        |text| text.select(Some(2), Some(2)).map(|_| ()),
    );
    read(&editor, || {
        assert_eq!(
            describe_root(),
            [
                r#"paragraph("Line of text\n\nPaHeading 3")"#,
                r#"paragraph("Some paragraphragraph 1")"#,
            ]
        );
    });
}
