//! Ports of `packages/lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx`.
//!
//! Upstream mounts the editor and starts from `init()`: a paragraph holding
//! an empty unmergeable text node. Its `updateDOM` tests check the model
//! half Rust has (rust/PARITY.md); the two whose `updateDOM` patches the DOM
//! are skipped.

use crate::constants::*;
use crate::upstream_tests::support::{create_test_segmented_node, initialize_unit_test, update};
use crate::{
    CreateEditorArgs, Editor, NodeRef, Which, create_paragraph_node, create_text_node,
    get_composition_key, get_root, get_selection, set_composition_key,
};

/// `init()`.
fn text_node_editor() -> Editor {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let text = create_text_node("");
        text.toggle_unmergeable();
        paragraph.append(&[text])?;
        get_root().append(&[paragraph])
    });
    editor
}

/// The empty text node of `init()`.
fn first_text() -> NodeRef {
    let paragraph = get_root().get_first_child().unwrap();
    assert_eq!(paragraph.get_type(), "paragraph");
    let text = paragraph.get_first_child().unwrap();
    assert!(text.is_text());
    text
}

/// `getEditorStateTextContent(editor.getEditorState())`.
fn editor_state_text_content(editor: &Editor) -> String {
    editor
        .get_editor_state()
        .read(|| get_root().get_text_content())
}

/// "Hello World" appended to the root in a new paragraph.
fn hello_world() -> Result<NodeRef, crate::LexicalError> {
    let paragraph_node = create_paragraph_node();
    let text_node = create_text_node("Hello World");
    paragraph_node.append(std::slice::from_ref(&text_node))?;
    get_root().append(&[paragraph_node])?;
    Ok(text_node)
}

fn point(selection: &crate::SelectionRef, which: Which) -> (NodeRef, usize) {
    let p = selection.point(which).unwrap();
    (p.get_node().unwrap(), p.offset)
}
// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportJSON() > should return and object conforming to the expected schema
#[test]
fn export_json_conforms_to_the_expected_schema() {
    let editor = text_node_editor();
    update(&editor, || {
        let node = create_text_node("");
        assert_eq!(
            crate::json::Value::Object(node.export_json(false)).to_string(),
            r#"{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}"#
        );
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > root.getTextContent() > writable nodes
#[test]
fn root_get_text_content_writable_nodes() {
    let editor = text_node_editor();
    let key = crate::upstream_tests::support::update_with(&editor, || {
        let text_node = create_text_node("Text");
        assert_eq!(text_node.get_text_content(), "Text");
        assert_eq!(text_node.text_data().get_text(), "Text");
        get_root()
            .get_first_child()
            .unwrap()
            .append(std::slice::from_ref(&text_node))?;
        Ok(text_node.get_key().to_owned())
    });
    assert_eq!(editor_state_text_content(&editor), "Text");
    update(&editor, move || {
        crate::get_node_by_key(&key).unwrap().mark_dirty();
        Ok(())
    });
    assert_eq!(editor_state_text_content(&editor), "Text");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > root.getTextContent() > prepend node
#[test]
fn root_get_text_content_prepend_node() {
    let editor = text_node_editor();
    update(&editor, || {
        let text_node = create_text_node("World");
        text_node.toggle_unmergeable();
        get_root().get_first_child().unwrap().append(&[text_node])
    });
    update(&editor, || {
        let text_node = create_text_node("Hello ");
        text_node.toggle_unmergeable();
        let previous_text_node = get_root()
            .get_first_child()
            .unwrap()
            .get_first_child()
            .unwrap();
        previous_text_node.insert_before(&text_node, true)?;
        Ok(())
    });
    assert_eq!(editor_state_text_content(&editor), "Hello World");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > setTextContent() > writable nodes
#[test]
fn set_text_content_writable_nodes() {
    let editor = text_node_editor();
    update(&editor, || {
        let text_node = create_text_node("My new text node");
        text_node.set_text_content("My newer text node");
        assert_eq!(text_node.get_text_content(), "My newer text node");
        Ok(())
    });
}

/// `getFormatFlags(flag)`.
fn get_format_flags(flag: &'static str, state_format: i32) {
    let editor = text_node_editor();
    update(&editor, move || {
        let text_node = first_text();
        let new_format = text_node.get_format_flags(flag, None);
        assert_eq!(new_format, state_format);
        text_node.set_format(f64::from(new_format));
        assert_eq!(text_node.get_format_flags(flag, None), 0);
        Ok(())
    });
}

/// `predicate for flag`.
fn predicate(flag: &'static str, state_format: i32) {
    let editor = text_node_editor();
    update(&editor, move || {
        let text_node = first_text();
        text_node.set_format(f64::from(state_format));
        assert!(text_node.has_format(flag));
        Ok(())
    });
}

/// `toggling for flag`.
fn toggling(flag: &'static str) {
    let editor = text_node_editor();
    update(&editor, move || {
        let text_node = first_text();
        assert!(!text_node.has_format(flag));
        text_node.toggle_format(flag);
        assert!(text_node.has_format(flag));
        text_node.toggle_format(flag);
        assert!(!text_node.has_format(flag));
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > bold flag > getFormatFlags(bold)
#[test]
fn get_format_flags_bold() {
    get_format_flags("bold", IS_BOLD);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > bold flag > predicate for bold
#[test]
fn predicate_for_bold() {
    predicate("bold", IS_BOLD);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > bold flag > toggling for bold
#[test]
fn toggling_for_bold() {
    toggling("bold");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > italic flag > getFormatFlags(italic)
#[test]
fn get_format_flags_italic() {
    get_format_flags("italic", IS_ITALIC);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > italic flag > predicate for italic
#[test]
fn predicate_for_italic() {
    predicate("italic", IS_ITALIC);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > italic flag > toggling for italic
#[test]
fn toggling_for_italic() {
    toggling("italic");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > strikethrough flag > getFormatFlags(strikethrough)
#[test]
fn get_format_flags_strikethrough() {
    get_format_flags("strikethrough", IS_STRIKETHROUGH);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > strikethrough flag > predicate for strikethrough
#[test]
fn predicate_for_strikethrough() {
    predicate("strikethrough", IS_STRIKETHROUGH);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > strikethrough flag > toggling for strikethrough
#[test]
fn toggling_for_strikethrough() {
    toggling("strikethrough");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > underline flag > getFormatFlags(underline)
#[test]
fn get_format_flags_underline() {
    get_format_flags("underline", IS_UNDERLINE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > underline flag > predicate for underline
#[test]
fn predicate_for_underline() {
    predicate("underline", IS_UNDERLINE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > underline flag > toggling for underline
#[test]
fn toggling_for_underline() {
    toggling("underline");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > code flag > getFormatFlags(code)
#[test]
fn get_format_flags_code() {
    get_format_flags("code", IS_CODE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > code flag > predicate for code
#[test]
fn predicate_for_code() {
    predicate("code", IS_CODE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > code flag > toggling for code
#[test]
fn toggling_for_code() {
    toggling("code");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > subscript flag > getFormatFlags(subscript)
#[test]
fn get_format_flags_subscript() {
    get_format_flags("subscript", IS_SUBSCRIPT);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > subscript flag > predicate for subscript
#[test]
fn predicate_for_subscript() {
    predicate("subscript", IS_SUBSCRIPT);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > subscript flag > toggling for subscript
#[test]
fn toggling_for_subscript() {
    toggling("subscript");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > superscript flag > getFormatFlags(superscript)
#[test]
fn get_format_flags_superscript() {
    get_format_flags("superscript", IS_SUPERSCRIPT);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > superscript flag > predicate for superscript
#[test]
fn predicate_for_superscript() {
    predicate("superscript", IS_SUPERSCRIPT);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > superscript flag > toggling for superscript
#[test]
fn toggling_for_superscript() {
    toggling("superscript");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > highlight flag > getFormatFlags(highlight)
#[test]
fn get_format_flags_highlight() {
    get_format_flags("highlight", IS_HIGHLIGHT);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > highlight flag > predicate for highlight
#[test]
fn predicate_for_highlight() {
    predicate("highlight", IS_HIGHLIGHT);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > highlight flag > toggling for highlight
#[test]
fn toggling_for_highlight() {
    toggling("highlight");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > lowercase flag > getFormatFlags(lowercase)
#[test]
fn get_format_flags_lowercase() {
    get_format_flags("lowercase", IS_LOWERCASE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > lowercase flag > predicate for lowercase
#[test]
fn predicate_for_lowercase() {
    predicate("lowercase", IS_LOWERCASE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > lowercase flag > toggling for lowercase
#[test]
fn toggling_for_lowercase() {
    toggling("lowercase");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > uppercase flag > getFormatFlags(uppercase)
#[test]
fn get_format_flags_uppercase() {
    get_format_flags("uppercase", IS_UPPERCASE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > uppercase flag > predicate for uppercase
#[test]
fn predicate_for_uppercase() {
    predicate("uppercase", IS_UPPERCASE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > uppercase flag > toggling for uppercase
#[test]
fn toggling_for_uppercase() {
    toggling("uppercase");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > capitalize flag > getFormatFlags(capitalize)
#[test]
fn get_format_flags_capitalize() {
    get_format_flags("capitalize", IS_CAPITALIZE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > capitalize flag > predicate for capitalize
#[test]
fn predicate_for_capitalize() {
    predicate("capitalize", IS_CAPITALIZE);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > capitalize flag > toggling for capitalize
#[test]
fn toggling_for_capitalize() {
    toggling("capitalize");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > setting subscript clears superscript
#[test]
fn setting_subscript_clears_superscript() {
    let editor = text_node_editor();
    update(&editor, || {
        let text_node = hello_world()?;
        text_node.toggle_format("superscript");
        text_node.toggle_format("subscript");
        assert!(text_node.has_format("subscript"));
        assert!(!text_node.has_format("superscript"));
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > setting superscript clears subscript
#[test]
fn setting_superscript_clears_subscript() {
    let editor = text_node_editor();
    update(&editor, || {
        let text_node = hello_world()?;
        text_node.toggle_format("subscript");
        text_node.toggle_format("superscript");
        assert!(text_node.has_format("superscript"));
        assert!(!text_node.has_format("subscript"));
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > clearing subscript does not set superscript
#[test]
fn clearing_subscript_does_not_set_superscript() {
    let editor = text_node_editor();
    update(&editor, || {
        let text_node = hello_world()?;
        text_node.toggle_format("subscript");
        text_node.toggle_format("subscript");
        assert!(!text_node.has_format("subscript"));
        assert!(!text_node.has_format("superscript"));
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > clearing superscript does not set subscript
#[test]
fn clearing_superscript_does_not_set_subscript() {
    let editor = text_node_editor();
    update(&editor, || {
        let text_node = hello_world()?;
        text_node.toggle_format("superscript");
        text_node.toggle_format("superscript");
        assert!(!text_node.has_format("superscript"));
        assert!(!text_node.has_format("subscript"));
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > capitalization formats are mutually exclusive
#[test]
fn capitalization_formats_are_mutually_exclusive() {
    let capitalization_formats = ["lowercase", "uppercase", "capitalize"];
    let editor = text_node_editor();
    update(&editor, move || {
        let text_node = hello_world()?;
        for format_to_set in capitalization_formats {
            text_node.toggle_format(format_to_set);
            for format in capitalization_formats
                .iter()
                .filter(|f| **f != format_to_set)
            {
                assert!(!text_node.has_format(format));
            }
            assert!(text_node.has_format(format_to_set));
        }
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > selectPrevious()
#[test]
fn select_previous() {
    let editor = text_node_editor();
    update(&editor, || {
        let paragraph_node = create_paragraph_node();
        let text_node = create_text_node("Hello World");
        let text_node2 = create_text_node("Goodbye Earth");
        paragraph_node.append(&[text_node.clone(), text_node2.clone()])?;
        get_root().append(std::slice::from_ref(&paragraph_node))?;
        let selection = text_node2.select_previous(None, None)?;
        assert_eq!(point(&selection, Which::Anchor), (text_node.clone(), 11));
        assert_eq!(point(&selection, Which::Focus), (text_node.clone(), 11));
        let selection = text_node.select_previous(None, None)?;
        assert_eq!(point(&selection, Which::Anchor), (paragraph_node, 0));
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > selectNext()
#[test]
fn select_next() {
    let editor = text_node_editor();
    update(&editor, || {
        let paragraph_node = create_paragraph_node();
        let text_node = create_text_node("Hello World");
        let text_node2 = create_text_node("Goodbye Earth");
        paragraph_node.append(&[text_node.clone(), text_node2.clone()])?;
        get_root().append(std::slice::from_ref(&paragraph_node))?;
        let selection = text_node.select_next(Some(1), Some(3))?;
        if selection.is_node() {
            return Ok(());
        }
        assert_eq!(point(&selection, Which::Anchor), (text_node2.clone(), 1));
        assert_eq!(point(&selection, Which::Focus), (text_node2.clone(), 3));
        let selection = text_node2.select_next(None, None)?;
        assert_eq!(point(&selection, Which::Anchor), (paragraph_node, 2));
        Ok(())
    });
}

/// `select(...%p)`.
fn select(anchor_offset: Option<usize>, focus_offset: Option<usize>, expected: (usize, usize)) {
    let editor = text_node_editor();
    update(&editor, move || {
        let text_node = hello_world()?;
        let selection = text_node.select(anchor_offset, focus_offset)?;
        assert_eq!(point(&selection, Which::Focus).0, text_node);
        assert_eq!(point(&selection, Which::Anchor).1, expected.0);
        assert_eq!(point(&selection, Which::Focus).0, text_node);
        assert_eq!(point(&selection, Which::Focus).1, expected.1);
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p)
#[test]
fn select_1() {
    select(Some(2), Some(4), (2, 4));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p) #2
#[test]
fn select_2() {
    select(Some(4), Some(2), (4, 2));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p) #3
#[test]
fn select_3() {
    select(None, Some(2), (11, 2));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p) #4
#[test]
fn select_4() {
    select(Some(2), None, (2, 11));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p) #5
#[test]
fn select_5() {
    select(None, None, (11, 11));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > convert segmented node into plain text
#[test]
fn split_text_convert_segmented_node_into_plain_text() {
    let editor = text_node_editor();
    update(&editor, || {
        let segmented_node = create_test_segmented_node("Hello World");
        let paragraph_node = create_paragraph_node();
        paragraph_node.append(std::slice::from_ref(&segmented_node))?;
        let parts = segmented_node.split_text(&[5])?;
        let children = paragraph_node.get_all_text_nodes();
        assert_eq!(paragraph_node.get_text_content(), "Hello World");
        assert!(children[0].is_simple_text());
        assert_eq!(children[0].get_text_content(), "Hello");
        assert_eq!(parts[0], children[0]);
        assert_eq!(parts[1], children[1]);
        Ok(())
    });
}

/// `"%s" splitText(...%p)`.
fn split_text(
    initial_string: &'static str,
    split_offsets: &'static [i64],
    split_strings: &'static [&'static str],
) {
    let editor = text_node_editor();
    update(&editor, move || {
        let paragraph_node = create_paragraph_node();
        let text_node = create_text_node(initial_string);
        paragraph_node.append(std::slice::from_ref(&text_node))?;
        let split_nodes = text_node.split_text(split_offsets)?;
        assert_eq!(paragraph_node.get_children().len(), split_strings.len());
        assert_eq!(
            split_nodes
                .iter()
                .map(NodeRef::get_text_content)
                .collect::<Vec<_>>(),
            split_strings
        );
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "a" splitText(...%p)
#[test]
fn split_text_1() {
    split_text("a", &[], &["a"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "a" splitText(...%p) #2
#[test]
fn split_text_2() {
    split_text("a", &[1], &["a"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "a" splitText(...%p) #3
#[test]
fn split_text_3() {
    split_text("a", &[5], &["a"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p)
#[test]
fn split_text_4() {
    split_text("Hello World", &[], &["Hello World"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) #2
#[test]
fn split_text_5() {
    split_text("Hello World", &[3], &["Hel", "lo World"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) #3
#[test]
fn split_text_6() {
    split_text("Hello World", &[3, 3], &["Hel", "lo World"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) #4
#[test]
fn split_text_7() {
    split_text("Hello World", &[3, 7], &["Hel", "lo W", "orld"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) #5
#[test]
fn split_text_8() {
    split_text("Hello World", &[7, 3], &["Hel", "lo W", "orld"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) #6
#[test]
fn split_text_9() {
    split_text("Hello World", &[3, 7, 99], &["Hel", "lo W", "orld"]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > splitText moves composition key to last node
#[test]
fn split_text_moves_composition_key_to_last_node() {
    let editor = text_node_editor();
    update(&editor, || {
        let paragraph_node = create_paragraph_node();
        let text_node = create_text_node("12345");
        paragraph_node.append(std::slice::from_ref(&text_node))?;
        set_composition_key(Some(text_node.get_key()));
        let parts = text_node.split_text(&[1])?;
        assert_eq!(get_composition_key().as_deref(), Some(parts[1].get_key()));
        Ok(())
    });
}

/// `"%s" splitText(...%p) with select(...%p)`.
fn split_text_with_select(
    initial_string: &'static str,
    split_offsets: &'static [i64],
    selection_offsets: (usize, usize),
    expected: (usize, usize, usize, usize),
) {
    let editor = text_node_editor();
    update(&editor, move || {
        let paragraph_node = create_paragraph_node();
        let text_node = create_text_node(initial_string);
        paragraph_node.append(std::slice::from_ref(&text_node))?;
        get_root().append(&[paragraph_node])?;
        let selection = text_node.select(Some(selection_offsets.0), Some(selection_offsets.1))?;
        let children_nodes = text_node.split_text(split_offsets)?;
        let (anchor_node_index, anchor_offset, focus_node_index, focus_offset) = expected;
        assert_eq!(
            point(&selection, Which::Anchor),
            (children_nodes[anchor_node_index].clone(), anchor_offset)
        );
        assert_eq!(
            point(&selection, Which::Focus),
            (children_nodes[focus_node_index].clone(), focus_offset)
        );
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello" splitText(...%p) with select(...%p)
#[test]
fn split_text_with_select_1() {
    split_text_with_select("Hello", &[4], (3, 3), (0, 3, 0, 3));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello" splitText(...%p) with select(...%p) #2
#[test]
fn split_text_with_select_2() {
    split_text_with_select("Hello", &[4], (5, 5), (1, 1, 1, 1));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) with select(...%p)
#[test]
fn split_text_with_select_3() {
    split_text_with_select("Hello World", &[4], (2, 7), (0, 2, 1, 3));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) with select(...%p) #2
#[test]
fn split_text_with_select_4() {
    split_text_with_select("Hello World", &[4], (2, 4), (0, 2, 0, 4));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) with select(...%p) #3
#[test]
fn split_text_with_select_5() {
    split_text_with_select("Hello World", &[4], (7, 2), (1, 3, 0, 2));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) with select(...%p) #4
#[test]
fn split_text_with_select_6() {
    split_text_with_select("Hello World", &[4, 6], (2, 9), (0, 2, 2, 3));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) with select(...%p) #5
#[test]
fn split_text_with_select_7() {
    split_text_with_select("Hello World", &[4, 6], (9, 2), (2, 3, 0, 2));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > "Hello World" splitText(...%p) with select(...%p) #6
#[test]
fn split_text_with_select_8() {
    split_text_with_select("Hello World", &[4, 6], (9, 9), (2, 3, 2, 3));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > with detached parent
#[test]
fn split_text_with_detached_parent() {
    let editor = text_node_editor();
    update(&editor, || {
        let text_node = create_text_node("foo");
        let splits = text_node.split_text(&[1, 2])?;
        assert_eq!(
            splits
                .iter()
                .map(NodeRef::get_text_content)
                .collect::<Vec<_>>(),
            ["f", "o", "o"]
        );
        Ok(())
    });
}

/// `createState('state', {parse: v => v, unparse: v => v})`, for strings.
fn string_state() -> crate::StateConfig<String> {
    crate::create_state("state", |v| v.as_str().unwrap_or_default().to_owned())
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > copies state to all nodes
#[test]
fn split_text_copies_state_to_all_nodes() {
    let editor = text_node_editor();
    update(&editor, || {
        let text_node = create_text_node("hello world");
        let state = string_state();
        crate::set_state(&text_node, &state, "foo".to_owned())?;
        let splits = text_node.split_text(&[3, 5])?;
        assert!(
            splits
                .iter()
                .all(|s| crate::get_state(s, &state).unwrap() == "foo")
        );
        // Check that the state value is not aliased to the original node.
        crate::set_state(&splits[0], &state, "bar".to_owned())?;
        assert_eq!(crate::get_state(&splits[0], &state)?, "bar");
        assert_eq!(crate::get_state(&splits[1], &state)?, "foo");
        assert_eq!(crate::get_state(&splits[2], &state)?, "foo");
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > copies state to all nodes (segmented)
#[test]
fn split_text_copies_state_to_all_nodes_segmented() {
    let editor = text_node_editor();
    update(&editor, || {
        let text_node = create_test_segmented_node("hello world");
        let state = string_state();
        crate::set_state(&text_node, &state, "foo".to_owned())?;
        let splits = text_node.split_text(&[3, 5])?;
        assert!(
            splits
                .iter()
                .all(|s| crate::get_state(s, &state).unwrap() == "foo")
        );
        Ok(())
    });
}

/// `editorConfig`: the text theme.
#[cfg(feature = "html")]
fn editor_config() -> crate::EditorConfig {
    crate::EditorConfig {
        theme: crate::json::parse(
            r#"{"text":{"bold":"my-bold-class","capitalize":"my-capitalize-class","code":"my-code-class","highlight":"my-highlight-class","italic":"my-italic-class","lowercase":"my-lowercase-class","strikethrough":"my-strikethrough-class","underline":"my-underline-class","underlineStrikethrough":"my-underline-strikethrough-class","uppercase":"my-uppercase-class"}}"#,
        )
        .unwrap()
        .as_object()
        .unwrap()
        .clone(),
    }
}

/// `textNode.createDOM(editorConfig)`.
#[cfg(feature = "html")]
fn create_dom(node: &NodeRef, editor: &Editor) -> crate::dom::DomNode {
    node.get_latest()
        .create_dom(node, &editor_config(), editor)
        .unwrap()
}

/// `%s text format type`.
#[cfg(feature = "html")]
fn create_dom_format(format: i32, contents: &'static str, expected_html: &'static str) {
    let editor = text_node_editor();
    let e = editor.clone();
    update(&editor, move || {
        let text_node = create_text_node(contents);
        text_node.set_format(f64::from(format));
        assert_eq!(create_dom(&text_node, &e).outer_html(), expected_html);
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > no formatting text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_no_formatting() {
    create_dom_format(0, "My text node", r#"<span>My text node</span>"#);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > bold text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_bold() {
    create_dom_format(
        IS_BOLD,
        "My text node",
        r#"<strong class="my-bold-class">My text node</strong>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > bold + empty text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_bold_empty() {
    create_dom_format(IS_BOLD, "", r#"<strong class="my-bold-class"></strong>"#);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > underline text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_underline() {
    create_dom_format(
        IS_UNDERLINE,
        "My text node",
        r#"<span class="my-underline-class">My text node</span>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > strikethrough text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_strikethrough() {
    create_dom_format(
        IS_STRIKETHROUGH,
        "My text node",
        r#"<span class="my-strikethrough-class">My text node</span>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > highlight text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_highlight() {
    create_dom_format(
        IS_HIGHLIGHT,
        "My text node",
        r#"<mark><span class="my-highlight-class">My text node</span></mark>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > italic text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_italic() {
    create_dom_format(
        IS_ITALIC,
        "My text node",
        r#"<em class="my-italic-class">My text node</em>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_code() {
    create_dom_format(
        IS_CODE,
        "My text node",
        r#"<code spellcheck="false"><span class="my-code-class">My text node</span></code>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > lowercase text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_lowercase() {
    create_dom_format(
        IS_LOWERCASE,
        "My text node",
        r#"<span class="my-lowercase-class">My text node</span>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > uppercase text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_uppercase() {
    create_dom_format(
        IS_UPPERCASE,
        "My text node",
        r#"<span class="my-uppercase-class">My text node</span>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > capitalize text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_capitalize() {
    create_dom_format(
        IS_CAPITALIZE,
        "My text node",
        r#"<span class="my-capitalize-class">My text node</span>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > underline + strikethrough text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_underline_strikethrough() {
    create_dom_format(
        IS_UNDERLINE | IS_STRIKETHROUGH,
        "My text node",
        r#"<span class="my-underline-strikethrough-class">My text node</span>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code + italic text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_code_italic() {
    create_dom_format(
        IS_CODE | IS_ITALIC,
        "My text node",
        r#"<code spellcheck="false"><em class="my-code-class my-italic-class">My text node</em></code>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code + underline + strikethrough text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_code_underline_strikethrough() {
    create_dom_format(
        IS_CODE | IS_UNDERLINE | IS_STRIKETHROUGH,
        "My text node",
        r#"<code spellcheck="false"><span class="my-underline-strikethrough-class my-code-class">My text node</span></code>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > highlight + italic text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_highlight_italic() {
    create_dom_format(
        IS_HIGHLIGHT | IS_ITALIC,
        "My text node",
        r#"<mark><em class="my-highlight-class my-italic-class">My text node</em></mark>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code + underline + strikethrough + bold + italic text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_code_underline_strikethrough_bold_italic() {
    create_dom_format(
        IS_CODE | IS_UNDERLINE | IS_STRIKETHROUGH | IS_BOLD | IS_ITALIC,
        "My text node",
        r#"<code spellcheck="false"><strong class="my-underline-strikethrough-class my-bold-class my-code-class my-italic-class">My text node</strong></code>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code + underline + strikethrough + bold + italic + highlight + uppercase text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_code_underline_strikethrough_bold_italic_highlight_uppercase() {
    create_dom_format(
        IS_CODE
            | IS_UNDERLINE
            | IS_STRIKETHROUGH
            | IS_BOLD
            | IS_ITALIC
            | IS_HIGHLIGHT
            | IS_UPPERCASE,
        "My text node",
        r#"<code spellcheck="false"><strong class="my-underline-strikethrough-class my-bold-class my-code-class my-highlight-class my-italic-class my-uppercase-class">My text node</strong></code>"#,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > applies styles with direct DOM property updates
#[cfg(feature = "html")]
#[test]
fn create_dom_applies_styles_with_direct_dom_property_updates() {
    let editor = text_node_editor();
    let e = editor.clone();
    update(&editor, move || {
        let text_node = create_text_node("My text node");
        text_node.set_style("color: red; background-color: blue !important; --custom: value;");
        let element = create_dom(&text_node, &e);
        let style = element.style();
        assert_eq!(style.get_property_value("color"), "red");
        assert_eq!(style.get_property_value("background-color"), "blue");
        assert_eq!(style.get_property_priority("background-color"), "important");
        assert_eq!(style.get_property_value("--custom"), "value");
        Ok(())
    });
}

/// `has parent node > %s text format type`.
#[cfg(feature = "html")]
fn create_dom_with_parent(contents: &'static str, expected_html: &'static str) {
    let editor = text_node_editor();
    let e = editor.clone();
    update(&editor, move || {
        let paragraph_node = create_paragraph_node();
        let text_node = create_text_node(contents);
        text_node.set_format(0.0);
        paragraph_node.append(std::slice::from_ref(&text_node))?;
        assert_eq!(create_dom(&text_node, &e).outer_html(), expected_html);
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > has parent node > no formatting text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_has_parent_node_no_formatting() {
    create_dom_with_parent("My text node", "<span>My text node</span>");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > has parent node > no formatting + empty string text format type
#[cfg(feature = "html")]
#[test]
fn create_dom_has_parent_node_no_formatting_empty_string() {
    create_dom_with_parent("", "<span></span>");
}

/// `updateDOM()`'s `%s`: Rust's `update_dom` is the model half
/// (rust/PARITY.md), which says whether the DOM is made again; where it
/// isn't and the text is the same, the element is left as it was.
#[cfg(feature = "html")]
fn update_dom(
    prev: (i32, &'static str),
    next: (i32, &'static str),
    result: bool,
    expected_html: Option<&'static str>,
) {
    let editor = text_node_editor();
    let e = editor.clone();
    update(&editor, move || {
        let prev_text_node = create_text_node(prev.1);
        prev_text_node.set_mode("normal")?;
        prev_text_node.set_format(f64::from(prev.0));
        let element = create_dom(&prev_text_node, &e);
        let text_node = create_text_node(next.1);
        text_node.set_mode("normal")?;
        text_node.set_format(f64::from(next.0));
        assert_eq!(
            text_node
                .get_latest()
                .update_dom(&*prev_text_node.get_latest()),
            result
        );
        if !result {
            assert_eq!(Some(element.outer_html().as_str()), expected_html);
        }
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > updateDOM() > different tags
#[cfg(feature = "html")]
#[test]
fn update_dom_different_tags() {
    update_dom(
        (IS_BOLD, "My text node"),
        (IS_ITALIC, "My text node"),
        true,
        None,
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > updateDOM() > no change in tags
#[cfg(feature = "html")]
#[test]
fn update_dom_no_change_in_tags() {
    update_dom(
        (IS_BOLD, "My text node"),
        (IS_BOLD, "My text node"),
        false,
        Some(r#"<strong class="my-bold-class">My text node</strong>"#),
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > updateDOM() > removing code block
#[cfg(feature = "html")]
#[test]
fn update_dom_removing_code_block() {
    update_dom(
        (IS_CODE | IS_BOLD, "My text node"),
        (IS_BOLD, "My new text node"),
        true,
        None,
    );
}

/// `exportDOM()`'s `%s`: `textNode.exportDOM(editor)`.
#[cfg(feature = "html")]
fn export_dom(format: i32, element_check: fn(&crate::dom::DomNode)) {
    let editor = text_node_editor();
    let e = editor.clone();
    update(&editor, move || {
        let text_node = create_text_node("Sample Text");
        text_node.set_format(f64::from(format));
        let element = text_node.get_latest().export_dom(&text_node, &e)?.element;
        element_check(&element.expect("an element"));
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportDOM() > lowercase format
#[cfg(feature = "html")]
#[test]
fn export_dom_lowercase_format() {
    export_dom(IS_LOWERCASE, |e| {
        assert_eq!(e.style().get_property_value("text-transform"), "lowercase")
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportDOM() > uppercase format
#[cfg(feature = "html")]
#[test]
fn export_dom_uppercase_format() {
    export_dom(IS_UPPERCASE, |e| {
        assert_eq!(e.style().get_property_value("text-transform"), "uppercase")
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportDOM() > capitalize format
#[cfg(feature = "html")]
#[test]
fn export_dom_capitalize_format() {
    export_dom(IS_CAPITALIZE, |e| {
        assert_eq!(e.style().get_property_value("text-transform"), "capitalize")
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportDOM() > combined bold and lowercase format
#[cfg(feature = "html")]
#[test]
fn export_dom_combined_bold_and_lowercase_format() {
    export_dom(IS_BOLD | IS_LOWERCASE, |e| {
        assert_eq!(e.tag_name().to_lowercase(), "b");
        let child_element = e.first_child().unwrap();
        assert_eq!(
            child_element.style().get_property_value("text-transform"),
            "lowercase"
        );
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > mergeWithSibling
#[test]
fn merge_with_sibling() {
    let editor = text_node_editor();
    update(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        assert!(paragraph.is_element());
        let text_node1 = create_text_node("1");
        let text_node2 = create_text_node("2");
        let text_node3 = create_text_node("3");
        paragraph.append(&[text_node1.clone(), text_node2.clone(), text_node3.clone()])?;
        text_node2.select(None, None)?;
        let selection = get_selection().unwrap();
        text_node2.merge_with_sibling(&text_node1)?;
        if !selection.is_range() {
            return Ok(());
        }
        assert_eq!(point(&selection, Which::Anchor), (text_node2.clone(), 1));
        assert_eq!(point(&selection, Which::Focus).1, 1);
        text_node2.merge_with_sibling(&text_node3)?;
        assert_eq!(point(&selection, Which::Anchor), (text_node2.clone(), 1));
        assert_eq!(point(&selection, Which::Focus).1, 1);
        Ok(())
    });
    assert_eq!(editor_state_text_content(&editor), "123");
}
