//! Ports of `packages/lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts`.

use super::support::{build, discrete};
use crate::extension::{LexicalEditorWithDispose, node_structure_extension};
use crate::json::{Object, Value};
use crate::list::{
    ListType, create_list_item_node, create_list_node, is_list_item_node, is_list_node,
    list_extension,
};
use crate::nodes::line_break::create_line_break_node;
use crate::rich_text::rich_text_extension;
use crate::{
    Editor, LexicalError, NodeRef, OUTDENT_CONTENT_COMMAND, PointType, UpdateOptions, Which,
    create_paragraph_node, create_range_selection, create_text_node, get_root, get_selection,
    set_selection,
};

fn build_editor(structure: bool) -> LexicalEditorWithDispose {
    let mut dependencies = vec![rich_text_extension().into(), list_extension().into()];
    if structure {
        dependencies.push(node_structure_extension().into());
    }
    build("nested-list-item-structure", dependencies)
}

/// `$shape(node)`: lists as `{listType: items}`, items as arrays of their
/// children.
fn shape_of(node: &NodeRef) -> Value {
    let children =
        |node: &NodeRef| Value::Array(node.get_children().iter().map(shape_of).collect());
    let keyed = |key: &str, value: Value| {
        let mut object = Object::new();
        object.insert(key, value);
        Value::Object(object)
    };
    if is_list_node(node) {
        keyed(node.get_list_type().unwrap().as_str(), children(node))
    } else if is_list_item_node(node) {
        children(node)
    } else if node.get_type() == "linebreak" {
        Value::String("\n".to_owned())
    } else if node.is_element() {
        keyed(node.get_type(), children(node))
    } else {
        Value::String(node.get_text_content())
    }
}

/// `shape(editor)`, as JSON.
fn shape(editor: &Editor) -> String {
    editor
        .read(|| Value::Array(get_root().get_children().iter().map(shape_of).collect()).to_string())
        .unwrap()
}

/// `expect(shape(editor)).toEqual(expected)`, with `expected` as JSON.
fn assert_shape(editor: &Editor, expected: &str) {
    assert_eq!(
        shape(editor),
        crate::json::parse(expected).unwrap().to_string()
    );
}

/// `$text(text)`: the text node holding `text`.
fn text(text: &str) -> NodeRef {
    get_root()
        .get_all_text_nodes()
        .into_iter()
        .find(|t| t.get_text_content() == text)
        .unwrap_or_else(|| panic!("no text node {text:?}"))
}

fn item(children: &[NodeRef]) -> NodeRef {
    let item = create_list_item_node(None);
    item.append(children).unwrap();
    item
}

fn text_item(t: &str) -> NodeRef {
    item(&[create_text_node(t)])
}

fn list(list_type: ListType, items: &[NodeRef]) -> NodeRef {
    let list = create_list_node(list_type, 1.0);
    list.append(items).unwrap();
    list
}

fn set_root(children: &[NodeRef]) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(children)
}

/// `$strayAfterNestedList()`: bullet [ "top", [ number ["one", "two",
/// "three"], "stray" ] ].
fn stray_after_nested_list() -> Result<(), LexicalError> {
    set_root(&[list(
        ListType::Bullet,
        &[
            text_item("top"),
            item(&[
                list(
                    ListType::Number,
                    &[text_item("one"), text_item("two"), text_item("three")],
                ),
                create_text_node("stray"),
            ]),
        ],
    )])
}

fn update_discrete(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

fn outdent(editor: &Editor, t: &str) {
    let inner = editor.clone();
    let t = t.to_owned();
    update_discrete(editor, move || {
        text(&t).select(Some(1), Some(1))?;
        inner.dispatch_command(&OUTDENT_CONTENT_COMMAND, ())?;
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, content after the nested list moves into a following item
#[test]
fn content_after_the_nested_list_moves_into_a_following_item() {
    let editor = build_editor(true);
    update_discrete(&editor, stray_after_nested_list);
    assert_shape(
        &editor,
        r#"[{"bullet":[["top"],[{"number":[["one"],["two"],["three"]]}],["stray"]]}]"#,
    );
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, content before the nested list stays in the item and the list moves into a following one
#[test]
fn content_before_the_nested_list_stays_in_the_item() {
    let editor = build_editor(true);
    update_discrete(&editor, || {
        set_root(&[list(
            ListType::Number,
            &[
                item(&[
                    create_text_node("before"),
                    list(ListType::Bullet, &[text_item("inner")]),
                ]),
                text_item("next"),
            ],
        )])
    });
    assert_shape(
        &editor,
        r#"[{"number":[["before"],[{"bullet":[["inner"]]}],["next"]]}]"#,
    );
    let values = editor
        .read(|| {
            get_root()
                .get_first_child_or_throw()
                .unwrap()
                .get_children()
                .iter()
                .map(|item| item.get_list_item_value().unwrap())
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(values, [1.0, 2.0, 2.0]);
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, two nested lists and the text between them each get their own item, in order
#[test]
fn two_nested_lists_and_the_text_between_them_each_get_their_own_item() {
    let editor = build_editor(true);
    update_discrete(&editor, || {
        set_root(&[list(
            ListType::Bullet,
            &[item(&[
                create_text_node("a"),
                list(ListType::Number, &[text_item("one")]),
                create_text_node("b"),
                create_line_break_node(),
                create_text_node("c"),
                list(ListType::Bullet, &[text_item("two")]),
            ])],
        )])
    });
    assert_shape(
        &editor,
        r#"[{"bullet":[["a"],[{"number":[["one"]]}],["b","\n","c"],[{"bullet":[["two"]]}]]}]"#,
    );
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, a later edit that adds content next to a nested list (or a list next to content) is repaired
#[test]
fn a_later_edit_that_adds_content_next_to_a_nested_list_is_repaired() {
    let editor = build_editor(true);
    update_discrete(&editor, || {
        set_root(&[list(
            ListType::Bullet,
            &[
                text_item("text"),
                item(&[list(ListType::Bullet, &[text_item("inner")])]),
            ],
        )])
    });
    update_discrete(&editor, || {
        let list_node = get_root().get_first_child_or_throw()?;
        assert!(is_list_node(&list_node));
        let last = list_node.get_last_child_or_throw()?;
        assert!(is_list_item_node(&last));
        last.append(&[create_text_node("x")])?;
        let first = list_node.get_first_child_or_throw()?;
        assert!(is_list_item_node(&first));
        first.append(&[list(ListType::Number, &[text_item("n")])])
    });
    assert_shape(
        &editor,
        r#"[{"bullet":[["text"],[{"number":[["n"]]}],[{"bullet":[["inner"]]}],["x"]]}]"#,
    );
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, a check state stays on the item that keeps it, and moved content starts unchecked
#[test]
fn a_check_state_stays_on_the_item_that_keeps_it() {
    let editor = build_editor(true);
    update_discrete(&editor, || {
        let checked = create_list_item_node(Some(true));
        checked.append(&[
            create_text_node("done"),
            list(ListType::Bullet, &[text_item("inner")]),
            create_text_node("after"),
        ])?;
        set_root(&[list(ListType::Check, &[checked])])
    });
    let items = editor
        .read(|| {
            get_root()
                .get_first_child_or_throw()
                .unwrap()
                .get_children()
                .iter()
                .map(|item| (item.get_text_content(), item.get_checked()))
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(
        items,
        [
            ("done".to_owned(), Some(true)),
            ("inner".to_owned(), Some(false)),
            ("after".to_owned(), Some(false)),
        ]
    );
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, an element point in the item follows the content it was next to
#[test]
fn an_element_point_in_the_item_follows_the_content_it_was_next_to() {
    let editor = build_editor(true);
    update_discrete(&editor, || {
        let wrapper = item(&[
            list(ListType::Number, &[text_item("one")]),
            create_text_node("stray"),
        ]);
        set_root(&[list(ListType::Bullet, std::slice::from_ref(&wrapper))])?;
        let selection = create_range_selection();
        // Before "stray" to the end of the item.
        selection.point_set(Which::Anchor, wrapper.get_key(), 1, PointType::Element)?;
        selection.point_set(Which::Focus, wrapper.get_key(), 2, PointType::Element)?;
        set_selection(Some(selection));
        Ok(())
    });
    editor
        .read(|| {
            let selection = get_selection().unwrap();
            assert!(selection.is_range());
            assert_eq!(selection.get_text_content().unwrap(), "stray");
            let anchor = selection.point(Which::Anchor).unwrap();
            assert_eq!(
                (anchor.get_node().unwrap(), anchor.offset),
                (text("stray"), 0)
            );
            let focus = selection.point(Which::Focus).unwrap();
            assert_eq!(
                (focus.get_node().unwrap(), focus.offset),
                (text("stray"), "stray".len())
            );
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, a shape the document already holds is kept by an unrelated edit
#[test]
fn a_shape_the_document_already_holds_is_kept_by_an_unrelated_edit() {
    let editor = build_editor(true);
    // A document stored that way (written without the normalization).
    editor
        .update(
            stray_after_nested_list,
            UpdateOptions {
                discrete: true,
                skip_transforms: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    // Typing elsewhere in the list, and in the item that holds the shape.
    for (t, offset) in [("top", 3), ("stray", 5), ("two", 3)] {
        update_discrete(&editor, move || {
            text(t).select(Some(offset), Some(offset))?;
            get_selection().unwrap().insert_text("!")
        });
    }
    assert_shape(
        &editor,
        r#"[{"bullet":[["top!"],[{"number":[["one"],["two!"],["three"]]},"stray!"]]}]"#,
    );
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > without NodeStructureExtension, the shape is kept as it is
#[test]
fn without_node_structure_extension_the_shape_is_kept_as_it_is() {
    let editor = build_editor(false);
    update_discrete(&editor, stray_after_nested_list);
    assert_shape(
        &editor,
        r#"[{"bullet":[["top"],[{"number":[["one"],["two"],["three"]]},"stray"]]}]"#,
    );
}

/// The paste-then-outdent case, with and without `NodeStructureExtension`.
fn paste_then_outdent(structure: bool) {
    let editor = build_editor(structure);
    update_discrete(&editor, || {
        set_root(&[list(
            ListType::Bullet,
            &[
                text_item("one"),
                item(&[list(
                    ListType::Bullet,
                    &[text_item("two"), text_item("three")],
                )]),
            ],
        )])
    });
    let inner = editor.clone();
    update_discrete(&editor, move || {
        text("three").select(Some(5), Some(5))?;
        let p1 = create_paragraph_node();
        p1.append(&[create_text_node("pasted one")])?;
        let p2 = create_paragraph_node();
        p2.append(&[create_text_node("pasted two")])?;
        crate::clipboard::insert_generated_nodes(&inner, vec![p1, p2], &get_selection().unwrap())
    });
    assert_shape(
        &editor,
        if structure {
            r#"[{"bullet":[["one"],[{"bullet":[["two"],["threepasted one"]]}],["pasted two"]]}]"#
        } else {
            r#"[{"bullet":[["one"],[{"bullet":[["two"],["threepasted one"]]},{"paragraph":["pasted two"]}]]}]"#
        },
    );
    // The caret stays at the end of the pasted text.
    editor
        .read(|| {
            let selection = get_selection().unwrap();
            assert!(selection.is_range() && selection.range().unwrap().is_collapsed());
            let anchor = selection.point(Which::Anchor).unwrap();
            assert_eq!(anchor.get_node().unwrap().get_text_content(), "pasted two");
            assert_eq!(anchor.offset, "pasted two".len());
        })
        .unwrap();
    outdent(&editor, "two");
    assert_eq!(
        editor.read(|| get_root().get_text_content()).unwrap(),
        "one\n\ntwo\n\nthreepasted one\n\npasted two"
    );
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > a paste of paragraphs at the end of a nested list item, then an outdent, keeps every word (NodeStructureExtension: true)
#[test]
fn a_paste_of_paragraphs_then_an_outdent_keeps_every_word_with_structure() {
    paste_then_outdent(true);
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > a paste of paragraphs at the end of a nested list item, then an outdent, keeps every word (NodeStructureExtension: false)
#[test]
fn a_paste_of_paragraphs_then_an_outdent_keeps_every_word_without_structure() {
    paste_then_outdent(false);
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::outdent next to other content in the wrapper item (without NodeStructureExtension) > outdenting a middle item keeps the text after the nested list, in reading order
#[test]
fn outdenting_a_middle_item_keeps_the_text_after_the_nested_list() {
    let editor = build_editor(false);
    update_discrete(&editor, stray_after_nested_list);
    outdent(&editor, "two");
    assert_shape(
        &editor,
        r#"[{"bullet":[["top"],[{"number":[["one"]]}],["two"],[{"number":[["three"]]}],["stray"]]}]"#,
    );
    // The caret stays in the outdented item.
    editor
        .read(|| {
            let selection = get_selection().unwrap();
            assert!(selection.is_range());
            let anchor = selection.point(Which::Anchor).unwrap();
            assert_eq!(anchor.get_node().unwrap().get_text_content(), "two");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::outdent next to other content in the wrapper item (without NodeStructureExtension) > outdenting the first and the last item keeps the text after the nested list
#[test]
fn outdenting_the_first_and_the_last_item_keeps_the_text_after_the_nested_list() {
    let editor = build_editor(false);
    update_discrete(&editor, stray_after_nested_list);
    outdent(&editor, "one");
    assert_shape(
        &editor,
        r#"[{"bullet":[["top"],["one"],[{"number":[["two"],["three"]]},"stray"]]}]"#,
    );
    outdent(&editor, "three");
    assert_shape(
        &editor,
        r#"[{"bullet":[["top"],["one"],[{"number":[["two"]]},"stray"],["three"]]}]"#,
    );
    outdent(&editor, "two");
    assert_shape(
        &editor,
        r#"[{"bullet":[["top"],["one"],["two"],["stray"],["three"]]}]"#,
    );
}

// upstream: lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::outdent next to other content in the wrapper item (without NodeStructureExtension) > text before the nested list stays before the items it preceded
#[test]
fn text_before_the_nested_list_stays_before_the_items_it_preceded() {
    let editor = build_editor(false);
    update_discrete(&editor, || {
        set_root(&[list(
            ListType::Bullet,
            &[item(&[
                create_text_node("before"),
                list(ListType::Bullet, &[text_item("one"), text_item("two")]),
            ])],
        )])
    });
    outdent(&editor, "two");
    assert_shape(
        &editor,
        r#"[{"bullet":[["before",{"bullet":[["one"]]}],["two"]]}]"#,
    );
    outdent(&editor, "one");
    assert_shape(&editor, r#"[{"bullet":[["before"],["one"],["two"]]}]"#);
}
