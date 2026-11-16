//! Ports of `packages/lexical-list/src/__tests__/unit/LexicalListNode.test.ts`,
//! less the tests of `ListNodeConfig` and `ListNodeSubclass`, subclasses of
//! `ListNode` (rust/parity/skips.json).
//!
//! Where upstream reads the mounted editor's `innerHTML`, these render each
//! committed list item with `createDOM`, as Swift's ports do.

use super::support::{discrete, item, list, text_item};
use crate::link::{LinkAttributes, create_link_node, is_link_node};
use crate::list::{
    ListType, create_list_item_node, create_list_node, is_list_item_node, is_list_node,
};
use crate::upstream_tests::support::{initialize_unit_test, update};
use crate::{CreateEditorArgs, Editor, create_paragraph_node, create_text_node, get_root};

fn editor() -> Editor {
    initialize_unit_test(CreateEditorArgs::default())
}

/// `editorConfig`'s list theme.
#[cfg(feature = "html")]
fn editor_config() -> crate::EditorConfig {
    let depths = |tag: &str| {
        (1..=7)
            .map(|i| format!("\"my-{tag}-list-class-{i}\""))
            .collect::<Vec<_>>()
            .join(",")
    };
    theme_config(&format!(
        r#"{{"list":{{"ol":"my-ol-list-class","olDepth":[{}],"ul":"my-ul-list-class","ulDepth":[{}]}}}}"#,
        depths("ol"),
        depths("ul")
    ))
}

#[cfg(feature = "html")]
fn theme_config(theme: &str) -> crate::EditorConfig {
    crate::EditorConfig {
        theme: crate::json::parse(theme)
            .unwrap()
            .as_object()
            .unwrap()
            .clone(),
    }
}

#[cfg(feature = "html")]
fn outer_html(
    node: &crate::NodeRef,
    config: &crate::EditorConfig,
    editor: &Editor,
) -> Result<String, crate::LexicalError> {
    Ok(node
        .get_latest()
        .create_dom(node, config, editor)?
        .outer_html())
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.constructor
#[test]
fn list_node_constructor() {
    let editor = editor();
    update(&editor, || {
        let list_node = create_list_node(ListType::Bullet, 1.0);
        assert_eq!(list_node.get_type(), "list");
        assert_eq!(list_node.get_list_tag(), Some("ul"));
        assert_eq!(list_node.get_text_content(), "");
        Ok(())
    });
    update(&editor, || {
        create_list_node(ListType::default(), 1.0);
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.updateFromJSON normalizes the legacy tag-form listType
#[test]
fn list_node_update_from_json_normalizes_the_legacy_tag_form_list_type() {
    let editor = editor();
    update(&editor, || {
        let legacy = |list_type: &str| {
            crate::json::parse(&format!(r#"{{"listType":"{list_type}"}}"#))
                .unwrap()
                .as_object()
                .unwrap()
                .clone()
        };
        let bullet = create_list_node(ListType::default(), 1.0);
        bullet.update_from_json(&legacy("ul"))?;
        assert_eq!(bullet.get_list_type(), Some(ListType::Bullet));
        assert_eq!(bullet.get_list_tag(), Some("ul"));
        let number = create_list_node(ListType::Bullet, 1.0);
        number.update_from_json(&legacy("ol"))?;
        assert_eq!(number.get_list_type(), Some(ListType::Number));
        assert_eq!(number.get_list_tag(), Some("ol"));
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.getTag()
#[test]
fn list_node_get_tag() {
    let editor = editor();
    update(&editor, || {
        assert_eq!(
            create_list_node(ListType::Bullet, 1.0).get_list_tag(),
            Some("ul")
        );
        assert_eq!(
            create_list_node(ListType::Number, 1.0).get_list_tag(),
            Some("ol")
        );
        assert_eq!(
            create_list_node(ListType::Check, 1.0).get_list_tag(),
            Some("ul")
        );
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.createDOM()
#[cfg(feature = "html")]
#[test]
fn list_node_create_dom() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let list_node = create_list_node(ListType::Bullet, 1.0);
        assert_eq!(
            outer_html(&list_node, &editor_config(), &e)?,
            r#"<ul class="my-ul-list-class my-ul-list-class-1"></ul>"#
        );
        assert_eq!(
            outer_html(&list_node, &theme_config(r#"{"list":{}}"#), &e)?,
            "<ul></ul>"
        );
        assert_eq!(
            outer_html(&list_node, &theme_config("{}"), &e)?,
            "<ul></ul>"
        );
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.createDOM() correctly applies classes to a nested ListNode
#[cfg(feature = "html")]
#[test]
fn list_node_create_dom_correctly_applies_classes_to_a_nested_list_node() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let lists: Vec<_> = (0..7)
            .map(|_| create_list_node(ListType::Bullet, 1.0))
            .collect();
        let items: Vec<_> = (0..4).map(|_| create_list_item_node(None)).collect();
        lists[0].append(&[items[0].clone()])?;
        items[0].append(&[lists[1].clone()])?;
        lists[1].append(&[items[1].clone()])?;
        items[1].append(&[lists[2].clone()])?;
        lists[2].append(&[items[2].clone()])?;
        items[2].append(&[lists[3].clone()])?;
        lists[3].append(&[items[3].clone()])?;
        lists[3].append(&[lists[4].clone()])?;
        lists[4].append(&[lists[5].clone()])?;
        lists[5].append(&[lists[6].clone()])?;

        assert_eq!(
            outer_html(&lists[0], &editor_config(), &e)?,
            r#"<ul class="my-ul-list-class my-ul-list-class-1"></ul>"#
        );
        assert_eq!(
            outer_html(&lists[0], &theme_config(r#"{"list":{}}"#), &e)?,
            "<ul></ul>"
        );
        assert_eq!(outer_html(&lists[0], &theme_config("{}"), &e)?, "<ul></ul>");
        for (i, list) in lists.iter().enumerate().skip(1) {
            assert_eq!(
                outer_html(list, &editor_config(), &e)?,
                format!(
                    r#"<ul class="my-ul-list-class my-ul-list-class-{}"></ul>"#,
                    i + 1
                )
            );
        }
        let ol_depth = (1..=7)
            .map(|i| format!("\"my-ol-list-class-{i}\""))
            .collect::<Vec<_>>()
            .join(",");
        let three_ul_depths = theme_config(&format!(
            r#"{{"list":{{"ol":"my-ol-list-class","olDepth":[{ol_depth}],"ul":"my-ul-list-class","ulDepth":["my-ul-list-class-1","my-ul-list-class-2","my-ul-list-class-3"]}}}}"#
        ));
        assert_eq!(
            outer_html(&lists[4], &three_ul_depths, &e)?,
            r#"<ul class="my-ul-list-class my-ul-list-class-2"></ul>"#
        );
        Ok(())
    });
}

/// `updateDOM` is its model half in Rust (rust/PARITY.md): it says the
/// element must be made again and leaves it as it is.
// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.updateDOM()
#[cfg(feature = "html")]
#[test]
fn list_node_update_dom() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let dom_element = list_node
            .get_latest()
            .create_dom(&list_node, &editor_config(), &e)?;
        assert_eq!(
            dom_element.outer_html(),
            r#"<ul class="my-ul-list-class my-ul-list-class-1"></ul>"#
        );
        let new_list_node = create_list_node(ListType::Number, 1.0);
        let result = new_list_node
            .get_latest()
            .update_dom(&*list_node.get_latest());
        assert!(result);
        assert_eq!(
            dom_element.outer_html(),
            r#"<ul class="my-ul-list-class my-ul-list-class-1"></ul>"#
        );
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.exportDOM() round-trips the dir attribute
#[cfg(feature = "html")]
#[test]
fn list_node_export_dom_round_trips_the_dir_attribute() {
    use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
    let editor = editor();
    let input = r#"
        <ul dir="rtl">
          <li dir="rtl">שלום</li>
        </ul>
      "#;
    let inner = editor.clone();
    editor
        .update(
            move || {
                let root = get_root();
                root.clear()?;
                let dom = crate::dom::parse_from_string(input);
                root.append(&generate_nodes_from_dom(&inner, &dom)?)
            },
            discrete(),
        )
        .unwrap();
    let e = editor.clone();
    editor
        .read(move || {
            let list_node = get_root().get_first_child().unwrap();
            assert!(is_list_node(&list_node), "expected a ListNode at the root");
            assert_eq!(
                list_node.element_data().get_direction(),
                Some(crate::Direction::Rtl)
            );
            assert!(
                generate_html_from_nodes(&e, None)
                    .unwrap()
                    .contains(r#"<ul dir="rtl">"#)
            );
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.append() should properly transform a ListItemNode
#[test]
fn list_node_append_should_properly_transform_a_list_item_node() {
    let editor = editor();
    update(&editor, || {
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let list_item_node = create_list_item_node(None);
        let text_node = create_text_node("Hello");
        list_item_node.append(&[text_node])?;
        list_node.append(std::slice::from_ref(&list_item_node))?;
        assert_eq!(list_node.get_first_child(), Some(list_item_node));
        assert_eq!(
            list_node.get_first_child().unwrap().get_text_content(),
            "Hello"
        );
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.append() should properly transform a ListNode
#[test]
fn list_node_append_should_properly_transform_a_list_node() {
    let editor = editor();
    update(&editor, || {
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let nested_list_node = create_list_node(ListType::Bullet, 1.0);
        let list_item_node = create_list_item_node(None);
        list_item_node.append(&[create_text_node("Hello")])?;
        nested_list_node.append(&[list_item_node])?;
        list_node.append(std::slice::from_ref(&nested_list_node))?;
        let first = list_node.get_first_child().unwrap();
        assert!(is_list_item_node(&first));
        assert_eq!(first.get_first_child(), Some(nested_list_node));
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.append() should properly transform a ParagraphNode
#[test]
fn list_node_append_should_properly_transform_a_paragraph_node() {
    let editor = editor();
    update(&editor, || {
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("Hello")])?;
        list_node.append(&[paragraph])?;
        let first = list_node.get_first_child().unwrap();
        assert!(is_list_item_node(&first));
        assert_eq!(first.get_text_content(), "Hello");
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.append() should wrap an InlineNode in a ListItemNode without converting it to TextNode
#[test]
fn list_node_append_should_wrap_an_inline_node_in_a_list_item_node() {
    let editor = editor();
    update(&editor, || {
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let link_node = create_link_node("https://lexical.dev/", &LinkAttributes::default());
        list_node.append(&[link_node])?;
        get_root().append(&[list_node])
    });
    editor
        .read(|| {
            let list_node = get_root().get_first_child().unwrap();
            assert!(is_list_node(&list_node));
            let first_child = list_node.get_first_child().unwrap();
            assert!(is_list_item_node(&first_child));
            let wrapped_node = first_child.get_first_child().unwrap();
            assert!(is_link_node(&wrapped_node));
            assert_eq!(
                wrapped_node.get_url().as_deref(),
                Some("https://lexical.dev/")
            );
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.splice() should wrap multiple non-ListItem nodes in individual ListItem nodes
#[test]
fn list_node_splice_should_wrap_multiple_non_list_item_nodes() {
    let editor = editor();
    update(&editor, || {
        let list = list(ListType::Bullet, &[text_item("A"), text_item("D")]);
        get_root().append(std::slice::from_ref(&list))?;
        list.splice(1, 0, &[create_text_node("B"), create_text_node("C")])?;
        Ok(())
    });
    editor
        .read(|| {
            let list = get_root().get_first_child().unwrap();
            assert!(is_list_node(&list), "First child must be a ListNode");
            let children = list.get_children();
            assert_eq!(children.len(), 4);
            assert!(is_list_item_node(&children[1]));
            assert!(is_list_item_node(&children[2]));
            assert_ne!(children[1], children[2]);
            assert_eq!(children[1].get_text_content(), "B");
            assert_eq!(children[2].get_text_content(), "C");
        })
        .unwrap();
}

/// Each list item of the committed document, rendered with `createDOM`.
#[cfg(feature = "html")]
fn items_html(editor: &Editor) -> Vec<String> {
    fn all_items(node: &crate::NodeRef, out: &mut Vec<crate::NodeRef>) {
        for child in node.get_children() {
            if is_list_item_node(&child) {
                out.push(child.clone());
            }
            if child.get_latest().element().is_some() {
                all_items(&child, out);
            }
        }
    }
    let e = editor.clone();
    editor
        .read(move || {
            let mut items = Vec::new();
            all_items(&get_root(), &mut items);
            items
                .iter()
                .map(|i| outer_html(i, e.config(), &e).unwrap())
                .collect()
        })
        .unwrap()
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > Should update list children when switching from checklist to bullet
#[cfg(feature = "html")]
#[test]
fn should_update_list_children_when_switching_from_checklist_to_bullet() {
    let editor = editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[list(ListType::Check, &[item(&[])])])
    });
    assert_eq!(
        items_html(&editor),
        [r#"<li role="checkbox" tabindex="-1" aria-checked="false" value="1"></li>"#]
    );
    update(&editor, || {
        get_root()
            .get_first_child()
            .unwrap()
            .set_list_type(ListType::Bullet);
        Ok(())
    });
    assert_eq!(items_html(&editor), [r#"<li value="1"></li>"#]);
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > Should clear checklist attributes when nesting lists
#[cfg(feature = "html")]
#[test]
fn should_clear_checklist_attributes_when_nesting_lists() {
    let editor = editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[list(ListType::Check, &[item(&[]), item(&[])])])
    });
    assert_eq!(
        items_html(&editor),
        [
            r#"<li role="checkbox" tabindex="-1" aria-checked="false" value="1"></li>"#,
            r#"<li role="checkbox" tabindex="-1" aria-checked="false" value="2"></li>"#,
        ]
    );
    update(&editor, || {
        let list_node = get_root().get_first_child().unwrap();
        assert!(is_list_node(&list_node));
        let list_item_node = list_node.get_child_at_index(1).unwrap();
        assert!(is_list_item_node(&list_item_node));
        list_item_node.append(&[list(ListType::Bullet, &[item(&[])])])
    });
    assert_eq!(
        items_html(&editor),
        [
            r#"<li role="checkbox" tabindex="-1" aria-checked="false" value="1"></li>"#,
            r#"<li value="2"></li>"#,
            r#"<li value="1"></li>"#,
        ]
    );
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > $createListNode()
#[test]
fn create_list_node_function() {
    let editor = editor();
    update(&editor, || {
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let created_list_node = create_list_node(ListType::Bullet, 1.0);
        assert_eq!(list_node.get_type(), created_list_node.get_type());
        assert_eq!(list_node.get_parent(), created_list_node.get_parent());
        assert_eq!(list_node.get_list_tag(), created_list_node.get_list_tag());
        assert_ne!(list_node.get_key(), created_list_node.get_key());
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > $isListNode()
#[test]
fn is_list_node_function() {
    let editor = editor();
    update(&editor, || {
        assert!(is_list_node(&create_list_node(ListType::Bullet, 1.0)));
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > $createListNode() with tag name (backward compatibility)
#[test]
fn create_list_node_with_tag_name() {
    let editor = editor();
    update(&editor, || {
        let number_list = create_list_node(ListType::Number, 1.0);
        let bullet_list = create_list_node(ListType::Bullet, 1.0);
        assert_eq!(number_list.get_list_type(), Some(ListType::Number));
        assert_eq!(bullet_list.get_list_type(), Some(ListType::Bullet));
        Ok(())
    });
}

/// `$getListItemValues()`: the value of each text node's list item.
fn get_list_item_values() -> Vec<Option<f64>> {
    get_root()
        .get_all_text_nodes()
        .iter()
        .map(|node| {
            node.get_parent()
                .filter(is_list_item_node)
                .and_then(|p| p.get_list_item_value())
        })
        .collect()
}

// upstream: lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode subclassing tests ($config) > ListNode as-is > applies transform
#[test]
fn list_node_as_is_applies_transform() {
    let editor = editor();
    editor
        .update(
            || {
                let root = get_root();
                root.clear()?;
                root.append(&[list(
                    ListType::default(),
                    &[text_item("first"), text_item("second")],
                )])?;
                assert_eq!(get_list_item_values(), [Some(1.0), Some(1.0)]);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| assert_eq!(get_list_item_values(), [Some(1.0), Some(2.0)]))
        .unwrap();
}
