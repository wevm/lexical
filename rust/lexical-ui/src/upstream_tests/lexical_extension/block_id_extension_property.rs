//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts`.
//!
//! Property test of BlockIdExtension over random local edit sequences:
//! typing, Enter, Backspace and forward-delete joins, range deletes, block
//! type changes, moves, duplicates, pastes of several copies, indent and
//! outdent, list conversion, moves into table cells, replace, undo and redo,
//! and reloads of the state with an injected duplicate id. After every step
//! each identified block must have exactly one id, the ids must be unique,
//! `$getBlockById` must find every identified block, blocks that existed
//! before the step must keep their ids (where the step does not restructure
//! them), and undo and redo must return to a state seen before. The seeds
//! are upstream's twenty, drawn from the same generator (mulberry32).

#![cfg(all(
    feature = "rich-text",
    feature = "list",
    feature = "history",
    feature = "table"
))]

use super::support::{counter, tagged};
use crate::extension::{
    BlockIdConfigOverrides, LexicalEditorWithDispose, block_id_extension,
    build_editor_from_extensions, get_block_by_id, get_block_id,
    get_extension_dependency_from_editor,
};
use crate::history::{HistoryConfigOverrides, HistoryNow, history_extension};
use crate::json::Value;
use crate::list::{
    INSERT_UNORDERED_LIST_COMMAND, ListType, create_list_item_node, create_list_node,
    list_extension,
};
use crate::rich_text::{
    HeadingTagType, create_heading_node, create_quote_node, rich_text_extension,
};
use crate::selection::set_blocks_type;
use crate::table::{
    InsertTableCommandPayloadHeaders, create_table_node_with_dimensions, is_table_cell_node,
    table_extension,
};
use crate::upstream_tests::support::mount_in_jsdom;
use crate::{
    DELETE_CHARACTER_COMMAND, Editor, HISTORY_MERGE_TAG, HISTORY_PUSH_TAG, INDENT_CONTENT_COMMAND,
    INSERT_PARAGRAPH_COMMAND, InitialEditorStateType, LexicalError, LexicalExtension, NodeRef,
    OUTDENT_CONTENT_COMMAND, PointType, REDO_COMMAND, UNDO_COMMAND, UpdateOptions, Which,
    config_extension, create_paragraph_node, create_text_node, define_extension, get_root,
    get_selection, get_slot, get_slot_names, parse_serialized_node, set_selection,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

const STEPS: usize = 40;

/// `build()`.
fn build() -> LexicalEditorWithDispose {
    let clock = Rc::new(Cell::new(0.0));
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            rich_text_extension().into(),
            list_extension().into(),
            table_extension().into(),
            config_extension(
                &history_extension(),
                [HistoryConfigOverrides {
                    disabled: Some(false),
                    now: Some(HistoryNow(Rc::new(move || {
                        clock.set(clock.get() + 10000.0);
                        clock.get()
                    }))),
                    ..Default::default()
                }],
            ),
            config_extension(
                &block_id_extension(),
                [BlockIdConfigOverrides {
                    generate_id: Some(counter("b")),
                    ..Default::default()
                }],
            ),
        ],
        ..LexicalExtension::<(), (), ()>::new("[block-id-property]", ())
    })
    .into()])
    .unwrap();
    // Deleting characters reads the DOM selection.
    mount_in_jsdom(&editor);
    // Have had content once, so that new blocks are edits, not a load.
    let options = || tagged(&[HISTORY_MERGE_TAG]);
    editor
        .update(|| get_root().append(&[create_paragraph_node()]), options())
        .unwrap();
    editor.update(|| get_root().clear(), options()).unwrap();
    editor
}

/// `$p(text)`.
fn p(text: &str) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)]).unwrap();
    paragraph
}

/// `$deepCopy(node)`.
fn deep_copy(node: &NodeRef) -> Result<NodeRef, LexicalError> {
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
    parse_serialized_node(&serialize(node))
}

/// `$all()`: every node below the root in document order (slots before
/// children).
fn all() -> Vec<NodeRef> {
    fn walk(n: &NodeRef, out: &mut Vec<NodeRef>) {
        out.push(n.clone());
        for name in get_slot_names(n) {
            if let Some(slot) = get_slot(n, &name) {
                walk(&slot, out);
            }
        }
        if n.is_element() {
            for child in n.get_children() {
                walk(&child, out);
            }
        }
    }
    let mut out = Vec::new();
    for child in get_root().get_children() {
        walk(&child, &mut out);
    }
    out
}

/// `Row`.
#[derive(Clone, Debug)]
struct Row {
    key: String,
    node_type: String,
    text: String,
    id: Option<String>,
    identified: bool,
}

/// `rows(editor)`.
fn rows(editor: &Editor) -> Vec<Row> {
    let is_identified = get_extension_dependency_from_editor(editor, &block_id_extension())
        .unwrap()
        .output
        .unwrap()
        .is_identified_block
        .clone();
    editor
        .read(|| {
            all()
                .into_iter()
                .filter(|n| !n.is_text())
                .map(|n| Row {
                    id: get_block_id(&n).unwrap(),
                    identified: (is_identified.0)(&n),
                    key: n.get_key().to_owned(),
                    text: n.get_text_content(),
                    node_type: n.get_type().to_owned(),
                })
                .collect()
        })
        .unwrap()
}

/// `outline(editor)`: `type:text:id` of the identified blocks in document
/// order.
fn outline(editor: &Editor) -> Vec<String> {
    rows(editor)
        .into_iter()
        .filter(|r| r.identified)
        .map(|r| {
            format!(
                "{}:{}:{}",
                r.node_type,
                r.text,
                r.id.as_deref().unwrap_or("null")
            )
        })
        .collect()
}

/// `invariantProblems(editor)`: missing ids, repeated ids, and blocks the
/// index does not find.
fn invariant_problems(editor: &Editor) -> Vec<String> {
    let mut problems = Vec::new();
    let identified: Vec<Row> = rows(editor).into_iter().filter(|r| r.identified).collect();
    let mut seen: HashMap<String, String> = HashMap::new();
    for r in &identified {
        let Some(id) = &r.id else {
            problems.push(format!("no id: {}:{}", r.node_type, r.text));
            continue;
        };
        if let Some(prev) = seen.get(id) {
            problems.push(format!("dup {id}: {prev} / {}:{}", r.node_type, r.text));
        }
        seen.insert(id.clone(), format!("{}:{}", r.node_type, r.text));
    }
    editor
        .read(|| {
            for r in &identified {
                if let Some(id) = &r.id {
                    let found = get_block_by_id(id).unwrap();
                    if found.as_ref().is_none_or(|f| f.get_key() != r.key) {
                        problems.push(format!(
                            "index {id} -> {:?} != {}",
                            found.map(|f| f.get_key().to_owned()),
                            r.key
                        ));
                    }
                }
            }
        })
        .unwrap();
    problems
}

/// `mulberry32(seed)`.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }

    /// `Math.floor(rnd() * n)`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() * n as f64).floor() as usize
    }
}

type Rnd = Rc<RefCell<Rng>>;

fn below(rnd: &Rnd, n: usize) -> usize {
    rnd.borrow_mut().below(n)
}

fn coin(rnd: &Rnd) -> bool {
    rnd.borrow_mut().next() < 0.5
}

/// `pick(xs)`.
fn pick<T: Clone>(rnd: &Rnd, xs: &[T]) -> T {
    xs[below(rnd, xs.len())].clone()
}

/// `$textBlocks()`.
fn text_blocks() -> Vec<NodeRef> {
    all()
        .into_iter()
        .filter(|n| matches!(n.get_type(), "paragraph" | "heading" | "quote" | "listitem"))
        .collect()
}

/// `$caretIn(block, offset)`.
fn caret_in(block: &NodeRef, offset: usize) -> Result<(), LexicalError> {
    let mut rest = offset as isize;
    for text in block.get_all_text_nodes() {
        let size = text.get_text_content_size() as isize;
        if text.get_parent().is_some() && rest <= size {
            text.select(Some(rest as usize), Some(rest as usize))?;
            return Ok(());
        }
        rest -= size;
    }
    block.select_start()?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    Type,
    Enter,
    Join,
    FwdJoin,
    RangeDelete,
    SetType,
    Move,
    Dup,
    PasteMany,
    Indent,
    Outdent,
    ToList,
    IntoCell,
    Replace,
    Undo,
    Redo,
    SetStateDup,
}

/// `OPS`.
const OPS: [Op; 18] = [
    Op::Type,
    Op::Enter,
    Op::Enter,
    Op::Join,
    Op::FwdJoin,
    Op::RangeDelete,
    Op::SetType,
    Op::Move,
    Op::Dup,
    Op::PasteMany,
    Op::Indent,
    Op::Outdent,
    Op::ToList,
    Op::IntoCell,
    Op::Replace,
    Op::Undo,
    Op::Redo,
    Op::SetStateDup,
];

/// `Kept`: which pre-existing blocks a step must leave on their ids.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kept {
    All,
    None,
    Enter,
}

fn range_selection() -> crate::SelectionRef {
    let selection = get_selection().filter(crate::SelectionRef::is_range);
    selection.expect("a range selection")
}

/// `$edit(editor, op, rnd, step)`.
fn edit(editor: &Editor, op: Op, rnd: &Rnd, step: usize) -> Result<Kept, LexicalError> {
    let blocks = text_blocks();
    if blocks.is_empty() {
        get_root().append(&[p("x")])?;
        return Ok(Kept::All);
    }
    let b = pick(rnd, &blocks);
    let off = below(rnd, b.get_text_content_size() + 1);
    Ok(match op {
        Op::Type => {
            caret_in(&b, off)?;
            range_selection().insert_text("z")?;
            Kept::All
        }
        Op::Enter => {
            caret_in(&b, off)?;
            editor.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
            Kept::Enter
        }
        Op::Join => {
            b.select_start()?;
            editor.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
            Kept::None
        }
        Op::FwdJoin => {
            b.select_end()?;
            editor.dispatch_command(&DELETE_CHARACTER_COMMAND, false)?;
            Kept::None
        }
        Op::RangeDelete => {
            let c = pick(rnd, &blocks);
            let (x, y) = if b.is_before(&c) { (b, c) } else { (c, b) };
            let xt = x.get_all_text_nodes();
            let yt = y.get_all_text_nodes();
            if !xt.is_empty() && !yt.is_empty() && x != y {
                let start = xt[0].get_text_content_size().min(1);
                xt[0].select(Some(start), Some(start))?;
                let selection = range_selection();
                let last = yt.last().unwrap();
                selection.point_set(
                    Which::Focus,
                    last.get_key(),
                    last.get_text_content_size().min(1),
                    PointType::Text,
                )?;
                selection.remove_text()?;
            }
            Kept::None
        }
        Op::SetType => {
            caret_in(&b, off)?;
            let rnd = rnd.clone();
            set_blocks_type(
                get_selection().as_ref(),
                move || match below(&rnd, 3) {
                    0 => create_heading_node(HeadingTagType::H2),
                    1 => create_quote_node(false),
                    _ => create_paragraph_node(),
                },
                None,
            )?;
            Kept::None
        }
        Op::Move => {
            set_selection(None);
            let kids = get_root().get_children();
            let moved = pick(rnd, &kids);
            let target = pick(rnd, &kids);
            if moved != target {
                if coin(rnd) {
                    target.insert_before(&moved, true)?;
                } else {
                    target.insert_after(&moved, true)?;
                }
            }
            Kept::All
        }
        Op::Dup => {
            set_selection(None);
            let copy = deep_copy(&b)?;
            if coin(rnd) {
                b.insert_before(&copy, true)?;
            } else {
                b.insert_after(&copy, true)?;
            }
            Kept::All
        }
        Op::PasteMany => {
            set_selection(None);
            let src = pick(rnd, &get_root().get_children());
            let n = 1 + below(rnd, 3);
            for _ in 0..n {
                pick(rnd, &get_root().get_children()).insert_after(&deep_copy(&src)?, true)?;
            }
            Kept::All
        }
        Op::Indent => {
            caret_in(&b, off)?;
            editor.dispatch_command(&INDENT_CONTENT_COMMAND, ())?;
            Kept::None
        }
        Op::Outdent => {
            caret_in(&b, off)?;
            editor.dispatch_command(&OUTDENT_CONTENT_COMMAND, ())?;
            Kept::None
        }
        Op::ToList => {
            caret_in(&b, off)?;
            editor.dispatch_command(&INSERT_UNORDERED_LIST_COMMAND, ())?;
            Kept::None
        }
        Op::IntoCell => {
            set_selection(None);
            let cells: Vec<NodeRef> = all().into_iter().filter(is_table_cell_node).collect();
            if !cells.is_empty() && b.get_type() != "listitem" {
                let cell = pick(rnd, &cells);
                if !b.is_parent_of(&cell) {
                    cell.append(&[b])?;
                }
            }
            Kept::All
        }
        Op::Replace => {
            set_selection(None);
            if b.get_type() != "listitem" {
                b.replace(&p(&format!("r{step}")), false)?;
            }
            Kept::None
        }
        Op::Undo | Op::Redo | Op::SetStateDup => panic!("not an edit: {op:?}"),
    })
}

/// `reloadWithDuplicate(editor, rnd)`: reloads the state with one root
/// child's id copied onto another.
fn reload_with_duplicate(editor: &Editor, rnd: &Rnd) {
    let mut json = editor.get_editor_state().to_json(false);
    let Value::Object(state) = &mut json else {
        panic!("an object")
    };
    let Some(Value::Object(root)) = state.get_mut("root") else {
        panic!("a root")
    };
    let Some(Value::Array(kids)) = root.get_mut("children") else {
        panic!("children")
    };
    if kids.len() < 2 {
        return;
    }
    let i = below(rnd, kids.len());
    let mut j = below(rnd, kids.len());
    if j == i {
        j = (j + 1) % kids.len();
    }
    let src = kids[i]
        .get("$")
        .and_then(|s| s.get("blockId"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let Some(src) = src else {
        return;
    };
    let Value::Object(kid) = &mut kids[j] else {
        panic!("an object")
    };
    let mut node_state = match kid.get("$") {
        Some(Value::Object(o)) => o.clone(),
        _ => crate::json::Object::new(),
    };
    node_state.insert("blockId", Value::from(src.as_str()));
    kid.insert("$", Value::Object(node_state));
    editor
        .set_editor_state(&editor.parse_editor_state_json(&json).unwrap(), None)
        .unwrap();
    let root_ids: Vec<Option<String>> = editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|n| get_block_id(n).unwrap())
                .collect()
        })
        .unwrap();
    // The earlier holder keeps the id.
    assert_eq!(root_ids[i.min(j)].as_deref(), Some(src.as_str()));
}

/// `expectKept(before, afterRows, kept)`: pre-existing identified blocks
/// keep their ids, as `kept` requires.
fn expect_kept(before: &[Row], after_rows: &[Row], kept: Kept) {
    if kept == Kept::None {
        return;
    }
    let after: HashMap<&str, &Row> = after_rows.iter().map(|r| (r.key.as_str(), r)).collect();
    for r in before {
        let Some(a) = after.get(r.key.as_str()) else {
            continue;
        };
        if !r.identified || !a.identified || r.id.is_none() {
            continue;
        }
        if kept == Kept::Enter && a.id != r.id {
            // Enter at the start moves the id with the text to the new block.
            let moved = after_rows.iter().find(|x| x.id == r.id);
            if let Some(moved) = moved
                && !before.iter().any(|x| x.key == moved.key)
                && moved.text == r.text
                && a.text.is_empty()
            {
                continue;
            }
        }
        assert_eq!(a.id, r.id, "{}:{} keeps its id", r.node_type, r.text);
    }
    if kept == Kept::All {
        // No id moves onto a different pre-existing block.
        let key_of_id: HashMap<&str, &str> = before
            .iter()
            .filter(|r| r.identified)
            .filter_map(|r| r.id.as_deref().map(|id| (id, r.key.as_str())))
            .collect();
        for a in after.values() {
            let k = if a.identified {
                a.id.as_deref().and_then(|id| key_of_id.get(id))
            } else {
                None
            };
            if let Some(k) = k
                && after.get(k).is_some_and(|r| r.identified)
            {
                assert_eq!(a.key, *k, "{:?} stays on {k}", a.id);
            }
        }
    }
}

fn state_string(editor: &Editor) -> String {
    editor.get_editor_state().to_json(false).to_string()
}

/// One seed of `BlockIdExtension: property test over random edits`.
fn run_seed(seed: u32) {
    let rnd: Rnd = Rc::new(RefCell::new(Rng(seed)));
    let editor = build();
    editor
        .update(
            || {
                let heading = create_heading_node(HeadingTagType::H1);
                heading.append(&[create_text_node("beta")])?;
                let list = create_list_node(ListType::Bullet, 1.0);
                for text in ["li1", "li2"] {
                    let item = create_list_item_node(None);
                    item.append(&[create_text_node(text)])?;
                    list.append(&[item])?;
                }
                get_root().append(&[
                    p("alpha"),
                    heading,
                    list,
                    create_table_node_with_dimensions(
                        2,
                        2,
                        InsertTableCommandPayloadHeaders::Both(false),
                    )?,
                    p("gamma"),
                ])
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    // Outlines on the undo and redo stacks, and every outline seen.
    let mut undo_stack: Vec<Vec<String>> = vec![outline(&editor)];
    let mut redo_stack: Vec<Vec<String>> = Vec::new();
    let mut seen: HashSet<String> = HashSet::from([outline(&editor).join("|")]);
    for step in 0..STEPS {
        let before = rows(&editor);
        let op = OPS[below(&rnd, OPS.len())];
        let mut kept = Kept::None;
        match op {
            Op::Undo => {
                if undo_stack.len() > 1 {
                    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
                    redo_stack.push(undo_stack.pop().unwrap());
                    assert!(seen.contains(&outline(&editor).join("|")), "undo at {step}");
                }
            }
            Op::Redo => {
                if let Some(outline_after) = redo_stack.pop() {
                    editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
                    undo_stack.push(outline_after);
                    assert!(seen.contains(&outline(&editor).join("|")), "redo at {step}");
                }
            }
            Op::SetStateDup => {
                reload_with_duplicate(&editor, &rnd);
                undo_stack = vec![outline(&editor)];
                redo_stack.clear();
                seen.insert(outline(&editor).join("|"));
            }
            _ => {
                let before_json = state_string(&editor);
                let result = Rc::new(Cell::new(Kept::None));
                let (e, r, out) = ((*editor).clone(), rnd.clone(), result.clone());
                editor
                    .update(
                        move || {
                            out.set(edit(&e, op, &r, step)?);
                            Ok(())
                        },
                        tagged(&[HISTORY_PUSH_TAG]),
                    )
                    .unwrap();
                kept = result.get();
                if state_string(&editor) != before_json {
                    undo_stack.push(outline(&editor));
                    redo_stack.clear();
                }
                seen.insert(outline(&editor).join("|"));
            }
        }
        assert_eq!(
            invariant_problems(&editor),
            Vec::<String>::new(),
            "seed {seed} step {step} {op:?}"
        );
        expect_kept(&before, &rows(&editor), kept);
    }
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 1
#[test]
fn seed_1() {
    run_seed(1);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 2
#[test]
fn seed_2() {
    run_seed(2);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 3
#[test]
fn seed_3() {
    run_seed(3);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 4
#[test]
fn seed_4() {
    run_seed(4);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 5
#[test]
fn seed_5() {
    run_seed(5);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 6
#[test]
fn seed_6() {
    run_seed(6);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 7
#[test]
fn seed_7() {
    run_seed(7);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 8
#[test]
fn seed_8() {
    run_seed(8);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 9
#[test]
fn seed_9() {
    run_seed(9);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 10
#[test]
fn seed_10() {
    run_seed(10);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 11
#[test]
fn seed_11() {
    run_seed(11);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 12
#[test]
fn seed_12() {
    run_seed(12);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 13
#[test]
fn seed_13() {
    run_seed(13);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 14
#[test]
fn seed_14() {
    run_seed(14);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 15
#[test]
fn seed_15() {
    run_seed(15);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 16
#[test]
fn seed_16() {
    run_seed(16);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 17
#[test]
fn seed_17() {
    run_seed(17);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 18
#[test]
fn seed_18() {
    run_seed(18);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 19
#[test]
fn seed_19() {
    run_seed(19);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 20
#[test]
fn seed_20() {
    run_seed(20);
}
