//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts`:
//! block ids in an editor that also enforces declared node structure, whose
//! normalization splits, joins, lifts and merges blocks in the same update
//! as the change that needs it.

#![cfg(all(feature = "rich-text", feature = "list", feature = "link"))]

use super::support::{counter, update_tagged};
use crate::extension::{
    BlockIdConfigOverrides, LexicalEditorWithDispose, NodeStructureConfigOverrides,
    block_id_extension, block_id_state, build_editor_from_extensions, get_block_by_id,
    get_block_id, get_extension_dependency_from_editor, node_structure_extension,
};
use crate::json::Value;
use crate::link::{LinkAttributes, create_link_node};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::rich_text::{HeadingTagType, create_heading_node, create_quote_node};
use crate::{
    AnyLexicalExtensionArgument, Editor, HISTORY_MERGE_TAG, InitialEditorStateType, LexicalError,
    LexicalExtension, NodeRef, UpdateOptions, config_extension, create_paragraph_node,
    create_text_node, define_extension, get_root, set_state,
};
use std::collections::HashSet;

/// The order the extensions are listed in, or the structure enabled after
/// the editor is built, which registers its normalization after block ids,
/// or no structure at all.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Order {
    BlockIdsFirst,
    StructureFirst,
    StructureEnabledLater,
    NoStructure,
}

/// `build(order, hadContent)`.
fn build(order: Order, had_content: bool) -> LexicalEditorWithDispose {
    let block_ids = || {
        config_extension(
            &block_id_extension(),
            [BlockIdConfigOverrides {
                generate_id: Some(counter("g")),
                ..Default::default()
            }],
        )
    };
    let mut dependencies: Vec<AnyLexicalExtensionArgument> = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::list::list_extension().into(),
        crate::link::link_extension().into(),
    ];
    match order {
        Order::NoStructure => dependencies.push(block_ids()),
        Order::BlockIdsFirst => {
            dependencies.push(block_ids());
            dependencies.push(node_structure_extension().into());
        }
        Order::StructureFirst => {
            dependencies.push(node_structure_extension().into());
            dependencies.push(block_ids());
        }
        Order::StructureEnabledLater => {
            dependencies.push(block_ids());
            dependencies.push(config_extension(
                &node_structure_extension(),
                [NodeStructureConfigOverrides {
                    disabled: Some(true),
                }],
            ));
        }
    }
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new("[block-id-structure-test]", ())
    })
    .into()])
    .unwrap();
    if order == Order::StructureEnabledLater {
        get_extension_dependency_from_editor(&editor, &node_structure_extension())
            .unwrap()
            .output
            .unwrap()
            .disabled
            .set_value(false);
    }
    if had_content {
        // Have had content once, so that new blocks are edits, not a load.
        update_tagged(
            &editor,
            || {
                get_root().append(&[create_paragraph_node()])?;
                Ok(())
            },
            &[HISTORY_MERGE_TAG],
        );
        update_tagged(&editor, || get_root().clear(), &[HISTORY_MERGE_TAG]);
    }
    editor
}

fn update(editor: &Editor, f: fn() -> Result<(), LexicalError>) {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

/// `$show(node)`: `type#id[children]`, with text in quotes and links as
/// `link[...]`.
fn show(node: &NodeRef) -> String {
    if node.is_text() {
        return Value::from(node.get_text_content()).to_string();
    }
    if !node.is_element() {
        return node.get_type().to_owned();
    }
    let id = get_block_id(node).unwrap();
    let children: Vec<String> = node.get_children().iter().map(show).collect();
    format!(
        "{}{}[{}]",
        node.get_type(),
        id.map(|id| format!("#{id}")).unwrap_or_default(),
        children.join(" ")
    )
}

fn shape(editor: &Editor) -> Vec<String> {
    editor
        .read(|| get_root().get_children().iter().map(show).collect())
        .unwrap()
}

/// `expectIndexed(editor)`: every id is unique and the index finds the
/// block that holds it.
fn expect_indexed(editor: &Editor) {
    editor
        .read(|| {
            let mut seen = HashSet::new();
            fn walk(node: &NodeRef, seen: &mut HashSet<String>) {
                if let Some(id) = get_block_id(node).unwrap() {
                    assert!(seen.insert(id.clone()), "{id} is repeated");
                    assert_eq!(
                        get_block_by_id(&id)
                            .unwrap()
                            .map(|n| n.get_key().to_owned()),
                        Some(node.get_key().to_owned()),
                        "{id}"
                    );
                }
                if node.is_element() {
                    for child in node.get_children() {
                        walk(&child, seen);
                    }
                }
            }
            walk(&get_root(), &mut seen);
        })
        .unwrap();
}

/// `$withId(node, id)`.
fn with_id(node: NodeRef, id: &str) -> NodeRef {
    set_state(&node, &block_id_state(), Some(id.to_owned())).unwrap();
    node
}

fn t(text: &str) -> NodeRef {
    create_text_node(text)
}

fn append(parent: NodeRef, children: Vec<NodeRef>) -> NodeRef {
    parent.append(&children).unwrap();
    parent
}

fn p(children: Vec<NodeRef>) -> NodeRef {
    append(create_paragraph_node(), children)
}

fn li(children: Vec<NodeRef>) -> NodeRef {
    append(create_list_item_node(None), children)
}

fn ul(children: Vec<NodeRef>) -> NodeRef {
    append(create_list_node(ListType::Bullet, 1.0), children)
}

fn h1(children: Vec<NodeRef>) -> NodeRef {
    append(create_heading_node(HeadingTagType::H1), children)
}

fn quote(children: Vec<NodeRef>) -> NodeRef {
    append(create_quote_node(false), children)
}

/// `$block(id)`.
fn block(id: &str) -> NodeRef {
    get_block_by_id(id).unwrap().unwrap()
}

fn root_append(children: Vec<NodeRef>) -> Result<(), LexicalError> {
    get_root().append(&children)?;
    Ok(())
}

struct Case {
    setup: fn() -> Result<(), LexicalError>,
    change: fn() -> Result<(), LexicalError>,
    expected: &'static [&'static str],
}

const CASES: &[Case] = &[
    // a paragraph split around a block lifted out of it: the first piece
    // keeps the id
    Case {
        setup: || root_append(vec![with_id(p(vec![t("a")]), "A")]),
        change: || {
            block("A").append(&[with_id(quote(vec![t("q")]), "Q"), t("b")])?;
            Ok(())
        },
        expected: &[
            r#"paragraph#A["a"]"#,
            r#"quote#Q["q"]"#,
            r#"paragraph#g1["b"]"#,
        ],
    },
    // a heading split around a list: the list and the second piece get new
    // ids
    Case {
        setup: || root_append(vec![with_id(h1(vec![t("a")]), "H")]),
        change: || {
            block("H").append(&[ul(vec![with_id(li(vec![t("x")]), "L")]), t("b")])?;
            Ok(())
        },
        expected: &[
            r#"heading#H["a"]"#,
            r#"list#g1[listitem#L["x"]]"#,
            r#"heading#g2["b"]"#,
        ],
    },
    // a block lifted out of a paragraph it emptied takes its place and its
    // id
    Case {
        setup: || root_append(vec![with_id(p(vec![t("x")]), "A")]),
        change: || {
            let paragraph = block("A");
            paragraph.clear()?;
            paragraph.append(&[quote(vec![t("q")])])?;
            Ok(())
        },
        expected: &[r#"quote#A["q"]"#],
    },
    // a list lifted out of a link: the items keep their ids and the link
    Case {
        setup: || root_append(vec![with_id(p(vec![t("z")]), "A")]),
        change: || {
            let paragraph = block("A");
            paragraph.clear()?;
            paragraph.append(&[append(
                create_link_node("https://lexical.dev", &LinkAttributes::default()),
                vec![ul(vec![
                    with_id(li(vec![t("x")]), "L1"),
                    with_id(li(vec![t("y")]), "L2"),
                ])],
            )])?;
            Ok(())
        },
        expected: &[r#"list#A[listitem#L1[link["x"]] listitem#L2[link["y"]]]"#],
    },
    // paragraphs joined as lines: the block that absorbs them keeps its id
    Case {
        setup: || root_append(vec![with_id(h1(vec![t("h")]), "H")]),
        change: || {
            block("H").append(&[
                with_id(p(vec![t("a")]), "PA"),
                with_id(p(vec![t("b")]), "PB"),
            ])?;
            Ok(())
        },
        expected: &[r#"heading#H["h" linebreak "a" linebreak "b"]"#],
    },
    // equal adjacent lists merge: the first keeps its id
    Case {
        setup: || {
            root_append(vec![
                with_id(ul(vec![with_id(li(vec![t("a")]), "LA")]), "U1"),
                with_id(p(vec![t("sep")]), "SEP"),
                with_id(ul(vec![with_id(li(vec![t("b")]), "LB")]), "U2"),
            ])
        },
        change: || block("SEP").remove(false),
        expected: &[r#"list#U1[listitem#LA["a"] listitem#LB["b"]]"#],
    },
    // nested lists joined at a seam: the first item keeps its id
    Case {
        setup: || {
            root_append(vec![
                with_id(
                    ul(vec![with_id(
                        li(vec![ul(vec![with_id(li(vec![t("a")]), "NA")])]),
                        "W1",
                    )]),
                    "U1",
                ),
                with_id(p(vec![t("sep")]), "SEP"),
                with_id(
                    ul(vec![with_id(
                        li(vec![ul(vec![with_id(li(vec![t("b")]), "NB")])]),
                        "W2",
                    )]),
                    "U2",
                ),
            ])
        },
        change: || block("SEP").remove(false),
        expected: &[r#"list#U1[listitem#W1[list[listitem#NA["a"] listitem#NB["b"]]]]"#],
    },
    // a new list without an id merged with the list after it continues that
    // list
    Case {
        setup: || {
            root_append(vec![
                with_id(p(vec![t("z")]), "Z"),
                with_id(ul(vec![with_id(li(vec![t("a")]), "LA")]), "U1"),
            ])
        },
        change: || {
            block("U1").insert_before(&ul(vec![li(vec![t("new")])]), true)?;
            Ok(())
        },
        expected: &[
            r#"paragraph#Z["z"]"#,
            r#"list#U1[listitem#g1["new"] listitem#LA["a"]]"#,
        ],
    },
    // a new list with an id merged with the list after it keeps its own id
    Case {
        setup: || {
            root_append(vec![
                with_id(p(vec![t("z")]), "Z"),
                with_id(ul(vec![with_id(li(vec![t("a")]), "LA")]), "U1"),
            ])
        },
        change: || {
            block("U1").insert_before(&with_id(ul(vec![li(vec![t("new")])]), "NEW"), true)?;
            Ok(())
        },
        expected: &[
            r#"paragraph#Z["z"]"#,
            r#"list#NEW[listitem#g1["new"] listitem#LA["a"]]"#,
        ],
    },
    // a copy of a list merged with the equal list after it continues that
    // list
    Case {
        setup: || {
            root_append(vec![
                with_id(ul(vec![with_id(li(vec![t("l")]), "LL")]), "L"),
                with_id(p(vec![t("sep")]), "SEP"),
                with_id(ul(vec![with_id(li(vec![t("m")]), "MM")]), "M"),
            ])
        },
        change: || {
            // A copy of list L, with L's ids, pasted right above the equal
            // list M.
            block("M").insert_before(
                &with_id(ul(vec![with_id(li(vec![t("l")]), "LL")]), "L"),
                true,
            )?;
            Ok(())
        },
        expected: &[
            r#"list#L[listitem#LL["l"]]"#,
            r#"paragraph#SEP["sep"]"#,
            r#"list#M[listitem#g1["l"] listitem#MM["m"]]"#,
        ],
    },
];

/// `s.replace(/#g\d+/g, '#g')`.
fn without_generated_numbers(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("#g") {
        out.push_str(&rest[..i + 2]);
        rest = &rest[i + 2..];
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 {
            continue;
        }
        rest = &rest[digits..];
    }
    out.push_str(rest);
    out
}

/// `s.replace(/#[\w-]+/g, '')`.
fn without_ids(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('#') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let len = after.len()
            - after
                .trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                .len();
        if len == 0 {
            out.push('#');
        }
        rest = &after[len..];
    }
    out.push_str(rest);
    out
}

/// The `describe(order)` block: each case, then `copies of a list, one
/// apart and one on each side of it: the merged list keeps its id`.
fn run_cases(order: Order) {
    for (i, case) in CASES.iter().enumerate() {
        let editor = build(order, true);
        update(&editor, case.setup);
        update(&editor, case.change);
        assert_eq!(shape(&editor), case.expected, "{order:?} case {i}");
        expect_indexed(&editor);
        editor.dispose();
    }
    let editor = build(order, true);
    update(&editor, || {
        root_append(vec![
            with_id(p(vec![t("z")]), "Z"),
            with_id(h1(vec![t("h")]), "H"),
            with_id(ul(vec![with_id(li(vec![t("a")]), "A")]), "U"),
        ])
    });
    update(&editor, || {
        // Copies of list U, with U's ids.
        let copy = || with_id(ul(vec![with_id(li(vec![t("a")]), "A")]), "U");
        block("Z").insert_after(&copy(), true)?;
        block("H").insert_after(&copy(), true)?;
        block("U").insert_after(&copy(), true)?;
        Ok(())
    });
    // Generated ids (`g<n>`) depend on how many were discarded.
    assert_eq!(
        shape(&editor)
            .iter()
            .map(|b| without_generated_numbers(b))
            .collect::<Vec<_>>(),
        [
            r#"paragraph#Z["z"]"#,
            r#"list#g[listitem#g["a"]]"#,
            r#"heading#H["h"]"#,
            r#"list#U[listitem#g["a"] listitem#A["a"] listitem#g["a"]]"#,
        ],
        "{order:?}"
    );
    expect_indexed(&editor);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > a paragraph split around a block lifted…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > a heading split around a list: the list…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > a block lifted out of a paragraph it em…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > a list lifted out of a link: the items …
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > paragraphs joined as lines: the block t…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > equal adjacent lists merge: the first k…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > nested lists joined at a seam: the firs…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > a new list without an id merged with th…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > a new list with an id merged with the l…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > a copy of a list merged with the equal …
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > copies of a list, one apart and one on each side of it: the merged list keeps its id
#[test]
fn block_ids_first() {
    run_cases(Order::BlockIdsFirst);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > a paragraph split around a block lifted…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > a heading split around a list: the list…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > a block lifted out of a paragraph it em…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > a list lifted out of a link: the items …
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > paragraphs joined as lines: the block t…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > equal adjacent lists merge: the first k…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > nested lists joined at a seam: the firs…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > a new list without an id merged with th…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > a new list with an id merged with the l…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > a copy of a list merged with the equal …
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > copies of a list, one apart and one on each side of it: the merged list keeps its id
#[test]
fn structure_first() {
    run_cases(Order::StructureFirst);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > a paragraph split around a block lifted…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > a heading split around a list: the list…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > a block lifted out of a paragraph it em…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > a list lifted out of a link: the items …
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > paragraphs joined as lines: the block t…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > equal adjacent lists merge: the first k…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > nested lists joined at a seam: the firs…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > a new list without an id merged with th…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > a new list with an id merged with the l…
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > a copy of a list merged with the equal …
// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > copies of a list, one apart and one on each side of it: the merged list keeps its id
#[test]
fn structure_enabled_later() {
    run_cases(Order::StructureEnabledLater);
}

/// `storedJSON($build)`: a document as an editor without either extension
/// stores it, unrepaired.
fn stored_json(f: fn() -> Result<(), LexicalError>) -> String {
    let writer = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
            crate::link::link_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[block-id-structure-test-writer]", ())
    })
    .into()])
    .unwrap();
    writer
        .update(
            f,
            UpdateOptions {
                discrete: true,
                skip_transforms: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    let json = writer.get_editor_state().to_json(false).to_string();
    writer.dispose();
    json
}

/// `loadedIds(order, json)`: the blocks a load of `json` gives, all with
/// positional ids.
fn loaded_ids(order: Order, json: &str) -> Vec<String> {
    let editor = build(order, false);
    editor
        .set_editor_state(&editor.parse_editor_state(json).unwrap(), None)
        .unwrap();
    expect_indexed(&editor);
    let blocks = shape(&editor);
    // No generated (`g<n>`) or missing ids.
    let joined = blocks.join(" ");
    assert_eq!(without_generated_numbers(&joined), joined, "{joined}");
    for missing in ["paragraph[", "quote[", "list[", "listitem["] {
        assert!(!joined.contains(missing), "{joined}");
    }
    editor.dispose();
    blocks
}

/// `idsOfLoadedDocument(order, json)`: the ids an editor without
/// NodeStructureExtension derives for the document that `order` loads
/// `json` into, stored again without ids.
fn ids_of_loaded_document(order: Order, json: &str) -> Vec<String> {
    let editor = build(order, false);
    editor
        .set_editor_state(&editor.parse_editor_state(json).unwrap(), None)
        .unwrap();
    let mut loaded = editor.get_editor_state().to_json(false);
    editor.dispose();
    fn strip(node: &mut Value) {
        let Value::Object(node) = node else {
            return;
        };
        if let Some(Value::Object(state)) = node.get_mut("$") {
            state.remove("blockId");
        }
        if let Some(Value::Array(children)) = node.get_mut("children") {
            children.iter_mut().for_each(strip);
        }
    }
    let Value::Object(root) = &mut loaded else {
        unreachable!()
    };
    strip(root.get_mut("root").unwrap());
    loaded_ids(Order::NoStructure, &loaded.to_string())
}

fn lists() -> String {
    stored_json(|| {
        root_append(vec![
            p(vec![t("z")]),
            ul(vec![li(vec![t("x")])]),
            ul(vec![li(vec![t("y")])]),
        ])
    })
}

fn split_quote() -> String {
    stored_json(|| {
        root_append(vec![
            p(vec![t("a"), quote(vec![t("q")]), t("b")]),
            p(vec![t("c")]),
        ])
    })
}

fn equal_adjacent_lists(order: Order) {
    let lists = lists();
    let ids = loaded_ids(order, &lists);
    assert_eq!(ids, ids_of_loaded_document(order, &lists));
    assert_eq!(ids, loaded_ids(Order::NoStructure, &lists));
    assert_eq!(
        ids.iter().map(|b| without_ids(b)).collect::<Vec<_>>(),
        [r#"paragraph["z"]"#, r#"list[listitem["x"] listitem["y"]]"#]
    );
}

fn split_document(order: Order) {
    let quote = split_quote();
    let ids = loaded_ids(order, &quote);
    assert_eq!(ids, ids_of_loaded_document(order, &quote));
    assert_eq!(
        ids.iter().map(|b| without_ids(b)).collect::<Vec<_>>(),
        [
            r#"paragraph["a"]"#,
            r#"quote["q"]"#,
            r#"paragraph["b"]"#,
            r#"paragraph["c"]"#
        ]
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension: loads > block ids first: equal adjacent lists get the ids of the merged document, as without the structure
#[test]
fn loads_block_ids_first_equal_adjacent_lists() {
    equal_adjacent_lists(Order::BlockIdsFirst);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension: loads > block ids first: a split document gets the ids of the document the load ends with
#[test]
fn loads_block_ids_first_split_document() {
    split_document(Order::BlockIdsFirst);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension: loads > structure first: equal adjacent lists get the ids of the merged document, as without the structure
#[test]
fn loads_structure_first_equal_adjacent_lists() {
    equal_adjacent_lists(Order::StructureFirst);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension: loads > structure first: a split document gets the ids of the document the load ends with
#[test]
fn loads_structure_first_split_document() {
    split_document(Order::StructureFirst);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension: loads > both orders derive the same ids
#[test]
fn loads_both_orders_derive_the_same_ids() {
    let (quote, lists) = (split_quote(), lists());
    assert_eq!(
        loaded_ids(Order::BlockIdsFirst, &quote),
        loaded_ids(Order::StructureFirst, &quote)
    );
    assert_eq!(
        loaded_ids(Order::BlockIdsFirst, &lists),
        loaded_ids(Order::StructureFirst, &lists)
    );
}
