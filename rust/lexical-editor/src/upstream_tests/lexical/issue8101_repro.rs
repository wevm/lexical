//! Ports of `packages/lexical/src/__tests__/unit/Issue8101Repro.test.ts`.

#![cfg(all(feature = "clipboard", feature = "html"))]

use super::super::support::{initialize_unit_test, update};
use crate::clipboard::{
    generate_json_from_selected_nodes, generate_nodes_from_serialized_nodes, get_html_content,
    insert_generated_nodes,
};
use crate::json::{Value, parse};
use crate::{
    CreateEditorArgs, Editor, LexicalError, NodeRef, PointType, SelectionRef, Which,
    create_paragraph_node, create_range_selection, create_text_node, get_root, get_selection,
    set_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

fn range_selection() -> SelectionRef {
    let selection = get_selection().filter(|s| s.is_range());
    selection.expect("Expected RangeSelection")
}

/// Selects the whole of `text`.
fn select_text(text: &NodeRef, from: usize, to: usize) -> Result<(), LexicalError> {
    let selection = create_range_selection();
    selection.point_set(Which::Anchor, text.get_key(), from, PointType::Text)?;
    selection.point_set(Which::Focus, text.get_key(), to, PointType::Text)?;
    set_selection(Some(selection));
    Ok(())
}

/// `$setUpAndSelectFirstParagraph(format)`.
fn set_up_and_select_first_paragraph(format: &str) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    let first = create_paragraph_node();
    first.set_element_format(format);
    let text = create_text_node("Hello");
    first.append(std::slice::from_ref(&text))?;
    let second = create_paragraph_node();
    second.append(&[create_text_node("World")])?;
    root.append(&[first, second])?;
    select_text(&text, 0, text.get_text_content_size())
}

fn editor() -> Editor {
    initialize_unit_test(CreateEditorArgs::default())
}

// upstream: lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > copying a whole aligned paragraph keeps the alignment in the Lexical clipboard payload
#[test]
fn copying_a_whole_aligned_paragraph_keeps_the_alignment_in_the_lexical_clipboard_payload() {
    let editor = editor();
    update(&editor, || set_up_and_select_first_paragraph("center"));
    let e = editor.clone();
    editor
        .read(move || {
            let selection = range_selection();
            let nodes = generate_json_from_selected_nodes(&e, Some(&selection))
                .unwrap()
                .nodes;
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].get("type"), Some(&Value::from("paragraph")));
            assert_eq!(nodes[0].get("format"), Some(&Value::from("center")));
        })
        .unwrap();
}

fn html_of_selection(editor: &Editor) -> String {
    let e = editor.clone();
    editor
        .read(move || get_html_content(&e, Some(&range_selection())).unwrap())
        .unwrap()
}

// upstream: lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > copying a whole aligned paragraph keeps the alignment in the HTML clipboard payload
#[test]
fn copying_a_whole_aligned_paragraph_keeps_the_alignment_in_the_html_clipboard_payload() {
    let editor = editor();
    update(&editor, || set_up_and_select_first_paragraph("right"));
    assert_eq!(
        html_of_selection(&editor),
        r#"<p style="text-align: right;"><span style="white-space: pre-wrap;">Hello</span></p>"#
    );
}

// upstream: lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > a paragraph with no block-level state of its own still copies as inline content
#[test]
fn a_paragraph_with_no_block_level_state_of_its_own_still_copies_as_inline_content() {
    let editor = editor();
    update(&editor, || set_up_and_select_first_paragraph(""));
    assert_eq!(
        html_of_selection(&editor),
        r#"<span style="white-space: pre-wrap;">Hello</span>"#
    );
}

/// `JSON.stringify($generateJSONFromSelectedNodes(editor, selection))`.
fn copy_payload(editor: &Editor) -> String {
    let e = editor.clone();
    editor
        .read(move || {
            generate_json_from_selected_nodes(&e, Some(&range_selection()))
                .unwrap()
                .to_json()
                .to_string()
        })
        .unwrap()
}

/// Pastes `payload` at the selection with `$insertGeneratedNodes`.
fn paste_payload(editor: &Editor, payload: String) {
    let e = editor.clone();
    update(editor, move || {
        let json = parse(&payload).unwrap();
        let nodes = json.as_object().unwrap().get("nodes").unwrap().clone();
        let Value::Array(nodes) = nodes else {
            panic!("nodes")
        };
        insert_generated_nodes(
            &e,
            generate_nodes_from_serialized_nodes(&nodes)?,
            &range_selection(),
        )
    });
}

// upstream: lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > copy a centered paragraph and paste it into an empty paragraph
#[test]
fn copy_a_centered_paragraph_and_paste_it_into_an_empty_paragraph() {
    let editor = editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let first = create_paragraph_node();
        first.set_element_format("center");
        let text = create_text_node("Hello");
        first.append(std::slice::from_ref(&text))?;
        root.append(&[first, create_paragraph_node()])?;
        select_text(&text, 0, text.get_text_content_size())
    });
    let payload = copy_payload(&editor);
    update(&editor, || {
        let target = get_root().get_last_child().unwrap();
        assert!(target.is_element(), "Expected an ElementNode");
        target.select(None, None)?;
        Ok(())
    });
    paste_payload(&editor, payload);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            let pasted = &children[1];
            assert!(pasted.is_element(), "Expected an ElementNode");
            assert_eq!(pasted.get_text_content(), "Hello");
            assert_eq!(pasted.element_data().get_format_type(), "center");
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > pasting a whole-line copy into the middle of another line still merges inline
#[test]
fn pasting_a_whole_line_copy_into_the_middle_of_another_line_still_merges_inline() {
    let editor = editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let first = create_paragraph_node();
        let text = create_text_node("Hello");
        first.append(std::slice::from_ref(&text))?;
        let second = create_paragraph_node();
        second.append(&[create_text_node("abcdef")])?;
        root.append(&[first, second])?;
        select_text(&text, 0, text.get_text_content_size())
    });
    let payload = copy_payload(&editor);
    update(&editor, || {
        let second = get_root().get_last_child().unwrap();
        assert!(second.is_element(), "Expected an ElementNode");
        let text = second.get_first_child().expect("Expected a TextNode");
        select_text(&text, 3, 3)
    });
    paste_payload(&editor, payload);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[1].get_text_content(), "abcHellodef");
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > copying part of a paragraph still yields inline content only
#[test]
fn copying_part_of_a_paragraph_still_yields_inline_content_only() {
    let editor = editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let first = create_paragraph_node();
        first.set_element_format("center");
        let text = create_text_node("Hello");
        first.append(std::slice::from_ref(&text))?;
        root.append(&[first])?;
        select_text(&text, 1, 4)
    });
    let e = editor.clone();
    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    editor
        .read(move || {
            let selection = range_selection();
            let nodes = generate_json_from_selected_nodes(&e, Some(&selection))
                .unwrap()
                .nodes;
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].get("type"), Some(&Value::from("text")));
            *s.borrow_mut() = Some(get_html_content(&e, Some(&selection)).unwrap());
        })
        .unwrap();
    assert_eq!(
        seen.take().unwrap(),
        r#"<span style="white-space: pre-wrap;">ell</span>"#
    );
}
