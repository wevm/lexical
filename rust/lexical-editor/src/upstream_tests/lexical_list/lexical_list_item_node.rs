//! Ports of `packages/lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts`,
//! less `ListItemNode.updateDOM() > nested list` (the DOM writes of
//! `updateDOM`) and the shadow-root table cell case (`@lexical/table`); see
//! rust/parity/skips.json.
//!
//! Where upstream reads the mounted editor's HTML, these render the committed
//! document with each node's `createDOM` ([`list_html`]), as Swift's ports
//! do. [`expected`] takes upstream's HTML to that form: without the
//! reconciler's `dir="auto"`, `data-lexical-text` and managed line breaks.

use super::support::{discrete, item, list, prettify_html, text_item};
use crate::json::Value;
use crate::list::{
    ListType, create_list_item_node, create_list_node, handle_indent, handle_list_insert_paragraph,
    is_list_item_node, is_list_node,
};
use crate::upstream_tests::support::{initialize_unit_test, json_to_equal, panics, update};
use crate::{
    CreateEditorArgs, Editor, EditorConfig, LexicalError, NodeRef, Which, create_paragraph_node,
    create_range_selection, create_text_node, get_root, get_selection,
};

/// `$isParagraphNode(node)`.
fn is_paragraph_node(node: &NodeRef) -> bool {
    node.get_type() == "paragraph"
}

fn editor() -> Editor {
    initialize_unit_test(CreateEditorArgs::default())
}

/// `editorConfig`.
fn editor_config() -> EditorConfig {
    theme_config(
        r#"{"list":{"listitem":"my-listItem-item-class","nested":{"listitem":"my-nested-list-listItem-class"}}}"#,
    )
}

fn theme_config(theme: &str) -> EditorConfig {
    EditorConfig {
        theme: crate::json::parse(theme)
            .unwrap()
            .as_object()
            .unwrap()
            .clone(),
    }
}

fn outer_html(node: &NodeRef, config: &EditorConfig, editor: &Editor) -> String {
    node.get_latest()
        .create_dom(node, config, editor)
        .unwrap()
        .outer_html()
}

/// The committed document as the reconciled HTML upstream compares: each
/// element's `createDOM` around its children, a text node as a `<span>`.
fn list_html(editor: &Editor) -> String {
    fn html(node: &NodeRef, editor: &Editor) -> String {
        if node.is_text() {
            return format!("<span>{}</span>", node.get_text_content());
        }
        let dom = outer_html(node, editor.config(), editor);
        let close = dom.rfind("</").expect("a closing tag");
        let children: String = node
            .get_children()
            .iter()
            .map(|c| html(c, editor))
            .collect();
        format!("{}{children}{}", &dom[..close], &dom[close..])
    }
    let e = editor.clone();
    editor
        .read_with_mode(crate::EditorReadMode::Latest, move || {
            get_root()
                .get_children()
                .iter()
                .map(|c| html(c, &e))
                .collect()
        })
        .unwrap()
}

/// Upstream's expected HTML in [`list_html`]'s form.
fn expected(html: &str) -> String {
    prettify_html(html)
        .replace(r#" dir="auto""#, "")
        .replace(r#" data-lexical-text="true""#, "")
        .replace(r#"<br data-lexical-managed-linebreak="true" />"#, "")
        .replace(r#"<br data-lexical-managed-linebreak="true">"#, "")
}

fn assert_html(editor: &Editor, html: &str) {
    assert_eq!(list_html(editor), expected(html));
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.constructor
#[test]
fn list_item_node_constructor() {
    let editor = editor();
    update(&editor, || {
        let list_item_node = create_list_item_node(None);
        assert_eq!(list_item_node.get_type(), "listitem");
        assert_eq!(list_item_node.get_text_content(), "");
        Ok(())
    });
    assert!(panics(|| create_list_item_node(None)));
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.createDOM()
#[test]
fn list_item_node_create_dom() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let list_item_node = create_list_item_node(None);
        assert_eq!(
            outer_html(&list_item_node, &editor_config(), &e),
            r#"<li value="1" class="my-listItem-item-class"></li>"#
        );
        assert_eq!(
            outer_html(&list_item_node, &theme_config("{}"), &e),
            r#"<li value="1"></li>"#
        );
        Ok(())
    });
}

/// `updateDOM` is its model half in Rust (rust/PARITY.md), which leaves
/// the element as it is.
// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.updateDOM() > base
#[test]
fn list_item_node_update_dom_base() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let list_item_node = create_list_item_node(None);
        let dom_element =
            list_item_node
                .get_latest()
                .create_dom(&list_item_node, &editor_config(), &e)?;
        assert_eq!(
            dom_element.outer_html(),
            r#"<li value="1" class="my-listItem-item-class"></li>"#
        );
        let new_list_item_node = create_list_item_node(None);
        let result = new_list_item_node
            .get_latest()
            .update_dom(&*list_item_node.get_latest());
        assert!(!result);
        assert_eq!(
            dom_element.outer_html(),
            r#"<li value="1" class="my-listItem-item-class"></li>"#
        );
        Ok(())
    });
}

const ONE_TWO_THREE: &str = r#"
    <ul dir="auto">
      <li value="1">
        <span data-lexical-text="true">one</span>
      </li>
      <li value="2">
        <span data-lexical-text="true">two</span>
      </li>
      <li value="3">
        <span data-lexical-text="true">three</span>
      </li>
    </ul>
"#;

/// The `ListItemNode.replace()` fixture (its `beforeEach`): a bullet list
/// of "one", "two" and "three".
fn replace_fixture() -> (Editor, Vec<NodeRef>) {
    let editor = editor();
    let items = Rc::new(RefCell::new(Vec::new()));
    let out = items.clone();
    update(&editor, move || {
        let root = get_root();
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let items: Vec<NodeRef> = ["one", "two", "three"]
            .iter()
            .map(|t| {
                let item = create_list_item_node(None);
                item.append(&[create_text_node(t)]).unwrap();
                item
            })
            .collect();
        root.append(std::slice::from_ref(&list_node))?;
        list_node.append(&items)?;
        *out.borrow_mut() = items;
        Ok(())
    });
    assert_html(&editor, ONE_TWO_THREE);
    let items = items.borrow().clone();
    (editor, items)
}

use std::cell::RefCell;
use std::rc::Rc;

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > another list item node
#[test]
fn replace_another_list_item_node() {
    let (editor, items) = replace_fixture();
    update(&editor, move || {
        let new_list_item_node = create_list_item_node(None);
        new_list_item_node.append(&[create_text_node("bar")])?;
        items[0].replace(&new_list_item_node, false).map(|_| ())
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">bar</span>
          </li>
          <li value="2">
            <span data-lexical-text="true">two</span>
          </li>
          <li value="3">
            <span data-lexical-text="true">three</span>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > first list item with a non list item node
#[test]
fn replace_first_list_item_with_a_non_list_item_node() {
    let (editor, items) = replace_fixture();
    update(&editor, || Ok(()));
    assert_html(&editor, ONE_TWO_THREE);
    update(&editor, move || {
        items[0]
            .replace(&create_paragraph_node(), false)
            .map(|_| ())
    });
    assert_html(
        &editor,
        r#"
        <p dir="auto"><br data-lexical-managed-linebreak="true" /></p>
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">two</span>
          </li>
          <li value="2">
            <span data-lexical-text="true">three</span>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > last list item with a non list item node
#[test]
fn replace_last_list_item_with_a_non_list_item_node() {
    let (editor, items) = replace_fixture();
    update(&editor, move || {
        items[2]
            .replace(&create_paragraph_node(), false)
            .map(|_| ())
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">one</span>
          </li>
          <li value="2">
            <span data-lexical-text="true">two</span>
          </li>
        </ul>
        <p dir="auto"><br data-lexical-managed-linebreak="true" /></p>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > middle list item with a non list item node
#[test]
fn replace_middle_list_item_with_a_non_list_item_node() {
    let (editor, items) = replace_fixture();
    update(&editor, move || {
        items[1]
            .replace(&create_paragraph_node(), false)
            .map(|_| ())
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">one</span>
          </li>
        </ul>
        <p dir="auto"><br data-lexical-managed-linebreak="true" /></p>
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">three</span>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > the only list item with a non list item node
#[test]
fn replace_the_only_list_item_with_a_non_list_item_node() {
    let (editor, items) = replace_fixture();
    let (two, three) = (items[1].clone(), items[2].clone());
    update(&editor, move || {
        two.remove(false)?;
        three.remove(false)
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">one</span>
          </li>
        </ul>
        "#,
    );
    update(&editor, move || {
        items[0]
            .replace(&create_paragraph_node(), false)
            .map(|_| ())
    });
    assert_html(
        &editor,
        r#"<p dir="auto"><br data-lexical-managed-linebreak="true" /></p>"#,
    );
}

/// The `ListItemNode.remove()` cases: `x` between the items `before` makes
/// and those `after` makes, in a bullet list; checks `initially` (when
/// given), removes `x` and checks `then`.
fn remove_x(before: fn() -> NodeRef, after: fn() -> NodeRef, initially: Option<&str>, then: &str) {
    let editor = editor();
    let x = Rc::new(RefCell::new(None));
    let out = x.clone();
    update(&editor, move || {
        let x = text_item("x");
        get_root().append(&[list(ListType::Bullet, &[before(), x.clone(), after()])])?;
        *out.borrow_mut() = Some(x);
        Ok(())
    });
    if let Some(initially) = initially {
        assert_html(&editor, initially);
    }
    let x = x.borrow().clone().unwrap();
    update(&editor, move || x.remove(false));
    assert_html(&editor, then);
}

/// A list item holding only a nested list of `list_type` with `items`.
fn nested(list_type: ListType, items: &[NodeRef]) -> NodeRef {
    item(&[list(list_type, items)])
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > siblings are not nested
#[test]
fn remove_siblings_are_not_nested() {
    remove_x(
        || text_item("A"),
        || text_item("B"),
        Some(
            r#"
            <ul dir="auto">
              <li value="1"><span data-lexical-text="true">A</span></li>
              <li value="2"><span data-lexical-text="true">x</span></li>
              <li value="3"><span data-lexical-text="true">B</span></li>
            </ul>
            "#,
        ),
        r#"
        <ul dir="auto">
          <li value="1"><span data-lexical-text="true">A</span></li>
          <li value="2"><span data-lexical-text="true">B</span></li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > the previous sibling is nested
#[test]
fn remove_the_previous_sibling_is_nested() {
    remove_x(
        || nested(ListType::Bullet, &[text_item("A")]),
        || text_item("B"),
        Some(
            r#"
            <ul dir="auto">
              <li value="1">
                <ul>
                  <li value="1"><span data-lexical-text="true">A</span></li>
                </ul>
              </li>
              <li value="1"><span data-lexical-text="true">x</span></li>
              <li value="2"><span data-lexical-text="true">B</span></li>
            </ul>
            "#,
        ),
        r#"
        <ul dir="auto">
          <li value="1">
            <ul>
              <li value="1"><span data-lexical-text="true">A</span></li>
            </ul>
          </li>
          <li value="1"><span data-lexical-text="true">B</span></li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > the next sibling is nested
#[test]
fn remove_the_next_sibling_is_nested() {
    remove_x(
        || text_item("A"),
        || nested(ListType::Bullet, &[text_item("B")]),
        Some(
            r#"
            <ul dir="auto">
              <li value="1"><span data-lexical-text="true">A</span></li>
              <li value="2"><span data-lexical-text="true">x</span></li>
              <li value="3">
                <ul>
                  <li value="1"><span data-lexical-text="true">B</span></li>
                </ul>
              </li>
            </ul>
            "#,
        ),
        r#"
        <ul dir="auto">
          <li value="1"><span data-lexical-text="true">A</span></li>
          <li value="2">
            <ul>
              <li value="1"><span data-lexical-text="true">B</span></li>
            </ul>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > both siblings are nested
#[test]
fn remove_both_siblings_are_nested() {
    remove_x(
        || nested(ListType::Bullet, &[text_item("A")]),
        || nested(ListType::Bullet, &[text_item("B")]),
        Some(
            r#"
            <ul dir="auto">
              <li value="1">
                <ul>
                  <li value="1"><span data-lexical-text="true">A</span></li>
                </ul>
              </li>
              <li value="1"><span data-lexical-text="true">x</span></li>
              <li value="2">
                <ul>
                  <li value="1"><span data-lexical-text="true">B</span></li>
                </ul>
              </li>
            </ul>
            "#,
        ),
        r#"
        <ul dir="auto">
          <li value="1">
            <ul>
              <li value="1"><span data-lexical-text="true">A</span></li>
              <li value="2"><span data-lexical-text="true">B</span></li>
            </ul>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > both siblings are nested with a different listType
#[test]
fn remove_both_siblings_are_nested_with_a_different_list_type() {
    remove_x(
        || nested(ListType::Bullet, &[text_item("A")]),
        || nested(ListType::Number, &[text_item("B")]),
        None,
        r#"
        <ul dir="auto">
          <li value="1">
            <ul>
              <li value="1"><span data-lexical-text="true">A</span></li>
            </ul>
          </li>
          <li value="1">
            <ol>
              <li value="1"><span data-lexical-text="true">B</span></li>
            </ol>
          </li>
        </ul>
        "#,
    );
}

/// `- A1`, then `- A2` nested deeper, all in one nested list item.
fn a1_a2() -> NodeRef {
    nested(
        ListType::Bullet,
        &[
            text_item("A1"),
            nested(ListType::Bullet, &[text_item("A2")]),
        ],
    )
}

/// `- B1` nested deeper, then `- B2`, all in one nested list item.
fn b1_b2() -> NodeRef {
    nested(
        ListType::Bullet,
        &[
            nested(ListType::Bullet, &[text_item("B1")]),
            text_item("B2"),
        ],
    )
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > the previous sibling is nested deeper than the next sibling
#[test]
fn remove_the_previous_sibling_is_nested_deeper_than_the_next_sibling() {
    remove_x(
        a1_a2,
        || nested(ListType::Bullet, &[text_item("B")]),
        Some(
            r#"
            <ul dir="auto">
              <li value="1">
                <ul>
                  <li value="1"><span data-lexical-text="true">A1</span></li>
                  <li value="2">
                    <ul>
                      <li value="1"><span data-lexical-text="true">A2</span></li>
                    </ul>
                  </li>
                </ul>
              </li>
              <li value="1"><span data-lexical-text="true">x</span></li>
              <li value="2">
                <ul>
                  <li value="1"><span data-lexical-text="true">B</span></li>
                </ul>
              </li>
            </ul>
            "#,
        ),
        r#"
        <ul dir="auto">
          <li value="1">
            <ul>
              <li value="1"><span data-lexical-text="true">A1</span></li>
              <li value="2">
                <ul>
                  <li value="1"><span data-lexical-text="true">A2</span></li>
                </ul>
              </li>
              <li value="2"><span data-lexical-text="true">B</span></li>
            </ul>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > the next sibling is nested deeper than the previous sibling
#[test]
fn remove_the_next_sibling_is_nested_deeper_than_the_previous_sibling() {
    remove_x(
        || nested(ListType::Bullet, &[text_item("A")]),
        b1_b2,
        Some(
            r#"
            <ul dir="auto">
              <li value="1">
                <ul>
                  <li value="1"><span data-lexical-text="true">A</span></li>
                </ul>
              </li>
              <li value="1"><span data-lexical-text="true">x</span></li>
              <li value="2">
                <ul>
                  <li value="1">
                    <ul>
                      <li value="1"><span data-lexical-text="true">B1</span></li>
                    </ul>
                  </li>
                  <li value="1"><span data-lexical-text="true">B2</span></li>
                </ul>
              </li>
            </ul>
            "#,
        ),
        r#"
        <ul dir="auto">
          <li value="1">
            <ul>
              <li value="1"><span data-lexical-text="true">A</span></li>
              <li value="2">
                <ul>
                  <li value="1"><span data-lexical-text="true">B1</span></li>
                </ul>
              </li>
              <li value="2"><span data-lexical-text="true">B2</span></li>
            </ul>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > both siblings are deeply nested
#[test]
fn remove_both_siblings_are_deeply_nested() {
    remove_x(
        a1_a2,
        b1_b2,
        Some(
            r#"
            <ul dir="auto">
              <li value="1">
                <ul>
                  <li value="1"><span data-lexical-text="true">A1</span></li>
                  <li value="2">
                    <ul>
                      <li value="1"><span data-lexical-text="true">A2</span></li>
                    </ul>
                  </li>
                </ul>
              </li>
              <li value="1"><span data-lexical-text="true">x</span></li>
              <li value="2">
                <ul>
                  <li value="1">
                    <ul>
                      <li value="1"><span data-lexical-text="true">B1</span></li>
                    </ul>
                  </li>
                  <li value="1"><span data-lexical-text="true">B2</span></li>
                </ul>
              </li>
            </ul>
            "#,
        ),
        r#"
        <ul dir="auto">
          <li value="1">
            <ul>
              <li value="1"><span data-lexical-text="true">A1</span></li>
              <li value="2">
                <ul>
                  <li value="1"><span data-lexical-text="true">A2</span></li>
                  <li value="2"><span data-lexical-text="true">B1</span></li>
                </ul>
              </li>
              <li value="2"><span data-lexical-text="true">B2</span></li>
            </ul>
          </li>
        </ul>
        "#,
    );
}

/// The `ListItemNode.insertNewAfter(): non-empty list items` fixture: the
/// list is appended to the root before its items get their text.
fn insert_new_after_fixture() -> (Editor, Vec<NodeRef>) {
    let editor = editor();
    let items = Rc::new(RefCell::new(Vec::new()));
    let out = items.clone();
    update(&editor, move || {
        let root = get_root();
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let items: Vec<NodeRef> = (0..3).map(|_| create_list_item_node(None)).collect();
        root.append(std::slice::from_ref(&list_node))?;
        list_node.append(&items)?;
        for (item, text) in items.iter().zip(["one", "two", "three"]) {
            item.append(&[create_text_node(text)])?;
        }
        *out.borrow_mut() = items;
        Ok(())
    });
    assert_html(&editor, ONE_TWO_THREE);
    let items = items.borrow().clone();
    (editor, items)
}

fn insert_new_after(node: NodeRef) -> impl FnOnce() -> Result<(), LexicalError> {
    move || {
        node.insert_new_after(&create_range_selection(), true)?;
        Ok(())
    }
}

const ONE_TWO_THREE_EMPTY: &str = r#"
    <ul dir="auto">
      <li value="1">
        <span data-lexical-text="true">one</span>
      </li>
      <li value="2">
        <span data-lexical-text="true">two</span>
      </li>
      <li value="3">
        <span data-lexical-text="true">three</span>
      </li>
      <li value="4"><br data-lexical-managed-linebreak="true" /></li>
    </ul>
"#;

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.insertNewAfter(): non-empty list items > first list item
#[test]
fn insert_new_after_first_list_item() {
    let (editor, items) = insert_new_after_fixture();
    update(&editor, insert_new_after(items[0].clone()));
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">one</span>
          </li>
          <li value="2"><br data-lexical-managed-linebreak="true" /></li>
          <li value="3">
            <span data-lexical-text="true">two</span>
          </li>
          <li value="4">
            <span data-lexical-text="true">three</span>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.insertNewAfter(): non-empty list items > last list item
#[test]
fn insert_new_after_last_list_item() {
    let (editor, items) = insert_new_after_fixture();
    update(&editor, insert_new_after(items[2].clone()));
    assert_html(&editor, ONE_TWO_THREE_EMPTY);
}

/// Upstream's "middle" case inserts after the third item, as the last
/// case does.
// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.insertNewAfter(): non-empty list items > middle list item
#[test]
fn insert_new_after_middle_list_item() {
    let (editor, items) = insert_new_after_fixture();
    update(&editor, insert_new_after(items[2].clone()));
    assert_html(&editor, ONE_TWO_THREE_EMPTY);
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.insertNewAfter(): non-empty list items > the only list item
#[test]
fn insert_new_after_the_only_list_item() {
    let (editor, items) = insert_new_after_fixture();
    let (two, three) = (items[1].clone(), items[2].clone());
    update(&editor, move || {
        two.remove(false)?;
        three.remove(false)
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">one</span>
          </li>
        </ul>
        "#,
    );
    update(&editor, insert_new_after(items[0].clone()));
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">one</span>
          </li>
          <li value="2"><br data-lexical-managed-linebreak="true" /></li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > $createListItemNode()
#[test]
fn create_list_item_node_function() {
    let editor = editor();
    update(&editor, || {
        let list_item_node = create_list_item_node(None);
        let created_list_item_node = create_list_item_node(None);
        assert_eq!(list_item_node.get_type(), created_list_item_node.get_type());
        assert_eq!(
            list_item_node.get_parent(),
            created_list_item_node.get_parent()
        );
        assert_ne!(list_item_node.get_key(), created_list_item_node.get_key());
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > $isListItemNode()
#[test]
fn is_list_item_node_function() {
    let editor = editor();
    update(&editor, || {
        assert!(is_list_item_node(&create_list_item_node(None)));
        Ok(())
    });
}

/// The `ListItemNode.setIndent()` fixture: a bullet list of "one" and
/// "two".
fn set_indent_fixture() -> (Editor, NodeRef) {
    let editor = editor();
    let one = Rc::new(RefCell::new(None));
    let out = one.clone();
    update(&editor, move || {
        let root = get_root();
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let one = create_list_item_node(None);
        let two = create_list_item_node(None);
        root.append(std::slice::from_ref(&list_node))?;
        list_node.append(&[one.clone(), two.clone()])?;
        one.append(&[create_text_node("one")])?;
        two.append(&[create_text_node("two")])?;
        *out.borrow_mut() = Some(one);
        Ok(())
    });
    let one = one.borrow().clone().unwrap();
    (editor, one)
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.setIndent() > indents and outdents list item
#[test]
fn set_indent_indents_and_outdents_list_item() {
    let (editor, one) = set_indent_fixture();
    let n = one.clone();
    update(&editor, move || n.set_indent(3.0));
    let n = one.clone();
    update(&editor, move || {
        assert_eq!(n.get_indent()?, 3.0);
        Ok(())
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <ul>
              <li value="1">
                <ul>
                  <li value="1">
                    <ul>
                      <li value="1">
                        <span data-lexical-text="true">one</span>
                      </li>
                    </ul>
                  </li>
                </ul>
              </li>
            </ul>
          </li>
          <li value="1">
            <span data-lexical-text="true">two</span>
          </li>
        </ul>
        "#,
    );
    let n = one.clone();
    update(&editor, move || n.set_indent(0.0));
    update(&editor, move || {
        assert_eq!(one.get_indent()?, 0.0);
        Ok(())
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">one</span>
          </li>
          <li value="2">
            <span data-lexical-text="true">two</span>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.setIndent() > handles fractional indent values
#[test]
fn set_indent_handles_fractional_indent_values() {
    let (editor, one) = set_indent_fixture();
    let n = one.clone();
    update(&editor, move || n.set_indent(0.5));
    update(&editor, move || {
        assert_eq!(one.get_indent()?, 0.0);
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > Can serialize a node that is not attached
#[test]
fn can_serialize_a_node_that_is_not_attached() {
    let editor = editor();
    update(&editor, || {
        let list_item_node = create_list_item_node(None);
        let list_node = create_list_node(ListType::default(), 1.0);
        list_node.append(std::slice::from_ref(&list_item_node))?;
        let expected = crate::json::parse(
            r#"{"children":[],"direction":null,"format":"","indent":0,"type":"listitem","value":1,"version":1}"#,
        )
        .unwrap();
        let actual = Value::Object(list_item_node.export_json(false));
        assert!(json_to_equal(&actual, &expected), "{actual}");
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode marker style inheritance on indent
#[test]
fn list_item_node_marker_style_inheritance_on_indent() {
    let editor = editor();
    update(&editor, || {
        let root = get_root();
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let list_item1 = create_list_item_node(None);
        let list_item2 = create_list_item_node(None);
        list_item2.set_text_style("font-size: 19px;");
        list_node.append(&[list_item1, list_item2.clone()])?;
        root.append(&[list_node])?;
        handle_indent(&list_item2)?;
        let parent_list_item = list_item2.get_parent_or_throw()?.get_parent_or_throw()?;
        assert!(is_list_item_node(&parent_list_item));
        assert_eq!(
            parent_list_item.element_data().get_text_style(),
            "font-size: 19px;"
        );
        Ok(())
    });
}

/// The split fixture: a numbered list of `A`, `B`, an empty item and `C`,
/// with the empty item selected.
fn split_fixture() -> Editor {
    let editor = editor();
    update(&editor, || {
        let empty_item = create_list_item_node(None);
        get_root().append(&[list(
            ListType::Number,
            &[
                text_item("A"),
                text_item("B"),
                empty_item.clone(),
                text_item("C"),
            ],
        )])?;
        empty_item.select(None, None).map(|_| ())
    });
    editor
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > Default: Splitting a list resets numbering to 1 (Backward Compatibility)
#[test]
fn default_splitting_a_list_resets_numbering_to_1() {
    let editor = split_fixture();
    update(&editor, || handle_list_insert_paragraph(false).map(|_| ()));
    assert_eq!(
        list_html(&editor),
        expected(concat!(
            r#"<ol dir="auto">"#,
            r#"<li value="1"><span data-lexical-text="true">A</span></li>"#,
            r#"<li value="2"><span data-lexical-text="true">B</span></li>"#,
            "</ol>",
            r#"<p dir="auto"><br data-lexical-managed-linebreak="true"></p>"#,
            r#"<ol dir="auto">"#,
            r#"<li value="1"><span data-lexical-text="true">C</span></li>"#,
            "</ol>",
        ))
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > Option Enabled: Splitting a list preserves numbering (Smart Behavior)
#[test]
fn option_enabled_splitting_a_list_preserves_numbering() {
    let editor = split_fixture();
    update(&editor, || handle_list_insert_paragraph(true).map(|_| ()));
    assert_eq!(
        list_html(&editor),
        expected(concat!(
            r#"<ol dir="auto">"#,
            r#"<li value="1"><span data-lexical-text="true">A</span></li>"#,
            r#"<li value="2"><span data-lexical-text="true">B</span></li>"#,
            "</ol>",
            r#"<p dir="auto"><br data-lexical-managed-linebreak="true"></p>"#,
            r#"<ol start="3" dir="auto">"#,
            r#"<li value="3"><span data-lexical-text="true">C</span></li>"#,
            "</ol>",
        ))
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > Option Enabled: Preserves numbering when a nested sublist precedes the split
#[test]
fn option_enabled_preserves_numbering_when_a_nested_sublist_precedes_the_split() {
    let editor = editor();
    update(&editor, || {
        let empty_item = create_list_item_node(None);
        get_root().append(&[list(
            ListType::Number,
            &[
                text_item("A"),
                nested(ListType::Bullet, &[text_item("A.1")]),
                text_item("B"),
                empty_item.clone(),
                text_item("C"),
            ],
        )])?;
        empty_item.select(None, None).map(|_| ())
    });
    update(&editor, || handle_list_insert_paragraph(true).map(|_| ()));
    editor
        .read_with_mode(crate::EditorReadMode::Latest, || {
            let children = get_root().get_children();
            let [first_list, paragraph, second_list] = &children[..] else {
                panic!("three children");
            };
            assert!(is_list_node(first_list));
            assert!(is_paragraph_node(paragraph));
            assert!(is_list_node(second_list));
            assert_eq!(second_list.get_list_start(), Some(3.0));
            let first_item = second_list.get_first_child().unwrap();
            assert!(is_list_item_node(&first_item));
            assert_eq!(first_item.get_list_item_value(), Some(3.0));
        })
        .unwrap();
}

// ListItemNode $transform wraps orphan ListItemNodes

fn update_discrete(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps a single orphan ListItemNode under root in a ListNode
#[test]
fn wraps_a_single_orphan_list_item_node_under_root_in_a_list_node() {
    let editor = editor();
    update_discrete(&editor, || get_root().append(&[text_item("orphan")]));
    editor
        .read(|| {
            let wrapper = get_root().get_first_child().unwrap();
            assert!(
                is_list_node(&wrapper),
                "orphan <li> should be wrapped in list"
            );
            assert_eq!(wrapper.get_list_type(), Some(ListType::Bullet));
        })
        .unwrap();
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1">
            <span data-lexical-text="true">orphan</span>
          </li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps adjacent orphan ListItemNodes in a single ListNode
#[test]
fn wraps_adjacent_orphan_list_item_nodes_in_a_single_list_node() {
    let editor = editor();
    update_discrete(&editor, || {
        get_root().append(&[text_item("a"), text_item("b"), text_item("c")])
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1"><span data-lexical-text="true">a</span></li>
          <li value="2"><span data-lexical-text="true">b</span></li>
          <li value="3"><span data-lexical-text="true">c</span></li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > does not merge orphan ListItemNodes separated by a ParagraphNode
#[test]
fn does_not_merge_orphan_list_item_nodes_separated_by_a_paragraph_node() {
    let editor = editor();
    update_discrete(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("middle")])?;
        get_root().append(&[text_item("a"), paragraph, text_item("b")])
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1"><span data-lexical-text="true">a</span></li>
        </ul>
        <p dir="auto"><span data-lexical-text="true">middle</span></p>
        <ul dir="auto">
          <li value="1"><span data-lexical-text="true">b</span></li>
        </ul>
        "#,
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > parses HTML with orphan <li> outside of <ul>
#[test]
fn parses_html_with_orphan_li_outside_of_ul() {
    let editor = editor();
    let input = r#"
          <div>
            <ul>
              <li>test</li>
              <li>test</li>
              <li>test</li>
            </ul>
            hello world
            <li>test</li>
          </div>
        "#;
    let inner = editor.clone();
    update_discrete(&editor, move || {
        let root = get_root();
        root.clear()?;
        root.select(None, None)?;
        let dom = crate::dom::parse_from_string(input);
        crate::insert_nodes(crate::html::generate_nodes_from_dom(&inner, &dom)?)
    });
    assert_html(
        &editor,
        r#"
        <ul dir="auto">
          <li value="1"><span data-lexical-text="true">test</span></li>
          <li value="2"><span data-lexical-text="true">test</span></li>
          <li value="3"><span data-lexical-text="true">test</span></li>
        </ul>
        <p dir="auto">
          <span data-lexical-text="true">hello world</span>
        </p>
        <ul dir="auto">
          <li value="1"><span data-lexical-text="true">test</span></li>
        </ul>
        "#,
    );
}

fn read_points() -> Vec<(String, usize)> {
    let selection = get_selection().expect("a selection");
    assert!(selection.is_range(), "selection should be a RangeSelection");
    [Which::Anchor, Which::Focus]
        .into_iter()
        .map(|which| {
            let point = selection.point(which).unwrap();
            (point.get_node().unwrap().get_text_content(), point.offset)
        })
        .collect()
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > preserves the selection on the orphan when it is wrapped
#[test]
fn preserves_the_selection_on_the_orphan_when_it_is_wrapped() {
    let editor = editor();
    update_discrete(&editor, || {
        let text = create_text_node("orphan");
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("hello world")])?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph, item(std::slice::from_ref(&text))])?;
        text.select(Some(2), Some(4)).map(|_| ())
    });
    editor
        .read(|| {
            assert_eq!(
                read_points(),
                [("orphan".to_owned(), 2), ("orphan".to_owned(), 4)]
            );
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > preserves a selection anchored outside the orphan
#[test]
fn preserves_a_selection_anchored_outside_the_orphan() {
    let editor = editor();
    update_discrete(&editor, || {
        let hello_text = create_text_node("hello world");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&hello_text))?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph, text_item("orphan")])?;
        hello_text.select(Some(3), Some(5)).map(|_| ())
    });
    editor
        .read(|| {
            assert_eq!(
                read_points(),
                [("hello world".to_owned(), 3), ("hello world".to_owned(), 5)]
            );
        })
        .unwrap();
}

/// `$getRoot().clear().append($createParagraphNode().append(...children))`.
fn paragraph_holding(children: Vec<NodeRef>) -> impl FnOnce() -> Result<(), LexicalError> {
    move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&children)?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    }
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps orphan ListItemNode inside a paragraph with no siblings
#[test]
fn wraps_orphan_list_item_node_inside_a_paragraph_with_no_siblings() {
    let editor = editor();
    update_discrete(&editor, || paragraph_holding(vec![text_item("item")])());
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 1);
            let list = root.get_first_child().unwrap();
            assert!(is_list_node(&list));
            assert_eq!(list.get_children_size(), 1);
            let list_item = list.get_first_child().unwrap();
            assert!(is_list_item_node(&list_item));
            assert_eq!(list_item.get_text_content(), "item");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps orphan ListItemNode inside a paragraph with prev siblings
#[test]
fn wraps_orphan_list_item_node_inside_a_paragraph_with_prev_siblings() {
    let editor = editor();
    update_discrete(&editor, || {
        paragraph_holding(vec![create_text_node("before"), text_item("item")])()
    });
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 2);
            let children = root.get_children();
            assert!(is_paragraph_node(&children[0]));
            assert_eq!(children[0].get_text_content(), "before");
            assert!(is_list_node(&children[1]));
            assert_eq!(children[1].get_children_size(), 1);
            let list_item = children[1].get_first_child().unwrap();
            assert!(is_list_item_node(&list_item));
            assert_eq!(list_item.get_text_content(), "item");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps orphan ListItemNode inside a paragraph with next siblings
#[test]
fn wraps_orphan_list_item_node_inside_a_paragraph_with_next_siblings() {
    let editor = editor();
    update_discrete(&editor, || {
        paragraph_holding(vec![text_item("item"), create_text_node("after")])()
    });
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 2);
            let children = root.get_children();
            assert!(is_paragraph_node(&children[1]));
            assert_eq!(children[1].get_text_content(), "after");
            assert!(is_list_node(&children[0]));
            assert_eq!(children[0].get_children_size(), 1);
            let list_item = children[0].get_first_child().unwrap();
            assert!(is_list_item_node(&list_item));
            assert_eq!(list_item.get_text_content(), "item");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps orphan ListItemNode inside a paragraph with both siblings
#[test]
fn wraps_orphan_list_item_node_inside_a_paragraph_with_both_siblings() {
    let editor = editor();
    update_discrete(&editor, || {
        paragraph_holding(vec![
            create_text_node("before"),
            text_item("item"),
            create_text_node("after"),
        ])()
    });
    editor
        .read(|| {
            let root = get_root();
            assert_eq!(root.get_children_size(), 3);
            let children = root.get_children();
            assert!(is_paragraph_node(&children[0]));
            assert_eq!(children[0].get_text_content(), "before");
            assert!(is_paragraph_node(&children[2]));
            assert_eq!(children[2].get_text_content(), "after");
            assert!(is_list_node(&children[1]));
            assert_eq!(children[1].get_children_size(), 1);
            let list_item = children[1].get_first_child().unwrap();
            assert!(is_list_item_node(&list_item));
            assert_eq!(list_item.get_text_content(), "item");
        })
        .unwrap();
}
