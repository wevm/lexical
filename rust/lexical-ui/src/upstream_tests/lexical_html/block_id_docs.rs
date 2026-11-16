//! Ports of `packages/lexical-html/src/__tests__/unit/BlockIdDocs.test.ts`:
//! the code examples of the Block IDs documentation page
//! (`packages/lexical-website/docs/extensions/block-ids.md`).

#![cfg(all(
    feature = "extension",
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "mark"
))]

use super::super::support::update;
use super::support::set_block_text;
use crate::extension::{
    BlockHandleInvalidReason, BlockHandleOptions, BlockHandleResult, BlockIdConfigOverrides,
    LexicalEditorWithDispose, block_id_extension, block_id_state, build_editor_from_extensions,
    get_block_by_id, get_block_handle, get_block_handles, get_block_id,
    get_extension_dependency_from_editor, resolve_block_handle,
};
use crate::html::{BlockIdHtmlConfigOverrides, block_id_html_extension};
use crate::link::{LinkAttributes, create_link_node, is_link_node};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::mark::create_mark_node;
use crate::node_state::set_state;
use crate::nodes::line_break::create_line_break_node;
use crate::nodes::tab::create_tab_node;
use crate::{
    AnyLexicalExtensionArgument, Editor, InitialEditorStateType, LexicalError, LexicalExtension,
    LexicalNode, NodeBase, NodeClass, NodeRef, PointType, Which, config_extension,
    create_paragraph_node, create_text_node, define_extension, get_root, get_selection, set_slot,
};
use std::cell::RefCell;
use std::rc::Rc;

/// An inline decorator without text.
#[derive(Clone, Debug, Default)]
struct EmptyInlineNode {
    base: NodeBase,
}

impl LexicalNode for EmptyInlineNode {
    fn get_type(&self) -> &'static str {
        "block-id-docs-empty-inline"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        true
    }
    fn decorate(&self, _editor: &Editor) -> Option<Rc<dyn std::any::Any>> {
        None
    }
}

fn paragraph(text: &str) -> NodeRef {
    let p = create_paragraph_node();
    p.append(&[create_text_node(text)]).unwrap();
    p
}

fn editor(
    name: &str,
    initial: Option<InitialEditorStateType>,
    dependencies: Vec<AnyLexicalExtensionArgument>,
    nodes: Vec<NodeClass>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: initial,
        dependencies,
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn rich_text() -> AnyLexicalExtensionArgument {
    crate::rich_text::rich_text_extension().into()
}

fn block_ids() -> AnyLexicalExtensionArgument {
    block_id_extension().into()
}

fn initial(f: impl Fn() -> Result<(), LexicalError> + 'static) -> Option<InitialEditorStateType> {
    Some(InitialEditorStateType::Update(Rc::new(move |_| f())))
}

#[derive(Clone, Copy, PartialEq)]
enum OpKind {
    Replace,
    InsertAfter,
    Delete,
}

/// 2. Applying what the agent answers, as on the page.
fn apply_ops(editor: &Editor, ops: &[(OpKind, &str, &str)]) {
    for (op, id, text) in ops {
        let (op, id, text) = (*op, (*id).to_owned(), (*text).to_owned());
        update(editor, move || {
            let Some(block) = get_block_by_id(&id)? else {
                return Ok(()); // Deleted meanwhile, or a mistyped id.
            };
            if op == OpKind::Delete {
                return block.remove(false);
            }
            let node = paragraph(&text);
            if op == OpKind::Replace {
                block.replace(&node, false)?; // Keeps the id.
                // A selection that ended inside the replaced block lost that
                // end: move it to the new block.
                if let Some(selection) = get_selection().filter(|s| s.is_range())
                    && [Which::Anchor, Which::Focus].into_iter().any(|which| {
                        !selection
                            .point(which)
                            .unwrap()
                            .get_node()
                            .unwrap()
                            .is_attached()
                    })
                {
                    node.select_end()?;
                }
            } else {
                block.insert_after(&node, true)?; // Gets a fresh id.
            }
            Ok(())
        });
    }
}

fn seed_seven_eight_nine(editor: &Editor) {
    update(editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&["seven", "eight", "nine"].map(paragraph))
    });
}

fn root_ids_and_texts(editor: &Editor) -> Vec<(Option<String>, String)> {
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|n| (get_block_id(n).unwrap(), n.get_text_content()))
                .collect()
        })
        .unwrap()
}

fn state_json(editor: &Editor) -> String {
    crate::json::stringify(&editor.get_editor_state().to_json(false)).unwrap()
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Setup and Addressing blocks
#[test]
fn setup_and_addressing_blocks() {
    // ## Setup
    let editor = editor("my-editor", None, vec![rich_text(), block_ids()], vec![]);
    seed_seven_eight_nine(&editor);
    // 1. What the agent reads (here, the top-level blocks).
    let blocks = root_ids_and_texts(&editor);
    let ids: Vec<String> = blocks.into_iter().map(|(id, _)| id.unwrap()).collect();
    let (b7, b8, b9) = (ids[0].clone(), ids[1].clone(), ids[2].clone());
    // A person's caret sits inside the block the agent rewrites.
    update(&editor, || {
        get_root().get_all_text_nodes()[0].select(Some(1), Some(1))?;
        Ok(())
    });
    // 2. What the agent answers.
    apply_ops(
        &editor,
        &[
            (OpKind::Replace, &b7, "New text"),
            (OpKind::InsertAfter, &b7, "Another paragraph"),
            (OpKind::Delete, &b9, ""),
        ],
    );
    let after = root_ids_and_texts(&editor);
    assert_eq!(
        after.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(),
        ["New text", "Another paragraph", "eight"]
    );
    assert_eq!(after[0].0.as_deref(), Some(b7.as_str()));
    assert_eq!(after[2].0.as_deref(), Some(b8.as_str()));
    let new_id = after[1].0.clone().unwrap();
    assert!(![&b7, &b8, &b9].contains(&&new_id));
    editor
        .read(|| assert!(get_block_by_id(&b9).unwrap().is_none()))
        .unwrap();

    // Why one update per operation: within one update, a block replaced
    // earlier is not found yet, and a block inserted where a removed block
    // sat takes the removed block's id.
    let (b7c, b8c) = (b7.clone(), b8.clone());
    update(&editor, move || {
        let again = paragraph("Again");
        get_block_by_id(&b7c)?.unwrap().replace(&again, false)?;
        again.select_end()?; // The caret was in b7.
        assert!(get_block_by_id(&b7c)?.is_none());
        get_block_by_id(&b8c)?.unwrap().remove(false)?;
        get_root().append(&[paragraph("Appended")])
    });
    editor
        .read(|| {
            assert_eq!(
                get_block_by_id(&b7).unwrap().unwrap().get_text_content(),
                "Again"
            );
            assert_eq!(
                get_block_by_id(&b8).unwrap().unwrap().get_text_content(),
                "Appended"
            );
        })
        .unwrap();

    // $setState(node, blockIdState, 'b7-intro')
    update(&editor, || {
        let node = create_paragraph_node();
        set_state(&node, &block_id_state(), Some("b7-intro".to_owned()))?;
        get_root().append(&[node])
    });
    editor
        .read(|| assert!(get_block_by_id("b7-intro").unwrap().is_some()))
        .unwrap();
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Addressing blocks: a selection from or into the replaced block
#[test]
fn addressing_blocks_a_selection_from_or_into_the_replaced_block() {
    for (from, to) in [(0, 1), (1, 2), (2, 1), (1, 1)] {
        let editor = editor("my-editor", None, vec![rich_text(), block_ids()], vec![]);
        seed_seven_eight_nine(&editor);
        let ids: Vec<Option<String>> = root_ids_and_texts(&editor)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        // The person's selection runs from block `from` to block `to`, and
        // the agent replaces the second block.
        update(&editor, move || {
            let texts = get_root().get_all_text_nodes();
            let selection = texts[from].select(Some(2), Some(2))?;
            selection.point_set(Which::Focus, texts[to].get_key(), 3, PointType::Text)
        });
        apply_ops(
            &editor,
            &[(OpKind::Replace, ids[1].as_deref().unwrap(), "New text")],
        );
        assert_eq!(
            root_ids_and_texts(&editor),
            [
                (ids[0].clone(), "seven".to_owned()),
                (ids[1].clone(), "New text".to_owned()),
                (ids[2].clone(), "nine".to_owned()),
            ],
            "{from} to {to}"
        );
        editor
            .read(|| {
                let selection = get_selection().unwrap();
                assert!(selection.is_range());
                for which in [Which::Anchor, Which::Focus] {
                    assert!(
                        selection
                            .point(which)
                            .unwrap()
                            .get_node()
                            .unwrap()
                            .is_attached()
                    );
                }
            })
            .unwrap();
        editor.dispose();
    }
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Headless
#[test]
fn headless() {
    let writer = editor(
        "writer",
        initial(|| get_root().append(&[paragraph("Intro")])),
        vec![rich_text()],
        vec![],
    );
    writer.read(|| {}).unwrap();
    let stored_json = state_json(&writer);

    let pipeline = editor(
        "docs-pipeline",
        Some(InitialEditorStateType::Null),
        vec![rich_text(), block_ids()],
        vec![],
    );
    pipeline
        .set_editor_state(&pipeline.parse_editor_state(&stored_json).unwrap(), None)
        .unwrap();
    let with_ids = state_json(&pipeline);
    // /"blockId":"[A-Za-z0-9_-]{21}"/
    let at = with_ids.find("\"blockId\":\"").expect("a blockId") + 11;
    let id = &with_ids[at..at + 22];
    assert!(
        id[..21]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            && id.ends_with('"'),
        "{with_ids}"
    );
    // Running it again changes nothing.
    pipeline
        .set_editor_state(&pipeline.parse_editor_state(&with_ids).unwrap(), None)
        .unwrap();
    assert_eq!(state_json(&pipeline), with_ids);
    writer.dispose();
    pipeline.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > HTML and Configuration
#[test]
fn html_and_configuration() {
    let n = Rc::new(std::cell::Cell::new(0));
    let editor = editor(
        "html",
        None,
        vec![
            rich_text(),
            config_extension(
                &block_id_extension(),
                [BlockIdConfigOverrides {
                    generate_id: Some(crate::extension::GenerateId(Rc::new(move || {
                        n.set(n.get() + 1);
                        format!("b{}", n.get())
                    }))),
                    ..Default::default()
                }],
            ),
            config_extension(
                &block_id_html_extension(),
                [BlockIdHtmlConfigOverrides {
                    export: Some(false),
                    ..Default::default()
                }],
            ),
        ],
        vec![],
    );
    // The first content of an empty editor is a load (positional ids);
    // later blocks get ids from generateId.
    for _ in 0..2 {
        update(&editor, || get_root().append(&[create_paragraph_node()]));
    }
    editor
        .read(|| {
            let last = get_root().get_last_child_or_throw().unwrap();
            assert_eq!(get_block_id(&last).unwrap().as_deref(), Some("b1"));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model
#[test]
fn showing_blocks_to_a_model() {
    const IDS: [&str; 2] = ["V1StGXR8_Z5jdHi6B-myT", "9xQeLm2Pq0aK7cTb_wYuA"];
    let editor = editor(
        "my-editor",
        initial(|| {
            for (i, text) in ["Q3 plan", "Revenue grew 4%."].into_iter().enumerate() {
                let node = paragraph(text);
                set_state(&node, &block_id_state(), Some(IDS[i].to_owned()))?;
                get_root().append(&[node])?;
            }
            Ok(())
        }),
        vec![rich_text(), block_ids()],
        vec![],
    );

    // 1. What the model reads, and the handles it is shown (id → handle).
    let (blocks, shown) = editor
        .read(|| {
            let blocks: Vec<(String, String)> = get_root()
                .get_children()
                .iter()
                .map(|n| (get_block_handle(n).unwrap().unwrap(), n.get_text_content()))
                .collect();
            (blocks, get_block_handles().unwrap())
        })
        .unwrap();
    assert_eq!(
        blocks,
        [
            ("2bNo".to_owned(), "Q3 plan".to_owned()),
            ("gTtF".to_owned(), "Revenue grew 4%.".to_owned())
        ]
    );
    let key_before = editor
        .read(|| {
            get_block_by_id(IDS[1])
                .unwrap()
                .unwrap()
                .get_key()
                .to_owned()
        })
        .unwrap();

    // 2. What the model answers, then 3. applying it, in its own update.
    let feedback = Rc::new(RefCell::new(Vec::<String>::new()));
    let apply = |block: &str, text: &str| {
        let (block, text) = (block.to_owned(), text.to_owned());
        let (feedback, shown) = (feedback.clone(), shown.clone());
        update(&editor, move || {
            let result =
                resolve_block_handle(&block, Some(&BlockHandleOptions { read: Some(shown) }))?;
            let message = match result {
                BlockHandleResult::Found { node } => (!set_block_text(&node, &text))
                    .then(|| format!("{block} holds line breaks or other blocks: edit its parts.")),
                BlockHandleResult::NotFound { handle } => {
                    Some(format!("No block {handle}: it may have been deleted."))
                }
                BlockHandleResult::Ambiguous { handle, candidates } => Some(format!(
                    "{handle} names several blocks: use one of {}.",
                    candidates.join(", ")
                )),
                BlockHandleResult::Invalid { handle, reason } => {
                    Some(if reason == BlockHandleInvalidReason::TooShort {
                        format!("{handle} is too short: block handles have at least 4 characters.")
                    } else {
                        format!("{handle} is not a block handle.")
                    })
                }
            };
            feedback.borrow_mut().extend(message);
            Ok(())
        });
    };
    apply("gTtF", "Revenue grew 6%.");
    editor
        .read(|| {
            let block = get_block_by_id(IDS[1]).unwrap().unwrap();
            assert_eq!(block.get_text_content(), "Revenue grew 6%.");
            // Changed in place: the same block, not a new one.
            assert_eq!(block.get_key(), key_before);
        })
        .unwrap();
    apply("gTtG", "typo");
    apply("gTt", "too short");
    apply("gTtF!", "invalid");
    // A collaborator adds a block whose handle would start with 2bNo: the
    // handle still names the block the model read.
    update(&editor, || {
        let node = create_paragraph_node();
        set_state(&node, &block_id_state(), Some("qfapyr".to_owned()))?;
        get_root().append(&[node])
    });
    apply("2bNo", "Q4 plan");
    // And deletes the other one.
    update(&editor, || get_block_by_id(IDS[1])?.unwrap().remove(false));
    apply("gTtF", "Revenue grew 7%.");
    // A paragraph with a line break is not changed in place.
    update(&editor, || {
        let block = get_block_by_id(IDS[0])?.unwrap();
        assert!(block.is_element());
        block.append(&[create_line_break_node()])
    });
    apply("2bNo", "Q5 plan");
    assert_eq!(
        *feedback.borrow(),
        [
            "No block gTtG: it may have been deleted.",
            "gTt is too short: block handles have at least 4 characters.",
            "gTtF! is not a block handle.",
            "No block gTtF: it may have been deleted.",
            "2bNo holds line breaks or other blocks: edit its parts.",
        ]
    );
    editor
        .read(|| {
            assert_eq!(
                get_block_by_id(IDS[0]).unwrap().unwrap().get_text_content(),
                "Q4 plan\n"
            )
        })
        .unwrap();
    editor.dispose();
}

/// Runs `refuse` on the root's children in an update and checks that the
/// editor state is unchanged.
fn assert_unchanged(editor: &Editor, refuse: impl Fn(Vec<NodeRef>) + 'static) {
    let before = state_json(editor);
    update(editor, move || {
        refuse(get_root().get_children());
        Ok(())
    });
    assert_eq!(state_json(editor), before);
}

fn null_editor(
    name: &str,
    extra: Vec<AnyLexicalExtensionArgument>,
    nodes: Vec<NodeClass>,
) -> LexicalEditorWithDispose {
    let mut dependencies = vec![rich_text()];
    dependencies.extend(extra);
    dependencies.push(block_ids());
    editor(
        name,
        Some(InitialEditorStateType::Null),
        dependencies,
        nodes,
    )
}

fn with_children(parent: NodeRef, children: &[NodeRef]) -> NodeRef {
    parent.append(children).unwrap();
    parent
}

fn set_root(nodes: Vec<NodeRef>) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(&nodes)
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText refuses blocks that are not just text
#[test]
fn set_block_text_refuses_blocks_that_are_not_just_text() {
    let editor = null_editor(
        "set-block-text-refuses",
        vec![crate::list::list_extension().into()],
        vec![NodeClass::of::<EmptyInlineNode>()],
    );
    update(&editor, || {
        let item =
            |text: &str| with_children(create_list_item_node(None), &[create_text_node(text)]);
        set_root(vec![
            with_children(
                create_paragraph_node(),
                &[
                    create_text_node("a"),
                    create_line_break_node(),
                    create_text_node("b"),
                ],
            ),
            with_children(
                create_list_node(ListType::Bullet, 1.0),
                &[item("one"), item("two")],
            ),
            // One item: the list's text is its item's text.
            with_children(create_list_node(ListType::Number, 1.0), &[item("one")]),
            // An inline decorator without text.
            with_children(
                create_paragraph_node(),
                &[
                    create_text_node("left"),
                    crate::create::<EmptyInlineNode>(),
                    create_text_node("right"),
                ],
            ),
        ])
    });
    assert_unchanged(&editor, |children| {
        let [line_break, list, one_item, decorated] = &children[..] else {
            panic!("four blocks");
        };
        assert!(!set_block_text(line_break, "a\nb"));
        assert!(!set_block_text(line_break, ""));
        assert!(!set_block_text(list, "one\n\ntwo"));
        assert_eq!(one_item.get_text_content(), "one");
        assert!(!set_block_text(one_item, "rewritten"));
        assert_eq!(decorated.get_text_content(), "leftright");
        assert!(!set_block_text(decorated, "changed"));
    });
    editor.dispose();
}

fn link(text: &str) -> NodeRef {
    with_children(
        create_link_node("https://example.com", &LinkAttributes::default()),
        &[create_text_node(text)],
    )
}

fn link_extension() -> AnyLexicalExtensionArgument {
    crate::link::link_extension().into()
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText refuses a block or inline element that hosts a slot
#[test]
fn set_block_text_refuses_a_block_or_inline_element_that_hosts_a_slot() {
    let editor = null_editor("set-block-text-slots", vec![link_extension()], vec![]);
    update(&editor, || {
        // A text-only block whose caption slot holds a line break.
        let host = paragraph("body");
        set_slot(
            &host,
            "caption",
            &with_children(
                create_paragraph_node(),
                &[
                    create_text_node("a"),
                    create_line_break_node(),
                    create_text_node("b"),
                ],
            ),
        )?;
        // A link, inline, that hosts a slot.
        let link = link("link");
        set_slot(&link, "note", &paragraph("note"))?;
        set_root(vec![
            host,
            with_children(create_paragraph_node(), &[create_text_node("see "), link]),
        ])
    });
    assert_unchanged(&editor, |children| {
        let (host, with_link) = (&children[0], &children[1]);
        assert!(!set_block_text(host, &host.get_text_content()));
        assert!(!set_block_text(host, "changed"));
        assert!(!set_block_text(with_link, &with_link.get_text_content()));
        assert!(!set_block_text(with_link, "see changed"));
    });
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText refuses special text nodes
#[test]
fn set_block_text_refuses_special_text_nodes() {
    let editor = null_editor("set-block-text-special", vec![], vec![]);
    update(&editor, || {
        let token = create_text_node("@Ada");
        token.set_mode("token")?;
        set_root(vec![
            // A tab: TabNode.spliceText throws for anything but a no-op.
            with_children(
                create_paragraph_node(),
                &[
                    create_text_node("a"),
                    create_tab_node(),
                    create_text_node("b"),
                ],
            ),
            // A token, which edits replace whole.
            with_children(create_paragraph_node(), &[create_text_node("see "), token]),
        ])
    });
    assert_unchanged(&editor, |children| {
        let (tab, token) = (&children[0], &children[1]);
        assert!(!set_block_text(tab, "a\tb"));
        assert!(!set_block_text(tab, "a\tbc"));
        assert!(!set_block_text(tab, "ab"));
        assert!(!set_block_text(token, "see @Bob"));
    });
    editor.dispose();
}

/// The first block's children as `kind:text`, `kind` from `kind_of`.
fn first_block_children(editor: &Editor, kind_of: fn(&NodeRef) -> &'static str) -> Vec<String> {
    editor
        .read(|| {
            let paragraph = get_root().get_first_child_or_throw().unwrap();
            assert!(paragraph.is_element());
            paragraph
                .get_children()
                .iter()
                .map(|c| format!("{}:{}", kind_of(c), c.get_text_content()))
                .collect()
        })
        .unwrap()
}

fn set_first_block_text(editor: &Editor, text: &str) {
    let text = text.to_owned();
    update(editor, move || {
        assert!(set_block_text(
            &get_root().get_first_child_or_throw()?,
            &text
        ));
        Ok(())
    });
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: text added at a link edge goes outside the link
#[test]
fn text_added_at_a_link_edge_goes_outside_the_link() {
    let editor = null_editor("set-block-text-link-edges", vec![link_extension()], vec![]);
    // [links: before, new text, children after]
    let cases: [(&str, &str, [&str; 3]); 5] = [
        (
            "mid",
            "See the plans for Q3.",
            ["text:See ", "link:the plan", "text:s for Q3."],
        ),
        (
            "mid",
            "See all the plan for Q3.",
            ["text:See all ", "link:the plan", "text: for Q3."],
        ),
        (
            "first",
            "Read the plan for Q3.",
            ["text:Read ", "link:the plan", "text: for Q3."],
        ),
        (
            "last",
            "See the plan!",
            ["text:See ", "link:the plan", "text:!"],
        ),
        // Inside the link, it stays in the link.
        (
            "mid",
            "See the new plan for Q3.",
            ["text:See ", "link:the new plan", "text: for Q3."],
        ),
    ];
    for (place, next, after) in cases {
        update(&editor, move || {
            let link = link("the plan");
            let children = match place {
                "first" => vec![link, create_text_node(" for Q3.")],
                "last" => vec![create_text_node("See "), link],
                _ => vec![create_text_node("See "), link, create_text_node(" for Q3.")],
            };
            set_root(vec![with_children(create_paragraph_node(), &children)])
        });
        set_first_block_text(&editor, next);
        assert_eq!(
            first_block_children(&editor, |c| if is_link_node(c) { "link" } else { "text" }),
            after,
            "{place} → {next}"
        );
    }
    editor.dispose();
}

fn mark_kind(c: &NodeRef) -> &'static str {
    if crate::mark::is_mark_node(c) {
        "mark"
    } else {
        "text"
    }
}

fn mark_link(text: &str) -> NodeRef {
    with_children(create_mark_node(&["comment".to_owned()]), &[link(text)])
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: text added at the edge of a link in a mark goes outside both
#[test]
fn text_added_at_the_edge_of_a_link_in_a_mark_goes_outside_both() {
    let editor = null_editor(
        "set-block-text-mark-link",
        vec![link_extension(), crate::mark::mark_extension().into()],
        vec![],
    );
    for (next, after) in [
        ("See plan!", vec!["text:See ", "mark:plan", "text:!"]),
        ("See the plan", vec!["text:See the ", "mark:plan"]),
    ] {
        update(&editor, || {
            set_root(vec![with_children(
                create_paragraph_node(),
                &[create_text_node("See "), mark_link("plan")],
            )])
        });
        set_first_block_text(&editor, next);
        assert_eq!(first_block_children(&editor, mark_kind), after, "{next}");
    }
    // At the start of the block: before both.
    update(&editor, || {
        set_root(vec![with_children(
            create_paragraph_node(),
            &[mark_link("Plan"), create_text_node(" is ready.")],
        )])
    });
    set_first_block_text(&editor, "The Plan is ready.");
    assert_eq!(
        first_block_children(&editor, mark_kind),
        ["text:The ", "mark:Plan", "text: is ready."]
    );
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText edits text in inline elements
#[test]
fn set_block_text_edits_text_in_inline_elements() {
    let editor = null_editor("set-block-text-link", vec![link_extension()], vec![]);
    update(&editor, || {
        set_root(vec![with_children(
            create_paragraph_node(),
            &[
                create_text_node("See "),
                link("the plan"),
                create_text_node(" for Q3."),
            ],
        )])
    });
    for text in ["See the new plan for Q3.", "See the new plan for Q4."] {
        set_first_block_text(&editor, text);
    }
    editor
        .read(|| {
            let paragraph = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(paragraph.get_text_content(), "See the new plan for Q4.");
            let link = paragraph.get_children()[1].clone();
            assert!(is_link_node(&link));
            assert_eq!(link.get_text_content(), "the new plan");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText edits only what differs
#[test]
fn set_block_text_edits_only_what_differs() {
    let editor = null_editor("set-block-text", vec![], vec![]);
    // [text nodes before, new text, text nodes after]
    let cases: [(&[&str], &str, &[&str]); 11] = [
        (
            &["Revenue grew 4%."],
            "Revenue grew 6%.",
            &["Revenue grew 6%."],
        ),
        (
            &["Revenue ", "grew", " 4%."],
            "Revenue fell 4%.",
            &["Revenue ", "fell", " 4%."],
        ),
        (
            &["Revenue ", "grew", " 4%."],
            "Revenue 4%.",
            &["Revenue 4%."],
        ),
        (&["ab", "cd", "ef"], "aXf", &["aXf"]),
        (&["ab", "cd"], "abXcd", &["abX", "cd"]),
        (&["ab", "cd"], "abd", &["ab", "d"]),
        (&["abc"], "abc", &["abc"]),
        (&["abc"], "", &[]),
        (&["abc"], "abcdef", &["abcdef"]),
        (&["abc"], "Xabc", &["Xabc"]),
        (&[], "new", &["new"]),
    ];
    for (before, next, after) in cases {
        let texts: Vec<String> = before.iter().map(|s| (*s).to_owned()).collect();
        update(&editor, move || {
            let block = create_paragraph_node();
            for (i, text) in texts.iter().enumerate() {
                // Alternate formats, so that Lexical keeps the nodes apart.
                let node = create_text_node(text);
                if i % 2 == 1 {
                    node.set_format_type("bold");
                } else {
                    node.set_format(0.0);
                }
                block.append(&[node])?;
            }
            set_root(vec![block])
        });
        let key = editor
            .read(|| {
                get_root()
                    .get_first_child_or_throw()
                    .unwrap()
                    .get_key()
                    .to_owned()
            })
            .unwrap();
        let text = next.to_owned();
        update(&editor, move || {
            set_block_text(&get_root().get_first_child_or_throw()?, &text);
            Ok(())
        });
        editor
            .read(|| {
                let block = get_root().get_first_child_or_throw().unwrap();
                assert_eq!(block.get_key(), key, "{before:?}");
                assert_eq!(
                    block
                        .get_all_text_nodes()
                        .iter()
                        .map(NodeRef::get_text_content)
                        .collect::<Vec<_>>(),
                    after,
                    "{before:?} → {next}"
                );
            })
            .unwrap();
    }
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Handles and collaboration: a block added since whose hash starts the same way
#[test]
fn handles_and_collaboration_a_block_added_since_whose_hash_starts_the_same_way() {
    let editor = editor(
        "my-editor",
        initial(|| {
            let node = paragraph("shown");
            set_state(&node, &block_id_state(), Some("p13e".to_owned()))?;
            get_root().append(&[node])
        }),
        vec![rich_text(), block_ids()],
        vec![],
    );
    let shown = editor.read(|| get_block_handles().unwrap()).unwrap();
    assert_eq!(
        shown
            .iter()
            .find(|(id, _)| id == "p13e")
            .map(|(_, h)| h.as_str()),
        Some("0ydd")
    );
    update(&editor, || {
        get_block_by_id("p13e")?.unwrap().remove(false)?;
        // Its hash starts with 0ydd too.
        let node = paragraph("new");
        set_state(&node, &block_id_state(), Some("p20d".to_owned()))?;
        get_root().append(&[node])
    });
    editor
        .read(|| {
            // Without the read, the block added since takes the handle.
            let BlockHandleResult::Found { node } = resolve_block_handle("0ydd", None).unwrap()
            else {
                panic!("expected found");
            };
            assert_eq!(get_block_id(&node).unwrap().as_deref(), Some("p20d"));
            // With it, the block the model read is gone.
            let result =
                resolve_block_handle("0ydd", Some(&BlockHandleOptions { read: Some(shown) }))
                    .unwrap();
            assert_eq!(result.status(), "not-found");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Handles in HTML
#[test]
fn handles_in_html() {
    let editor = editor(
        "my-editor",
        None,
        vec![
            rich_text(),
            block_ids(),
            config_extension(
                &block_id_html_extension(),
                [BlockIdHtmlConfigOverrides {
                    handles: Some(true),
                    ..Default::default()
                }],
            ),
        ],
        vec![],
    );
    let output = get_extension_dependency_from_editor(&editor, &block_id_html_extension())
        .unwrap()
        .output
        .unwrap();
    assert!(output.export.value());
    assert!(output.handles.value());
    editor.dispose();
}
