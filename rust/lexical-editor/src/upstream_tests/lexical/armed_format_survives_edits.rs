//! Ports of `packages/lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts`.
//!
//! A format toggled with nothing selected is armed for the *next* insertion
//! and is deliberately not backed by any node yet. Edits reposition the
//! caret through `ElementNode.select()`/`TextNode.select()` as an
//! implementation detail, so anything that re-derives the format from the
//! node those land on silently cancels what the user armed.

#![cfg(all(feature = "extension", feature = "rich-text", feature = "list"))]

use crate::constants::IS_BOLD;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::rich_text::{HeadingTagType, create_heading_node, create_quote_node};
use crate::upstream_tests::support::update;
use crate::{
    LexicalError, LexicalExtension, NodeRef, SelectionRef, create_paragraph_node, create_text_node,
    define_extension, get_root, get_selection,
};

fn build() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("@test/armed-format", ())
    })
    .into()])
    .unwrap()
}

/// `$arm()`.
fn arm() -> Result<SelectionRef, LexicalError> {
    let selection = get_selection()
        .filter(SelectionRef::is_range)
        .expect("Expected RangeSelection");
    selection.format_text("bold", None)?;
    assert_eq!(selection.range().unwrap().format, f64::from(IS_BOLD));
    Ok(selection)
}

type Seed = fn() -> Result<(), LexicalError>;

/// `armBoldThen($seed, $op)`.
fn arm_bold_then(
    seed: Seed,
    op: impl FnOnce(&SelectionRef) -> Result<(), LexicalError> + 'static,
) -> f64 {
    let editor = build();
    update(&editor, seed);
    update(&editor, move || op(&arm()?));
    let format = editor
        .read(|| {
            get_selection()
                .and_then(|s| s.range())
                .expect("Expected RangeSelection")
                .format
        })
        .unwrap();
    editor.dispose();
    format
}

/// `$seedAtEndOf($make)`.
fn seed_at_end_of(block: NodeRef) -> Result<(), LexicalError> {
    get_root().clear()?;
    get_root().append(std::slice::from_ref(&block))?;
    block.select_end()?;
    Ok(())
}

fn with_abc(block: NodeRef) -> NodeRef {
    block.append(&[create_text_node("abc")]).unwrap();
    block
}

/// `$seedParagraph`.
fn seed_paragraph() -> Result<(), LexicalError> {
    seed_at_end_of(with_abc(create_paragraph_node()))
}

fn enter_in(seed: Seed) {
    assert_eq!(
        arm_bold_then(seed, |selection| selection.insert_paragraph().map(|_| ())),
        f64::from(IS_BOLD)
    );
}

// upstream: lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in a heading
#[test]
fn enter_in_a_heading() {
    enter_in(|| seed_at_end_of(with_abc(create_heading_node(HeadingTagType::H1))));
}

// upstream: lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in a paragraph
#[test]
fn enter_in_a_paragraph() {
    enter_in(seed_paragraph);
}

// upstream: lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in a quote
#[test]
fn enter_in_a_quote() {
    enter_in(|| seed_at_end_of(with_abc(create_quote_node(false))));
}

// upstream: lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in a list item
#[test]
fn enter_in_a_list_item() {
    enter_in(|| {
        let list = create_list_node(ListType::Bullet, 1.0);
        list.append(&[with_abc(create_list_item_node(None))])?;
        seed_at_end_of(list)
    });
}

// upstream: lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in the middle of a paragraph
#[test]
fn enter_in_the_middle_of_a_paragraph() {
    // Splitting mid-block moves the trailing TextNode into the new block, so
    // the caret lands on a node that carries its own (unbolded) format.
    enter_in(|| {
        let text = create_text_node("abcdef");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        text.select(Some(3), Some(3))?;
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Shift+Enter
#[test]
fn shift_enter() {
    assert_eq!(
        arm_bold_then(seed_paragraph, |selection| selection
            .insert_line_break(false)),
        f64::from(IS_BOLD)
    );
}

// upstream: lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > pasting inline content
#[test]
fn pasting_inline_content() {
    assert_eq!(
        arm_bold_then(seed_paragraph, |selection| {
            selection.insert_nodes(vec![create_text_node("pasted")])
        }),
        f64::from(IS_BOLD)
    );
}

// upstream: lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > pasting a block
#[test]
fn pasting_a_block() {
    assert_eq!(
        arm_bold_then(seed_paragraph, |selection| {
            selection.insert_nodes(vec![with_pasted(create_paragraph_node())])
        }),
        f64::from(IS_BOLD)
    );
}

fn with_pasted(block: NodeRef) -> NodeRef {
    block.append(&[create_text_node("pasted")]).unwrap();
    block
}

// upstream: lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > the armed format reaches the text typed after Enter
#[test]
fn the_armed_format_reaches_the_text_typed_after_enter() {
    let editor = build();
    update(&editor, seed_paragraph);
    update(&editor, || {
        let selection = arm()?;
        selection.insert_paragraph()?;
        selection.insert_text("typed")
    });
    editor
        .read(|| {
            let typed = get_root().get_last_descendant().expect("a descendant");
            assert!(typed.is_text());
            assert_eq!(typed.get_text_content(), "typed");
            assert_eq!(typed.text_data().get_format(), f64::from(IS_BOLD));
        })
        .unwrap();
    editor.dispose();
}
