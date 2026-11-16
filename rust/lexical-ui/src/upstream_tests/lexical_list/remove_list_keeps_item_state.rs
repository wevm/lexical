//! Ports of `packages/lexical-list/src/__tests__/unit/RemoveListKeepsItemState.test.ts`.

use super::support::{build, item, list, text_item, update_discrete};
use crate::extension::LexicalEditorWithDispose;
use crate::list::{ListType, create_list_item_node, insert_list, list_extension, remove_list};
use crate::{Direction, create_paragraph_node, create_text_node, get_root, select_all};

fn build_editor() -> LexicalEditorWithDispose {
    build("remove-list-state-host", vec![list_extension().into()])
}

/// `{direction, format, indent, style, text}` of each paragraph.
type State = (Option<Direction>, &'static str, f64, String, String);

fn paragraph_state() -> Vec<State> {
    get_root()
        .get_children()
        .iter()
        .filter(|p| p.get_type() == "paragraph")
        .map(|p| {
            let data = p.element_data();
            (
                data.get_direction(),
                data.get_format_type(),
                p.get_indent().unwrap(),
                data.get_style().to_owned(),
                p.get_text_content(),
            )
        })
        .collect()
}

// upstream: lexical-list/src/__tests__/unit/RemoveListKeepsItemState.test.ts::$removeList keeps the list item state > the paragraphs keep the items format, indent, direction and style
#[test]
fn the_paragraphs_keep_the_items_format_indent_direction_and_style() {
    let editor = build_editor();
    update_discrete(&editor, || {
        let outer = create_list_item_node(None);
        outer.set_element_format("center");
        outer.set_direction(Some(Direction::Rtl));
        outer.set_element_style("color: red;");
        outer.append(&[create_text_node("a")])?;
        // A nested item renders one level in; getIndent() derives that from
        // the nesting, so the paragraph has to carry it to look the same.
        let wrapper = item(&[list(ListType::Bullet, &[text_item("b")])]);
        let list = list(ListType::Bullet, &[outer, wrapper]);
        get_root().clear()?;
        get_root().append(&[list])
    });
    update_discrete(&editor, || {
        select_all()?;
        remove_list()
    });
    editor
        .read(|| {
            assert_eq!(
                paragraph_state(),
                [
                    (
                        Some(Direction::Rtl),
                        "center",
                        0.0,
                        "color: red;".to_owned(),
                        "a".to_owned()
                    ),
                    (None, "", 1.0, String::new(), "b".to_owned()),
                ]
            );
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/RemoveListKeepsItemState.test.ts::$removeList keeps the list item state > a formatted paragraph round-trips through a list
#[test]
fn a_formatted_paragraph_round_trips_through_a_list() {
    let editor = build_editor();
    update_discrete(&editor, || {
        // $createListOrMerge copies the block's format and indent onto the
        // list item it creates, so removing the list again has to give them
        // back.
        let paragraph = create_paragraph_node();
        paragraph.set_element_format("right");
        paragraph.set_indent(2.0)?;
        paragraph.append(&[create_text_node("x")])?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
    update_discrete(&editor, || {
        select_all()?;
        insert_list(ListType::Bullet)
    });
    update_discrete(&editor, || {
        select_all()?;
        remove_list()
    });
    editor
        .read(|| {
            assert_eq!(
                paragraph_state(),
                [(None, "right", 2.0, String::new(), "x".to_owned())]
            );
        })
        .unwrap();
    editor.dispose();
}
