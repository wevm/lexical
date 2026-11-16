//! Ports of `packages/lexical-list/src/__tests__/unit/ListImportExtension.test.ts`,
//! less `wrapper items come from ListNode.createListItemNode()`, which
//! subclasses `ListNode` and `ListItemNode` (rust/parity/skips.json).

use super::support::{build, discrete};
use crate::extension::{
    LexicalEditorWithDispose, horizontal_rule_extension, node_structure_extension,
};
use crate::html::import::generate_nodes_from_dom_via_extension;
use crate::list::{
    ListType, is_list_item_node, is_list_node, list_extension, word_list_import_extension,
};
use crate::{Editor, NodeRef, get_root, js_trim};

fn build_editor() -> LexicalEditorWithDispose {
    build("list-host", vec![list_extension().into()])
}

/// `$generate(html)`: the active editor's `DOMImportExtension` import of
/// `html` in a document's body.
fn generate(html: &str) -> Result<Vec<NodeRef>, crate::LexicalError> {
    let dom =
        crate::dom::parse_from_string(&format!("<!doctype html><html><body>{html}</body></html>"));
    generate_nodes_from_dom_via_extension(&dom, None)
}

fn import_into(editor: &Editor, html: &str) {
    let html = html.to_owned();
    editor
        .update(
            move || {
                let nodes = generate(&html)?;
                let root = get_root();
                root.clear()?;
                root.append(&nodes)
            },
            discrete(),
        )
        .unwrap();
}

fn root_list() -> NodeRef {
    let node = get_root().get_first_child().unwrap();
    assert!(is_list_node(&node), "expected ListNode at root");
    node
}

fn items(list: &NodeRef) -> Vec<NodeRef> {
    list.get_children()
        .into_iter()
        .filter(is_list_item_node)
        .collect()
}

fn trimmed_texts(list: &NodeRef) -> Vec<String> {
    items(list)
        .iter()
        .map(|li| js_trim(&li.get_text_content()).to_owned())
        .collect()
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > <ul><li>a</li><li>b</li></ul> → bullet list with two items
#[test]
fn ul_li_a_li_b_bullet_list_with_two_items() {
    let editor = build_editor();
    import_into(&editor, "<ul><li>a</li><li>b</li></ul>");
    editor
        .read(|| {
            let list = root_list();
            assert_eq!(list.get_list_type(), Some(ListType::Bullet));
            let items = items(&list);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].get_text_content(), "a");
            assert_eq!(items[1].get_text_content(), "b");
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > <ol start="3"> → number list with start=3
#[test]
fn ol_start_3_number_list_with_start_3() {
    let editor = build_editor();
    import_into(&editor, r#"<ol start="3"><li>x</li></ol>"#);
    editor
        .read(|| {
            let list = root_list();
            assert_eq!(list.get_list_type(), Some(ListType::Number));
            assert_eq!(list.get_list_start(), Some(3.0));
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > GitHub task-list-item → checklist item
#[test]
fn github_task_list_item_checklist_item() {
    let editor = build_editor();
    import_into(
        &editor,
        r#"<ul class="contains-task-list"><li class="task-list-item"><input type="checkbox" checked/>done</li><li class="task-list-item"><input type="checkbox"/>todo</li></ul>"#,
    );
    editor
        .read(|| {
            let list = root_list();
            assert_eq!(list.get_list_type(), Some(ListType::Check));
            let items = items(&list);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].get_checked(), Some(true));
            assert_eq!(items[1].get_checked(), Some(false));
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > aria-checked drives checklist state
#[test]
fn aria_checked_drives_checklist_state() {
    let editor = build_editor();
    import_into(&editor, r#"<ul><li aria-checked="true">a</li></ul>"#);
    editor
        .read(|| {
            let list = root_list();
            assert_eq!(list.get_list_type(), Some(ListType::Check));
            assert_eq!(items(&list)[0].get_checked(), Some(true));
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > stray text inside <ul> gets wrapped via $normalizeListChildren
#[test]
fn stray_text_inside_ul_gets_wrapped() {
    let editor = build_editor();
    import_into(&editor, "<ul>stray <li>real item</li></ul>");
    editor
        .read(|| {
            let items = items(&root_list());
            assert!(!items.is_empty());
            assert!(
                items
                    .iter()
                    .any(|i| i.get_text_content().contains("real item"))
            );
        })
        .unwrap();
}

// MS Word paste

fn build_word_paste_editor() -> LexicalEditorWithDispose {
    build("word-paste-host", vec![word_list_import_extension().into()])
}

fn import_html_document(editor: &Editor, html: &str) {
    let html = html.to_owned();
    editor
        .update(
            move || {
                let dom = crate::dom::parse_from_string(&html);
                let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
                let root = get_root();
                root.clear()?;
                root.splice(0, 0, &nodes)
            },
            discrete(),
        )
        .unwrap();
}

const WORD_HTML_WITH_LISTS: &str = r#"<!doctype html>
<html xmlns:o="urn:schemas-microsoft-com:office:office"
      xmlns:w="urn:schemas-microsoft-com:office:word">
<head>
  <meta name="Generator" content="Microsoft Word 15">
</head>
<body>
  <p class="MsoNormal">Body text<o:p></o:p></p>
  <p class="MsoListParagraphCxSpFirst" style="mso-list:l2 level1 lfo1">
    <span><span style="mso-list:Ignore">1.</span></span>
    Numbered List Item 1<o:p></o:p>
  </p>
  <p class="MsoListParagraphCxSpMiddle" style="mso-list:l2 level1 lfo1">
    <span><span style="mso-list:Ignore">2.</span></span>
    Numbered List Item 2<o:p></o:p>
  </p>
  <p class="MsoListParagraphCxSpLast" style="mso-list:l2 level1 lfo1">
    <span><span style="mso-list:Ignore">3.</span></span>
    Numbered List Item 3<o:p></o:p>
  </p>
  <p class="MsoNormal"><o:p>&nbsp;</o:p></p>
  <p class="MsoListParagraphCxSpFirst" style="mso-list:l0 level1 lfo2">
    <span><span style="mso-list:Ignore">&middot;</span></span>
    Bullet List Item 1<o:p></o:p>
  </p>
  <p class="MsoListParagraphCxSpLast" style="mso-list:l0 level1 lfo2">
    <span><span style="mso-list:Ignore">&middot;</span></span>
    Bullet List Item 2<o:p></o:p>
  </p>
  <p class="MsoNormal"><o:p>&nbsp;</o:p></p>
  <p class="MsoListParagraphCxSpFirst" style="mso-list:l1 level1 lfo3">
    <span><span style="mso-list:Ignore">1)</span></span>
    Outline numbered 1<o:p></o:p>
  </p>
  <p class="MsoListParagraphCxSpMiddle" style="mso-list:l1 level2 lfo3">
    <span><span style="mso-list:Ignore">a)</span></span>
    Outline numbered 1.a<o:p></o:p>
  </p>
  <p class="MsoListParagraphCxSpMiddle" style="mso-list:l1 level2 lfo3">
    <span><span style="mso-list:Ignore">b)</span></span>
    Outline numbered 1.b<o:p></o:p>
  </p>
  <p class="MsoListParagraphCxSpLast" style="mso-list:l1 level1 lfo3">
    <span><span style="mso-list:Ignore">2)</span></span>
    Outline numbered 2<o:p></o:p>
  </p>
</body>
</html>"#;

fn root_lists() -> Vec<NodeRef> {
    get_root()
        .get_children()
        .into_iter()
        .filter(is_list_node)
        .collect()
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::MS Word paste — preprocess-installed overlay > converts MsoListParagraph runs into nested ListNodes
#[test]
fn converts_mso_list_paragraph_runs_into_nested_list_nodes() {
    let editor = build_word_paste_editor();
    import_html_document(&editor, WORD_HTML_WITH_LISTS);
    editor
        .read(|| {
            let lists = root_lists();
            assert_eq!(lists.len(), 3);
            let [numbered, bullet, outline] = &lists[..] else {
                unreachable!()
            };
            assert_eq!(numbered.get_list_type(), Some(ListType::Number));
            assert_eq!(
                trimmed_texts(numbered),
                [
                    "Numbered List Item 1",
                    "Numbered List Item 2",
                    "Numbered List Item 3"
                ]
            );
            assert_eq!(bullet.get_list_type(), Some(ListType::Bullet));
            assert_eq!(
                trimmed_texts(bullet),
                ["Bullet List Item 1", "Bullet List Item 2"]
            );
            assert_eq!(outline.get_list_type(), Some(ListType::Number));
            let outline_items = items(outline);
            assert_eq!(outline_items.len(), 3);
            let first_content = outline_items[0]
                .get_first_child()
                .expect("expected first content child");
            assert_eq!(
                js_trim(&first_content.get_text_content()),
                "Outline numbered 1"
            );
            let nested = outline_items[1].get_first_child().unwrap();
            assert!(
                is_list_node(&nested),
                "expected nested sublist on wrapper item"
            );
            assert_eq!(nested.get_list_type(), Some(ListType::Number));
            assert_eq!(
                trimmed_texts(&nested),
                ["Outline numbered 1.a", "Outline numbered 1.b"]
            );
            let last_content = outline_items[2]
                .get_first_child()
                .expect("expected last content child");
            assert_eq!(
                js_trim(&last_content.get_text_content()),
                "Outline numbered 2"
            );
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::MS Word paste — preprocess-installed overlay > survives the stylesheet-inlining preprocess
#[test]
fn survives_the_stylesheet_inlining_preprocess() {
    let editor = build_word_paste_editor();
    import_html_document(
        &editor,
        &WORD_HTML_WITH_LISTS.replacen(
            "</head>",
            r#"<style>
          p.MsoListParagraphCxSpFirst, p.MsoListParagraphCxSpMiddle,
          p.MsoListParagraphCxSpLast, p.MsoListParagraph
            {margin-top:0in; margin-left:.5in;}
        </style></head>"#,
            1,
        ),
    );
    editor
        .read(|| {
            let lists = root_lists();
            assert_eq!(lists.len(), 3);
            assert_eq!(lists[0].get_list_type(), Some(ListType::Number));
            assert_eq!(lists[1].get_list_type(), Some(ListType::Bullet));
            let outline_items = items(&lists[2]);
            assert_eq!(outline_items.len(), 3);
            let nested = outline_items[1].get_first_child().unwrap();
            assert!(
                is_list_node(&nested),
                "expected nested sublist on wrapper item"
            );
            assert_eq!(
                trimmed_texts(&nested),
                ["Outline numbered 1.a", "Outline numbered 1.b"]
            );
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::MS Word paste — preprocess-installed overlay > without the Generator meta the overlay is not installed
#[test]
fn without_the_generator_meta_the_overlay_is_not_installed() {
    let editor = build_word_paste_editor();
    // `.replace(/<meta name="Generator"[^>]*>/, '')`.
    let start = WORD_HTML_WITH_LISTS
        .find(r#"<meta name="Generator""#)
        .unwrap();
    let end = start + WORD_HTML_WITH_LISTS[start..].find('>').unwrap() + 1;
    let html_without_meta = format!(
        "{}{}",
        &WORD_HTML_WITH_LISTS[..start],
        &WORD_HTML_WITH_LISTS[end..]
    );
    import_html_document(&editor, &html_without_meta);
    editor
        .read(|| assert!(!get_root().get_children().iter().any(is_list_node)))
        .unwrap();
}

// ListItemNode block flattening

/// `$liChildTypes()`: `type:text` of each child of the first list item.
fn li_child_types() -> Vec<String> {
    let list = get_root().get_first_child().unwrap();
    assert!(is_list_node(&list), "expected a ListNode");
    let li = list.get_first_child().unwrap();
    assert!(is_list_item_node(&li), "expected a ListItemNode");
    li.get_children()
        .iter()
        .map(|c| format!("{}:{}", c.get_type(), c.get_text_content()))
        .collect()
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListItemNode block flattening > keeps contiguous inlines together, breaking only at block boundaries
#[test]
fn keeps_contiguous_inlines_together() {
    let editor = build_editor();
    import_into(&editor, "<ul><li>a <b>b</b><div>c</div></li></ul>");
    editor
        .read(|| {
            assert_eq!(
                li_child_types(),
                ["text:a ", "text:b", "linebreak:\n", "text:c"]
            );
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListItemNode block flattening > treats a non-paragraph block (<hr>) as a boundary, not inline content
#[test]
fn treats_a_non_paragraph_block_hr_as_a_boundary() {
    let editor = build(
        "list-hr-host",
        vec![horizontal_rule_extension().into(), list_extension().into()],
    );
    import_into(&editor, "<ul><li>x<hr />y</li></ul>");
    editor
        .read(|| {
            let types: Vec<String> = li_child_types()
                .iter()
                .map(|s| s.split(':').next().unwrap().to_owned())
                .collect();
            assert_eq!(
                types,
                ["text", "linebreak", "horizontalrule", "linebreak", "text"]
            );
        })
        .unwrap();
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListItemNode block flattening > with NodeStructureExtension, no line break is left next to a block decorator
#[test]
fn with_node_structure_extension_no_line_break_is_left_next_to_a_block_decorator() {
    let editor = build(
        "list-hr-structure-host",
        vec![
            horizontal_rule_extension().into(),
            list_extension().into(),
            node_structure_extension().into(),
        ],
    );
    fn shape_of(node: &NodeRef) -> String {
        if is_list_node(node) {
            let mut parts = vec![node.get_list_start().unwrap().to_string()];
            for item in node.get_children() {
                parts.push(if item.is_element() {
                    format!(
                        "[{}]",
                        item.get_children()
                            .iter()
                            .map(|c| c.get_type().to_owned())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                } else {
                    item.get_type().to_owned()
                });
            }
            format!("[{}]", parts.join(","))
        } else {
            node.get_type().to_owned()
        }
    }
    let shapes = |editor: &Editor| {
        editor
            .read(|| {
                get_root()
                    .get_children()
                    .iter()
                    .map(shape_of)
                    .collect::<Vec<_>>()
            })
            .unwrap()
    };
    import_into(&editor, "<ol><li>x</li><li>a<hr />b</li><li>y</li></ol>");
    assert_eq!(
        shapes(&editor),
        ["[1,[text],[text]]", "horizontalrule", "[3,[text],[text]]"]
    );
    import_into(&editor, "<ul><li>a<p>b</p><hr /><p>c</p>d</li></ul>");
    assert_eq!(
        shapes(&editor),
        [
            "[1,[text,linebreak,text]]",
            "horizontalrule",
            "[1,[text,linebreak,text]]"
        ]
    );
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListItemNode block flattening > with NodeStructureExtension, a pasted link around a list item stays on its text
#[cfg(feature = "rich-text")]
#[test]
fn with_node_structure_extension_a_pasted_link_around_a_list_item_stays_on_its_text() {
    use crate::link::{is_link_node, link_extension};
    use crate::{create_paragraph_node, create_text_node, get_selection};
    fn shape_of(node: &NodeRef) -> String {
        if is_link_node(node) {
            let mut parts = vec![format!("link:{}", node.get_url().unwrap_or_default())];
            parts.extend(node.get_children().iter().map(shape_of));
            format!("[{}]", parts.join(","))
        } else if node.is_element() {
            let mut parts = vec![node.get_type().to_owned()];
            parts.extend(node.get_children().iter().map(shape_of));
            format!("[{}]", parts.join(","))
        } else {
            node.get_text_content()
        }
    }
    let run = |with_structure: bool| -> Vec<String> {
        let mut dependencies = vec![
            crate::rich_text::rich_text_extension().into(),
            link_extension().into(),
            list_extension().into(),
        ];
        if with_structure {
            dependencies.push(node_structure_extension().into());
        }
        let editor = build("list-link-structure-host", dependencies);
        editor
            .update(
                || {
                    let text = create_text_node("AB");
                    let paragraph = create_paragraph_node();
                    paragraph.append(std::slice::from_ref(&text))?;
                    let root = get_root();
                    root.clear()?;
                    root.append(&[paragraph])?;
                    text.select(Some(2), Some(2)).map(|_| ())
                },
                discrete(),
            )
            .unwrap();
        let e = editor.clone();
        editor
            .update(
                move || {
                    let data = crate::DataTransfer::new();
                    data.set_data("text/html", r#"<a href="https://a">l<li>x</li></a>"#);
                    crate::clipboard::insert_data_transfer_for_rich_text(
                        &data,
                        &get_selection().unwrap(),
                        &e,
                    )
                },
                discrete(),
            )
            .unwrap();
        editor
            .read(|| get_root().get_children().iter().map(shape_of).collect())
            .unwrap()
    };
    assert_eq!(run(true), run(false));
    assert_eq!(
        run(true),
        [
            "[paragraph,AB,[link:https://a,l]]",
            "[list,[listitem,[link:https://a,x]]]"
        ]
    );
}

// upstream: lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListItemNode block flattening > preserves a nested list instead of flattening it
#[test]
fn preserves_a_nested_list_instead_of_flattening_it() {
    let editor = build_editor();
    import_into(&editor, "<ul><li>parent<ul><li>child</li></ul></li></ul>");
    editor
        .read(|| {
            fn find_nested(node: &NodeRef) -> bool {
                if is_list_item_node(node) {
                    return node.get_children().iter().any(is_list_node);
                }
                is_list_node(node) && node.get_children().iter().any(find_nested)
            }
            let outer = get_root().get_first_child().unwrap();
            assert!(is_list_node(&outer), "expected a ListNode");
            assert!(outer.get_children().iter().any(find_nested));
        })
        .unwrap();
}
