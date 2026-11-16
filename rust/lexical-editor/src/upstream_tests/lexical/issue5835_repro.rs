//! Ports of `packages/lexical/src/__tests__/unit/Issue5835Repro.test.ts`.
//!
//! Upstream mounts the editor on a jsdom element; here it is a
//! [`TestRootElement`](super::super::support::TestRootElement).

#![cfg(all(
    feature = "extension",
    feature = "rich-text",
    feature = "list",
    feature = "code"
))]

use super::super::support::{
    TestShadowRootNode, create_test_shadow_root_node, range_selection, set_test_root_element,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::nodes::element::ElementData;
use crate::rich_text::{HeadingTagType, create_heading_node, create_quote_node};
use crate::{
    LexicalError, LexicalExtension, LexicalNode, NodeBase, NodeClass, NodeRef, SelectionRef,
    UpdateOptions, create_paragraph_node, create_text_node, define_extension, get_root, select_all,
    select_all_in, set_slot,
};

/// `StubbornNode` (`issue5835_stubborn`): an element whose
/// `collapseAtStart` is the default, false.
#[derive(Clone, Debug, Default)]
struct StubbornNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for StubbornNode {
    fn get_type(&self) -> &'static str {
        "issue5835_stubborn"
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

/// `HostNode` (`issue5835_host`): an element with a `title` slot.
#[derive(Clone, Debug, Default)]
struct HostNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for HostNode {
    fn get_type(&self) -> &'static str {
        "issue5835_host"
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
    fn declared_slots(&self) -> &'static [&'static str] {
        &["title"]
    }
}

/// `createEditor()`: `ext` on a root element.
fn create_editor() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
            crate::code::code_extension().into(),
        ],
        nodes: vec![
            NodeClass::of::<StubbornNode>(),
            NodeClass::of::<HostNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[5835]", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn with(node: NodeRef, children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    node.append(children)?;
    Ok(node)
}

fn text(t: &str) -> NodeRef {
    create_text_node(t)
}

fn item(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    with(create_list_item_node(None), children)
}

fn bullet(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    with(create_list_node(ListType::Bullet, 1.0), children)
}

fn set_root(children: &[NodeRef]) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(children)
}

fn set_up(
    editor: &LexicalEditorWithDispose,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
) {
    editor.update(f, discrete()).unwrap();
}

#[derive(Clone, Copy)]
enum Gesture {
    Character,
    Word,
    Line,
}

/// `$applyGesture(selection, gesture, isBackward)`.
fn apply_gesture(
    selection: &SelectionRef,
    gesture: Gesture,
    is_backward: bool,
) -> Result<(), LexicalError> {
    match gesture {
        Gesture::Character => selection.delete_character(is_backward),
        Gesture::Word => selection.delete_word(is_backward),
        Gesture::Line => selection.delete_line(is_backward),
    }
}

/// `selectAllAndDelete(editor, gesture, isBackward)`.
fn select_all_and_delete(editor: &LexicalEditorWithDispose, gesture: Gesture, is_backward: bool) {
    editor
        .update(
            move || apply_gesture(&select_all()?, gesture, is_backward),
            discrete(),
        )
        .unwrap();
}

/// `readRootTypes(editor)`.
fn read_root_types(editor: &LexicalEditorWithDispose) -> Vec<String> {
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|n| n.get_type().to_owned())
                .collect()
        })
        .unwrap()
}

fn no_text_nodes(editor: &LexicalEditorWithDispose) {
    assert!(
        editor
            .read(|| get_root().get_all_text_nodes().is_empty())
            .unwrap()
    );
}

fn empty_text(editor: &LexicalEditorWithDispose) {
    assert_eq!(editor.read(|| get_root().get_text_content()).unwrap(), "");
}

fn heading_only(gesture: Gesture, is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        set_root(&[with(
            create_heading_node(HeadingTagType::H1),
            &[text("hi")],
        )?])
    });
    select_all_and_delete(&editor, gesture, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    if !matches!(gesture, Gesture::Line) {
        no_text_nodes(&editor);
    }
    editor.dispose();
}

fn quote_only(gesture: Gesture, is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        set_root(&[with(create_quote_node(false), &[text("hi")])?])
    });
    select_all_and_delete(&editor, gesture, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    editor.dispose();
}

fn list_followed_by_a_paragraph(gesture: Gesture, is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        set_root(&[
            bullet(&[item(&[text("one")])?, item(&[text("two")])?])?,
            with(create_paragraph_node(), &[text("after")])?,
        ])
    });
    select_all_and_delete(&editor, gesture, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    no_text_nodes(&editor);
    editor.dispose();
}

fn nested_list(gesture: Gesture, is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        set_root(&[bullet(&[
            item(&[text("one")])?,
            item(&[bullet(&[item(&[text("two")])?])?])?,
        ])?])
    });
    select_all_and_delete(&editor, gesture, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    no_text_nodes(&editor);
    editor.dispose();
}

fn a_nest_with_no_plain_sibling_item(gesture: Gesture, is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        set_root(&[bullet(&[item(&[bullet(&[item(&[text("two")])?])?])?])?])
    });
    select_all_and_delete(&editor, gesture, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    empty_text(&editor);
    editor.dispose();
}

fn a_deeply_nested_list_unwraps_every_level(gesture: Gesture, is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        let mut it = item(&[text("deep")])?;
        for _ in 0..4 {
            it = item(&[bullet(&[it])?])?;
        }
        set_root(&[bullet(&[it])?])
    });
    select_all_and_delete(&editor, gesture, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    empty_text(&editor);
    editor.dispose();
}

fn paragraph_as_the_first_block_is_left_as_a_paragraph(gesture: Gesture, is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        set_root(&[
            with(create_paragraph_node(), &[text("hi")])?,
            with(create_heading_node(HeadingTagType::H1), &[text("there")])?,
        ])
    });
    select_all_and_delete(&editor, gesture, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: true > heading as the only block
#[test]
fn delete_character_backward_heading() {
    heading_only(Gesture::Character, true);
}
// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: true > quote as the only block
#[test]
fn delete_character_backward_quote() {
    quote_only(Gesture::Character, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: true > list followed by a paragraph
#[test]
fn delete_character_backward_list_then_paragraph() {
    list_followed_by_a_paragraph(Gesture::Character, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: true > nested list
#[test]
fn delete_character_backward_nested_list() {
    nested_list(Gesture::Character, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: true > a nest with no plain sibling item
#[test]
fn delete_character_backward_nest_without_plain_item() {
    a_nest_with_no_plain_sibling_item(Gesture::Character, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: true > a deeply nested list unwraps every level
#[test]
fn delete_character_backward_deeply_nested_list() {
    a_deeply_nested_list_unwraps_every_level(Gesture::Character, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: true > paragraph as the first block is left as a paragraph
#[test]
fn delete_character_backward_paragraph_first() {
    paragraph_as_the_first_block_is_left_as_a_paragraph(Gesture::Character, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: false > heading as the only block
#[test]
fn delete_character_forward_heading() {
    heading_only(Gesture::Character, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: false > quote as the only block
#[test]
fn delete_character_forward_quote() {
    quote_only(Gesture::Character, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: false > list followed by a paragraph
#[test]
fn delete_character_forward_list_then_paragraph() {
    list_followed_by_a_paragraph(Gesture::Character, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: false > nested list
#[test]
fn delete_character_forward_nested_list() {
    nested_list(Gesture::Character, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: false > a nest with no plain sibling item
#[test]
fn delete_character_forward_nest_without_plain_item() {
    a_nest_with_no_plain_sibling_item(Gesture::Character, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: false > a deeply nested list unwraps every level
#[test]
fn delete_character_forward_deeply_nested_list() {
    a_deeply_nested_list_unwraps_every_level(Gesture::Character, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteCharacter > isBackward: false > paragraph as the first block is left as a paragraph
#[test]
fn delete_character_forward_paragraph_first() {
    paragraph_as_the_first_block_is_left_as_a_paragraph(Gesture::Character, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: true > heading as the only block
#[test]
fn delete_word_backward_heading() {
    heading_only(Gesture::Word, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: true > quote as the only block
#[test]
fn delete_word_backward_quote() {
    quote_only(Gesture::Word, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: true > list followed by a paragraph
#[test]
fn delete_word_backward_list_then_paragraph() {
    list_followed_by_a_paragraph(Gesture::Word, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: true > nested list
#[test]
fn delete_word_backward_nested_list() {
    nested_list(Gesture::Word, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: true > a nest with no plain sibling item
#[test]
fn delete_word_backward_nest_without_plain_item() {
    a_nest_with_no_plain_sibling_item(Gesture::Word, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: true > a deeply nested list unwraps every level
#[test]
fn delete_word_backward_deeply_nested_list() {
    a_deeply_nested_list_unwraps_every_level(Gesture::Word, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: true > paragraph as the first block is left as a paragraph
#[test]
fn delete_word_backward_paragraph_first() {
    paragraph_as_the_first_block_is_left_as_a_paragraph(Gesture::Word, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: false > heading as the only block
#[test]
fn delete_word_forward_heading() {
    heading_only(Gesture::Word, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: false > quote as the only block
#[test]
fn delete_word_forward_quote() {
    quote_only(Gesture::Word, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: false > list followed by a paragraph
#[test]
fn delete_word_forward_list_then_paragraph() {
    list_followed_by_a_paragraph(Gesture::Word, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: false > nested list
#[test]
fn delete_word_forward_nested_list() {
    nested_list(Gesture::Word, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: false > a nest with no plain sibling item
#[test]
fn delete_word_forward_nest_without_plain_item() {
    a_nest_with_no_plain_sibling_item(Gesture::Word, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: false > a deeply nested list unwraps every level
#[test]
fn delete_word_forward_deeply_nested_list() {
    a_deeply_nested_list_unwraps_every_level(Gesture::Word, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteWord > isBackward: false > paragraph as the first block is left as a paragraph
#[test]
fn delete_word_forward_paragraph_first() {
    paragraph_as_the_first_block_is_left_as_a_paragraph(Gesture::Word, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteLine (isBackward: true) > heading as the only block
#[test]
fn delete_line_backward_heading() {
    heading_only(Gesture::Line, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteLine (isBackward: true) > quote as the only block
#[test]
fn delete_line_backward_quote() {
    quote_only(Gesture::Line, true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteLine (isBackward: false) > heading as the only block
#[test]
fn delete_line_forward_heading() {
    heading_only(Gesture::Line, false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > deleteLine (isBackward: false) > quote as the only block
#[test]
fn delete_line_forward_quote() {
    quote_only(Gesture::Line, false);
}

// removes whatever block types the document holds

fn a_code_block(is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        set_root(&[with(
            crate::code::create_code_node(None, None),
            &[text("const x = 1;")],
        )?])
    });
    select_all_and_delete(&editor, Gesture::Character, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    editor.dispose();
}

fn a_third_party_block_node(is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        let stubborn = crate::lexical_utils::set_node_key(Box::new(StubbornNode::default()));
        set_root(&[with(stubborn, &[text("note")])?])
    });
    select_all_and_delete(&editor, Gesture::Character, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    editor.dispose();
}

fn a_quote_wrapping_a_list(is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        set_root(&[with(
            create_quote_node(false),
            &[bullet(&[item(&[text("one")])?])?],
        )?])
    });
    select_all_and_delete(&editor, Gesture::Character, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    empty_text(&editor);
    editor.dispose();
}

fn create_host_node() -> NodeRef {
    crate::lexical_utils::set_node_key(Box::new(HostNode::default()))
}

fn a_slot_host_slots_and_all(is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        let host = with(
            create_host_node(),
            &[with(
                create_heading_node(HeadingTagType::H1),
                &[text("body")],
            )?],
        )?;
        set_root(std::slice::from_ref(&host))?;
        set_slot(
            &host,
            "title",
            &with(
                create_test_shadow_root_node(),
                &[with(create_paragraph_node(), &[text("title")])?],
            )?,
        )?;
        Ok(())
    });
    select_all_and_delete(&editor, Gesture::Character, is_backward);
    assert_eq!(read_root_types(&editor), ["paragraph"]);
    empty_text(&editor);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > removes whatever block types the document holds > a code block (isBackward: true)
#[test]
fn a_code_block_backward() {
    a_code_block(true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > removes whatever block types the document holds > a code block (isBackward: false)
#[test]
fn a_code_block_forward() {
    a_code_block(false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > removes whatever block types the document holds > a third-party block node (isBackward: true)
#[test]
fn a_third_party_block_node_backward() {
    a_third_party_block_node(true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > removes whatever block types the document holds > a third-party block node (isBackward: false)
#[test]
fn a_third_party_block_node_forward() {
    a_third_party_block_node(false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > removes whatever block types the document holds > a quote wrapping a list (isBackward: true)
#[test]
fn a_quote_wrapping_a_list_backward() {
    a_quote_wrapping_a_list(true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > removes whatever block types the document holds > a quote wrapping a list (isBackward: false)
#[test]
fn a_quote_wrapping_a_list_forward() {
    a_quote_wrapping_a_list(false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > removes whatever block types the document holds > a slot host, slots and all (isBackward: true)
#[test]
fn a_slot_host_slots_and_all_backward() {
    a_slot_host_slots_and_all(true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > removes whatever block types the document holds > a slot host, slots and all (isBackward: false)
#[test]
fn a_slot_host_slots_and_all_forward() {
    a_slot_host_slots_and_all(false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > a select-all scoped to a slot only clears the slot
#[test]
fn a_select_all_scoped_to_a_slot_only_clears_the_slot() {
    let editor = create_editor();
    set_up(&editor, || {
        let host = with(
            create_host_node(),
            &[with(
                create_heading_node(HeadingTagType::H1),
                &[text("body")],
            )?],
        )?;
        set_root(std::slice::from_ref(&host))?;
        let slot_paragraph = with(create_paragraph_node(), &[text("title")])?;
        set_slot(
            &host,
            "title",
            &with(
                create_test_shadow_root_node(),
                std::slice::from_ref(&slot_paragraph),
            )?,
        )?;
        let t = slot_paragraph.get_first_child_or_throw()?;
        assert!(t.is_text(), "Expected a TextNode");
        t.select(Some(1), Some(1))?;
        Ok(())
    });
    editor
        .update(
            || select_all_in(&range_selection())?.delete_character(true),
            discrete(),
        )
        .unwrap();
    assert_eq!(read_root_types(&editor), ["issue5835_host"]);
    assert!(
        editor
            .read(|| get_root().get_text_content())
            .unwrap()
            .contains("body")
    );
    editor.dispose();
}

fn a_partial_delete_that_only_empties_the_first_block(is_backward: bool) {
    let editor = create_editor();
    set_up(&editor, || {
        let heading = with(create_heading_node(HeadingTagType::H1), &[text("abc")])?;
        set_root(&[
            heading.clone(),
            with(create_paragraph_node(), &[text("def")])?,
        ])?;
        let t = heading.get_first_child_or_throw()?;
        assert!(t.is_text(), "Expected a TextNode");
        t.select(Some(0), Some(3))?;
        Ok(())
    });
    editor
        .update(
            move || range_selection().delete_character(is_backward),
            discrete(),
        )
        .unwrap();
    assert_eq!(read_root_types(&editor), ["heading", "paragraph"]);
    assert_eq!(
        editor
            .read(|| get_root()
                .get_last_child_or_throw()
                .unwrap()
                .get_text_content())
            .unwrap(),
        "def"
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > a partial delete that only empties the first block is unaffected (isBackward: true)
#[test]
fn a_partial_delete_that_only_empties_the_first_block_is_unaffected_backward() {
    a_partial_delete_that_only_empties_the_first_block(true);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > a partial delete that only empties the first block is unaffected (isBackward: false)
#[test]
fn a_partial_delete_that_only_empties_the_first_block_is_unaffected_forward() {
    a_partial_delete_that_only_empties_the_first_block(false);
}

// upstream: lexical/src/__tests__/unit/Issue5835Repro.test.ts::select-all + delete collapses to an empty paragraph (#5835) > replacing all text in a lone non-paragraph block keeps its type
#[test]
fn replacing_all_text_in_a_lone_non_paragraph_block_keeps_its_type() {
    let editor = create_editor();
    set_up(&editor, || {
        set_root(&[with(
            create_heading_node(HeadingTagType::H1),
            &[text("hi")],
        )?])
    });
    editor
        .update(|| select_all()?.insert_text("there"), discrete())
        .unwrap();
    assert_eq!(read_root_types(&editor), ["heading"]);
    editor
        .read(|| {
            assert_eq!(get_root().get_text_content(), "there");
            assert_ne!(
                get_root().get_first_child().unwrap().get_type(),
                "paragraph"
            );
        })
        .unwrap();
    editor.dispose();
}
