//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts`.
//!
//! Assertions JS makes inside an update are made on the values the update
//! returns ([`update_with`]).

#![cfg(all(feature = "rich-text", feature = "list"))]

use super::super::support::{update, update_with};
use super::support::{as_edited, tagged};
use crate::extension::block_handles::{
    HANDLE_KEY_LENGTH, HandleTable, build_handle_table, handle_in_table, handle_key,
    ids_with_prefix,
};
use crate::extension::{
    BlockHandleOptions, BlockHandleResult, BlockIdConfigOverrides, GenerateId,
    LexicalEditorWithDispose, MIN_BLOCK_HANDLE_LENGTH, block_id_extension, block_id_state,
    build_editor_from_extensions, get_block_by_id, get_block_handle, get_block_handles,
    get_block_id, get_extension_dependency_from_editor, resolve_block_handle,
};
use crate::json::Value;
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::updates::read_editor_state;
use crate::{
    AnyLexicalExtensionArgument, COLLABORATION_TAG, Editor, InitialEditorStateType,
    LexicalExtension, UpdateOptions, config_extension, create_paragraph_node, create_text_node,
    define_extension, get_root, set_state,
};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

/// `fixedIds(ids)`: a generateId that hands out `ids` in order, then a
/// counter.
fn fixed_ids(ids: &[&str]) -> GenerateId {
    let ids: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
    let (i, n) = (Cell::new(0), Cell::new(0));
    GenerateId(Rc::new(move || {
        if i.get() < ids.len() {
            i.set(i.get() + 1);
            ids[i.get() - 1].clone()
        } else {
            n.set(n.get() + 1);
            format!("zz{}", n.get())
        }
    }))
}

fn editor_of(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn block_ids(disabled: bool, generate_id: Option<GenerateId>) -> AnyLexicalExtensionArgument {
    config_extension(
        &block_id_extension(),
        [BlockIdConfigOverrides {
            disabled: Some(disabled),
            generate_id,
            ..Default::default()
        }],
    )
}

/// `build(ids, {disabled})`.
fn build(ids: &[&str]) -> LexicalEditorWithDispose {
    as_edited(editor_of(
        "[block-handle-test]",
        vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
            block_ids(false, Some(fixed_ids(ids))),
        ],
    ))
}

/// `load(ids, {disabled})`: an editor holding one paragraph per id, loaded
/// from JSON.
fn load(ids: &[&str], disabled: bool) -> LexicalEditorWithDispose {
    let editor = editor_of(
        "[block-handle-load]",
        vec![
            crate::rich_text::rich_text_extension().into(),
            block_ids(disabled, None),
        ],
    );
    let children: Vec<String> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            format!(
                r#"{{"$":{{"blockId":{}}},"children":[{{"text":"Block {i}","type":"text"}}],"type":"paragraph"}}"#,
                Value::from(*id)
            )
        })
        .collect();
    let json = format!(
        r#"{{"root":{{"children":[{}],"type":"root"}}}}"#,
        children.join(",")
    );
    editor
        .set_editor_state(&editor.parse_editor_state(&json).unwrap(), None)
        .unwrap();
    editor
}

/// `appendParagraphs(editor, texts)`: appends one paragraph per text and
/// returns their ids.
fn append_paragraphs(editor: &Editor, texts: &[&str]) -> Vec<String> {
    let owned: Vec<String> = texts.iter().map(|t| (*t).to_owned()).collect();
    update(editor, move || {
        for text in &owned {
            let p = create_paragraph_node();
            p.append(&[create_text_node(text)])?;
            get_root().append(&[p])?;
        }
        Ok(())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            children[children.len() - texts.len()..]
                .iter()
                .map(|node| get_block_id(node).unwrap().unwrap())
                .collect()
        })
        .unwrap()
}

type Handles = BTreeMap<String, Option<String>>;

fn handles_in_read() -> Handles {
    get_root()
        .get_children()
        .iter()
        .map(|node| {
            (
                get_block_id(node).unwrap().unwrap_or_else(|| "null".into()),
                get_block_handle(node).unwrap(),
            )
        })
        .collect()
}

/// `handlesOf(editor)`: id → handle of every top-level block, through
/// `$getBlockHandle`.
fn handles_of(editor: &Editor) -> Handles {
    editor.read(handles_in_read).unwrap()
}

fn map(entries: &[(&str, &str)]) -> Handles {
    entries
        .iter()
        .map(|(id, h)| ((*id).to_owned(), Some((*h).to_owned())))
        .collect()
}

/// `resolve`'s result: a found block by its id.
#[derive(Debug, PartialEq)]
enum R {
    Found(Option<String>),
    NotFound(String),
    Ambiguous(String, Vec<String>),
    Invalid(String, &'static str),
}

fn found(id: &str) -> R {
    R::Found(Some(id.to_owned()))
}

fn not_found(handle: &str) -> R {
    R::NotFound(handle.to_owned())
}

fn ambiguous(handle: &str, candidates: &[&str]) -> R {
    R::Ambiguous(
        handle.to_owned(),
        candidates.iter().map(|c| (*c).to_owned()).collect(),
    )
}

fn invalid(handle: &str, reason: &'static str) -> R {
    R::Invalid(handle.to_owned(), reason)
}

fn shown(result: BlockHandleResult) -> R {
    match result {
        BlockHandleResult::Found { node } => R::Found(get_block_id(&node).unwrap()),
        BlockHandleResult::NotFound { handle } => R::NotFound(handle),
        BlockHandleResult::Ambiguous { handle, candidates } => R::Ambiguous(handle, candidates),
        BlockHandleResult::Invalid { handle, reason } => R::Invalid(handle, reason.as_str()),
    }
}

fn resolve(editor: &Editor, handle: &str) -> R {
    editor
        .read(|| shown(resolve_block_handle(handle, None).unwrap()))
        .unwrap()
}

/// `bruteForceHandle(id, ids)`: the shortest prefix of `id`'s key, at least
/// 4 long, no other key starts.
fn brute_force_handle(id: &str, ids: &[String]) -> String {
    let key = handle_key(id);
    let others: Vec<String> = ids
        .iter()
        .filter(|o| *o != id)
        .map(|o| handle_key(o))
        .collect();
    let mut length = MIN_BLOCK_HANDLE_LENGTH;
    while length < key.len() && others.iter().any(|o| o.starts_with(&key[..length])) {
        length += 1;
    }
    key[..length].to_owned()
}

const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// `oneCharTypos(s)`: every string one substitution, deletion or insertion
/// away from `s` (ASCII).
fn one_char_typos(s: &str) -> Vec<String> {
    let mut result = HashSet::new();
    for i in 0..s.len() {
        result.insert(format!("{}{}", &s[..i], &s[i + 1..]));
        for c in ALPHABET.chars() {
            result.insert(format!("{}{c}{}", &s[..i], &s[i + 1..]));
        }
    }
    for i in 0..=s.len() {
        for c in ALPHABET.chars() {
            result.insert(format!("{}{c}{}", &s[..i], &s[i..]));
        }
    }
    result.remove(s);
    result.into_iter().collect()
}

/// `measure(editor)`: the handle lengths of every block, and the
/// one-character typos of their handles that name another block. A typo may
/// name another block only by being that block's handle or a longer prefix
/// of its key.
fn measure(editor: &Editor) -> (Vec<usize>, usize, usize) {
    editor
        .read(|| {
            let handles = get_block_handles().unwrap();
            let handle_of = |id: &str| {
                handles
                    .iter()
                    .find(|(i, _)| i == id)
                    .map(|(_, h)| h.clone())
            };
            let (mut tried, mut hits) = (0, 0);
            for (id, handle) in &handles {
                for typo in one_char_typos(handle) {
                    tried += 1;
                    if let BlockHandleResult::Found { node } =
                        resolve_block_handle(&typo, None).unwrap()
                    {
                        let other = get_block_id(&node).unwrap().unwrap();
                        if &other != id {
                            hits += 1;
                            assert!(
                                handle_key(&other).starts_with(&typo)
                                    && typo.starts_with(&handle_of(&other).unwrap()),
                                "typo {typo} of {handle} named {other}"
                            );
                        }
                    }
                }
            }
            (handles.iter().map(|(_, h)| h.len()).collect(), tried, hits)
        })
        .unwrap()
}

/// `mulberry32(seed)`.
fn mulberry32(seed: u32) -> impl FnMut() -> f64 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6d2b79f5);
        let mut t = a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }
}

/// `n.toString(radix)` of a non-negative integer.
fn radix(mut n: u64, radix: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = vec![];
    loop {
        out.push(DIGITS[(n % radix) as usize]);
        n /= radix;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

/// `uuidv7s(count)`: UUIDv7s in their standard layout, made 0 to 2 ms
/// apart.
fn uuidv7s(count: usize) -> Vec<String> {
    let mut random = mulberry32(7);
    let hex = |random: &mut dyn FnMut() -> f64, digits: usize| -> String {
        (0..digits)
            .map(|_| radix((random() * 16.0).floor() as u64, 16))
            .collect()
    };
    let mut ms: u64 = 1793357300000;
    (0..count)
        .map(|_| {
            ms += (random() * 3.0).floor() as u64;
            let time = format!("{:0>12}", radix(ms, 16));
            let variant = radix(8 + (random() * 4.0).floor() as u64, 16);
            let a = hex(&mut random, 3);
            let b = hex(&mut random, 3);
            let c = hex(&mut random, 12);
            format!("{}-{}-7{a}-{variant}{b}-{c}", &time[..8], &time[8..])
        })
        .collect()
}

/// `derivedIds(count)`: ids shaped like `@lexical/y`'s derived ids: `y`, a
/// client, `_`, a clock.
fn derived_ids(count: usize) -> Vec<String> {
    let mut random = mulberry32(36);
    let client = radix((random() * 2f64.powi(53)).floor() as u64, 36);
    let mut clock = 0;
    (0..count)
        .map(|_| {
            clock += 10 + (random() * 70.0).floor() as u64;
            format!("y{client}_{}", radix(clock, 36))
        })
        .collect()
}

fn counter_ids(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("block-{i:0>6}")).collect()
}

const V1ST: &str = "V1StGXR8_Z5jdHi6B-myT";
const XQE: &str = "9xQeLm2Pq0aK7cTb_wYuA";
const RK8P: &str = "Rk8pZc41Uo-HsVd2NnFjB";
// Ids picked for their hashes: each of these starts its hash with the same
// 4 characters as the one it is named after.
const LIKE_V1ST: &str = "qfapyr";
const LIKE_XQE: &str = "q71spi";
// Pairs whose hashes start with the same 4 characters.
const PAIR_A: [&str; 2] = ["p13e", "p20d"];
const PAIR_B: [&str; 2] = ["p1i4", "p2pq"];
// Eleven ids whose hashes all start with sclT.
const ELEVEN: [&str; 11] = [
    "same0",
    "same3abof",
    "same4onh9",
    "samesnq2r",
    "samev3hf4",
    "same1acey1",
    "same1lwsd9",
    "same1o9hsa",
    "same1sf64m",
    "same1yzxvi",
    "same1zd3zy",
];

fn table_of(ids: &[&str]) -> HandleTable {
    build_handle_table(ids.iter().copied())
}

fn handles_map(table: &HandleTable) -> Handles {
    table
        .handles
        .iter()
        .map(|(id, h)| (id.to_owned(), Some(h.clone())))
        .collect()
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > handleKey: 64 bits of a hash of the id in base64url
#[test]
fn handle_key_is_64_bits_of_a_hash_in_base64url() {
    assert_eq!(handle_key(V1ST), "2bNoqlijdOg");
    assert_eq!(handle_key(XQE), "gTtFDmoxN_s");
    assert_eq!(handle_key(RK8P), "xt6zBAcASRs");
    assert_eq!(HANDLE_KEY_LENGTH, 11);
    for id in ["", "a", "b1", V1ST, &"y".repeat(64)] {
        let key = handle_key(id);
        assert_eq!(key.len(), 11, "{id}");
        assert!(key.chars().all(|c| ALPHABET.contains(c)), "{id}");
    }
    // The picked ids do share the start of their hashes.
    assert_eq!(&handle_key(LIKE_V1ST)[..4], "2bNo");
    assert_eq!(&handle_key(LIKE_XQE)[..4], "gTtF");
    for [a, b] in [PAIR_A, PAIR_B] {
        assert_eq!(handle_key(a)[..4], handle_key(b)[..4]);
        assert_ne!(handle_key(a)[4..5], handle_key(b)[4..5]);
    }
    for id in ELEVEN {
        assert_eq!(&handle_key(id)[..4], "sclT");
    }
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > handles are the shortest unique prefix of the hash, at least 4 characters
#[test]
fn handles_are_the_shortest_unique_prefix() {
    assert_eq!(
        handles_map(&table_of(&[V1ST, XQE, RK8P])),
        map(&[(RK8P, "xt6z"), (V1ST, "2bNo"), (XQE, "gTtF")])
    );
    assert_eq!(
        handles_map(&table_of(&[V1ST, XQE, LIKE_V1ST, LIKE_XQE])),
        map(&[
            (LIKE_V1ST, "2bNoV"),
            (LIKE_XQE, "gTtFc"),
            (V1ST, "2bNoq"),
            (XQE, "gTtFD"),
        ])
    );
    assert_eq!(MIN_BLOCK_HANDLE_LENGTH, 4);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > matches the brute-force definition, whatever the ids look like
#[test]
fn handles_match_the_brute_force_definition() {
    let mut ids: Vec<String> = ["b1", "b12", "b123", "c", "cc"]
        .iter()
        .chain(&PAIR_A)
        .chain(&PAIR_B)
        .chain(&ELEVEN)
        .chain(&[V1ST, LIKE_V1ST])
        .map(|s| (*s).to_owned())
        .collect();
    ids.extend(derived_ids(40));
    ids.extend(uuidv7s(40));
    let table = build_handle_table(ids.iter().map(String::as_str));
    for id in &ids {
        assert_eq!(
            table.handles.get(id).cloned(),
            Some(brute_force_handle(id, &ids)),
            "{id}"
        );
    }
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > ids of any shape get handles of 4 or 5 characters
#[test]
fn ids_of_any_shape_get_handles_of_4_or_5_characters() {
    let shapes = [
        ("counter", counter_ids(1000)),
        ("derived", derived_ids(1000)),
        (
            "short",
            (0..1000)
                .map(|i| format!("b{}", radix(i + 1, 36)))
                .collect(),
        ),
        ("uuidv7", uuidv7s(1000)),
    ];
    for (shape, ids) in shapes {
        let table = build_handle_table(ids.iter().map(String::as_str));
        let lengths: Vec<usize> = table.handles.values().map(String::len).collect();
        assert_eq!(lengths.iter().min(), Some(&4), "{shape}");
        assert!(*lengths.iter().max().unwrap() <= 5, "{shape}");
    }
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > duplicates are counted once
#[test]
fn duplicates_are_counted_once() {
    let table = table_of(&[V1ST, V1ST, XQE]);
    assert_eq!(table.sorted, [handle_key(V1ST), handle_key(XQE)]);
    assert_eq!(table.handles.get(V1ST).map(String::as_str), Some("2bNo"));
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > the handle of an id outside the table, against the table
#[test]
fn the_handle_of_an_id_outside_the_table() {
    let table = table_of(&[V1ST, XQE]);
    assert_eq!(handle_in_table(&table, LIKE_V1ST), "2bNoV");
    assert_eq!(handle_in_table(&table, RK8P), "xt6z");
    assert_eq!(handle_in_table(&table, XQE), "gTtF");
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > idsWithPrefix lists the ids whose hash starts with a prefix
#[test]
fn ids_with_prefix_lists_the_ids() {
    let table = table_of(&[V1ST, LIKE_V1ST, XQE]);
    assert_eq!(ids_with_prefix(&table, "2bNo"), [LIKE_V1ST, V1ST]);
    assert_eq!(ids_with_prefix(&table, "2bNoq"), [V1ST]);
    assert_eq!(ids_with_prefix(&table, "gTtF"), [XQE]);
    assert!(ids_with_prefix(&table, "zzzz").is_empty());
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > gives each block its handle
#[test]
fn get_block_handle_gives_each_block_its_handle() {
    let editor = build(&[V1ST, XQE, RK8P]);
    append_paragraphs(&editor, &["Q3 plan", "Revenue grew 4% in Q3.", "Hiring"]);
    assert_eq!(
        handles_of(&editor),
        map(&[(RK8P, "xt6z"), (V1ST, "2bNo"), (XQE, "gTtF")])
    );
    let mut all = editor.read(get_block_handles).unwrap().unwrap();
    // A Map's toEqual ignores the order of its entries.
    all.sort();
    assert_eq!(all, sorted_entries());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > short ids, and ids that prefix other ids, get hashed handles too
#[test]
fn short_ids_get_hashed_handles_too() {
    let ids = ["b1", "b12", "b123456", "xy"];
    let editor = build(&ids);
    append_paragraphs(&editor, &["a", "b", "c", "d"]);
    let expected = handles_map(&table_of(&ids));
    assert_eq!(handles_of(&editor), expected);
    for id in ids {
        assert_eq!(resolve(&editor, expected[id].as_ref().unwrap()), found(id));
    }
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > null for a node without an id or that is not an identified block
#[test]
fn get_block_handle_null_for_unidentified_nodes() {
    let ids = ["listid", "item1", "item2"];
    let table = table_of(&ids);
    let handle = |id: &str| table.handles.get(id).cloned();
    let editor = build(&ids);
    update(&editor, || {
        let list = create_list_node(ListType::Bullet, 1.0);
        let item1 = create_list_item_node(None);
        item1.append(&[create_text_node("one")])?;
        // Created in JS's order, which the ids are handed out in.
        let item2 = create_list_item_node(None);
        let nested_list = create_list_node(ListType::Bullet, 1.0);
        let nested_item = create_list_item_node(None);
        nested_item.append(&[create_text_node("nested")])?;
        nested_list.append(&[nested_item])?;
        item2.append(&[nested_list])?;
        list.append(&[item1, item2])?;
        get_root().append(&[list])?;
        Ok(())
    });
    let got = editor
        .read(|| {
            let list = get_root().get_first_child_or_throw().unwrap();
            let item = list.get_first_child_or_throw().unwrap();
            let second_item = list.get_last_child_or_throw().unwrap();
            let nested_list = second_item.get_first_child_or_throw().unwrap();
            [
                get_block_handle(&list).unwrap(),
                get_block_handle(&item).unwrap(),
                // A text node, and a nested list (a wrapper, not
                // identified).
                get_block_handle(&item.get_first_child_or_throw().unwrap()).unwrap(),
                get_block_handle(&second_item).unwrap(),
                get_block_handle(&nested_list).unwrap(),
            ]
        })
        .unwrap();
    assert_eq!(
        got,
        [
            handle("listid"),
            handle("item1"),
            None,
            handle("item2"),
            None
        ]
    );
    // Detached.
    let detached = update_with(&editor, || {
        let list = get_root().get_first_child_or_throw()?;
        list.remove(false)?;
        get_block_handle(&list)
    });
    assert_eq!(detached, None);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > null for a block named since the last commit
#[test]
fn get_block_handle_null_for_a_block_named_since_the_last_commit() {
    let editor = build(&[V1ST, XQE]);
    append_paragraphs(&editor, &["a", "b"]);
    let (handle, by_id) = update_with(&editor, || {
        // Its handle would share 2bNo with V1ST's.
        let block = create_paragraph_node();
        set_state(&block, &block_id_state(), Some(LIKE_V1ST.to_owned()))?;
        get_root().append(std::slice::from_ref(&block))?;
        Ok((
            get_block_handle(&block)?,
            get_block_by_id(LIKE_V1ST)?.is_some(),
        ))
    });
    assert_eq!(handle, None);
    assert!(!by_id);
    // Once committed, both have longer handles.
    assert_eq!(
        handles_of(&editor),
        map(&[(LIKE_V1ST, "2bNoV"), (V1ST, "2bNoq"), (XQE, "gTtF")])
    );
    assert_eq!(resolve(&editor, "2bNoV"), found(LIKE_V1ST));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > null for a block of an earlier state whose id is gone
#[test]
fn get_block_handle_null_for_a_block_of_an_earlier_state() {
    let editor = build(&[PAIR_B[0], PAIR_B[1], XQE]);
    append_paragraphs(&editor, &["a", "b", "c"]);
    let earlier = editor.get_editor_state();
    update(&editor, || {
        get_block_by_id(PAIR_B[0])?.unwrap().remove(false)
    });
    let handle = read_editor_state(Some(&editor), &earlier, || {
        get_block_handle(&get_root().get_first_child_or_throw().unwrap()).unwrap()
    });
    assert_eq!(handle, None);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > null for a copy whose repeated id awaits its repair
#[test]
fn get_block_handle_null_for_a_copy_awaiting_repair() {
    let editor = build(&[V1ST, XQE, "cccc12345"]);
    append_paragraphs(&editor, &["a", "b", "c"]);
    // Read in the commit that brings the duplicate, before the repair
    // update that the extension's own listener (registered first)
    // schedules.
    type Seen = (Option<String>, Option<String>, Option<String>, bool);
    let seen: Rc<RefCell<Vec<Seen>>> = Rc::default();
    let (record, weak) = (seen.clone(), editor.weak());
    let unregister = editor.register_update_listener(move |payload| {
        if payload.tags.iter().any(|t| t == COLLABORATION_TAG) {
            let editor = Editor(weak.upgrade().unwrap());
            read_editor_state(Some(&editor), &payload.editor_state, || {
                let children = get_root().get_children();
                let (first, last) = (&children[0], &children[2]);
                let result = resolve_block_handle("2bNo", None).unwrap();
                record.borrow_mut().push((
                    get_block_id(last).unwrap(),
                    get_block_handle(first).unwrap(),
                    get_block_handle(last).unwrap(),
                    matches!(result, BlockHandleResult::Found { node } if node.get_key() == first.get_key()),
                ));
            });
        }
    });
    // A duplicate committed without transforms, as a remote merge is.
    editor
        .update(
            || {
                set_state(
                    &get_root().get_last_child_or_throw()?,
                    &block_id_state(),
                    Some(V1ST.to_owned()),
                )?;
                Ok(())
            },
            UpdateOptions {
                skip_transforms: true,
                ..tagged(&[COLLABORATION_TAG])
            },
        )
        .unwrap();
    unregister();
    assert_eq!(
        *seen.borrow(),
        [(Some(V1ST.to_owned()), Some("2bNo".to_owned()), None, true)]
    );
    // After the repair, the copy has an id and a handle of its own.
    let handles = handles_of(&editor);
    assert_eq!(handles.len(), 3);
    assert_eq!(handles[V1ST].as_deref(), Some("2bNo"));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > handles change only when ids do, and at once
#[test]
fn handles_change_only_when_ids_do() {
    let editor = build(&[V1ST, XQE]);
    append_paragraphs(&editor, &["a", "b"]);
    let first = handles_of(&editor);
    // Typing changes no id: same handles.
    update(&editor, || {
        let text = get_root().get_first_descendant().unwrap();
        assert!(text.is_text());
        text.set_text_content("changed");
        Ok(())
    });
    assert_eq!(handles_of(&editor), first);
    // A new id whose hash shares 2bNo makes the handle longer at once.
    append_paragraphs(&editor, &["c"]);
    update(&editor, || {
        set_state(
            &get_root().get_last_child_or_throw()?,
            &block_id_state(),
            Some(LIKE_V1ST.to_owned()),
        )?;
        Ok(())
    });
    assert_eq!(
        handles_of(&editor),
        map(&[(LIKE_V1ST, "2bNoV"), (V1ST, "2bNoq"), (XQE, "gTtF")])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > finds the block of a handle, or of a whole id
#[test]
fn resolve_finds_the_block_of_a_handle_or_an_id() {
    let editor = build(&[V1ST, XQE, RK8P]);
    append_paragraphs(&editor, &["a", "b", "c"]);
    assert_eq!(resolve(&editor, "gTtF"), found(XQE));
    // A longer prefix of the hash (a handle read while another block's hash
    // shared more of it) still finds the block.
    assert_eq!(resolve(&editor, "gTtFDmo"), found(XQE));
    assert_eq!(resolve(&editor, &handle_key(XQE)), found(XQE));
    // A string longer than any handle is a whole id.
    assert_eq!(resolve(&editor, XQE), found(XQE));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > ids of 11 characters or fewer are found by their handle only
#[test]
fn short_ids_are_found_by_their_handle_only() {
    let ids = ["b1", "b123456", "abcdefghijk", "abcdefghijkl"];
    let editor = build(&ids);
    append_paragraphs(&editor, &["a", "b", "c", "d"]);
    let table = table_of(&ids);
    for id in ids {
        assert_eq!(resolve(&editor, table.handles.get(id).unwrap()), found(id));
    }
    assert_eq!(resolve(&editor, "b1"), invalid("b1", "too-short"));
    assert_eq!(resolve(&editor, "b123456"), not_found("b123456"));
    assert_eq!(resolve(&editor, "abcdefghijk"), not_found("abcdefghijk"));
    // 12 characters: a whole id.
    assert_eq!(resolve(&editor, "abcdefghijkl"), found("abcdefghijkl"));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > a typo is not found; fewer than 4 characters is too short
#[test]
fn resolve_a_typo_is_not_found() {
    let editor = build(&[V1ST, XQE, RK8P]);
    append_paragraphs(&editor, &["a", "b", "c"]);
    assert_eq!(resolve(&editor, "gTtG"), not_found("gTtG"));
    // Case matters.
    assert_eq!(resolve(&editor, "GTTF"), not_found("GTTF"));
    // Never a handle: a dropped character, not a deleted block.
    for short in ["gTt", "gT", "g"] {
        assert_eq!(resolve(&editor, short), invalid(short, "too-short"));
    }
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > invalid input
#[test]
fn resolve_invalid_input() {
    let editor = build(&[V1ST]);
    append_paragraphs(&editor, &["a"]);
    let long = "a".repeat(65);
    for input in [
        "", "2bNo ", " 2bNo", "2bNo!", "2bNo/Q", &long, "2bNo\n", "ß123",
    ] {
        assert_eq!(
            resolve(&editor, input),
            invalid(input, "malformed"),
            "{input:?}"
        );
    }
    // Upstream's untrusted input that is not even a string (null, 42,
    // objects without a usable toString) cannot be passed here: the
    // handle is a `&str`.
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > a deleted block is not found
#[test]
fn resolve_a_deleted_block_is_not_found() {
    let editor = build(&[V1ST, XQE, RK8P]);
    append_paragraphs(&editor, &["a", "b", "c"]);
    update(&editor, || get_block_by_id(RK8P)?.unwrap().remove(false));
    assert_eq!(resolve(&editor, "xt6z"), not_found("xt6z"));
    assert_eq!(resolve(&editor, RK8P), not_found(RK8P));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > a block deleted earlier in the same update is not found
#[test]
fn resolve_a_block_deleted_in_the_same_update_is_not_found() {
    let editor = build(&[V1ST, XQE]);
    append_paragraphs(&editor, &["a", "b"]);
    let (deleted, kept) = update_with(&editor, || {
        get_block_by_id(XQE)?.unwrap().remove(false)?;
        Ok((
            shown(resolve_block_handle("gTtF", None)?),
            shown(resolve_block_handle("2bNo", None)?),
        ))
    });
    assert_eq!(deleted, not_found("gTtF"));
    assert_eq!(kept, found(V1ST));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > a new block whose hash starts the same way makes an old handle ambiguous
#[test]
fn resolve_a_new_block_makes_an_old_handle_ambiguous() {
    let editor = build(&[V1ST, XQE, LIKE_V1ST]);
    append_paragraphs(&editor, &["Q3 plan", "Revenue"]);
    assert_eq!(handles_of(&editor)[V1ST].as_deref(), Some("2bNo"));
    // A collaborator adds a block whose handle would start with 2bNo too.
    append_paragraphs(&editor, &["Q4 plan"]);
    assert_eq!(
        resolve(&editor, "2bNo"),
        ambiguous("2bNo", &["2bNoV", "2bNoq"])
    );
    // The next handles computed are longer, and each resolves.
    assert_eq!(
        handles_of(&editor),
        map(&[(LIKE_V1ST, "2bNoV"), (V1ST, "2bNoq"), (XQE, "gTtF")])
    );
    assert_eq!(resolve(&editor, "2bNoq"), found(V1ST));
    assert_eq!(resolve(&editor, "2bNoV"), found(LIKE_V1ST));
    // Once the new block is deleted, the old handle works again.
    update(&editor, || {
        get_root().get_last_child_or_throw()?.remove(false)
    });
    assert_eq!(resolve(&editor, "2bNo"), found(V1ST));
    assert_eq!(resolve(&editor, "2bNoq"), found(V1ST));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > lists at most 10 candidates
#[test]
fn resolve_lists_at_most_10_candidates() {
    let editor = build(&ELEVEN);
    append_paragraphs(&editor, &ELEVEN);
    let table = table_of(&ELEVEN);
    let result = editor
        .read(|| resolve_block_handle("sclT", None).unwrap())
        .unwrap();
    let BlockHandleResult::Ambiguous { candidates, .. } = result else {
        panic!("ambiguous");
    };
    let expected: Vec<String> = table.sorted[..10]
        .iter()
        .map(|key| {
            table
                .handles
                .get(&table.ids_of_key.get(key).unwrap()[0])
                .unwrap()
                .clone()
        })
        .collect();
    assert_eq!(candidates, expected);
    editor.dispose();
}

/// The handles of V1ST, XQE and RK8P, sorted.
fn sorted_entries() -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = [(RK8P, "xt6z"), (V1ST, "2bNo"), (XQE, "gTtF")]
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .into();
    entries.sort();
    entries
}

/// `readOf(editor)`.
fn read_of(editor: &Editor) -> Vec<(String, String)> {
    editor.read(get_block_handles).unwrap().unwrap()
}

/// `resolveRead(editor, handle, read)`.
fn resolve_read(editor: &Editor, handle: &str, read: &[(String, String)]) -> R {
    let options = BlockHandleOptions {
        read: Some(read.to_vec()),
    };
    editor
        .read(|| shown(resolve_block_handle(handle, Some(&options)).unwrap()))
        .unwrap()
}

fn read_get<'a>(read: &'a [(String, String)], id: &str) -> Option<&'a str> {
    read.iter().find(|(i, _)| i == id).map(|(_, h)| h.as_str())
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > a block deleted since the read is not found, even if a block added since takes its handle
#[test]
fn read_a_block_deleted_since_the_read_is_not_found() {
    let editor = build(&[PAIR_A[0], XQE, PAIR_A[1]]);
    append_paragraphs(&editor, &["a", "b"]);
    let read = read_of(&editor);
    assert_eq!(read_get(&read, PAIR_A[0]), Some("0ydd"));
    update(&editor, || {
        get_block_by_id(PAIR_A[0])?.unwrap().remove(false)
    });
    append_paragraphs(&editor, &["c"]);
    // Without the read, the handle now names the block added since.
    assert_eq!(resolve(&editor, "0ydd"), found(PAIR_A[1]));
    // With it, the block the model read is gone.
    assert_eq!(resolve_read(&editor, "0ydd", &read), not_found("0ydd"));
    assert_eq!(resolve_read(&editor, "gTtF", &read), found(XQE));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > a block added since cannot make the handle ambiguous
#[test]
fn read_a_block_added_since_cannot_make_the_handle_ambiguous() {
    let editor = build(&[V1ST, XQE, LIKE_V1ST]);
    append_paragraphs(&editor, &["a", "b"]);
    let read = read_of(&editor);
    append_paragraphs(&editor, &["c"]);
    assert!(matches!(resolve(&editor, "2bNo"), R::Ambiguous(..)));
    assert_eq!(resolve_read(&editor, "2bNo", &read), found(V1ST));
    // The whole id of a block the model did not read is not one of its
    // blocks either.
    update(&editor, || {
        let node = create_paragraph_node();
        set_state(
            &node,
            &block_id_state(),
            Some("a-whole-id-not-read".to_owned()),
        )?;
        get_root().append(&[node])?;
        Ok(())
    });
    assert_eq!(
        resolve_read(&editor, "a-whole-id-not-read", &read),
        not_found("a-whole-id-not-read")
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > typos fail against the ids of the read
#[test]
fn read_typos_fail_against_the_ids_of_the_read() {
    let editor = build(&[V1ST, LIKE_V1ST, XQE]);
    append_paragraphs(&editor, &["a", "b", "c"]);
    let read = read_of(&editor);
    assert_eq!(resolve_read(&editor, "gTtG", &read), not_found("gTtG"));
    // Shorter than the handles shown: the read's handles as candidates.
    assert_eq!(
        resolve_read(&editor, "2bNo", &read),
        ambiguous("2bNo", &["2bNoV", "2bNoq"])
    );
    assert_eq!(
        resolve_read(&editor, "gTt", &read),
        invalid("gTt", "too-short")
    );
    assert_eq!(
        resolve_read(&editor, "gTtF!", &read),
        invalid("gTtF!", "malformed")
    );
    // A longer prefix of the hash, and the whole id, work.
    assert_eq!(resolve_read(&editor, "gTtFDm", &read), found(XQE));
    assert_eq!(resolve_read(&editor, XQE, &read), found(XQE));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > a map changed between calls is read as it is now
#[test]
fn read_a_map_changed_between_calls_is_read_as_it_is_now() {
    let editor = build(&[PAIR_A[0], "wxyz2222", PAIR_A[1]]);
    append_paragraphs(&editor, &["a", "b"]);
    let mut shown = read_of(&editor);
    assert_eq!(resolve_read(&editor, "0ydd", &shown), found(PAIR_A[0]));
    // The same map, refilled from a later read. (The options take the
    // read's entries, so each call reads them as they are at the call.)
    append_paragraphs(&editor, &["c"]);
    shown.clear();
    shown.extend(read_of(&editor));
    let fresh = shown.clone();
    for read in [&shown, &fresh] {
        assert_eq!(
            resolve_read(&editor, "0ydd", read),
            ambiguous("0ydd", &["0yddA", "0yddC"])
        );
    }
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > works with a read of another editor state of the same document
#[test]
fn read_works_with_another_editor_state_of_the_same_document() {
    let editor = build(&[V1ST, XQE]);
    append_paragraphs(&editor, &["a", "b"]);
    let json = editor.get_editor_state().to_json(false);
    let read = read_of(&editor);
    let other = build(&[]);
    other
        .set_editor_state(&other.parse_editor_state_json(&json).unwrap(), None)
        .unwrap();
    assert_eq!(resolve_read(&other, "gTtF", &read), found(XQE));
    other.dispose();
    editor.dispose();
}

/// `typo safety and handle length for ids of any shape`.
fn typo_safety(ids: Vec<String>) {
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let editor = load(&refs, false);
    let (lengths, tried, hits) = measure(&editor);
    assert_eq!(lengths.len(), ids.len());
    assert_eq!(lengths.iter().min(), Some(&4));
    assert!(*lengths.iter().max().unwrap() <= 5);
    // About 400 of 64^4 four-character strings are handles.
    assert!((hits as f64) / (tried as f64) < 1e-3, "{hits}/{tried}");
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::typo safety and handle length for ids of any shape > UUIDv7 ids
#[test]
fn typo_safety_uuidv7_ids() {
    typo_safety(uuidv7s(400));
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::typo safety and handle length for ids of any shape > derived-like ids
#[test]
fn typo_safety_derived_like_ids() {
    typo_safety(derived_ids(400));
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::typo safety and handle length for ids of any shape > counter ids
#[test]
fn typo_safety_counter_ids() {
    typo_safety(counter_ids(400));
}

fn set_last_id(id: &'static str) {
    set_state(
        &get_root().get_last_child_or_throw().unwrap(),
        &block_id_state(),
        Some(id.to_owned()),
    )
    .unwrap();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::without the index > a disabled extension falls back to a scan
#[test]
fn disabled_falls_back_to_a_scan() {
    let editor = build(&[V1ST, XQE]);
    append_paragraphs(&editor, &["a", "b"]);
    let before = handles_of(&editor);
    let disabled = editor_of(
        "[block-handle-disabled]",
        vec![
            crate::rich_text::rich_text_extension().into(),
            block_ids(true, None),
        ],
    );
    disabled
        .set_editor_state(
            &disabled
                .parse_editor_state_json(&editor.get_editor_state().to_json(false))
                .unwrap(),
            None,
        )
        .unwrap();
    assert_eq!(handles_of(&disabled), before);
    assert_eq!(resolve(&disabled, "gTtF"), found(XQE));
    assert_eq!(resolve(&disabled, "2bNo"), found(V1ST));
    assert_eq!(resolve(&disabled, "zzzz"), not_found("zzzz"));
    // A block added while disabled gets no id, and no handle.
    update(&disabled, || {
        get_root().append(&[create_paragraph_node()])?;
        Ok(())
    });
    assert_eq!(
        disabled
            .read(|| get_block_handle(&get_root().get_last_child_or_throw().unwrap()).unwrap())
            .unwrap(),
        None
    );
    // Its id set by hand is seen at once by the scan.
    update(&disabled, || {
        set_last_id(LIKE_V1ST);
        Ok(())
    });
    assert_eq!(
        handles_of(&disabled),
        map(&[(LIKE_V1ST, "2bNoV"), (V1ST, "2bNoq"), (XQE, "gTtF")])
    );
    // And an id changed in a nested position, or a block removed.
    update(&disabled, || {
        get_block_by_id(LIKE_V1ST)?.unwrap().remove(false)
    });
    assert_eq!(handles_of(&disabled), map(&[(V1ST, "2bNo"), (XQE, "gTtF")]));
    disabled.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::without the index > disabling and enabling at runtime
#[test]
fn disabling_and_enabling_at_runtime() {
    let editor = build(&[V1ST, XQE, LIKE_V1ST]);
    append_paragraphs(&editor, &["a", "b"]);
    let disabled = get_extension_dependency_from_editor(&editor, &block_id_extension())
        .unwrap()
        .output
        .unwrap()
        .disabled
        .clone();
    disabled.set_value(true);
    assert_eq!(handles_of(&editor), map(&[(V1ST, "2bNo"), (XQE, "gTtF")]));
    update(&editor, || {
        get_root().append(&[create_paragraph_node()])?;
        set_last_id(LIKE_V1ST);
        Ok(())
    });
    // The scan sees the id set by hand while disabled.
    assert_eq!(
        resolve(&editor, "2bNo"),
        ambiguous("2bNo", &["2bNoV", "2bNoq"])
    );
    disabled.set_value(false);
    // Indexed again, with the same handles.
    assert_eq!(
        handles_of(&editor),
        map(&[(LIKE_V1ST, "2bNoV"), (V1ST, "2bNoq"), (XQE, "gTtF")])
    );
    assert_eq!(resolve(&editor, "2bNoV"), found(LIKE_V1ST));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::without the index > while disabled, a repeated id names its first live holder
#[test]
fn disabled_a_repeated_id_names_its_first_live_holder() {
    const ID: &str = "duplicate-id-0001";
    let editor = load(&[ID, XQE, ID], true);
    let handle = read_get(&read_of(&editor), ID).unwrap().to_owned();
    assert_eq!(resolve(&editor, &handle), found(ID));
    let holders = |editor: &Editor| -> Vec<String> {
        editor
            .read(|| {
                get_root()
                    .get_children()
                    .iter()
                    .filter(|node| get_block_id(node).unwrap().as_deref() == Some(ID))
                    .map(|node| node.get_key().to_owned())
                    .collect()
            })
            .unwrap()
    };
    let [first, second] = <[String; 2]>::try_from(holders(&editor)).unwrap();
    fn key_of(input: &str) -> Option<String> {
        match resolve_block_handle(input, None).unwrap() {
            BlockHandleResult::Found { node } => Some(node.get_key().to_owned()),
            _ => None,
        }
    }
    assert_eq!(editor.read(|| key_of(&handle)).unwrap(), Some(first));
    // The first holder removed: the second one now.
    let h = handle.clone();
    let inside = update_with(&editor, move || {
        get_root().get_first_child_or_throw()?.remove(false)?;
        Ok(key_of(&h))
    });
    assert_eq!(inside, Some(second.clone()));
    assert_eq!(editor.read(|| key_of(&handle)).unwrap(), Some(second));
    assert_eq!(
        editor
            .read(|| get_block_handle(&get_root().get_last_child_or_throw().unwrap()).unwrap())
            .unwrap(),
        Some(handle.clone())
    );
    // Two holders reordered in an update: the one first now.
    let reordered = load(&[ID, XQE, ID], true);
    let [a, b] = <[String; 2]>::try_from(holders(&reordered)).unwrap();
    let h = handle.clone();
    let (resolved, by_id) = update_with(&reordered, move || {
        let root = get_root();
        root.get_first_child_or_throw()?
            .insert_before(&root.get_last_child_or_throw()?, true)?;
        Ok((
            key_of(&h),
            get_block_by_id(ID)?.map(|n| n.get_key().to_owned()),
        ))
    });
    assert_eq!(resolved, Some(b.clone()));
    assert_eq!(by_id, Some(b.clone()));
    assert_ne!(a, b);
    reordered.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::without the index > enabled or disabled, handles come from the last committed state
#[test]
fn handles_come_from_the_last_committed_state() {
    for disabled in [false, true] {
        let editor = load(&[V1ST, XQE], disabled);
        let earlier = editor.get_editor_state();
        assert_eq!(handles_of(&editor), map(&[(V1ST, "2bNo"), (XQE, "gTtF")]));
        let (handle, all) = update_with(&editor, || {
            let node = create_paragraph_node();
            set_state(&node, &block_id_state(), Some(LIKE_V1ST.to_owned()))?;
            get_root().append(std::slice::from_ref(&node))?;
            // Not committed yet: no handle for the new block, the old ones.
            Ok((get_block_handle(&node)?, get_block_handles()?))
        });
        assert_eq!(handle, None, "disabled: {disabled}");
        let all: Handles = all.into_iter().map(|(i, h)| (i, Some(h))).collect();
        assert_eq!(all, map(&[(V1ST, "2bNo"), (XQE, "gTtF")]));
        assert_eq!(handles_of(&editor)[V1ST].as_deref(), Some("2bNoq"));
        // An earlier state, read: its blocks that the committed state still
        // has, with the committed state's handles.
        let earlier_handles: Handles = read_editor_state(Some(&editor), &earlier, || {
            get_block_handles()
                .unwrap()
                .into_iter()
                .map(|(i, h)| (i, Some(h)))
                .collect()
        });
        assert_eq!(earlier_handles, map(&[(V1ST, "2bNoq"), (XQE, "gTtF")]));
        // An id changed below the root (the root itself is not cloned).
        update(&editor, || {
            let node = match get_block_by_id(XQE)? {
                Some(node) => node,
                None => get_root().get_child_at_index(1).unwrap(),
            };
            set_state(&node, &block_id_state(), Some(LIKE_XQE.to_owned()))?;
            Ok(())
        });
        assert_eq!(
            handles_of(&editor),
            map(&[(LIKE_V1ST, "2bNoV"), (LIKE_XQE, "gTtF"), (V1ST, "2bNoq")]),
            "disabled: {disabled}"
        );
        editor.dispose();
    }
}

/// `stored`: three paragraphs with ids, as JSON.
fn stored() -> Value {
    let children: Vec<String> = [V1ST, XQE, RK8P]
        .iter()
        .enumerate()
        .map(|(i, id)| {
            format!(
                r#"{{"$":{{"blockId":"{id}"}},"children":[{{"detail":0,"format":0,"mode":"normal","style":"","text":"Block {i}","type":"text","version":1}}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}}"#
            )
        })
        .collect();
    crate::json::parse(&format!(
        r#"{{"root":{{"children":[{}],"direction":null,"format":"","indent":0,"type":"root","version":1}}}}"#,
        children.join(",")
    ))
    .unwrap()
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::headless and JSON-only use > a headless editor reads handles and applies an edit by handle
#[test]
fn a_headless_editor_reads_handles_and_edits_by_handle() {
    let editor = editor_of(
        "[block-handle-headless]",
        vec![
            crate::headless::headless_extension().into(),
            crate::rich_text::rich_text_extension().into(),
            block_id_extension().into(),
        ],
    );
    editor
        .set_editor_state(&editor.parse_editor_state_json(&stored()).unwrap(), None)
        .unwrap();
    assert_eq!(
        handles_of(&editor),
        map(&[(RK8P, "xt6z"), (V1ST, "2bNo"), (XQE, "gTtF")])
    );
    update(&editor, || {
        let BlockHandleResult::Found { node } = resolve_block_handle("gTtF", None)? else {
            panic!("found");
        };
        assert!(node.is_element());
        let text = node.get_first_descendant().unwrap();
        assert!(text.is_text());
        text.set_text_content("Revenue grew 6%.");
        Ok(())
    });
    let json = editor.get_editor_state().to_json(false);
    let second = &json
        .get("root")
        .unwrap()
        .get("children")
        .unwrap()
        .as_array()
        .unwrap()[1];
    assert_eq!(
        second.get("$").unwrap().get("blockId").unwrap().as_str(),
        Some(XQE)
    );
    assert_eq!(
        second.get("children").unwrap().as_array().unwrap()[0]
            .get("text")
            .unwrap()
            .as_str(),
        Some("Revenue grew 6%.")
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::headless and JSON-only use > reading handles from a parsed state, without a DOM
#[test]
fn reading_handles_from_a_parsed_state() {
    let editor = editor_of(
        "[block-handle-json]",
        vec![
            crate::rich_text::rich_text_extension().into(),
            block_id_extension().into(),
        ],
    );
    let state = editor.parse_editor_state(&stored().to_string()).unwrap();
    editor.set_editor_state(&state, None).unwrap();
    let mut handles = read_editor_state(Some(&editor), &editor.get_editor_state(), || {
        get_block_handles().unwrap()
    });
    // A Map's toEqual ignores the order of its entries.
    handles.sort();
    assert_eq!(handles, sorted_entries());
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::headless and JSON-only use > a document stored without ids gets handles of its positional ids
#[test]
fn a_document_without_ids_gets_handles_of_its_positional_ids() {
    let editor = editor_of(
        "[block-handle-positional]",
        vec![
            crate::rich_text::rich_text_extension().into(),
            block_id_extension().into(),
        ],
    );
    let mut without_ids = stored();
    if let Value::Object(json) = &mut without_ids
        && let Some(Value::Object(root)) = json.get_mut("root")
        && let Some(Value::Array(children)) = root.get_mut("children")
    {
        for child in children {
            if let Value::Object(child) = child {
                child.remove("$");
            }
        }
    }
    editor
        .set_editor_state(&editor.parse_editor_state_json(&without_ids).unwrap(), None)
        .unwrap();
    let handles = handles_of(&editor);
    assert_eq!(handles.len(), 3);
    for (id, handle) in handles {
        let handle = handle.unwrap();
        assert_eq!(id.len(), 21);
        assert!(handle.len() >= 4);
        assert!(handle_key(&id).starts_with(&handle));
        assert_eq!(resolve(&editor, &handle), found(&id));
    }
    editor.dispose();
}
