//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts`.
//!
//! Upstream's `build` also lists `TableExtension`; the editors here have it
//! with the `table` feature, which the tests that make a table (in
//! [`tables`]) need.

#![cfg(all(
    feature = "rich-text",
    feature = "list",
    feature = "history",
    feature = "markdown",
    feature = "clipboard",
    feature = "code",
    feature = "link"
))]

use super::super::support::update;
use super::support::{
    as_edited, caret, counter, nth, paragraph, seed, set_root_children, some, strings,
    update_tagged,
};
use crate::extension::positional_block_id::{
    derive_duplicate_id, derive_positional_id, document_key,
};
use crate::extension::{
    BlockIdConfigOverrides, GenerateId, IsIdentifiedBlock, LexicalEditorWithDispose,
    block_id_extension, block_id_state, build_editor_from_extensions, create_horizontal_rule_node,
    default_is_identified_block, generate_block_id, get_block_by_id, get_block_id,
    get_extension_dependency_from_editor, horizontal_rule_extension, is_root_child_block,
};
use crate::json::Value;
use crate::list::{
    INSERT_UNORDERED_LIST_COMMAND, ListType, create_list_item_node, create_list_node,
};
use crate::rich_text::{HeadingTagType, create_heading_node, create_quote_node, is_heading_node};
use crate::selection::set_blocks_type;
use crate::{
    COLLABORATION_TAG, DELETE_CHARACTER_COMMAND, Editor, HISTORY_PUSH_TAG,
    INSERT_PARAGRAPH_COMMAND, InitialEditorStateType, LexicalError, LexicalExtension, NodeRef,
    PASTE_TAG, PointType, REDO_COMMAND, UNDO_COMMAND, UpdateOptions, Which, config_extension,
    copy_node, create_paragraph_node, create_text_node, define_extension, get_root, get_selection,
    get_state, parse_serialized_node, set_state,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// `D(id, k)`: the id of the k-th later holder of the repeated id `id`.
fn dup(id: &str, k: usize) -> String {
    derive_duplicate_id(id, &k.to_string())
}

#[derive(Default)]
struct BuildOptions {
    generate_id: Option<GenerateId>,
    is_identified_block: Option<IsIdentifiedBlock>,
    disabled: bool,
    /// Skip [`as_edited`]: the first content the editor gets is a load.
    fresh: bool,
    history: bool,
}

/// `build(options)`.
fn build(options: BuildOptions) -> LexicalEditorWithDispose {
    let clock = Rc::new(Cell::new(0.0));
    let mut dependencies = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::list::list_extension().into(),
    ];
    #[cfg(feature = "table")]
    dependencies.push(crate::table::table_extension().into());
    dependencies.push(horizontal_rule_extension().into());
    if options.history {
        dependencies.push(config_extension(
            &crate::history::history_extension(),
            [crate::history::HistoryConfigOverrides {
                // History is disabled by default where there is no window.
                disabled: Some(false),
                now: Some(crate::history::HistoryNow(Rc::new(move || {
                    clock.set(clock.get() + 10000.0);
                    clock.get()
                }))),
                ..Default::default()
            }],
        ));
    }
    dependencies.push(config_extension(
        &block_id_extension(),
        [BlockIdConfigOverrides {
            generate_id: Some(options.generate_id.unwrap_or_else(|| counter("b"))),
            is_identified_block: options.is_identified_block,
            disabled: Some(options.disabled),
            ..Default::default()
        }],
    ));
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new("[block-id-test]", ())
    })
    .into()])
    .unwrap();
    if options.fresh {
        editor
    } else {
        as_edited(editor)
    }
}

fn build_default() -> LexicalEditorWithDispose {
    build(BuildOptions::default())
}

/// `P(editor, place)`: the positional id of `place` in the editor's
/// current content.
fn positional(editor: &Editor, place: &str) -> String {
    derive_positional_id(&editor.read(document_key).unwrap(), place)
}

/// `aliases(editor)`: positional ids of places as `@place`.
fn aliases(editor: &Editor) -> HashMap<String, String> {
    let key = editor.read(document_key).unwrap();
    let mut map = HashMap::new();
    for i in 0..32 {
        let mut places = vec![i.to_string()];
        places.extend((0..8).map(|j| format!("{i}.{j}")));
        for place in places {
            map.insert(derive_positional_id(&key, &place), format!("@{place}"));
            let retry = format!("{place}#1");
            map.insert(derive_positional_id(&key, &retry), format!("@{retry}"));
        }
    }
    map
}

fn alias(shown: &HashMap<String, String>, id: Option<String>) -> Option<String> {
    id.map(|id| shown.get(&id).cloned().unwrap_or(id))
}

fn show(id: Option<String>) -> String {
    id.unwrap_or_else(|| "null".to_owned())
}

/// `blocks(editor)`: `type:text:id` for each child of the root.
fn blocks(editor: &Editor) -> Vec<String> {
    let shown = aliases(editor);
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|node| {
                    format!(
                        "{}:{}:{}",
                        node.get_type(),
                        node.get_text_content(),
                        show(alias(&shown, get_block_id(node).unwrap()))
                    )
                })
                .collect()
        })
        .unwrap()
}

/// `ids(editor)`: the ids of the root's children, aliased.
fn ids(editor: &Editor) -> Vec<Option<String>> {
    let shown = aliases(editor);
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|node| alias(&shown, get_block_id(node).unwrap()))
                .collect()
        })
        .unwrap()
}

/// The ids of the root's children, unaliased.
fn raw_ids(editor: &Editor) -> Vec<Option<String>> {
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|node| get_block_id(node).unwrap())
                .collect()
        })
        .unwrap()
}

/// `$deepCopy(node)`: a copy that keeps NodeState, as a Lexical-JSON paste
/// does.
fn deep_copy(node: &NodeRef) -> NodeRef {
    fn serialize(n: &NodeRef) -> Value {
        let mut json = n.export_json(false);
        if n.is_element() {
            json.insert(
                "children",
                Value::Array(n.get_children().iter().map(serialize).collect()),
            );
        }
        Value::Object(json)
    }
    parse_serialized_node(&serialize(node)).unwrap()
}

/// `$selectBlocks(from, to)`: from the start of block `from` to the end of
/// block `to`.
fn select_blocks(from: usize, to: usize) -> Result<(), LexicalError> {
    let first = nth(from).get_first_descendant().unwrap();
    let last = nth(to).get_last_descendant().unwrap();
    assert!(first.is_text() && last.is_text());
    first.select(Some(0), Some(0))?;
    let range = get_selection()
        .filter(crate::SelectionRef::is_range)
        .unwrap();
    range.point_set(
        Which::Focus,
        last.get_key(),
        last.get_text_content_size(),
        PointType::Text,
    )?;
    Ok(())
}

/// `paragraphJSON(text, id)`, with `id` as JSON text.
fn paragraph_json(text: &str, id: Option<&str>) -> String {
    let state = id.map_or(String::new(), |id| format!(r#""$":{{"blockId":{id}}},"#));
    format!(
        r#"{{{state}"children":[{{"detail":0,"format":0,"mode":"normal","style":"","text":{},"type":"text","version":1}}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}}"#,
        crate::json::stringify(&Value::from(text)).unwrap()
    )
}

/// The JSON text of a string id.
fn q(id: &str) -> String {
    crate::json::stringify(&Value::from(id)).unwrap()
}

/// `docJSON(children)`.
fn doc_json(children: &[String]) -> String {
    format!(
        r#"{{"root":{{"children":[{}],"direction":null,"format":"","indent":0,"type":"root","version":1}}}}"#,
        children.join(",")
    )
}

/// `editor.setEditorState(editor.parseEditorState(json))`.
fn load(editor: &Editor, json: &str) {
    editor
        .set_editor_state(&editor.parse_editor_state(json).unwrap(), None)
        .unwrap();
}

/// `editor.getEditorState().toJSON()`.
fn state_json(editor: &Editor) -> Value {
    editor.get_editor_state().to_json(false)
}

/// `outline(editor)`: every element and decorator below the root as
/// `type`, with `:id` when it has one, indented by depth.
fn outline(editor: &Editor) -> Vec<String> {
    let shown = aliases(editor);
    editor
        .read(|| {
            let mut out = Vec::new();
            fn walk(
                node: &NodeRef,
                depth: isize,
                shown: &HashMap<String, String>,
                out: &mut Vec<String>,
            ) {
                if node.is_text() {
                    return;
                }
                if depth >= 0 {
                    let id = alias(shown, get_block_id(node).unwrap());
                    out.push(format!(
                        "{}{}{}",
                        "  ".repeat(depth as usize),
                        node.get_type(),
                        id.map_or(String::new(), |id| format!(":{id}"))
                    ));
                }
                if node.is_element() {
                    for child in node.get_children() {
                        walk(&child, depth + 1, shown, out);
                    }
                }
            }
            walk(&get_root(), -1, &shown, &mut out);
            out
        })
        .unwrap()
}

/// Replaces a trailing `:b<digits>` with `:id`.
fn generic_id(line: &str) -> String {
    if let Some((head, tail)) = line.rsplit_once(":b")
        && !tail.is_empty()
        && tail.bytes().all(|c| c.is_ascii_digit())
    {
        return format!("{head}:id");
    }
    line.to_owned()
}

/// `expectAllIndexed(editor)`: every identified block is found by
/// `$getBlockById` under its own id.
fn expect_all_indexed(editor: &Editor) {
    let predicate = get_extension_dependency_from_editor(editor, &block_id_extension())
        .unwrap()
        .output
        .unwrap()
        .is_identified_block
        .clone();
    editor
        .read(|| {
            fn walk(node: &NodeRef, predicate: &IsIdentifiedBlock) {
                if (predicate.0)(node) {
                    let id = get_block_id(node).unwrap();
                    let id = id.unwrap_or_else(|| panic!("{} has an id", node.get_type()));
                    assert_eq!(
                        get_block_by_id(&id)
                            .unwrap()
                            .map(|n| n.get_key().to_owned()),
                        Some(node.get_key().to_owned()),
                        "$getBlockById({id})"
                    );
                }
                if node.is_element() {
                    for child in node.get_children() {
                        walk(&child, predicate);
                    }
                }
            }
            walk(&get_root(), &predicate);
        })
        .unwrap();
}

/// `expectUnique(editor)`.
fn expect_unique(editor: &Editor) {
    let all = ids(editor);
    assert!(all.iter().all(Option::is_some));
    assert_eq!(all.iter().collect::<HashSet<_>>().len(), all.len());
}

fn discrete_remote() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        skip_transforms: true,
        tags: vec![COLLABORATION_TAG.to_owned()],
        ..UpdateOptions::default()
    }
}

fn text(node: Option<NodeRef>) -> String {
    node.expect("a block").get_text_content()
}

fn heading(tag: HeadingTagType, text: &str) -> NodeRef {
    let h = create_heading_node(tag);
    h.append(&[create_text_node(text)]).unwrap();
    h
}

fn bullet_list(items: &[&str]) -> NodeRef {
    let list = create_list_node(ListType::Bullet, 1.0);
    for t in items {
        let item = create_list_item_node(None);
        item.append(&[create_text_node(t)]).unwrap();
        list.append(&[item]).unwrap();
    }
    list
}

// --- assignment

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > a block gets its id in the commit that creates it
#[test]
fn a_block_gets_its_id_in_the_commit_that_creates_it() {
    let editor = build_default();
    let committed: Rc<RefCell<Vec<Vec<Option<String>>>>> = Rc::default();
    let c = committed.clone();
    let _ = editor.register_update_listener(move |payload| {
        c.borrow_mut().push(payload.editor_state.read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|n| get_block_id(n).unwrap())
                .collect()
        }));
    });
    update(&editor, || {
        get_root().append(&[paragraph("one"), create_horizontal_rule_node()])?;
        Ok(())
    });
    assert_eq!(*committed.borrow(), vec![some(&["b1", "b2"])]);
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:one:b1", "horizontalrule:\n:b2"])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > importJSON assigns ids in the same commit, keeps existing ones
#[test]
fn import_json_assigns_ids_in_the_same_commit_keeps_existing_ones() {
    let editor = build(BuildOptions {
        fresh: true,
        ..Default::default()
    });
    let commits = Rc::new(Cell::new(0));
    let c = commits.clone();
    let _ = editor.register_update_listener(move |_| c.set(c.get() + 1));
    load(
        &editor,
        &doc_json(&[
            paragraph_json("kept", Some(&q("keep-me"))),
            paragraph_json("missing", None),
            paragraph_json("also kept", Some(&q("x_9-Z"))),
        ]),
    );
    assert_eq!(commits.get(), 1);
    assert_eq!(
        blocks(&editor),
        strings(&[
            "paragraph:kept:keep-me",
            "paragraph:missing:@1",
            "paragraph:also kept:x_9-Z"
        ])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > a document opened without ids gets positional ids in the commit that loads it
#[test]
fn a_document_opened_without_ids_gets_positional_ids_in_the_commit_that_loads_it() {
    let json = doc_json(&[paragraph_json("a", None), paragraph_json("b", None)]);
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Json(json)),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            config_extension(
                &block_id_extension(),
                [BlockIdConfigOverrides {
                    generate_id: Some(counter("b")),
                    ..Default::default()
                }],
            ),
        ],
        ..LexicalExtension::<(), (), ()>::new("[opened]", ())
    })
    .into()])
    .unwrap();
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:a:@0", "paragraph:b:@1"])
    );
    editor.dispose();
}

/// An editor with RichTextExtension and BlockIdExtension (a counter), no
/// initial state.
fn rich_text_block_id_editor(
    name: &str,
    initial: InitialEditorStateType,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(initial),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            config_extension(
                &block_id_extension(),
                [BlockIdConfigOverrides {
                    generate_id: Some(counter("b")),
                    ..Default::default()
                }],
            ),
        ],
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > a node a transform creates but never inserts does not delay the ids of a load
#[test]
fn a_node_a_transform_creates_but_never_inserts_does_not_delay_the_ids_of_a_load() {
    let json = doc_json(&[paragraph_json("a", None), paragraph_json("b", None)]);
    let run = |scratch: bool| {
        let editor = rich_text_block_id_editor("[scratch]", InitialEditorStateType::Null);
        if scratch {
            let _ = editor.register_node_transform("paragraph", |_| {
                create_text_node("scratch");
                Ok(())
            });
        }
        let committed: Rc<RefCell<Vec<Vec<Option<String>>>>> = Rc::default();
        let c = committed.clone();
        let _ = editor.register_update_listener(move |payload| {
            c.borrow_mut().push(payload.editor_state.read(|| {
                get_root()
                    .get_children()
                    .iter()
                    .map(|n| get_block_id(n).unwrap())
                    .collect()
            }));
        });
        load(&editor, &json);
        editor.dispose();
        committed.take()
    };
    let expected = run(false);
    assert_eq!(expected.len(), 1);
    assert!(expected[0].iter().all(Option::is_some));
    assert_eq!(run(true), expected);
}

/// An editor without the extension, with `$initialEditorState`.
fn plain_editor(
    initial: impl Fn() -> Result<(), LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[plain]", ())
    })
    .into()])
    .unwrap();
    editor.read(|| {}).unwrap();
    editor
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > a state from an editor without the extension is repaired
#[test]
fn a_state_from_an_editor_without_the_extension_is_repaired() {
    let plain = plain_editor(|| {
        get_root().append(&[paragraph("a"), paragraph("b")])?;
        Ok(())
    });
    let editor = build_default();
    editor
        .set_editor_state(&plain.get_editor_state(), None)
        .unwrap();
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:a:@0", "paragraph:b:@1"])
    );
    editor.dispose();
    plain.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > headless one-write recipe from the docs
#[test]
fn headless_one_write_recipe_from_the_docs() {
    let stored = doc_json(&[
        paragraph_json("Intro", None),
        paragraph_json("Body", Some(&q("b-body"))),
    ]);
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            config_extension(
                &block_id_extension(),
                [BlockIdConfigOverrides {
                    generate_id: Some(counter("id")),
                    ..Default::default()
                }],
            ),
        ],
        ..LexicalExtension::<(), (), ()>::new("[server]", ())
    })
    .into()])
    .unwrap();
    load(&editor, &stored);
    let intro = positional(&editor, "0");
    let states = |json: &Value| -> Vec<Value> {
        json.get("root")
            .and_then(|r| r.get("children"))
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .map(|c| c.get("$").cloned().unwrap_or(Value::Undefined))
            .collect()
    };
    let expected = vec![
        crate::json::parse(&format!(r#"{{"blockId":{}}}"#, q(&intro))).unwrap(),
        crate::json::parse(r#"{"blockId":"b-body"}"#).unwrap(),
    ];
    assert_eq!(states(&state_json(&editor)), expected);
    update(&editor, || {
        let block = get_block_by_id("b-body")?.expect("b-body");
        block.replace(&paragraph("New body"), false)?;
        Ok(())
    });
    assert_eq!(states(&state_json(&editor)), expected);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > invalid stored ids read as absent and are replaced by place
#[test]
fn invalid_stored_ids_read_as_absent_and_are_replaced_by_place() {
    let editor = build(BuildOptions {
        fresh: true,
        ..Default::default()
    });
    // As JSON text: `'lone\ud800'` is a lone surrogate, which only JSON's
    // escape can hold.
    let invalid = [
        "42".to_owned(),
        q(""),
        q(&"x".repeat(65)),
        q("a b"),
        q(" padded "),
        q("\"><script>"),
        q("line\r\nbreak"),
        q("nul\u{0}x"),
        r#""lone\ud800""#.to_owned(),
        q("émoji👋"),
        q("doc:12"),
    ];
    let mut children: Vec<String> = invalid
        .iter()
        .map(|id| paragraph_json("invalid", Some(id)))
        .collect();
    children.push(paragraph_json("max", Some(&q(&"y".repeat(64)))));
    children.push(paragraph_json("short", Some(&q("bk3j9x0a2zq"))));
    children.push(paragraph_json(
        "uuid",
        Some(&q("123e4567-e89b-12d3-a456-426614174000")),
    ));
    load(&editor, &doc_json(&children));
    let mut expected: Vec<Option<String>> =
        (0..invalid.len()).map(|i| Some(format!("@{i}"))).collect();
    expected.push(Some("y".repeat(64)));
    expected.extend(some(&[
        "bk3j9x0a2zq",
        "123e4567-e89b-12d3-a456-426614174000",
    ]));
    assert_eq!(ids(&editor), expected);
    let saved = crate::json::stringify(&state_json(&editor)).unwrap();
    assert!(!saved.contains("a b"));
    assert!(!saved.contains(&"x".repeat(65)));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > an invalid id from another replica is repaired like a missing one
#[test]
fn an_invalid_id_from_another_replica_is_repaired_like_a_missing_one() {
    let editor = build_default();
    seed(&editor, &["one"]);
    let json = crate::json::parse(&paragraph_json("remote", Some(&q("not valid!")))).unwrap();
    editor
        .update(
            move || {
                get_root().append(&[parse_serialized_node(&json)?])?;
                Ok(())
            },
            discrete_remote(),
        )
        .unwrap();
    assert_eq!(ids(&editor), some(&["b1", "@1"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > an invalid id set with $setState reads as absent and is replaced
#[test]
fn an_invalid_id_set_with_set_state_reads_as_absent_and_is_replaced() {
    let editor = build_default();
    seed(&editor, &["one"]);
    update(&editor, || {
        let node = paragraph("two");
        set_state(&node, &block_id_state(), Some("not valid!".to_owned()))?;
        get_root().append(std::slice::from_ref(&node))?;
        assert_eq!(get_block_id(&node)?, None);
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b1", "b2"]));
    assert!(
        !crate::json::stringify(&state_json(&editor))
            .unwrap()
            .contains("not valid")
    );
    expect_all_indexed(&editor);
    editor.dispose();
}

/// A generator that hands out `values`, then `rest`.
fn sequence(values: &[&str], rest: &'static str) -> GenerateId {
    let values = RefCell::new(strings(values));
    GenerateId(Rc::new(move || {
        let mut values = values.borrow_mut();
        if values.is_empty() {
            rest.to_owned()
        } else {
            values.remove(0)
        }
    }))
}

/// The error of an update that fails.
fn update_error(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) -> String {
    let result = editor.update(
        f,
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        },
    );
    result.expect_err("the update fails").message().to_owned()
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > generateId collisions are retried and a broken generator throws
#[test]
fn generate_id_collisions_are_retried_and_a_broken_generator_throws() {
    let editor = build(BuildOptions {
        generate_id: Some(sequence(&["dup", "dup", "dup", "fresh"], "dup")),
        ..Default::default()
    });
    update(&editor, || {
        get_root().append(&[paragraph("a"), paragraph("b")])?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["dup", "fresh"]));
    let message = update_error(&editor, || {
        get_root().append(&[paragraph("c")])?;
        Ok(())
    });
    assert!(
        message.contains("generateId returned 32 ids in a row"),
        "{message}"
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > invalid generateId values are discarded and generateId is called again
#[test]
fn invalid_generate_id_values_are_discarded_and_generate_id_is_called_again() {
    // `generateId` returns a `String`, so the upstream case of a value that
    // is not a string at all (`undefined`) can't occur: the type rules it
    // out. The invalid strings are checked as upstream checks them.
    let values: Rc<RefCell<Vec<String>>> = Rc::default();
    let v = values.clone();
    let editor = build(BuildOptions {
        generate_id: Some(GenerateId(Rc::new(move || {
            let mut values = v.borrow_mut();
            if values.is_empty() {
                "setup".to_owned()
            } else {
                values.remove(0)
            }
        }))),
        ..Default::default()
    });
    *values.borrow_mut() = vec![
        String::new(),
        "x".repeat(65),
        "a b".to_owned(),
        "é".to_owned(),
        "ok".to_owned(),
    ];
    update(&editor, || {
        get_root().append(&[paragraph("a")])?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["ok"]));
    let broken = build(BuildOptions {
        generate_id: Some(GenerateId(Rc::new(|| "not valid".to_owned()))),
        ..Default::default()
    });
    let message = update_error(&broken, || {
        get_root().append(&[paragraph("a")])?;
        Ok(())
    });
    assert!(
        message.contains("generateId returned 32 ids in a row"),
        "{message}"
    );
    broken.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > the default generator makes 126-bit URL-safe ids
#[test]
fn the_default_generator_makes_126_bit_url_safe_ids() {
    let mut seen = HashSet::new();
    let mut used = HashSet::new();
    for _ in 0..1000 {
        let id = generate_block_id();
        assert_eq!(id.len(), 21);
        assert!(
            id.bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        );
        assert_eq!(
            block_id_state().parse(&Value::from(id.as_str())),
            Some(id.clone())
        );
        used.extend(id.chars());
        seen.insert(id);
    }
    assert_eq!(seen.len(), 1000);
    assert_eq!(used.len(), 64);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > markdown import assigns ids
#[test]
fn markdown_import_assigns_ids() {
    use crate::markdown::{Transformer, heading, ordered_list, quote, unordered_list};
    let editor = build_default();
    update(&editor, || {
        crate::markdown::convert_from_markdown_string(
            "# Title\n\nText\n\n> Quote\n\n- one\n- two\n\n1. first",
            &[
                Transformer::from(heading()),
                quote().into(),
                unordered_list().into(),
                ordered_list().into(),
            ],
            None,
            false,
            false,
        )
    });
    assert_eq!(
        outline(&editor)
            .iter()
            .map(|l| generic_id(l))
            .collect::<Vec<_>>(),
        strings(&[
            "heading:id",
            "paragraph:id",
            "quote:id",
            "list:id",
            "  listitem:id",
            "  listitem:id",
            "list:id",
            "  listitem:id",
        ])
    );
    expect_all_indexed(&editor);
    editor.dispose();
}

// --- uniqueness

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > duplicates in imported JSON: the earliest keeps the id
#[test]
fn duplicates_in_imported_json_the_earliest_keeps_the_id() {
    let editor = build(BuildOptions {
        fresh: true,
        ..Default::default()
    });
    load(
        &editor,
        &doc_json(&[
            paragraph_json("first", Some(&q("x"))),
            paragraph_json("second", Some(&q("y"))),
            paragraph_json("copy of first", Some(&q("x"))),
            paragraph_json("another copy", Some(&q("x"))),
            paragraph_json("copy of second", Some(&q("y"))),
        ]),
    );
    assert_eq!(
        ids(&editor),
        some(&["x", "y", &dup("x", 1), &dup("x", 2), &dup("y", 1)])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > deterministic and idempotent
#[test]
fn deterministic_and_idempotent() {
    let json = doc_json(&[
        paragraph_json("c", Some(&q("z"))),
        paragraph_json("a", Some(&q("z"))),
        paragraph_json("b", None),
        paragraph_json("d", Some(&q("z"))),
    ]);
    let run = || {
        let editor = build(BuildOptions {
            fresh: true,
            generate_id: Some(GenerateId(Rc::new(generate_block_id))),
            ..Default::default()
        });
        load(&editor, &json);
        let out = state_json(&editor);
        editor.dispose();
        out
    };
    let first = run();
    assert_eq!(run(), first);
    let editor = build(BuildOptions {
        generate_id: Some(counter("again")),
        ..Default::default()
    });
    load(&editor, &crate::json::stringify(&first).unwrap());
    assert_eq!(state_json(&editor), first);
    update(&editor, || {
        nth(0).mark_dirty();
        Ok(())
    });
    assert_eq!(state_json(&editor), first);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > a duplicate inserted after the original gets a fresh id
#[test]
fn a_duplicate_inserted_after_the_original_gets_a_fresh_id() {
    let editor = build_default();
    seed(&editor, &["one", "two"]);
    update(&editor, || {
        let original = nth(0);
        let copy = parse_serialized_node(&Value::Object(original.export_json(false)))?;
        assert_eq!(get_block_id(&copy)?.as_deref(), Some("b1"));
        nth(1).insert_after(&copy, true)?;
        Ok(())
    });
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:one:b1", "paragraph:two:b2", "paragraph::b3"])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > a duplicate inserted above the original yields to it
#[test]
fn a_duplicate_inserted_above_the_original_yields_to_it() {
    let editor = build_default();
    seed(&editor, &["one", "two"]);
    update(&editor, || {
        let copy = parse_serialized_node(&Value::Object(nth(1).export_json(false)))?;
        assert_eq!(get_block_id(&copy)?.as_deref(), Some("b2"));
        nth(0).insert_before(&copy, true)?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b3", "b1", "b2"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > duplicating a block upward keeps the id on the original
#[test]
fn duplicating_a_block_upward_keeps_the_id_on_the_original() {
    let editor = build_default();
    seed(&editor, &["one", "two", "three"]);
    update(&editor, || {
        nth(0).insert_before(&deep_copy(&nth(2)), true)?;
        Ok(())
    });
    assert_eq!(
        blocks(&editor),
        strings(&[
            "paragraph:three:b4",
            "paragraph:one:b1",
            "paragraph:two:b2",
            "paragraph:three:b3",
        ])
    );
    editor
        .read(|| {
            assert_eq!(
                get_block_by_id("b3")
                    .unwrap()
                    .map(|n| n.get_key().to_owned()),
                Some(nth(3).get_key().to_owned())
            );
        })
        .unwrap();
    editor.dispose();
}

/// `$paragraph(text)` with the id `same`.
fn same_id_paragraph(text: &str) -> NodeRef {
    let node = paragraph(text);
    set_state(&node, &block_id_state(), Some("same".to_owned())).unwrap();
    node
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > blocks that all get an id in one local update: the first created keeps it
#[test]
fn blocks_that_all_get_an_id_in_one_local_update_the_first_created_keeps_it() {
    let editor = build_default();
    seed(&editor, &["one"]);
    update(&editor, || {
        nth(0).insert_after(&same_id_paragraph("later"), true)?;
        nth(0).insert_after(&same_id_paragraph("earlier"), true)?;
        Ok(())
    });
    assert_eq!(
        blocks(&editor),
        strings(&[
            "paragraph:one:b1",
            "paragraph:earlier:b2",
            "paragraph:later:same"
        ])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > a committed duplicate: the earliest in document order keeps it, whatever the keys
#[test]
fn a_committed_duplicate_the_earliest_in_document_order_keeps_it_whatever_the_keys() {
    let mut repaired = Vec::new();
    for create_later_first in [false, true] {
        let editor = build_default();
        seed(&editor, &["one"]);
        editor
            .update(
                move || {
                    let (earlier, later) = if create_later_first {
                        let later = same_id_paragraph("later");
                        (same_id_paragraph("earlier"), later)
                    } else {
                        let earlier = same_id_paragraph("earlier");
                        (earlier, same_id_paragraph("later"))
                    };
                    nth(0).insert_after(&earlier, true)?;
                    earlier.insert_after(&later, true)?;
                    Ok(())
                },
                discrete_remote(),
            )
            .unwrap();
        repaired.push(blocks(&editor));
        editor.dispose();
    }
    assert_eq!(
        repaired[0],
        vec![
            "paragraph:one:b1".to_owned(),
            "paragraph:earlier:same".to_owned(),
            format!("paragraph:later:{}", dup("same", 1)),
        ]
    );
    assert_eq!(repaired[1], repaired[0]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > $copyNode does not copy the id
#[test]
fn copy_node_does_not_copy_the_id() {
    let editor = build_default();
    seed(&editor, &["one"]);
    update(&editor, || {
        let copy = copy_node(&nth(0));
        assert_eq!(get_state(&copy, &block_id_state())?, None);
        nth(0).insert_before(&copy, true)?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b2", "b1"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > $copyNode right after loading JSON does not copy the stored id
#[test]
fn copy_node_right_after_loading_json_does_not_copy_the_stored_id() {
    let editor = build_default();
    load(
        &editor,
        &doc_json(&[paragraph_json("stored", Some(&q("x")))]),
    );
    update(&editor, || {
        let copy = copy_node(&nth(0));
        assert_eq!(get_state(&copy, &block_id_state())?, None);
        get_root().append(&[copy])?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["x", "b1"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > an explicit id that is already in use is repaired
#[test]
fn an_explicit_id_that_is_already_in_use_is_repaired() {
    let editor = build_default();
    seed(&editor, &["one", "two"]);
    update(&editor, || {
        set_state(&nth(1), &block_id_state(), Some("b1".to_owned()))?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b1", "b3"]));
    update(&editor, || {
        set_state(&nth(0), &block_id_state(), Some("b3".to_owned()))?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b4", "b3"]));
    editor.dispose();
}

type Copied = Rc<RefCell<Vec<Value>>>;

/// `$copyBlocks(editor, from, to)`: the clipboard JSON of the blocks.
fn copy_blocks(editor: &Editor, from: usize, to: usize) -> Result<Vec<Value>, LexicalError> {
    select_blocks(from, to)?;
    Ok(
        crate::clipboard::generate_json_from_selected_nodes(editor, get_selection().as_ref())?
            .nodes
            .into_iter()
            .map(Value::Object)
            .collect(),
    )
}

/// `$insertGeneratedNodes(editor, $generateNodesFromSerializedNodes(nodes),
/// $getSelection())`.
fn paste_nodes(editor: &Editor, nodes: &[Value]) -> Result<(), LexicalError> {
    crate::clipboard::insert_generated_nodes(
        editor,
        crate::clipboard::generate_nodes_from_serialized_nodes(nodes)?,
        &get_selection().expect("a selection"),
    )
}

/// `$pasteInto(editor, copied, block)`.
fn paste_into(editor: &Editor, copied: &[Value], block: &NodeRef) -> Result<(), LexicalError> {
    block.select(None, None)?;
    paste_nodes(editor, copied)
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > copy and paste within one editor and between editors
#[test]
fn copy_and_paste_within_one_editor_and_between_editors() {
    let source = build(BuildOptions {
        generate_id: Some(counter("s")),
        ..Default::default()
    });
    let target = build(BuildOptions {
        generate_id: Some(counter("t")),
        ..Default::default()
    });
    seed(&source, &["alpha", "beta"]);
    seed(&target, &["gamma"]);
    let copied: Copied = Rc::default();
    let (c, e) = (copied.clone(), (*source).clone());
    update(&source, move || {
        *c.borrow_mut() = copy_blocks(&e, 0, 1)?;
        Ok(())
    });
    assert_eq!(
        copied
            .borrow()
            .iter()
            .map(|n| n.get("$").cloned())
            .collect::<Vec<_>>(),
        vec![
            Some(crate::json::parse(r#"{"blockId":"s1"}"#).unwrap()),
            Some(crate::json::parse(r#"{"blockId":"s2"}"#).unwrap()),
        ]
    );
    let paste = |editor: &Editor| {
        let (c, e) = (copied.clone(), editor.clone());
        update(editor, move || {
            get_root().select_end()?;
            paste_nodes(&e, &c.borrow())
        });
    };
    paste(&target);
    assert_eq!(
        blocks(&target),
        strings(&["paragraph:gammaalpha:t1", "paragraph:beta:s2"])
    );
    paste(&source);
    expect_unique(&source);
    assert_eq!(ids(&source)[..2], some(&["s1", "s2"])[..]);
    source.dispose();
    target.dispose();
}

/// Appends an empty paragraph, `alpha` and `beta`.
fn empty_alpha_beta(editor: &Editor) {
    update(editor, || {
        get_root().append(&[
            create_paragraph_node(),
            paragraph("alpha"),
            paragraph("beta"),
        ])?;
        Ok(())
    });
}

/// Copies blocks 1 and 2, and pastes them into block 0 (tagged `PASTE_TAG`).
fn copy_and_paste_above(editor: &Editor) {
    let copied: Copied = Rc::default();
    let (c, e) = (copied.clone(), editor.clone());
    update(editor, move || {
        *c.borrow_mut() = copy_blocks(&e, 1, 2)?;
        Ok(())
    });
    let e = editor.clone();
    update_tagged(
        editor,
        move || paste_into(&e, &copied.borrow(), &nth(0)),
        &[PASTE_TAG],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > pasting a copy above the original: the original keeps its id
#[test]
fn pasting_a_copy_above_the_original_the_original_keeps_its_id() {
    let editor = build_default();
    empty_alpha_beta(&editor);
    assert_eq!(ids(&editor), some(&["b1", "b2", "b3"]));
    copy_and_paste_above(&editor);
    assert_eq!(
        blocks(&editor),
        strings(&[
            "paragraph:alpha:b4",
            "paragraph:beta:b5",
            "paragraph:alpha:b2",
            "paragraph:beta:b3",
        ])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > cut and paste moves the ids with the blocks
#[test]
fn cut_and_paste_moves_the_ids_with_the_blocks() {
    let editor = build_default();
    update(&editor, || {
        get_root().append(&[
            paragraph("one"),
            paragraph("two"),
            paragraph("three"),
            create_paragraph_node(),
        ])?;
        Ok(())
    });
    let copied: Copied = Rc::default();
    let (c, e) = (copied.clone(), (*editor).clone());
    update(&editor, move || {
        *c.borrow_mut() = copy_blocks(&e, 0, 1)?;
        nth(1).remove(false)?;
        nth(0).remove(false)?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b3", "b4"]));
    let e = (*editor).clone();
    update_tagged(
        &editor,
        move || paste_into(&e, &copied.borrow(), &nth(1)),
        &[PASTE_TAG],
    );
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:three:b3", "paragraph:one:b1", "paragraph:two:b2"])
    );
    update(&editor, || {
        let copy = deep_copy(&nth(0));
        nth(0).remove(false)?;
        nth(1).insert_after(&copy, true)?;
        Ok(())
    });
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:one:b1", "paragraph:two:b2", "paragraph:three:b3"])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > undo of a paste restores the ids, redo the repaired ones
#[test]
fn undo_of_a_paste_restores_the_ids_redo_the_repaired_ones() {
    let editor = build(BuildOptions {
        history: true,
        ..Default::default()
    });
    empty_alpha_beta(&editor);
    let before = blocks(&editor);
    copy_and_paste_above(&editor);
    let after = blocks(&editor);
    assert_eq!(
        after,
        strings(&[
            "paragraph:alpha:b4",
            "paragraph:beta:b5",
            "paragraph:alpha:b2",
            "paragraph:beta:b3",
        ])
    );
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(blocks(&editor), before);
    editor
        .read(|| {
            assert!(get_block_by_id("b4").unwrap().is_none());
            assert!(get_block_by_id("b5").unwrap().is_none());
            assert_eq!(text(get_block_by_id("b2").unwrap()), "alpha");
        })
        .unwrap();
    editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
    assert_eq!(blocks(&editor), after);
    editor.dispose();
}

// --- split and merge

/// `enter(editor, index, offset)`.
fn enter(editor: &Editor, index: usize, offset: usize) {
    let e = editor.clone();
    update(editor, move || {
        caret(index, offset)?;
        e.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
        Ok(())
    });
}

fn with_next(expected: &[&str]) -> Vec<String> {
    let mut out = strings(expected);
    out.push("paragraph:next:b2".to_owned());
    out
}

fn enter_in_paragraph(offset: usize, expected: &[&str]) {
    let editor = build_default();
    seed(&editor, &["hello", "next"]);
    enter(&editor, 0, offset);
    assert_eq!(blocks(&editor), with_next(expected));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the start of a paragraph
#[test]
fn enter_at_the_start_of_a_paragraph() {
    enter_in_paragraph(0, &["paragraph::b3", "paragraph:hello:b1"]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the middle of a paragraph
#[test]
fn enter_at_the_middle_of_a_paragraph() {
    enter_in_paragraph(2, &["paragraph:he:b1", "paragraph:llo:b3"]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the end of a paragraph
#[test]
fn enter_at_the_end_of_a_paragraph() {
    enter_in_paragraph(5, &["paragraph:hello:b1", "paragraph::b3"]);
}

fn enter_in_block(make: fn() -> NodeRef, offset: usize, expected: &[&str]) {
    let editor = build_default();
    update(&editor, move || {
        let block = make();
        block.append(&[create_text_node("hello")])?;
        get_root().append(&[block, paragraph("next")])?;
        Ok(())
    });
    enter(&editor, 0, offset);
    assert_eq!(blocks(&editor), with_next(expected));
    editor.dispose();
}

fn h1() -> NodeRef {
    create_heading_node(HeadingTagType::H1)
}

fn plain_quote() -> NodeRef {
    create_quote_node(false)
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the start of a heading
#[test]
fn enter_at_the_start_of_a_heading() {
    enter_in_block(h1, 0, &["paragraph::b3", "heading:hello:b1"]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the middle of a heading
#[test]
fn enter_at_the_middle_of_a_heading() {
    enter_in_block(h1, 2, &["heading:he:b1", "heading:llo:b3"]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the end of a heading
#[test]
fn enter_at_the_end_of_a_heading() {
    enter_in_block(h1, 5, &["heading:hello:b1", "paragraph::b3"]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the start of a quote
#[test]
fn enter_at_the_start_of_a_quote() {
    enter_in_block(plain_quote, 0, &["quote::b3", "paragraph:hello:b1"]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the middle of a quote
#[test]
fn enter_at_the_middle_of_a_quote() {
    enter_in_block(plain_quote, 2, &["quote:he:b1", "paragraph:llo:b3"]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the end of a quote
#[test]
fn enter_at_the_end_of_a_quote() {
    enter_in_block(plain_quote, 5, &["quote:hello:b1", "paragraph::b3"]);
}

fn enter_in_list_item(offset: usize, expected: &[&str]) {
    let editor = build_default();
    seed(&editor, &["one", "two"]);
    let e = (*editor).clone();
    update(&editor, move || {
        select_blocks(0, 1)?;
        e.dispatch_command(&INSERT_UNORDERED_LIST_COMMAND, ())?;
        Ok(())
    });
    let items = |editor: &Editor| -> Vec<String> {
        editor
            .read(|| {
                nth(0)
                    .get_children()
                    .iter()
                    .map(|item| {
                        format!(
                            "{}:{}",
                            item.get_text_content(),
                            show(get_block_id(item).unwrap())
                        )
                    })
                    .collect()
            })
            .unwrap()
    };
    assert_eq!(ids(&editor), some(&["b3"]));
    assert_eq!(items(&editor), strings(&["one:b1", "two:b2"]));
    let e = (*editor).clone();
    update(&editor, move || {
        let item = nth(0).get_first_child_or_throw()?;
        assert!(item.is_element());
        let text = item.get_first_descendant().unwrap();
        assert!(text.is_text());
        text.select(Some(offset), Some(offset))?;
        e.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
        Ok(())
    });
    let mut want = strings(expected);
    want.push("two:b2".to_owned());
    assert_eq!(items(&editor), want);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the start of a list item (nested ids)
#[test]
fn enter_at_the_start_of_a_list_item() {
    enter_in_list_item(0, &[":b4", "one:b1"]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the middle of a list item (nested ids)
#[test]
fn enter_at_the_middle_of_a_list_item() {
    enter_in_list_item(1, &["o:b1", "ne:b4"]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the end of a list item (nested ids)
#[test]
fn enter_at_the_end_of_a_list_item() {
    enter_in_list_item(3, &["one:b1", ":b4"]);
}

/// Backspace (or Delete when `!backward`) at `offset` in block `index`.
fn delete_at(editor: &Editor, index: usize, offset: usize, backward: bool) {
    let e = editor.clone();
    update(editor, move || {
        caret(index, offset)?;
        e.dispatch_command(&DELETE_CHARACTER_COMMAND, backward)?;
        Ok(())
    });
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Backspace join keeps the surviving block id
#[test]
fn backspace_join_keeps_the_surviving_block_id() {
    let editor = build_default();
    seed(&editor, &["one", "two", "three"]);
    delete_at(&editor, 1, 0, true);
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:onetwo:b1", "paragraph:three:b3"])
    );
    delete_at(&editor, 0, 6, false);
    assert_eq!(blocks(&editor), strings(&["paragraph:onetwothree:b1"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Backspace into a heading keeps the heading id
#[test]
fn backspace_into_a_heading_keeps_the_heading_id() {
    let editor = build_default();
    update(&editor, || {
        get_root().append(&[heading(HeadingTagType::H2, "Title"), paragraph("body")])?;
        Ok(())
    });
    delete_at(&editor, 1, 0, true);
    assert_eq!(blocks(&editor), strings(&["heading:Titlebody:b1"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Backspace at the start of a heading after a paragraph joins into the paragraph
#[test]
fn backspace_at_the_start_of_a_heading_after_a_paragraph_joins_into_the_paragraph() {
    let editor = build_default();
    update(&editor, || {
        get_root().append(&[paragraph("before"), heading(HeadingTagType::H2, "Title")])?;
        Ok(())
    });
    delete_at(&editor, 1, 0, true);
    assert_eq!(blocks(&editor), strings(&["paragraph:beforeTitle:b1"]));
    editor.dispose();
}

// --- type changes and moves

fn paragraph_to(make: fn() -> NodeRef, node_type: &str) {
    let editor = build_default();
    seed(&editor, &["one", "two"]);
    update(&editor, move || {
        caret(1, 1)?;
        set_blocks_type(get_selection().as_ref(), make, None)
    });
    assert_eq!(
        blocks(&editor),
        vec!["paragraph:one:b1".to_owned(), format!("{node_type}:two:b2")]
    );
    update(&editor, || {
        caret(1, 1)?;
        set_blocks_type(get_selection().as_ref(), create_paragraph_node, None)
    });
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:one:b1", "paragraph:two:b2"])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > paragraph to heading keeps the id
#[test]
fn paragraph_to_heading_keeps_the_id() {
    paragraph_to(h1, "heading");
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > paragraph to quote keeps the id
#[test]
fn paragraph_to_quote_keeps_the_id() {
    paragraph_to(plain_quote, "quote");
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > several blocks at once, including empty ones, keep their ids
#[test]
fn several_blocks_at_once_including_empty_ones_keep_their_ids() {
    let editor = build_default();
    update(&editor, || {
        get_root().append(&[
            paragraph("a"),
            create_paragraph_node(),
            create_paragraph_node(),
            paragraph("d"),
        ])?;
        Ok(())
    });
    update(&editor, || {
        get_root().select(Some(0), Some(4))?;
        set_blocks_type(
            get_selection().as_ref(),
            || create_heading_node(HeadingTagType::H3),
            None,
        )
    });
    assert_eq!(
        blocks(&editor),
        strings(&["heading:a:b1", "heading::b2", "heading::b3", "heading:d:b4"])
    );
    editor.dispose();
}

fn paragraph_to_list(options: BuildOptions, expected: &[&str]) {
    let editor = build(options);
    seed(&editor, &["one", "two"]);
    let e = (*editor).clone();
    update(&editor, move || {
        caret(1, 0)?;
        e.dispatch_command(&INSERT_UNORDERED_LIST_COMMAND, ())?;
        Ok(())
    });
    assert_eq!(outline(&editor), strings(expected));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > paragraph to list: the item keeps the id, the list is new
#[test]
fn paragraph_to_list_the_item_keeps_the_id_the_list_is_new() {
    paragraph_to_list(
        BuildOptions::default(),
        &["paragraph:b1", "list:b3", "  listitem:b2"],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > paragraph to list with $isRootChildBlock keeps the id on the list
#[test]
fn paragraph_to_list_with_is_root_child_block_keeps_the_id_on_the_list() {
    paragraph_to_list(
        BuildOptions {
            is_identified_block: Some(IsIdentifiedBlock(Rc::new(is_root_child_block))),
            ..Default::default()
        },
        &["paragraph:b1", "list:b2", "  listitem"],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > moving a block (drag and drop) moves its id
#[test]
fn moving_a_block_drag_and_drop_moves_its_id() {
    let editor = build_default();
    seed(&editor, &["one", "two", "three"]);
    update(&editor, || {
        nth(2).insert_after(&nth(0), true)?;
        Ok(())
    });
    assert_eq!(
        blocks(&editor),
        strings(&["paragraph:two:b2", "paragraph:three:b3", "paragraph:one:b1"])
    );
    update(&editor, || {
        nth(0).insert_before(&nth(2), true)?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b1", "b2", "b3"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > replacing a block by hand keeps its id unless the new block has one
#[test]
fn replacing_a_block_by_hand_keeps_its_id_unless_the_new_block_has_one() {
    let editor = build_default();
    seed(&editor, &["one", "two"]);
    update(&editor, || {
        nth(0).replace(&paragraph("uno"), false)?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b1", "b2"]));
    update(&editor, || {
        let replacement = paragraph("dos");
        set_state(&replacement, &block_id_state(), Some("explicit".to_owned()))?;
        nth(1).replace(&replacement, false)?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b1", "explicit"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > removing a block and appending a new one elsewhere does not reuse the id
#[test]
fn removing_a_block_and_appending_a_new_one_elsewhere_does_not_reuse_the_id() {
    let editor = build_default();
    seed(&editor, &["one", "two"]);
    update(&editor, || {
        nth(0).remove(false)?;
        get_root().append(&[paragraph("three")])?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b2", "b3"]));
    editor.dispose();
}

// --- undo and redo

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: undo and redo > restore exact ids
#[test]
fn restore_exact_ids() {
    let editor = build(BuildOptions {
        history: true,
        ..Default::default()
    });
    seed(&editor, &["one", "two"]);
    let mut snapshots = vec![blocks(&editor)];
    let e = (*editor).clone();
    let steps: Vec<Box<dyn FnOnce() -> Result<(), LexicalError>>> = vec![
        Box::new({
            let e = e.clone();
            move || {
                caret(0, 1)?;
                e.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
                Ok(())
            }
        }),
        Box::new(|| {
            caret(2, 0)?;
            set_blocks_type(get_selection().as_ref(), h1, None)
        }),
        Box::new({
            let e = e.clone();
            move || {
                caret(1, 0)?;
                e.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
                Ok(())
            }
        }),
        Box::new(|| {
            nth(1).insert_before(&nth(0), true)?;
            Ok(())
        }),
    ];
    for step in steps {
        update_tagged(&editor, step, &[HISTORY_PUSH_TAG]);
        snapshots.push(blocks(&editor));
    }
    for i in (0..snapshots.len() - 1).rev() {
        editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
        assert_eq!(blocks(&editor), snapshots[i]);
        editor
            .read(|| {
                for block in get_root().get_children() {
                    let id = get_block_id(&block).unwrap().unwrap();
                    assert_eq!(
                        get_block_by_id(&id)
                            .unwrap()
                            .map(|n| n.get_key().to_owned()),
                        Some(block.get_key().to_owned())
                    );
                }
            })
            .unwrap();
    }
    for snapshot in snapshots.iter().skip(1) {
        editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
        assert_eq!(&blocks(&editor), snapshot);
    }
    editor.dispose();
}

// --- index

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: index > $getBlockById finds blocks and follows changes
#[test]
fn get_block_by_id_finds_blocks_and_follows_changes() {
    let editor = build_default();
    seed(&editor, &["one", "two"]);
    editor
        .read(|| {
            assert_eq!(text(get_block_by_id("b1").unwrap()), "one");
            assert_eq!(text(get_block_by_id("b2").unwrap()), "two");
            assert!(get_block_by_id("nope").unwrap().is_none());
        })
        .unwrap();
    update(&editor, || {
        nth(0).remove(false)?;
        assert!(get_block_by_id("b1")?.is_none());
        Ok(())
    });
    editor
        .read(|| {
            assert!(get_block_by_id("b1").unwrap().is_none());
            assert_eq!(text(get_block_by_id("b2").unwrap()), "two");
        })
        .unwrap();
    update(&editor, || {
        set_state(&nth(0), &block_id_state(), Some("renamed".to_owned()))?;
        Ok(())
    });
    editor
        .read(|| {
            assert!(get_block_by_id("b2").unwrap().is_none());
            assert_eq!(text(get_block_by_id("renamed").unwrap()), "two");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: index > typing does not evaluate $isIdentifiedBlock for untouched blocks
#[test]
fn typing_does_not_evaluate_is_identified_block_for_untouched_blocks() {
    // `vi.fn($defaultIsIdentifiedBlock)`: the default, counting its calls.
    let calls = Rc::new(Cell::new(0usize));
    let c = calls.clone();
    let editor = build(BuildOptions {
        is_identified_block: Some(IsIdentifiedBlock(Rc::new(move |node| {
            c.set(c.get() + 1);
            default_is_identified_block(node)
        }))),
        ..Default::default()
    });
    let texts: Vec<String> = (0..1000).map(|i| format!("block {i}")).collect();
    seed(
        &editor,
        &texts.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(ids(&editor).into_iter().collect::<HashSet<_>>().len(), 1000);
    calls.set(0);
    update(&editor, || {
        caret(500, 3)?;
        get_selection()
            .filter(crate::SelectionRef::is_range)
            .unwrap()
            .insert_text("x")
    });
    assert!(calls.get() < 10, "{}", calls.get());
    calls.set(0);
    enter(&editor, 500, 3);
    assert!(calls.get() < 20, "{}", calls.get());
    expect_unique(&editor);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: index > disabled stops assignment; enabling assigns everything
#[test]
fn disabled_stops_assignment_enabling_assigns_everything() {
    let editor = build(BuildOptions {
        disabled: true,
        ..Default::default()
    });
    let disabled = get_extension_dependency_from_editor(&editor, &block_id_extension())
        .unwrap()
        .output
        .unwrap()
        .disabled
        .clone();
    seed(&editor, &["one", "two"]);
    assert_eq!(ids(&editor), vec![None, None]);
    update(&editor, || {
        set_state(&nth(1), &block_id_state(), Some("manual".to_owned()))?;
        Ok(())
    });
    editor
        .read(|| assert_eq!(text(get_block_by_id("manual").unwrap()), "two"))
        .unwrap();
    disabled.set_value(false);
    assert_eq!(ids(&editor), some(&["@0", "manual"]));
    let first = positional(&editor, "0");
    let f = first.clone();
    editor
        .read(move || assert_eq!(text(get_block_by_id(&f).unwrap()), "one"))
        .unwrap();
    disabled.set_value(true);
    update(&editor, || {
        get_root().append(&[paragraph("three")])?;
        Ok(())
    });
    assert_eq!(
        ids(&editor),
        vec![Some(first), Some("manual".to_owned()), None]
    );
    editor.dispose();
}

// --- transforms skipped

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: transforms skipped > a change applied without transforms (remote) is repaired in a follow-up
#[test]
fn a_change_applied_without_transforms_remote_is_repaired_in_a_follow_up() {
    let editor = build_default();
    seed(&editor, &["one"]);
    let tagged_commits = Rc::new(Cell::new(0));
    let follow_ups = Rc::new(Cell::new(0));
    let (t, f) = (tagged_commits.clone(), follow_ups.clone());
    let _ = editor.register_update_listener(move |payload| {
        if payload.tags.iter().any(|tag| tag == COLLABORATION_TAG) {
            t.set(t.get() + 1);
        } else {
            f.set(f.get() + 1);
        }
    });
    editor
        .update(
            || {
                let copy = parse_serialized_node(&Value::Object(nth(0).export_json(false)))?;
                get_root().append(&[copy, paragraph("no id")])?;
                Ok(())
            },
            discrete_remote(),
        )
        .unwrap();
    let after = blocks(&editor);
    assert_eq!(tagged_commits.get(), 1);
    assert_eq!(follow_ups.get(), 1);
    assert_eq!(
        after,
        vec![
            "paragraph:one:b1".to_owned(),
            format!("paragraph::{}", dup("b1", 1)),
            "paragraph:no id:@2".to_owned(),
        ]
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: transforms skipped > transforms in an update tagged COLLABORATION_TAG leave the repair to the follow-up
#[test]
fn transforms_in_an_update_tagged_collaboration_tag_leave_the_repair_to_the_follow_up() {
    let editor = build_default();
    seed(&editor, &["one"]);
    let commits: Rc<RefCell<Vec<Vec<String>>>> = Rc::default();
    let c = commits.clone();
    let _ = editor.register_update_listener(move |payload| {
        let mut row = vec![if payload.tags.iter().any(|tag| tag == COLLABORATION_TAG) {
            "tagged".to_owned()
        } else {
            "follow-up".to_owned()
        }];
        row.extend(payload.editor_state.read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|n| show(get_block_id(n).unwrap()))
                .collect::<Vec<_>>()
        }));
        c.borrow_mut().push(row);
    });
    update_tagged(
        &editor,
        || {
            get_root().append(&[paragraph("remote")])?;
            Ok(())
        },
        &[COLLABORATION_TAG],
    );
    editor.read(|| {}).unwrap();
    assert_eq!(
        *commits.borrow(),
        vec![
            strings(&["tagged", "b1", "null"]),
            vec![
                "follow-up".to_owned(),
                "b1".to_owned(),
                positional(&editor, "1")
            ],
        ]
    );
    editor.dispose();
}

// --- misc

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: misc > headings created by Enter at start keep ids unique
#[test]
fn headings_created_by_enter_at_start_keep_ids_unique() {
    let editor = build_default();
    update(&editor, || {
        get_root().append(&[heading(HeadingTagType::H1, "A")])?;
        Ok(())
    });
    enter(&editor, 0, 0);
    assert_eq!(blocks(&editor), strings(&["paragraph::b2", "heading:A:b1"]));
    editor
        .read(|| {
            assert_eq!(nth(0).get_type(), "paragraph");
            assert!(is_heading_node(&nth(1)));
        })
        .unwrap();
    editor.dispose();
}

// --- documents stored without ids

/// `reader(options)`: a fresh editor with the default, random generator.
fn reader(is_identified_block: Option<IsIdentifiedBlock>) -> LexicalEditorWithDispose {
    build(BuildOptions {
        is_identified_block,
        fresh: true,
        generate_id: Some(GenerateId(Rc::new(generate_block_id))),
        ..Default::default()
    })
}

fn stored() -> String {
    doc_json(&[
        paragraph_json("Intro", None),
        paragraph_json("Body", None),
        paragraph_json("Outro", None),
    ])
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > two reads name the blocks the same way, by place, without a write
#[test]
fn two_reads_name_the_blocks_the_same_way_by_place_without_a_write() {
    let first = reader(None);
    let second = reader(None);
    load(&first, &stored());
    load(&second, &stored());
    assert_eq!(raw_ids(&first), raw_ids(&second));
    assert_eq!(ids(&first), some(&["@0", "@1", "@2"]));
    for id in raw_ids(&first) {
        let id = id.unwrap();
        assert_eq!(id.len(), 21);
        assert!(
            id.bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        );
    }
    first.dispose();
    second.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > blocks at the same place in different documents or versions get different ids
#[test]
fn blocks_at_the_same_place_in_different_documents_or_versions_get_different_ids() {
    let v1 = reader(None);
    let v2 = reader(None);
    load(&v1, &stored());
    load(
        &v2,
        &doc_json(&[
            paragraph_json("Budget", None),
            paragraph_json("Body", None),
            paragraph_json("Outro", None),
        ]),
    );
    let a = raw_ids(&v1);
    let b = raw_ids(&v2);
    assert!(a.iter().all(|id| !b.contains(id)));
    v1.dispose();
    v2.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > stored ids are kept, missing ones are positional, and a taken positional id is retried
#[test]
fn stored_ids_are_kept_missing_ones_are_positional_and_a_taken_positional_id_is_retried() {
    let probe = reader(None);
    load(&probe, &stored());
    let place0 = positional(&probe, "0");
    let editor = reader(None);
    load(
        &editor,
        &doc_json(&[
            paragraph_json("Intro", None),
            paragraph_json("Body", Some(&q(&place0))),
            paragraph_json("Outro", Some(&q("kept"))),
        ]),
    );
    assert_eq!(ids(&editor), some(&["@0#1", "@0", "kept"]));
    assert_eq!(raw_ids(&editor)[1], Some(place0));
    probe.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > blocks created later get generated ids
#[test]
fn blocks_created_later_get_generated_ids() {
    let probe = reader(None);
    load(&probe, &stored());
    let read = raw_ids(&probe);
    let editor = build(BuildOptions {
        fresh: true,
        ..Default::default()
    });
    load(&editor, &stored());
    enter(&editor, 0, 5);
    update(&editor, || {
        nth(3).insert_after(&deep_copy(&nth(0)), true)?;
        Ok(())
    });
    let r = |i: usize| read[i].clone().unwrap();
    assert_eq!(
        blocks(&editor),
        vec![
            format!("paragraph:Intro:{}", r(0)),
            "paragraph::b1".to_owned(),
            format!("paragraph:Body:{}", r(1)),
            format!("paragraph:Outro:{}", r(2)),
            "paragraph:Intro:b2".to_owned(),
        ]
    );
    probe.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > the first content of an empty editor is a load: initial state functions and the default paragraph
#[test]
fn the_first_content_of_an_empty_editor_is_a_load() {
    let make = |initial: bool| {
        build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: initial.then(|| {
                InitialEditorStateType::Update(Rc::new(|_| {
                    get_root().append(&[paragraph("one"), paragraph("two")])?;
                    Ok(())
                }))
            }),
            dependencies: vec![
                crate::rich_text::rich_text_extension().into(),
                block_id_extension().into(),
            ],
            ..LexicalExtension::<(), (), ()>::new("[initial]", ())
        })
        .into()])
        .unwrap()
    };
    let a = make(true);
    let b = make(true);
    assert_eq!(raw_ids(&a), raw_ids(&b));
    assert_eq!(ids(&a), some(&["@0", "@1"]));
    let c = make(false);
    let d = make(false);
    assert_eq!(raw_ids(&c), raw_ids(&d));
    assert_eq!(ids(&c), some(&["@0"]));
    for editor in [&a, &b] {
        update(editor, || {
            get_root().append(&[paragraph("three")])?;
            Ok(())
        });
    }
    assert_ne!(raw_ids(&a)[2], raw_ids(&b)[2]);
    for editor in [a, b, c, d] {
        editor.dispose();
    }
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > a state that replaces the content wholesale is a load; blocks added by updates are edits
#[test]
fn a_state_that_replaces_the_content_wholesale_is_a_load() {
    let editor = build_default();
    seed(&editor, &["old"]);
    assert_eq!(ids(&editor), some(&["b1"]));
    load(&editor, &stored());
    assert_eq!(ids(&editor), some(&["@0", "@1", "@2"]));
    let loaded = raw_ids(&editor);
    update(&editor, || {
        get_root().append(&[paragraph("new")])?;
        Ok(())
    });
    let mut want = loaded.clone();
    want.push(Some("b2".to_owned()));
    assert_eq!(raw_ids(&editor), want);
    update(&editor, || {
        set_root_children(
            &["x", "y", "z", "w", "v", "u"]
                .iter()
                .map(|t| paragraph(t))
                .collect::<Vec<_>>(),
        )
    });
    want.extend(some(&["b3", "b4"]));
    assert_eq!(raw_ids(&editor), want);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > a state from an editor without the extension gets the ids its JSON gets
#[test]
fn a_state_from_an_editor_without_the_extension_gets_the_ids_its_json_gets() {
    let plain = plain_editor(|| {
        get_root().append(&[paragraph("a"), create_heading_node(HeadingTagType::H2)])?;
        Ok(())
    });
    let from_state = reader(None);
    from_state
        .set_editor_state(&plain.get_editor_state(), None)
        .unwrap();
    let from_json = reader(None);
    load(
        &from_json,
        &crate::json::stringify(&state_json(&plain)).unwrap(),
    );
    assert_eq!(raw_ids(&from_state), raw_ids(&from_json));
    assert_eq!(ids(&from_state), some(&["@0", "@1"]));
    for editor in [plain, from_state, from_json] {
        editor.dispose();
    }
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > a person's first edits keep the ids a reader read
#[test]
fn a_persons_first_edits_keep_the_ids_a_reader_read() {
    let probe = reader(None);
    load(&probe, &stored());
    let read = raw_ids(&probe);
    let editor = build(BuildOptions {
        fresh: true,
        ..Default::default()
    });
    load(&editor, &stored());
    enter(&editor, 0, 0);
    update(&editor, || {
        caret(2, 0)?;
        set_blocks_type(get_selection().as_ref(), h1, None)
    });
    let r = |i: usize| read[i].clone().unwrap();
    assert_eq!(
        blocks(&editor),
        vec![
            "paragraph::b1".to_owned(),
            format!("paragraph:Intro:{}", r(0)),
            format!("heading:Body:{}", r(1)),
            format!("paragraph:Outro:{}", r(2)),
        ]
    );
    let read2 = read.clone();
    editor
        .read(move || {
            assert_eq!(
                read2
                    .iter()
                    .map(|id| text(get_block_by_id(id.as_deref().unwrap()).unwrap()))
                    .collect::<Vec<_>>(),
                strings(&["Intro", "Body", "Outro"])
            );
        })
        .unwrap();
    probe.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > nested blocks are named by their path among identified blocks
#[test]
fn nested_blocks_are_named_by_their_path_among_identified_blocks() {
    let plain = plain_editor(|| {
        crate::markdown::convert_from_markdown_string(
            "Title\n\n- one\n- two\n\nEnd",
            &[crate::markdown::unordered_list().into()],
            None,
            false,
            false,
        )
    });
    let json = crate::json::stringify(&state_json(&plain)).unwrap();
    let editor = reader(None);
    load(&editor, &json);
    let key = editor.read(document_key).unwrap();
    let expected: Vec<Option<String>> = ["0", "1", "1.0", "1.1", "2"]
        .iter()
        .map(|place| Some(derive_positional_id(&key, place)))
        .collect();
    let actual = editor
        .read(|| {
            let mut out = Vec::new();
            for block in get_root().get_children() {
                out.push(get_block_id(&block).unwrap());
                if block.is_element() && block.get_type() == "list" {
                    out.extend(
                        block
                            .get_children()
                            .iter()
                            .map(|n| get_block_id(n).unwrap()),
                    );
                }
            }
            out
        })
        .unwrap();
    assert_eq!(actual, expected);
    plain.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > a load assigns every id in its own commit and is idempotent
#[test]
fn a_load_assigns_every_id_in_its_own_commit_and_is_idempotent() {
    let editor = reader(None);
    let commits = Rc::new(Cell::new(0));
    let c = commits.clone();
    let _ = editor.register_update_listener(move |_| c.set(c.get() + 1));
    load(&editor, &stored());
    assert_eq!(commits.get(), 1);
    let once = state_json(&editor);
    let again = reader(None);
    load(&again, &crate::json::stringify(&once).unwrap());
    assert_eq!(state_json(&again), once);
    editor.dispose();
    again.dispose();
}

// --- the index finds every identified block

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the index finds every identified block > a list given an id with $setState before it is inserted
#[test]
fn a_list_given_an_id_with_set_state_before_it_is_inserted() {
    let editor = build_default();
    seed(&editor, &["p"]);
    update(&editor, || {
        let block = bullet_list(&["item"]);
        set_state(&block, &block_id_state(), Some("my-block".to_owned()))?;
        get_root().append(&[block])?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b1", "my-block"]));
    expect_all_indexed(&editor);
    editor
        .read(|| {
            assert_eq!(
                get_block_by_id("my-block")
                    .unwrap()
                    .map(|n| n.get_key().to_owned()),
                Some(nth(1).get_key().to_owned())
            );
        })
        .unwrap();
    editor.dispose();
}

/// Sets the text of the first item of the list at block 1.
fn set_first_item_text(text: &'static str) -> Result<(), LexicalError> {
    let item = nth(1).get_first_child_or_throw()?;
    assert!(item.is_element());
    let t = item.get_first_child_or_throw()?;
    assert!(t.is_text());
    t.set_text_content(text);
    Ok(())
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the index finds every identified block > a copy of a list whose items got ids alongside it yields to it
#[test]
fn a_copy_of_a_list_whose_items_got_ids_alongside_it_yields_to_it() {
    let editor = build_default();
    update(&editor, || {
        let list = bullet_list(&["item"]);
        set_state(&list, &block_id_state(), Some("my-list".to_owned()))?;
        get_root().append(&[paragraph("before"), list, paragraph("after")])?;
        Ok(())
    });
    update(&editor, || set_first_item_text("item!"));
    let original_key = editor.read(|| nth(1).get_key().to_owned()).unwrap();
    update(&editor, || {
        nth(0).insert_before(&deep_copy(&nth(1)), true)?;
        Ok(())
    });
    expect_unique(&editor);
    expect_all_indexed(&editor);
    editor
        .read(move || {
            assert_eq!(
                get_block_by_id("my-list")
                    .unwrap()
                    .unwrap()
                    .get_key()
                    .to_owned(),
                original_key
            );
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the index finds every identified block > setEditorState of a list whose items repeat an id
#[test]
fn set_editor_state_of_a_list_whose_items_repeat_an_id() {
    let editor = build_default();
    seed(&editor, &["one"]);
    let mut json = state_json(&editor);
    let item = |text: &str| {
        format!(
            r#"{{"$":{{"blockId":"X"}},"children":[{{"detail":0,"format":0,"mode":"normal","style":"","text":"{text}","type":"text","version":1}}],"direction":null,"format":"","indent":0,"type":"listitem","value":1,"version":1}}"#
        )
    };
    let list = crate::json::parse(&format!(
        r#"{{"$":{{"blockId":"L"}},"children":[{},{}],"direction":null,"format":"","indent":0,"listType":"bullet","start":1,"tag":"ul","type":"list","version":1}}"#,
        item("a"),
        item("b")
    ))
    .unwrap();
    if let Value::Object(root) = &mut json
        && let Some(Value::Object(r)) = root.get_mut("root")
        && let Some(Value::Array(children)) = r.get_mut("children")
    {
        children.push(list);
    } else {
        panic!("root.children");
    }
    editor
        .set_editor_state(&editor.parse_editor_state_json(&json).unwrap(), None)
        .unwrap();
    expect_all_indexed(&editor);
    editor
        .read(|| {
            assert_eq!(get_block_by_id("L").unwrap().unwrap().get_type(), "list");
            assert_eq!(text(get_block_by_id("X").unwrap()), "a");
        })
        .unwrap();
    editor.dispose();
}

// --- copies of lists that merge

/// p("p") and a bullet list with one item "a"; the list's and item's ids.
fn paragraph_and_list(editor: &Editor) -> (String, String) {
    update(editor, || {
        get_root().append(&[paragraph("p"), bullet_list(&["a"])])?;
        Ok(())
    });
    editor
        .read(|| {
            (
                get_block_id(&nth(1)).unwrap().unwrap(),
                get_block_id(&nth(1).get_first_child_or_throw().unwrap())
                    .unwrap()
                    .unwrap(),
            )
        })
        .unwrap()
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: copies of lists that merge > pasting copies, one of them right above the original: the merged list keeps the id
#[test]
fn pasting_copies_one_of_them_right_above_the_original_the_merged_list_keeps_the_id() {
    let editor = build_default();
    let (list_id, item_id) = paragraph_and_list(&editor);
    update(&editor, || {
        let list = nth(1);
        nth(0).insert_before(&deep_copy(&list), true)?;
        list.insert_before(&deep_copy(&list), true)?;
        Ok(())
    });
    expect_unique(&editor);
    expect_all_indexed(&editor);
    editor
        .read(move || {
            let holder = get_block_by_id(&list_id).unwrap().unwrap();
            assert!(holder.is_element());
            assert!(
                holder
                    .get_children()
                    .iter()
                    .any(|item| get_block_id(item).unwrap().as_deref() == Some(&item_id))
            );
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: copies of lists that merge > copies on both sides of the original, merged over several transform passes: the merged list keeps the id
#[test]
fn copies_on_both_sides_of_the_original_merged_over_several_transform_passes() {
    let editor = build_default();
    let (list_id, item_id) = paragraph_and_list(&editor);
    update(&editor, || {
        let list = nth(1);
        list.insert_before(&deep_copy(&list), true)?;
        list.insert_before(&deep_copy(&list), true)?;
        list.insert_after(&deep_copy(&list), true)?;
        Ok(())
    });
    assert_eq!(ids(&editor), some(&["b1", &list_id]));
    expect_unique(&editor);
    expect_all_indexed(&editor);
    editor
        .read(move || {
            let items: Vec<Option<String>> = nth(1)
                .get_children()
                .iter()
                .map(|n| get_block_id(n).unwrap())
                .collect();
            assert!(items.contains(&Some(item_id)));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: a copy of a list pasted above another list > the list it merges into keeps its id
#[test]
fn the_list_it_merges_into_keeps_its_id() {
    let editor = build_default();
    update(&editor, || {
        get_root().append(&[bullet_list(&["l"]), paragraph("sep"), bullet_list(&["m"])])?;
        Ok(())
    });
    let [copied_id, copied_item_id, list_id, item_id] = editor
        .read(|| {
            let id = |n: &NodeRef| get_block_id(n).unwrap().unwrap();
            [
                id(&nth(0)),
                id(&nth(0).get_first_child_or_throw().unwrap()),
                id(&nth(2)),
                id(&nth(2).get_first_child_or_throw().unwrap()),
            ]
        })
        .unwrap();
    update(&editor, || {
        nth(2).insert_before(&deep_copy(&nth(0)), true)?;
        Ok(())
    });
    expect_unique(&editor);
    expect_all_indexed(&editor);
    editor
        .read(move || {
            assert_eq!(get_block_id(&nth(0)).unwrap(), Some(copied_id));
            assert_eq!(
                get_block_id(&nth(0).get_first_child_or_throw().unwrap()).unwrap(),
                Some(copied_item_id.clone())
            );
            let merged = nth(2);
            assert_eq!(get_block_id(&merged).unwrap(), Some(list_id));
            let children: Vec<Option<String>> = merged
                .get_children()
                .iter()
                .map(|n| get_block_id(n).unwrap())
                .collect();
            assert_ne!(children[0], Some(copied_item_id));
            assert!(children[0].is_some());
            assert_eq!(children[1], Some(item_id));
        })
        .unwrap();
    editor.dispose();
}

// --- more predicates and nesting

/// `buildWith()`: the default predicate, upstream's decorators unused here.
fn build_with() -> LexicalEditorWithDispose {
    as_edited(
        build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Null),
            dependencies: vec![
                crate::rich_text::rich_text_extension().into(),
                crate::list::list_extension().into(),
                config_extension(
                    &block_id_extension(),
                    [BlockIdConfigOverrides {
                        generate_id: Some(counter("b")),
                        ..Default::default()
                    }],
                ),
            ],
            ..LexicalExtension::<(), (), ()>::new("[block-id-predicates]", ())
        })
        .into()])
        .unwrap(),
    )
}

fn copy_or_cut_nested_list_items(cut: bool) {
    let editor = build_with();
    update(&editor, || {
        get_root().append(&[bullet_list(&["one", "two"]), create_paragraph_node()])?;
        Ok(())
    });
    let item_ids = |editor: &Editor| -> Vec<String> {
        editor
            .read(|| {
                get_root()
                    .get_children()
                    .iter()
                    .filter(|b| b.is_element() && b.get_type() == "list")
                    .flat_map(|b| b.get_children())
                    .map(|item| {
                        format!(
                            "{}:{}",
                            item.get_text_content(),
                            show(get_block_id(&item).unwrap())
                        )
                    })
                    .collect()
            })
            .unwrap()
    };
    let before = item_ids(&editor);
    let copied: Copied = Rc::default();
    let (c, e) = (copied.clone(), (*editor).clone());
    update(&editor, move || {
        let list = nth(0);
        let first = list.get_first_child_or_throw()?;
        let last = list.get_last_child_or_throw()?;
        assert!(first.is_element() && last.is_element());
        let selection = first.select(Some(0), Some(0))?;
        selection.point_set(
            Which::Focus,
            last.get_key(),
            last.get_children_size(),
            PointType::Element,
        )?;
        *c.borrow_mut() =
            crate::clipboard::generate_json_from_selected_nodes(&e, get_selection().as_ref())?
                .nodes
                .into_iter()
                .map(Value::Object)
                .collect();
        if cut {
            list.remove(false)?;
        }
        Ok(())
    });
    let e = (*editor).clone();
    update(&editor, move || {
        get_root().get_last_child_or_throw()?.select_end()?;
        paste_nodes(&e, &copied.borrow())
    });
    let after = item_ids(&editor);
    let all: Vec<&str> = after
        .iter()
        .map(|entry| entry.split(':').nth(1).unwrap())
        .collect();
    assert_eq!(all.iter().collect::<HashSet<_>>().len(), all.len());
    if cut {
        assert_eq!(after, before);
    } else {
        assert_eq!(after[..2], before[..]);
        assert_eq!(after.len(), 4);
    }
    expect_all_indexed(&editor);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: more predicates and nesting > copy and paste of nested list items through the Lexical clipboard
#[test]
fn copy_and_paste_of_nested_list_items_through_the_lexical_clipboard() {
    copy_or_cut_nested_list_items(false);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: more predicates and nesting > cut and paste of nested list items through the Lexical clipboard
#[test]
fn cut_and_paste_of_nested_list_items_through_the_lexical_clipboard() {
    copy_or_cut_nested_list_items(true);
}

/// The tests that make a table.
#[cfg(feature = "table")]
mod tables {
    use super::*;
    use crate::code::create_code_node;
    use crate::link::{LinkAttributes, create_link_node};
    use crate::nodes::element::ElementData;
    use crate::table::{
        InsertTableCommandPayloadHeaders, create_table_cell_node, create_table_node,
        create_table_node_with_dimensions, create_table_row_node, is_table_cell_node,
        table_extension,
    };
    use crate::{AnyLexicalExtensionArgument, LexicalNode, NodeBase, NodeClass, create};

    /// `$createTableNodeWithDimensions(rows, columns, false)`.
    fn table(rows: usize, columns: usize) -> NodeRef {
        create_table_node_with_dimensions(
            rows,
            columns,
            InsertTableCommandPayloadHeaders::Both(false),
        )
        .unwrap()
    }

    /// `$createTableCellNode().append(...children)`.
    fn cell(children: &[NodeRef]) -> NodeRef {
        let cell = create_table_cell_node(0, 1.0, None);
        cell.append(children).unwrap();
        cell
    }

    /// `$createTableNode().append($createTableRowNode().append(...cells))`.
    fn one_row_table(cells: &[NodeRef]) -> NodeRef {
        let row = create_table_row_node(None);
        row.append(cells).unwrap();
        let table = create_table_node();
        table.append(&[row]).unwrap();
        table
    }

    fn element_of(node: NodeRef, children: &[NodeRef]) -> NodeRef {
        node.append(children).unwrap();
        node
    }

    fn id_of(node: &NodeRef) -> Option<String> {
        get_block_id(node).unwrap()
    }

    fn type_of_block(id: &str) -> String {
        get_block_by_id(id).unwrap().unwrap().get_type().to_owned()
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: index > nested blocks in lists and tables, with table cells added
    #[test]
    fn nested_blocks_in_lists_and_tables_with_table_cells_added() {
        let editor = build(BuildOptions {
            is_identified_block: Some(IsIdentifiedBlock(Rc::new(|node: &NodeRef| {
                default_is_identified_block(node) || is_table_cell_node(node)
            }))),
            ..Default::default()
        });
        update(&editor, || {
            get_root().append(&[table(2, 2), paragraph("a"), paragraph("b")])?;
            Ok(())
        });
        let e = (*editor).clone();
        update(&editor, move || {
            select_blocks(1, 2)?;
            e.dispatch_command(&INSERT_UNORDERED_LIST_COMMAND, ())?;
            Ok(())
        });
        assert_eq!(
            outline(&editor),
            strings(&[
                "table:b1",
                "  tablerow",
                "    tablecell:b2",
                "      paragraph:b3",
                "    tablecell:b4",
                "      paragraph:b5",
                "  tablerow",
                "    tablecell:b6",
                "      paragraph:b7",
                "    tablecell:b8",
                "      paragraph:b9",
                "list:b12",
                "  listitem:b10",
                "  listitem:b11",
            ])
        );
        editor
            .read(|| {
                assert_eq!(type_of_block("b4"), "tablecell");
                assert_eq!(
                    get_block_by_id("b11").unwrap().unwrap().get_text_content(),
                    "b"
                );
                assert_eq!(type_of_block("b12"), "list");
            })
            .unwrap();
        editor.dispose();
    }

    /// `BoxNode`: an element that declares no structure, like most app
    /// classes.
    #[derive(Clone, Debug, Default)]
    struct BoxNode {
        base: NodeBase,
        element: ElementData,
    }

    impl LexicalNode for BoxNode {
        fn get_type(&self) -> &'static str {
            "block-id-box"
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

    /// `buildWithBox()`.
    fn build_with_box() -> LexicalEditorWithDispose {
        as_edited(
            build_editor_from_extensions([define_extension(LexicalExtension {
                initial_editor_state: Some(InitialEditorStateType::Null),
                dependencies: vec![
                    crate::rich_text::rich_text_extension().into(),
                    crate::list::list_extension().into(),
                    table_extension().into(),
                    horizontal_rule_extension().into(),
                    crate::code::code_extension().into(),
                    crate::link::link_extension().into(),
                    config_extension(
                        &block_id_extension(),
                        [BlockIdConfigOverrides {
                            generate_id: Some(counter("b")),
                            ..Default::default()
                        }],
                    ),
                ],
                nodes: vec![NodeClass::of::<BoxNode>()],
                ..LexicalExtension::<(), (), ()>::new("[block-id-default]", ())
            })
            .into()])
            .unwrap(),
        )
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the default predicate > root children, nested blocks that hold text, nested block decorators
    #[test]
    fn root_children_nested_blocks_that_hold_text_nested_block_decorators() {
        let editor = build_with_box();
        update(&editor, || {
            // The nodes are created in the order upstream's expression
            // creates them (the table and its row before the cells'
            // content), which orders the ids of one update.
            let table = create_table_node();
            let row = create_table_row_node(None);
            let first_cell = cell(&[
                heading(HeadingTagType::H2, "h"),
                element_of(create_quote_node(false), &[create_text_node("q")]),
                element_of(create_code_node(None, None), &[create_text_node("c")]),
            ]);
            let second_cell = cell(&[
                create_horizontal_rule_node(),
                element_of(create::<BoxNode>(), &[paragraph("in a box")]),
                element_of(create_quote_node(true), &[paragraph("sq")]),
                element_of(
                    create_paragraph_node(),
                    &[element_of(
                        create_link_node("https://lexical.dev", &LinkAttributes::default()),
                        &[create_text_node("link")],
                    )],
                ),
            ]);
            get_root().append(&[
                element_of(table, &[element_of(row, &[first_cell, second_cell])]),
                element_of(
                    create_list_node(ListType::Bullet, 1.0),
                    &[
                        element_of(create_list_item_node(None), &[create_text_node("one")]),
                        element_of(create_list_item_node(None), &[bullet_list(&["two"])]),
                    ],
                ),
                element_of(create::<BoxNode>(), &[paragraph("in a box")]),
                element_of(create_quote_node(true), &[paragraph("sq")]),
            ])?;
            Ok(())
        });
        // A list item that only holds a nested list is still a list item:
        // its class declares inline children.
        assert_eq!(
            outline(&editor),
            strings(&[
                "table:b1",
                "  tablerow",
                "    tablecell",
                "      heading:b2",
                "      quote:b3",
                "      code:b4",
                "    tablecell",
                "      horizontalrule:b5",
                "      block-id-box",
                "        paragraph:b6",
                "      quote",
                "        paragraph:b7",
                "      paragraph:b8",
                "        link",
                "list:b9",
                "  listitem:b10",
                "  listitem:b11",
                "    list",
                "      listitem:b12",
                "block-id-box:b13",
                "  paragraph:b14",
                "quote:b15",
                "  paragraph:b16",
            ])
        );
        editor
            .read(|| {
                fn visit(node: &NodeRef) {
                    if !node.is_text() {
                        // Exactly the nodes the predicate identifies have ids.
                        assert_eq!(
                            id_of(node).is_some(),
                            default_is_identified_block(node),
                            "{}",
                            node.get_type()
                        );
                    }
                    if node.is_element() {
                        node.get_children().iter().for_each(visit);
                    }
                }
                get_root().get_children().iter().for_each(visit);
            })
            .unwrap();
        expect_unique(&editor);
        editor.dispose();
    }

    /// Ids on the root's children only, as `$isRootChildBlock` writes them
    /// (`rootOnlyDocument()`).
    fn root_only_document() -> String {
        let writer = build(BuildOptions {
            is_identified_block: Some(IsIdentifiedBlock(Rc::new(is_root_child_block))),
            generate_id: Some(counter("r")),
            ..Default::default()
        });
        update(&writer, || {
            get_root().append(&[
                paragraph("Title"),
                bullet_list(&["one", "two"]),
                table(1, 2),
            ])?;
            Ok(())
        });
        let json = state_json(&writer).to_string();
        writer.dispose();
        json
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the default predicate > ids stored on the root children are kept; nested blocks get positional ids
    #[test]
    fn ids_stored_on_the_root_children_are_kept_nested_blocks_get_positional_ids() {
        // Written with the previous default: ids on the root's children only.
        let stored = root_only_document();
        let read = || {
            let editor = reader(None);
            load(&editor, &stored);
            editor
        };
        let first = read();
        let second = read();
        assert_eq!(
            outline(&first),
            strings(&[
                "paragraph:r1",
                "list:r2",
                "  listitem:@1.0",
                "  listitem:@1.1",
                "table:r3",
                "  tablerow",
                "    tablecell",
                "      paragraph:@2.0",
                "    tablecell",
                "      paragraph:@2.1",
            ])
        );
        assert_eq!(state_json(&first), state_json(&second));
        // Stored once, they are ordinary ids: loading again changes nothing.
        let saved = state_json(&first).to_string();
        let again = reader(None);
        load(&again, &saved);
        assert_eq!(state_json(&again).to_string(), saved);
        for editor in [first, second, again] {
            editor.dispose();
        }
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the index finds every identified block > a document with ids on the root children only, via setEditorState
    #[test]
    fn a_document_with_ids_on_the_root_children_only_via_set_editor_state() {
        let editor = build(BuildOptions {
            fresh: true,
            ..Default::default()
        });
        load(&editor, &root_only_document());
        assert_eq!(ids(&editor), some(&["r1", "r2", "r3"]));
        expect_all_indexed(&editor);
        // Typing in an item keeps it that way.
        update(&editor, || set_first_item_text("one!"));
        expect_all_indexed(&editor);
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the index finds every identified block > a document with ids on the root children only, as the initial state
    #[test]
    fn a_document_with_ids_on_the_root_children_only_as_the_initial_state() {
        let stored = root_only_document();
        let dependencies: Vec<AnyLexicalExtensionArgument> = vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
            table_extension().into(),
            block_id_extension().into(),
        ];
        let editor = build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Json(stored)),
            dependencies,
            ..LexicalExtension::<(), (), ()>::new("[block-id-initial]", ())
        })
        .into()])
        .unwrap();
        assert_eq!(ids(&editor), some(&["r1", "r2", "r3"]));
        expect_all_indexed(&editor);
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the index finds every identified block > a table given an id with $setState before it is inserted
    #[test]
    fn a_table_given_an_id_with_set_state_before_it_is_inserted() {
        let editor = build_default();
        seed(&editor, &["p"]);
        update(&editor, || {
            let block = table(1, 1);
            set_state(&block, &block_id_state(), Some("my-block".to_owned()))?;
            get_root().append(&[block])?;
            Ok(())
        });
        assert_eq!(ids(&editor), some(&["b1", "my-block"]));
        expect_all_indexed(&editor);
        editor
            .read(|| {
                assert_eq!(
                    get_block_by_id("my-block")
                        .unwrap()
                        .map(|n| n.get_key().to_owned()),
                    Some(nth(1).get_key().to_owned())
                );
            })
            .unwrap();
        editor.dispose();
    }

    /// `nestTable(editor)`: a root table moved into a cell of another
    /// table is not identified; returns its id.
    fn nest_table(editor: &Editor) -> String {
        update(editor, || {
            get_root().append(&[table(1, 1), paragraph("mid"), table(1, 1)])?;
            Ok(())
        });
        let id = editor.read(|| id_of(&nth(2)).unwrap()).unwrap();
        update(editor, || {
            let row = nth(0).get_first_child_or_throw()?;
            assert!(row.is_element());
            let cell = row.get_first_child_or_throw()?;
            assert!(is_table_cell_node(&cell));
            cell.append(&[nth(2)])?;
            Ok(())
        });
        id
    }

    /// `$nestedTable()`.
    fn nested_table() -> NodeRef {
        let row = nth(0).get_first_child_or_throw().unwrap();
        assert!(row.is_element());
        let cell = row.get_first_child_or_throw().unwrap();
        assert!(is_table_cell_node(&cell));
        let table = cell.get_last_child_or_throw().unwrap();
        assert!(table.is_element());
        table
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: ids left on blocks that are not identified > the id stays in NodeState but is not indexed, and comes back with the block
    #[test]
    fn the_id_stays_in_node_state_but_is_not_indexed_and_comes_back_with_the_block() {
        let editor = build_default();
        let id = nest_table(&editor);
        let i = id.clone();
        editor
            .read(move || {
                assert_eq!(id_of(&nested_table()), Some(i.clone()));
                assert!(get_block_by_id(&i).unwrap().is_none());
            })
            .unwrap();
        update(&editor, || {
            get_root().append(&[nested_table()])?;
            Ok(())
        });
        assert_eq!(ids(&editor)[2], Some(id));
        expect_all_indexed(&editor);
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: ids left on blocks that are not identified > a block that took the id meanwhile keeps it, wherever the original returns
    #[test]
    fn a_block_that_took_the_id_meanwhile_keeps_it_wherever_the_original_returns() {
        for above in [true, false] {
            let editor = build_default();
            let id = nest_table(&editor);
            // A copy of the nested table at the root: nothing identified
            // holds the id, so the copy keeps it.
            update(&editor, || {
                nth(1).insert_after(&deep_copy(&nested_table()), true)?;
                Ok(())
            });
            let copy_key = editor.read(|| nth(2).get_key().to_owned()).unwrap();
            assert_eq!(ids(&editor)[2], Some(id.clone()));
            update(&editor, move || {
                let original = nested_table();
                if above {
                    nth(0).insert_before(&original, true)?;
                } else {
                    get_root().append(&[original])?;
                }
                Ok(())
            });
            expect_unique(&editor);
            expect_all_indexed(&editor);
            editor
                .read(|| {
                    assert_eq!(
                        get_block_by_id(&id).unwrap().unwrap().get_key(),
                        copy_key,
                        "above: {above}"
                    );
                })
                .unwrap();
            editor.dispose();
        }
    }

    macro_rules! decorator_node {
        ($(#[$doc:meta])* $name:ident, $type:literal, inline: $inline:expr) => {
            $(#[$doc])*
            #[derive(Clone, Debug, Default)]
            struct $name {
                base: NodeBase,
            }

            impl LexicalNode for $name {
                fn get_type(&self) -> &'static str {
                    $type
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
                    $inline
                }
            }
        };
    }

    decorator_node!(
        /// `InlineDecorator`.
        InlineDecorator,
        "block-id-inline-decorator",
        inline: true
    );
    decorator_node!(
        /// `BlockDecorator`.
        BlockDecorator,
        "block-id-block-decorator",
        inline: false
    );

    /// `buildWith($isIdentifiedBlock)`.
    fn build_with_decorators(predicate: Option<IsIdentifiedBlock>) -> LexicalEditorWithDispose {
        as_edited(
            build_editor_from_extensions([define_extension(LexicalExtension {
                initial_editor_state: Some(InitialEditorStateType::Null),
                dependencies: vec![
                    crate::rich_text::rich_text_extension().into(),
                    crate::list::list_extension().into(),
                    table_extension().into(),
                    config_extension(
                        &block_id_extension(),
                        [BlockIdConfigOverrides {
                            generate_id: Some(counter("b")),
                            is_identified_block: predicate,
                            ..Default::default()
                        }],
                    ),
                ],
                nodes: vec![
                    NodeClass::of::<InlineDecorator>(),
                    NodeClass::of::<BlockDecorator>(),
                ],
                ..LexicalExtension::<(), (), ()>::new("[block-id-predicates]", ())
            })
            .into()])
            .unwrap(),
        )
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: more predicates and nesting > nested inline decorators get no id; a block decorator in a cell does
    #[test]
    fn nested_inline_decorators_get_no_id_a_block_decorator_in_a_cell_does() {
        let editor = build_with_decorators(None);
        update(&editor, || {
            get_root().append(&[
                element_of(
                    create_paragraph_node(),
                    &[create_text_node("a"), create::<InlineDecorator>()],
                ),
                element_of(
                    create_list_node(ListType::Bullet, 1.0),
                    &[element_of(
                        create_list_item_node(None),
                        &[create_text_node("i"), create::<InlineDecorator>()],
                    )],
                ),
                one_row_table(&[cell(&[create::<BlockDecorator>()])]),
            ])?;
            Ok(())
        });
        let lines: Vec<String> = outline(&editor).iter().map(|l| generic_id(l)).collect();
        assert_eq!(
            lines,
            strings(&[
                "paragraph:id",
                "  block-id-inline-decorator",
                "list:id",
                "  listitem:id",
                "    block-id-inline-decorator",
                "table:id",
                "  tablerow",
                "    tablecell",
                "      block-id-block-decorator:id",
            ])
        );
        expect_all_indexed(&editor);
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: more predicates and nesting > a custom predicate that is not composed from the default: headings at any depth
    #[test]
    fn a_custom_predicate_that_is_not_composed_from_the_default_headings_at_any_depth() {
        let editor = build_with_decorators(Some(IsIdentifiedBlock(Rc::new(is_heading_node))));
        update(&editor, || {
            get_root().append(&[
                paragraph("p"),
                heading(HeadingTagType::H1, "h"),
                one_row_table(&[cell(&[heading(HeadingTagType::H2, "nested")])]),
            ])?;
            Ok(())
        });
        assert_eq!(
            outline(&editor),
            strings(&[
                "paragraph",
                "heading:b1",
                "table",
                "  tablerow",
                "    tablecell",
                "      heading:b2",
            ])
        );
        editor
            .read(|| {
                assert_eq!(
                    get_block_by_id("b2").unwrap().unwrap().get_text_content(),
                    "nested"
                );
            })
            .unwrap();
        editor.dispose();
    }
}
