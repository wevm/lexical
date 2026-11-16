//! Ports of `packages/lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx`.
//!
//! Upstream reads the mounted editor's HTML after each update; Rust has no
//! DOM reconciler (rust/PARITY.md), so these read the committed document it
//! renders: its [`shape`], and the indent and direction the HTML shows as
//! `padding-inline-start` and `dir`.

use crate::nodes::tab::{TabNode, create_tab_node};
use crate::upstream_tests::support::{initialize_unit_test, shape, update};
use crate::{
    CaretDirection, CreateEditorArgs, Editor, NodeRef, create_paragraph_node, get_caret_range,
    get_caret_range_in_direction, get_root, get_selection, get_text_point_caret, insert_nodes,
    select_all, set_selection_from_caret_range,
};

/// `beforeEach`: an empty, selected paragraph.
fn tab_node_editor() -> Editor {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select(None, None)?;
        Ok(())
    });
    editor
}

fn is_tab_node(node: &NodeRef) -> bool {
    node.get_latest().downcast_ref::<TabNode>().is_some()
}

#[cfg(feature = "clipboard")]
fn data_transfer(format: &str, data: &str) -> std::rc::Rc<crate::DataTransfer> {
    let data_transfer = crate::DataTransfer::new();
    data_transfer.set_data(format, data);
    std::rc::Rc::new(data_transfer)
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can paste plain text with tabs and newlines in plain text
#[cfg(feature = "clipboard")]
#[test]
fn can_paste_plain_text_with_tabs_and_newlines_in_plain_text() {
    let editor = tab_node_editor();
    let data = data_transfer("text/plain", "hello\tworld\nhello\tworld");
    update(&editor, move || {
        let selection = get_selection()
            .filter(|s| s.is_range())
            .expect("isRangeSelection(selection)");
        crate::clipboard::insert_data_transfer_for_plain_text(&data, &selection)
    });
    assert_eq!(
        shape(&editor),
        "<paragraph>[hello][\t][world]<br>[hello][\t][world]</paragraph>"
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can paste plain text with tabs and newlines in rich text
#[cfg(feature = "clipboard")]
#[test]
fn can_paste_plain_text_with_tabs_and_newlines_in_rich_text() {
    let editor = tab_node_editor();
    let data = data_transfer("text/plain", "hello\tworld\nhello\tworld");
    let e = editor.clone();
    update(&editor, move || {
        let selection = get_selection()
            .filter(|s| s.is_range())
            .expect("isRangeSelection(selection)");
        crate::clipboard::insert_data_transfer_for_rich_text(&data, &selection, &e)
    });
    assert_eq!(
        shape(&editor),
        "<paragraph>[hello][\t][world]</paragraph><paragraph>[hello][\t][world]</paragraph>"
    );
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can paste HTML with tabs and new lines (2)
#[cfg(all(feature = "clipboard", feature = "html"))]
#[test]
fn can_paste_html_with_tabs_and_new_lines_2() {
    let editor = tab_node_editor();
    let span = r#"<span style="font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">"#;
    let tab = r#"<span class="Apple-tab-span" style="white-space:pre;">	</span>"#;
    let html = format!(
        r#"<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-123"><p dir="ltr" style="line-height:1.38;margin-left: 36pt;margin-top:0pt;margin-bottom:0pt;">{span}Hello</span>{span}{tab}</span>{span}world</span></p>{span}Hello</span>{span}{tab}</span>{span}world</span></b>"#
    );
    let data = data_transfer("text/html", &html);
    let e = editor.clone();
    update(&editor, move || {
        let selection = get_selection()
            .filter(|s| s.is_range())
            .expect("isRangeSelection(selection)");
        crate::clipboard::insert_data_transfer_for_rich_text(&data, &selection, &e)
    });
    assert_eq!(
        shape(&editor),
        "<paragraph>[Hello][\t][world]</paragraph><paragraph>[Hello][\t][world]</paragraph>"
    );
    // `<p dir="ltr">` then `<p dir="auto">`.
    let directions = editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|p| p.element_data().get_direction().map(|d| d.as_str()))
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(directions, [Some("ltr"), None]);
}

/// `registerRichText(editor)`, `registerTabIndentation(editor)`, `setup`,
/// then Tab.
#[cfg(all(feature = "rich-text", feature = "extension"))]
fn tab(setup: impl FnOnce() -> Result<(), crate::LexicalError> + 'static) -> Editor {
    use crate::extension::{
        CanIndentPredicate, default_can_indent, register_tab_indentation, signal,
    };
    let editor = tab_node_editor();
    let _ = crate::rich_text::register_rich_text(&editor);
    let _ = register_tab_indentation(
        &editor,
        signal(None).as_readonly(),
        signal(CanIndentPredicate(std::rc::Rc::new(default_can_indent))).as_readonly(),
    );
    update(&editor, setup);
    editor
        .dispatch_command(
            &crate::KEY_TAB_COMMAND,
            std::rc::Rc::new(crate::KeyboardEvent::default()),
        )
        .unwrap();
    editor.read(|| ()).unwrap();
    editor
}

/// Each top-level block's indent.
#[cfg(all(feature = "rich-text", feature = "extension"))]
fn indents(editor: &Editor) -> Vec<f64> {
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|b| b.get_indent().unwrap())
                .collect()
        })
        .unwrap()
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > element indents when selection at the start of the block
#[cfg(all(feature = "rich-text", feature = "extension"))]
#[test]
fn element_indents_when_selection_at_the_start_of_the_block() {
    let editor = tab(|| {
        get_selection().unwrap().insert_text("foo")?;
        get_root().select_start()?;
        Ok(())
    });
    assert_eq!(shape(&editor), "<paragraph>[foo]</paragraph>");
    assert_eq!(indents(&editor), [1.0]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > elements indent when selection spans across multiple blocks
#[cfg(all(feature = "rich-text", feature = "extension", feature = "list"))]
#[test]
fn elements_indent_when_selection_spans_across_multiple_blocks() {
    use crate::list::{ListType, create_list_item_node, create_list_node};
    use crate::rich_text::{HeadingTagType, create_heading_node};
    use crate::{PointType, Which, create_range_selection, create_text_node, set_selection};
    let editor = tab(|| {
        let root = get_root();
        let paragraph = root.get_first_child().unwrap();
        assert!(paragraph.is_element());
        let heading = create_heading_node(HeadingTagType::H1);
        let list = create_list_node(ListType::Number, 1.0);
        let list_item = create_list_item_node(None);
        let paragraph_text = create_text_node("foo");
        let heading_text = create_text_node("bar");
        let list_item_text = create_text_node("xyz");
        root.append(&[heading.clone(), list.clone()])?;
        paragraph.append(std::slice::from_ref(&paragraph_text))?;
        heading.append(&[heading_text])?;
        list.append(std::slice::from_ref(&list_item))?;
        list_item.append(std::slice::from_ref(&list_item_text))?;
        let selection = create_range_selection();
        selection.point_set(Which::Focus, paragraph_text.get_key(), 1, PointType::Text)?;
        selection.point_set(Which::Anchor, list_item_text.get_key(), 1, PointType::Text)?;
        set_selection(Some(selection));
        Ok(())
    });
    assert_eq!(
        shape(&editor),
        "<paragraph>[foo]</paragraph><heading>[bar]</heading><list><listitem><list><listitem>[xyz]</listitem></list></listitem></list>"
    );
    // The list item is indented by nesting it, which leaves the list at 0.
    assert_eq!(indents(&editor), [1.0, 1.0, 0.0]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > element tabs when selection is not at the start (1)
#[cfg(all(feature = "rich-text", feature = "extension"))]
#[test]
fn element_tabs_when_selection_is_not_at_the_start_1() {
    let editor = tab(|| get_selection().unwrap().insert_text("foo"));
    assert_eq!(shape(&editor), "<paragraph>[foo][\t]</paragraph>");
    assert_eq!(indents(&editor), [0.0]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > element tabs when selection is not at the start (2)
#[cfg(all(feature = "rich-text", feature = "extension"))]
#[test]
fn element_tabs_when_selection_is_not_at_the_start_2() {
    let editor = tab(|| {
        get_selection().unwrap().insert_text("foo")?;
        let text_node = get_root().get_last_descendant().unwrap();
        assert!(text_node.is_text());
        text_node.select(Some(1), Some(1))?;
        Ok(())
    });
    assert_eq!(shape(&editor), "<paragraph>[f][\t][oo]</paragraph>");
    assert_eq!(indents(&editor), [0.0]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > element tabs when selection is not at the start (3)
#[cfg(all(feature = "rich-text", feature = "extension"))]
#[test]
fn element_tabs_when_selection_is_not_at_the_start_3() {
    let editor = tab(|| {
        get_selection().unwrap().insert_text("foo")?;
        let text_node = get_root().get_last_descendant().unwrap();
        assert!(text_node.is_text());
        text_node.select(Some(1), Some(2))?;
        Ok(())
    });
    assert_eq!(shape(&editor), "<paragraph>[f][\t][o]</paragraph>");
    assert_eq!(indents(&editor), [0.0]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > elements tabs when selection is not at the start and overlaps another tab
#[cfg(all(feature = "rich-text", feature = "extension"))]
#[test]
fn elements_tabs_when_selection_is_not_at_the_start_and_overlaps_another_tab() {
    use crate::{PointType, Which, create_range_selection, set_selection};
    let editor = tab(|| {
        get_selection().unwrap().insert_raw_text("hello\tworld")?;
        let root = get_root();
        let first_text_node = root.get_first_descendant().unwrap();
        let last_text_node = root.get_last_descendant().unwrap();
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, first_text_node.get_key(), 4, PointType::Text)?;
        selection.point_set(Which::Focus, last_text_node.get_key(), 2, PointType::Text)?;
        set_selection(Some(selection));
        Ok(())
    });
    assert_eq!(shape(&editor), "<paragraph>[hell][\t][rld]</paragraph>");
    assert_eq!(indents(&editor), [0.0]);
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can type between two (leaf nodes) canInsertBeforeAfter false
#[test]
fn can_type_between_two_leaf_nodes_can_insert_before_after_false() {
    let editor = tab_node_editor();
    update(&editor, || {
        let tab1 = create_tab_node();
        let tab2 = create_tab_node();
        insert_nodes(vec![tab1.clone(), tab2])?;
        tab1.select(Some(1), Some(1))?;
        get_selection().unwrap().insert_text("f")
    });
    assert_eq!(shape(&editor), "<paragraph>[\t][f][\t]</paragraph>");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can be serialized and deserialized
#[test]
fn can_be_serialized_and_deserialized() {
    let editor = tab_node_editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_tab_node()])?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        let text_nodes = get_root().get_all_text_nodes();
        assert_eq!(text_nodes.len(), 1);
        assert!(is_tab_node(&text_nodes[0]));
        Ok(())
    });
    let json = editor.get_editor_state().to_json(false);
    update(&editor, || {
        get_root().clear()?;
        get_root().append(&[create_paragraph_node()])
    });
    editor
        .read(|| assert!(get_root().get_all_text_nodes().is_empty()))
        .unwrap();
    let state = editor.parse_editor_state_json(&json).unwrap();
    editor.set_editor_state(&state, None).unwrap();
    editor
        .read(|| {
            let text_nodes = get_root().get_all_text_nodes();
            assert_eq!(text_nodes.len(), 1);
            assert!(is_tab_node(&text_nodes[0]));
        })
        .unwrap();
}

/// `TabNode "x\tx" to <output> at <start> (<direction>)`.
fn tab_node_at_selection_boundaries(direction: CaretDirection, start: usize, output: &str) {
    let editor = tab_node_editor();
    update(&editor, move || {
        select_all()?.insert_raw_text("x\tx")?;
        let text_nodes = get_root().get_all_text_nodes();
        assert_eq!(
            text_nodes.iter().map(is_tab_node).collect::<Vec<_>>(),
            [false, true, false]
        );
        let caret_range = get_caret_range(
            get_text_point_caret(&text_nodes[start], CaretDirection::Next, 0)?,
            get_text_point_caret(
                &text_nodes[start + 1],
                CaretDirection::Next,
                CaretDirection::Next,
            )?,
        )?;
        let range = set_selection_from_caret_range(&get_caret_range_in_direction(
            &caret_range,
            direction,
        )?)?;
        range.insert_raw_text("y")
    });
    // Using read here because we want to see the post-normalization merged state.
    editor
        .read(|| {
            let expect_nodes = get_root().get_all_text_nodes();
            assert_eq!(
                expect_nodes
                    .iter()
                    .map(NodeRef::get_text_content)
                    .collect::<Vec<_>>(),
                [output]
            );
            assert_eq!(
                expect_nodes.iter().map(is_tab_node).collect::<Vec<_>>(),
                [false]
            );
        })
        .unwrap();
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > TabNode at selection boundaries with normal TextNode sibling (#7602) > TabNode "x\tx" to "yx" at 0 (next)
#[test]
fn tab_node_x_tab_x_to_yx_at_0_next() {
    tab_node_at_selection_boundaries(CaretDirection::Next, 0, "yx");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > TabNode at selection boundaries with normal TextNode sibling (#7602) > TabNode "x\tx" to "xy" at 1 (next)
#[test]
fn tab_node_x_tab_x_to_xy_at_1_next() {
    tab_node_at_selection_boundaries(CaretDirection::Next, 1, "xy");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > TabNode at selection boundaries with normal TextNode sibling (#7602) > TabNode "x\tx" to "yx" at 0 (previous)
#[test]
fn tab_node_x_tab_x_to_yx_at_0_previous() {
    tab_node_at_selection_boundaries(CaretDirection::Previous, 0, "yx");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > TabNode at selection boundaries with normal TextNode sibling (#7602) > TabNode "x\tx" to "xy" at 1 (previous)
#[test]
fn tab_node_x_tab_x_to_xy_at_1_previous() {
    tab_node_at_selection_boundaries(CaretDirection::Previous, 1, "xy");
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > setTextContent normalizes back to \t without throwing (#8596)
#[test]
fn set_text_content_normalizes_back_to_tab_without_throwing() {
    let editor = tab_node_editor();
    update(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        assert!(paragraph.is_element());
        let tab_node = create_tab_node();
        paragraph.append(std::slice::from_ref(&tab_node))?;
        for text in ["\tㅁ", "arbitrary", ""] {
            tab_node.set_text_content(text);
            assert_eq!(tab_node.get_text_content(), "\t");
        }
        Ok(())
    });
}
