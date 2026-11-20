//! Ports of `packages/lexical-extension/src/__tests__/unit/NodeStructureProperty.test.ts`.
//!
//! Property test of NodeStructureExtension over random documents that break
//! every declared rule, nested: blocks in paragraphs, headings, quotes, code,
//! list items and links; paragraphs in quotes, list items and lists; inline
//! content and stray blocks in lists; equal adjacent lists and links; block
//! decorators anywhere in ordered list items; tables, shadow root quotes and
//! the character limit's OverflowNode; empty links, marks and OverflowNodes,
//! which Lexical removes; links, autolinks and marks around blocks; equal
//! adjacent lists whose items at the seam hold a nested list, alone or with
//! other content.
//!
//! The seeds are upstream's 200 (from 1), drawn from the same generator
//! (mulberry32) in the same order. `NODE_STRUCTURE_SEEDS` runs more seeds
//! than the default (200), from `NODE_STRUCTURE_FIRST_SEED` (1).
//! `ElementNode.prototype.splice.call` is [`element::splice`]; an update
//! that throws in JS is one that returns an error here.

#![cfg(all(
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "code",
    feature = "overflow",
    feature = "mark",
    feature = "table"
))]

use crate::code::{CodeNode, create_code_node};
use crate::extension::{
    LexicalEditorWithDispose, NodeStructureConfigOverrides, build_editor_from_extensions,
    create_horizontal_rule_node, get_extension_dependency_from_editor, horizontal_rule_extension,
    node_structure_extension, normalize_inline_elements_extension,
};
use crate::json::Value;
use crate::link::{
    AutoLinkAttributes, AutoLinkNode, LinkAttributes, create_auto_link_node, create_link_node,
    is_link_node, link_extension,
};
use crate::list::{
    ListItemNode, ListType, create_list_item_node, create_list_node, is_list_item_node,
    list_extension,
};
use crate::mark::{MarkNode, create_mark_node, is_mark_node};
use crate::nodes::element;
use crate::nodes::line_break::create_line_break_node;
use crate::overflow::{OverflowNode, create_overflow_node, is_overflow_node};
use crate::rich_text::{
    HeadingTagType, create_heading_node, create_quote_node, is_heading_node, rich_text_extension,
};
use crate::table::{
    TableConfigOverrides, create_table_cell_node, create_table_node, create_table_row_node,
    is_table_node, table_extension,
};
use crate::{
    AnyLexicalExtensionArgument, Editor, LexicalError, LexicalExtension, MergeAdjacentEntry,
    NodeChildEntry, NodeChildrenConfig, NodeClass, NodeRef, PointType, UpdateOptions, Which,
    config_extension, create_paragraph_node, create_range_selection, create_text_node,
    define_extension, get_declared_node_structure, get_root, get_selection,
    internal_normalize_node_structure, is_root_or_shadow_root, iter_static_node_config_chain,
    parse_serialized_node, set_selection,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// `rng(seed)`: mulberry32.
struct Rng(u32);

impl Rng {
    /// `rng(seed)`, with the seed as `seed >>> 0` reads it.
    fn new(seed: u64) -> Self {
        Rng(seed as u32)
    }

    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }

    /// `Math.floor(r() * n)`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() * n as f64).floor() as usize
    }
}

/// `pick(items)`.
fn pick<T: Clone>(r: &mut Rng, items: &[T]) -> T {
    items[r.below(items.len())].clone()
}

thread_local! {
    /// `token`.
    static TOKEN: Cell<u32> = const { Cell::new(0) };
    /// `built`: editors built for the current seed, disposed after it.
    static BUILT: RefCell<Vec<LexicalEditorWithDispose>> = const { RefCell::new(Vec::new()) };
}

fn set_token(n: u32) {
    TOKEN.with(|t| t.set(n));
}

/// `` `[${token++}]` ``.
fn next_token() -> String {
    TOKEN.with(|t| {
        let n = t.get();
        t.set(n + 1);
        format!("[{n}]")
    })
}

/// `disposeBuilt()`.
fn dispose_built() {
    for editor in BUILT.with(|b| b.borrow_mut().drain(..).collect::<Vec<_>>()) {
        editor.dispose();
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Structure {
    On,
    Off,
    Disabled,
}

/// `build(structure, extra)`.
fn build(structure: Structure, extra: Vec<AnyLexicalExtensionArgument>) -> Editor {
    let mut deps: Vec<AnyLexicalExtensionArgument> = vec![
        rich_text_extension().into(),
        list_extension().into(),
        link_extension().into(),
        config_extension(
            &table_extension(),
            [TableConfigOverrides {
                has_horizontal_scroll: Some(false),
                ..Default::default()
            }],
        ),
        horizontal_rule_extension().into(),
    ];
    deps.extend(extra);
    if structure != Structure::Off {
        deps.push(config_extension(
            &node_structure_extension(),
            [NodeStructureConfigOverrides {
                disabled: Some(structure == Structure::Disabled),
            }],
        ));
    }
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: deps,
        nodes: vec![
            NodeClass::of::<AutoLinkNode>(),
            NodeClass::of::<CodeNode>(),
            NodeClass::of::<MarkNode>(),
            NodeClass::of::<OverflowNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("node-structure-property", ())
    })
    .into()])
    .unwrap();
    let out = editor.editor().clone();
    BUILT.with(|b| b.borrow_mut().push(editor));
    out
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn skip_transforms() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        skip_transforms: true,
        ..UpdateOptions::default()
    }
}

/// `$put(parent, children)`: ElementNode's own splice, so that a list can
/// hold what it may not.
fn put(parent: NodeRef, children: Vec<NodeRef>) -> Result<NodeRef, LexicalError> {
    element::splice(&parent, parent.get_children_size(), 0, &children)?;
    Ok(parent)
}

fn text(s: &str) -> NodeRef {
    create_text_node(s)
}

fn link(url: &str) -> NodeRef {
    create_link_node(url, &LinkAttributes::default())
}

fn mark(id: &str) -> NodeRef {
    create_mark_node(&[id.to_owned()])
}

const URLS: [&str; 2] = ["https://a", "https://b"];
const MARK_IDS: [&str; 2] = ["m1", "m2"];

/// `leaf()` of `$gen`.
fn leaf(r: &mut Rng) -> NodeRef {
    let x = r.next();
    if x < 0.6 {
        text(&next_token())
    } else if x < 0.75 {
        create_line_break_node()
    } else {
        create_horizontal_rule_node()
    }
}

/// `kids(n)` of `$gen`: `n` defaults to `Math.floor(r() * 4)`.
fn kids(
    r: &mut Rng,
    depth: i32,
    extended: bool,
    n: Option<usize>,
) -> Result<Vec<NodeRef>, LexicalError> {
    let n = n.unwrap_or_else(|| r.below(4));
    (0..n).map(|_| generate(r, depth - 1, extended)).collect()
}

/// `$gen(r, depth, extended)`: a random broken subtree. `extended` adds
/// what an editor without the extension throws on or loses content in often
/// enough that its own results cannot serve as a reference: autolinks around
/// blocks, and equal adjacent lists that merge (see `$seamLists`).
fn generate(r: &mut Rng, depth: i32, extended: bool) -> Result<NodeRef, LexicalError> {
    if depth <= 0 {
        return Ok(leaf(r));
    }
    Ok(match r.below(if extended { 19 } else { 18 }) {
        0 | 1 => leaf(r),
        2 | 3 => {
            let p = create_paragraph_node();
            put(p, kids(r, depth, extended, None)?)?
        }
        4 => {
            let tag = pick(r, &[HeadingTagType::H1, HeadingTagType::H2]);
            let heading = create_heading_node(tag);
            put(heading, kids(r, depth, extended, None)?)?
        }
        5 => {
            let quote = create_quote_node(false);
            put(quote, kids(r, depth, extended, None)?)?
        }
        6 => {
            let code = create_code_node(None, None);
            put(code, kids(r, depth, extended, None)?)?
        }
        7 | 8 => {
            let list_type = pick(
                r,
                &[
                    ListType::Number,
                    ListType::Number,
                    ListType::Bullet,
                    ListType::Check,
                ],
            );
            let start = 1 + r.below(5);
            let list = create_list_node(list_type, start as f64);
            let n = 1 + r.below(3);
            for _ in 0..n {
                let child = if r.next() < 0.85 {
                    let item = create_list_item_node(None);
                    put(item, kids(r, depth, extended, None)?)?
                } else {
                    generate(r, depth - 1, extended)?
                };
                put(list.clone(), vec![child])?;
            }
            list
        }
        9 | 10 => {
            let url = pick(r, &URLS);
            let node = link(url);
            put(node, kids(r, depth, extended, None)?)?
        }
        11 => {
            let node = create_auto_link_node("https://a", &AutoLinkAttributes::default());
            let children = if extended {
                kids(r, depth, extended, None)?
            } else {
                kids(r, depth, extended, Some(1))?
            };
            put(node, children)?
        }
        12 => {
            let id = pick(r, &MARK_IDS);
            let node = mark(id);
            put(node, kids(r, depth, extended, None)?)?
        }
        13 => {
            let table = create_table_node();
            let row = create_table_row_node(None);
            let first_cell = create_table_cell_node(0, 1.0, None);
            let p = create_paragraph_node();
            let first = put(first_cell, vec![put(p, kids(r, depth, extended, None)?)?])?;
            let second_cell = create_table_cell_node(0, 1.0, None);
            let second = put(second_cell, kids(r, depth, extended, None)?)?;
            row.append(&[first, second])?;
            table.append(&[row])?;
            table
        }
        14 => {
            let node = create_overflow_node();
            put(node, kids(r, depth, extended, None)?)?
        }
        15 => {
            let quote = create_quote_node(true);
            put(quote, kids(r, depth, extended, None)?)?
        }
        // A list item outside a list.
        16 => {
            let item = create_list_item_node(None);
            put(item, kids(r, depth, extended, None)?)?
        }
        17 => {
            if !extended {
                return Ok(create_paragraph_node());
            }
            // Equal adjacent lists, in a block that does not hold them (they
            // are lifted out next to each other) or in an inline element.
            let holder = match r.below(5) {
                0 => create_paragraph_node(),
                1 => create_quote_node(false),
                2 => create_list_item_node(None),
                3 => link("https://a"),
                _ => mark("m1"),
            };
            put(holder, seam_lists(r, depth - 1)?)?
        }
        _ => create_paragraph_node(),
    })
}

/// `item()` of `$seamLists`.
fn seam_list_item(r: &mut Rng, depth: i32) -> Result<NodeRef, LexicalError> {
    let item = create_list_item_node(None);
    let n = r.below(3);
    let children = (0..n)
        .map(|_| generate(r, depth - 1, true))
        .collect::<Result<Vec<_>, _>>()?;
    put(item, children)
}

/// `$seamLists(r, depth)`: two equal adjacent lists (which merge) whose
/// items at the seam each hold a nested list, alone (they are joined) or
/// with other content before or after it (nothing of which may be lost or
/// moved).
fn seam_lists(r: &mut Rng, depth: i32) -> Result<Vec<NodeRef>, LexicalError> {
    let list_type = pick(r, &[ListType::Number, ListType::Bullet]);
    let nested_type = pick(r, &[ListType::Number, ListType::Bullet]);
    let seam_item = |r: &mut Rng| -> Result<NodeRef, LexicalError> {
        let nested_list = create_list_node(nested_type, 1.0);
        let nested = put(nested_list, vec![seam_list_item(r, depth)?])?;
        if r.next() < 0.5 {
            return put(create_list_item_node(None), vec![nested]);
        }
        let other = if r.next() < 0.7 {
            text(&next_token())
        } else {
            generate(r, depth - 1, true)?
        };
        let item = create_list_item_node(None);
        let children = if r.next() < 0.5 {
            vec![nested, other]
        } else {
            vec![other, nested]
        };
        put(item, children)
    };
    let first_list = create_list_node(list_type, 1.0);
    let mut first_children = Vec::new();
    if r.next() < 0.5 {
        first_children.push(seam_list_item(r, depth)?);
    }
    first_children.push(seam_item(r)?);
    let first = put(first_list, first_children)?;
    let second_list = create_list_node(list_type, 1.0);
    let mut second_children = vec![seam_item(r)?];
    if r.next() < 0.5 {
        second_children.push(seam_list_item(r, depth)?);
    }
    let second = put(second_list, second_children)?;
    Ok(vec![first, second])
}

fn is_line_break(node: &NodeRef) -> bool {
    node.get_type() == "linebreak"
}

/// `isInlineNode(node)`.
fn is_inline_node(node: &NodeRef) -> bool {
    node.is_text() || is_line_break(node) || (node.is_element() && node.is_inline())
}

/// The children of an editor state's root.
fn root_children(editor: &Editor) -> Vec<Value> {
    let json = editor.get_editor_state().to_json(false);
    json.get("root")
        .and_then(|r| r.get("children"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The result of `makeInput(seed)`.
struct Input {
    generated: Vec<Value>,
    input: Option<Vec<Value>>,
    plain: Option<Vec<Value>>,
}

/// `makeInput(seed)`: a random broken document: `plain` as an editor
/// without the extension leaves it (its own transforms ran), `input` the
/// same with empty inline elements put in as they are.
fn make_input(seed: u32) -> Input {
    let generated = generate_document(seed);
    let editor = build(Structure::Off, vec![]);
    let r = Rc::new(RefCell::new(Rng::new(u64::from(seed))));
    set_token(0);
    let result = editor.update(
        move || {
            let r = &mut *r.borrow_mut();
            let n = 1 + r.below(4);
            for _ in 0..n {
                let mut node = generate(r, 4, false)?;
                while is_inline_node(&node) {
                    node = generate(r, 4, false)?;
                }
                get_root().append(&[node])?;
            }
            Ok(())
        },
        discrete(),
    );
    if result.is_err() {
        return Input {
            generated,
            input: None,
            plain: None,
        };
    }
    let plain = root_children(&editor);
    Input {
        generated,
        input: Some(inject_empty_inlines(&plain, seed)),
        plain: Some(plain),
    }
}

/// `generateDocument(seed)`: a random broken document from the extended
/// generator, as it is built (before any transform runs), with equal
/// adjacent lists at the root too.
fn generate_document(seed: u32) -> Vec<Value> {
    let r = Rc::new(RefCell::new(Rng::new(u64::from(seed) * 2741 + 1)));
    set_token(0);
    let out = Rc::new(RefCell::new(Vec::new()));
    let result = out.clone();
    build(Structure::Off, vec![])
        .update(
            move || {
                let r = &mut *r.borrow_mut();
                let n = 1 + r.below(4);
                for _ in 0..n {
                    if r.next() < 0.15 {
                        put(get_root(), seam_lists(r, 3)?)?;
                        continue;
                    }
                    // Not a list item either: one at the root cannot be
                    // serialized.
                    let mut node = generate(r, 4, true)?;
                    while is_inline_node(&node) || is_list_item_node(&node) {
                        node = generate(r, 4, true)?;
                    }
                    get_root().append(&[node])?;
                }
                *result.borrow_mut() = get_root().get_children().iter().map(to_json).collect();
                Ok(())
            },
            skip_transforms(),
        )
        .unwrap();
    out.take()
}

/// `$emptyInline(r, holder)`: an empty inline element that Lexical removes
/// (directly, or once what it holds is removed). Half the time it is the
/// only content of a run next to a block its holder is split around, where
/// whether it is still there decides whether that run's part stays.
fn empty_inline(r: &mut Rng, holder: bool) -> Result<NodeRef, LexicalError> {
    fn empty(r: &mut Rng) -> NodeRef {
        match r.below(3) {
            0 => link(pick(r, &URLS)),
            1 => mark(pick(r, &MARK_IDS)),
            _ => create_overflow_node(),
        }
    }
    let outer = empty(r);
    let nested = if r.next() < 0.5 {
        outer
    } else {
        let inner = empty(r);
        put(outer, vec![inner])?
    };
    if !holder {
        return Ok(nested);
    }
    let block = if r.next() < 0.5 {
        create_horizontal_rule_node()
    } else {
        let heading = create_heading_node(HeadingTagType::H1);
        put(heading, vec![text(&next_token())])?
    };
    let mut parts = vec![nested, block];
    if r.next() < 0.5 {
        parts.reverse();
    }
    if r.next() < 0.5 {
        parts.push(text(&next_token()));
    }
    match r.below(3) {
        0 => put(create_quote_node(false), parts),
        1 => put(create_paragraph_node(), parts),
        _ => {
            let list = create_list_node(ListType::Number, 1.0);
            let item = put(create_list_item_node(None), parts)?;
            put(list, vec![item])
        }
    }
}

fn json_type(json: &Value) -> &str {
    json.get("type").and_then(Value::as_str).unwrap_or("")
}

fn json_children(json: &Value) -> Option<&Vec<Value>> {
    json.get("children").and_then(Value::as_array)
}

fn json_children_mut(json: &mut Value) -> Option<&mut Vec<Value>> {
    match json {
        Value::Object(o) => match o.get_mut("children") {
            Some(Value::Array(children)) => Some(children),
            _ => None,
        },
        _ => None,
    }
}

/// `injectEmptyInlines(input, seed)`: `input` with empty inline elements put
/// in at random places, as they are: the transforms of the editor that
/// generated it would have removed them.
fn inject_empty_inlines(input: &[Value], seed: u32) -> Vec<Value> {
    let r = Rc::new(RefCell::new(Rng::new(u64::from(seed) * 389)));
    let out = Rc::new(RefCell::new(input.to_vec()));
    let tree = out.clone();
    fn insert(children: &mut Vec<Value>, holder: bool, r: &mut Rng) -> Result<(), LexicalError> {
        let at = r.below(children.len() + 1);
        children.insert(at, to_json(&empty_inline(r, holder)?));
        Ok(())
    }
    fn walk(tree: &mut Value, r: &mut Rng) -> Result<(), LexicalError> {
        if matches!(json_type(tree), "table" | "tablerow") {
            return Ok(());
        }
        let Some(children) = json_children_mut(tree) else {
            return Ok(());
        };
        for child in children.iter_mut() {
            walk(child, r)?;
        }
        if r.next() < 0.12 {
            let holder = r.next() < 0.5;
            insert(children, holder, r)?;
        }
        Ok(())
    }
    build(Structure::Off, vec![])
        .update(
            move || {
                let r = &mut *r.borrow_mut();
                let out = &mut *tree.borrow_mut();
                for top in out.iter_mut() {
                    walk(top, r)?;
                }
                if r.next() < 0.3 {
                    insert(out, true, r)?;
                }
                Ok(())
            },
            skip_transforms(),
        )
        .unwrap();
    out.take()
}

/// `load(editor, input)`.
fn load(editor: &Editor, input: &[Value]) -> Result<(), LexicalError> {
    let input = input.to_vec();
    editor.update(
        move || {
            let nodes = input
                .iter()
                .map(parse_serialized_node)
                .collect::<Result<Vec<_>, _>>()?;
            let root = get_root();
            root.clear()?;
            root.append(&nodes)
        },
        discrete(),
    )
}

/// `$allNodes()`.
fn all_nodes() -> Vec<NodeRef> {
    fn walk(node: &NodeRef, out: &mut Vec<NodeRef>) {
        out.push(node.clone());
        if node.is_element() {
            for child in node.get_children() {
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

/// `Entry`s of a document, flattened: `entries[i]`'s JSON and the indices
/// of its children.
struct Entries {
    json: Vec<Value>,
    children: Vec<Vec<usize>>,
    tops: Vec<usize>,
}

/// `entries(json)` of each top-level node, with every entry in `all` (in
/// document order).
fn entries(input: &[Value]) -> Entries {
    fn walk(json: &Value, out: &mut Entries) -> usize {
        let at = out.json.len();
        out.json.push(json.clone());
        out.children.push(Vec::new());
        let kids: Vec<usize> = json_children(json)
            .map(|c| c.iter().map(|c| walk(c, out)).collect())
            .unwrap_or_default();
        out.children[at] = kids;
        at
    }
    let mut out = Entries {
        json: Vec::new(),
        children: Vec::new(),
        tops: Vec::new(),
    };
    for top in input {
        let at = walk(top, &mut out);
        out.tops.push(at);
    }
    out
}

/// `Order`: the orders the nodes of a document are created in (which is the
/// order Lexical runs their transforms in).
#[derive(Clone, Copy)]
enum Order {
    /// Each node after its children.
    InnerFirst,
    /// `outerFirst`: each node before its children, in document order:
    /// what loading does.
    OuterFirst,
    Reverse,
    Shuffled,
}

/// `orders`, in `Object.entries` order.
const ORDERS: [(&str, Order); 4] = [
    ("innerFirst", Order::InnerFirst),
    ("outerFirst", Order::OuterFirst),
    ("reverse", Order::Reverse),
    ("shuffled", Order::Shuffled),
];

/// `order(all, seed)`.
fn apply_order(order: Order, all: &Entries, seed: u32) -> Vec<usize> {
    let n = all.json.len();
    match order {
        Order::InnerFirst => {
            fn walk(e: usize, all: &Entries, out: &mut Vec<usize>) {
                for &c in &all.children[e] {
                    walk(c, all, out);
                }
                out.push(e);
            }
            let mut out = Vec::new();
            for &top in &all.tops {
                walk(top, all, &mut out);
            }
            out
        }
        Order::OuterFirst => (0..n).collect(),
        Order::Reverse => (0..n).rev().collect(),
        Order::Shuffled => shuffle((0..n).collect(), u64::from(seed) * 7919),
    }
}

/// The Fisher-Yates shuffle of `orders.shuffled` and
/// `dirtyOrders.shuffled`, with `rng(seed)`.
fn shuffle<T>(mut c: Vec<T>, seed: u64) -> Vec<T> {
    let mut r = Rng::new(seed);
    for i in (1..c.len()).rev() {
        let j = r.below(i + 1);
        c.swap(i, j);
    }
    c
}

/// `{...json, children: []}` for an element's JSON, `json` otherwise.
fn without_children(json: &Value) -> Value {
    let mut json = json.clone();
    if json_children(&json).is_some()
        && let Value::Object(o) = &mut json
    {
        o.insert("children", Value::Array(Vec::new()));
    }
    json
}

/// `loadInOrder(editor, input, order, seed, faithful, skipTransforms)`: load
/// `input` creating its nodes in `order`, then linking them exactly as
/// `$parseSerializedNode` does (each child appended after its own
/// children).
fn load_in_order(
    editor: &Editor,
    input: &[Value],
    order: Order,
    seed: u32,
    faithful: bool,
    skip: bool,
) -> Result<(), LexicalError> {
    let all = entries(input);
    let options = if skip { skip_transforms() } else { discrete() };
    editor.update(
        move || {
            let mut nodes: HashMap<usize, NodeRef> = HashMap::new();
            for e in apply_order(order, &all, seed) {
                nodes.insert(e, parse_serialized_node(&without_children(&all.json[e]))?);
            }
            fn link_entry(
                e: usize,
                all: &Entries,
                nodes: &HashMap<usize, NodeRef>,
                faithful: bool,
            ) -> Result<NodeRef, LexicalError> {
                let node = nodes[&e].clone();
                for &child in &all.children[e] {
                    let child = link_entry(child, all, nodes, faithful)?;
                    if faithful {
                        put(node.clone(), vec![child])?;
                    } else {
                        node.append(&[child])?;
                    }
                }
                Ok(node)
            }
            let root = get_root();
            root.clear()?;
            let tops = all
                .tops
                .iter()
                .map(|&e| link_entry(e, &all, &nodes, faithful))
                .collect::<Result<Vec<_>, _>>()?;
            root.append(&tops)
        },
        options,
    )
}

/// `loadAsIs(editor, input, $then)`: load `input` as it is, as an update
/// applied with `skipTransforms` does (no transform runs, so nothing is
/// normalized yet), running `then` in the same update.
fn load_as_is(
    editor: &Editor,
    input: &[Value],
    then: impl FnOnce() -> Result<(), LexicalError> + 'static,
) -> Result<(), LexicalError> {
    let input = input.to_vec();
    editor.update(
        move || {
            let root = get_root();
            root.clear()?;
            let nodes = input
                .iter()
                .map(build_as_is)
                .collect::<Result<Vec<_>, _>>()?;
            root.append(&nodes)?;
            then()
        },
        skip_transforms(),
    )
}

fn nothing() -> Result<(), LexicalError> {
    Ok(())
}

/// The orders in which every node of a loaded document is marked dirty.
#[derive(Clone, Copy)]
enum DirtyOrder {
    /// Each node before its children, in document order.
    Forward,
    /// Each node after its children.
    InnerFirst,
    Reverse,
    Shuffled,
}

/// `dirtyOrders`, in `Object.entries` order.
const DIRTY_ORDERS: [(&str, DirtyOrder); 4] = [
    ("forward", DirtyOrder::Forward),
    ("innerFirst", DirtyOrder::InnerFirst),
    ("reverse", DirtyOrder::Reverse),
    ("shuffled", DirtyOrder::Shuffled),
];

/// `dirtyOrders[name](nodes, seed)`.
fn apply_dirty_order(order: DirtyOrder, nodes: Vec<NodeRef>, seed: u32) -> Vec<NodeRef> {
    match order {
        DirtyOrder::Forward => nodes,
        DirtyOrder::InnerFirst => {
            fn walk(node: &NodeRef, out: &mut Vec<NodeRef>) {
                if node.is_element() {
                    for child in node.get_children() {
                        walk(&child, out);
                    }
                }
                out.push(node.clone());
            }
            let mut out = Vec::new();
            for child in get_root().get_children() {
                walk(&child, &mut out);
            }
            out.retain(|n| nodes.contains(n));
            out
        }
        DirtyOrder::Reverse => nodes.into_iter().rev().collect(),
        DirtyOrder::Shuffled => shuffle(nodes, u64::from(seed) * 6271),
    }
}

/// Every node marked dirty, in `order`.
fn mark_all_dirty(editor: &Editor, order: DirtyOrder, seed: u32) -> Result<(), LexicalError> {
    editor.update(
        move || {
            for n in apply_dirty_order(order, all_nodes(), seed) {
                n.mark_dirty();
            }
            Ok(())
        },
        discrete(),
    )
}

/// `withDistinctStyles(input)`: `input` with a style of its own on every
/// text node, so that text normalization never merges two of them: Lexical
/// moves an element point between two text nodes it merges, which is not
/// the normalization's doing.
fn with_distinct_styles(input: &[Value]) -> Vec<Value> {
    fn walk(json: &mut Value, n: &mut u32) {
        if json_type(json) == "text"
            && let Value::Object(o) = json
        {
            o.insert("style", Value::from(format!("--n: {n}")));
            *n += 1;
        }
        if let Some(children) = json_children_mut(json) {
            for child in children {
                walk(child, n);
            }
        }
    }
    let mut n = 0;
    let mut copy = input.to_vec();
    for json in &mut copy {
        walk(json, &mut n);
    }
    copy
}

/// `leafSize(node)`: what a leaf adds to a position in the content: text,
/// a decorator 1.
fn leaf_size(node: &NodeRef) -> i64 {
    if node.is_text() {
        node.get_text_content_size() as i64
    } else if node.is_decorator() {
        1
    } else {
        0
    }
}

/// `$sizeOf(node)`.
fn size_of(node: &NodeRef) -> i64 {
    if node.is_element() {
        node.get_children().iter().map(size_of).sum()
    } else {
        leaf_size(node)
    }
}

/// `$contentOffset(key, offset)`: where a point is in the content: the size
/// of every leaf before it in document order (line breaks count 0, since
/// joining paragraphs adds them).
fn content_offset(key: &str, offset: usize) -> i64 {
    fn walk(node: &NodeRef, key: &str, offset: usize, before: &mut i64, found: &mut bool) {
        if *found {
            return;
        }
        if node.get_key() == key {
            *found = true;
            *before += if node.is_element() {
                node.get_children().iter().take(offset).map(size_of).sum()
            } else {
                offset as i64
            };
            return;
        }
        if node.is_element() {
            for child in node.get_children() {
                walk(&child, key, offset, before, found);
            }
        } else {
            *before += leaf_size(node);
        }
    }
    let mut before = 0;
    let mut found = false;
    walk(&get_root(), key, offset, &mut before, &mut found);
    if found { before } else { -1 }
}

/// `Leaf`.
struct Leaf {
    key: String,
    desc: String,
    kind: Option<String>,
    in_table: bool,
}

/// `blockKindOf(node)`: the block a text node is a line of: its nearest
/// ancestor that is neither inline nor a plain paragraph (which has no kind
/// of its own and becomes lines of the block around it). Its kind must
/// survive the repairs; only a list's stray inline content gains one (it is
/// wrapped in a list item).
fn block_kind_of(node: &NodeRef) -> String {
    for p in node.get_parents() {
        if p.get_type() == "paragraph" || is_overflow_node(&p) || p.is_inline() {
            continue;
        }
        return match p.get_heading_tag() {
            Some(tag) if is_heading_node(&p) => tag.as_str().to_owned(),
            _ => p.get_type().to_owned(),
        };
    }
    "root".to_owned()
}

/// `leaves(editor)`.
fn leaves(editor: &Editor) -> Vec<Leaf> {
    editor
        .read(|| {
            all_nodes()
                .into_iter()
                .filter(|n| !n.is_element())
                .map(|n| Leaf {
                    desc: if n.is_text() {
                        n.get_text_content()
                    } else {
                        n.get_type().to_owned()
                    },
                    in_table: n.get_parents().iter().any(is_table_node),
                    key: n.get_key().to_owned(),
                    kind: n.is_text().then(|| block_kind_of(&n)),
                })
                .collect()
        })
        .unwrap()
}

/// `text.match(/\[\d+\]|<hr>/g) || []`.
fn units(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if text[i..].starts_with("<hr>") {
            out.push(&text[i..i + 4]);
            i += 4;
            continue;
        }
        if bytes[i] == b'[' {
            let digits = bytes[i + 1..]
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count();
            if digits > 0 && bytes.get(i + 1 + digits) == Some(&b']') {
                out.push(&text[i..i + digits + 2]);
                i += digits + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// `text.match(/\[\d+\]/g) || []`.
fn text_units(text: &str) -> Vec<&str> {
    units(text).into_iter().filter(|u| *u != "<hr>").collect()
}

/// `keepsContent(result, created, kept)`: whether `result` keeps every piece
/// of content `kept` has (as many times), in the order `created` has it,
/// and nothing else.
fn keeps_content(result: &str, created: &str, kept: &str) -> bool {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for unit in units(result) {
        *counts.entry(unit).or_default() += 1;
    }
    for unit in units(kept) {
        let n = counts.get(unit).copied().unwrap_or(0);
        if n == 0 {
            return false;
        }
        counts.insert(unit, n - 1);
    }
    // A subsequence of what was created.
    let all = units(created);
    let mut at = 0;
    for unit in units(result) {
        while at < all.len() && all[at] != unit {
            at += 1;
        }
        if at == all.len() {
            return false;
        }
        at += 1;
    }
    true
}

/// `tokens(editor)`.
fn tokens(editor: &Editor) -> String {
    editor
        .read(|| {
            all_nodes()
                .iter()
                .map(|n| {
                    if n.is_text() {
                        n.get_text_content()
                    } else if n.is_decorator() {
                        "<hr>".to_owned()
                    } else {
                        String::new()
                    }
                })
                .collect::<String>()
        })
        .unwrap()
}

/// `annotations(editor)`: each piece of text (a `[n]` token) with the
/// links, autolinks and marks around it, outermost first: what they say
/// about it (a URL, comment ids) must stay on it.
fn annotations(editor: &Editor) -> Vec<String> {
    editor
        .read(|| {
            let mut out = Vec::new();
            for node in all_nodes() {
                if !node.is_text() {
                    continue;
                }
                let around = node
                    .get_parents()
                    .iter()
                    .rev()
                    .filter_map(|p| {
                        if is_mark_node(p) {
                            Some(format!(
                                "mark:{}",
                                p.get_ids().unwrap_or_default().join(",")
                            ))
                        } else if is_link_node(p) {
                            Some(format!(
                                "{}:{}",
                                p.get_type(),
                                p.get_url().unwrap_or_default()
                            ))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(">");
                for unit in text_units(&node.get_text_content()) {
                    out.push(format!("{unit} {around}"));
                }
            }
            out
        })
        .unwrap()
}

/// `firstDifference(a, b)`: the first difference between two lists of
/// annotated text, for a report.
fn first_difference(a: &[String], b: &[String]) -> String {
    let at = |l: &[String], i: usize| l.get(i).cloned().unwrap_or("undefined".to_owned());
    for i in 0..a.len().max(b.len()) {
        if a.get(i) != b.get(i) {
            return format!("{} -> {}", at(a, i), at(b, i));
        }
    }
    String::new()
}

/// `lexicalRejects(children)`: whether Lexical itself throws on `children`
/// without the extension (some random edits trip transforms of their own,
/// such as a list inside a link).
fn lexical_rejects(children: &[Value]) -> bool {
    let children = children.to_vec();
    build(Structure::Off, vec![])
        .update(
            move || {
                let root = get_root();
                root.clear()?;
                let nodes = children
                    .iter()
                    .map(build_as_is)
                    .collect::<Result<Vec<_>, _>>()?;
                root.append(&nodes)
            },
            discrete(),
        )
        .is_err()
}

/// `$buildAsIs(json)`: a node built from JSON as it is (no override
/// rewrites its children).
fn build_as_is(json: &Value) -> Result<NodeRef, LexicalError> {
    let Some(children) = json_children(json) else {
        return parse_serialized_node(json);
    };
    let node = parse_serialized_node(&without_children(json))?;
    let children = children
        .iter()
        .map(build_as_is)
        .collect::<Result<Vec<_>, _>>()?;
    put(node, children)
}

/// `$toJSON(node)`: serialize a subtree, children included, before any
/// transform runs.
fn to_json(node: &NodeRef) -> Value {
    let mut json = node.export_json(false);
    if node.is_element() {
        json.insert(
            "children",
            Value::Array(node.get_children().iter().map(to_json).collect()),
        );
    }
    Value::Object(json)
}

/// What `randomEdit` returns.
struct Edit {
    path: Vec<usize>,
    index: usize,
    node: Value,
    edited_children: Vec<Value>,
}

/// `randomEdit(root, seed, extended)`: a random broken subtree inserted at
/// a random place of `root`.
fn random_edit(root: &Value, seed: u64, extended: bool) -> Option<Edit> {
    let r = Rc::new(RefCell::new(Rng::new(seed * 104729)));
    fn walk(tree: &Value, path: Vec<usize>, targets: &mut Vec<(Vec<usize>, usize)>) {
        if let Some(children) = json_children(tree)
            && !matches!(json_type(tree), "table" | "tablerow")
        {
            targets.push((path.clone(), children.len()));
            for (i, c) in children.iter().enumerate() {
                let mut child_path = path.clone();
                child_path.push(i);
                walk(c, child_path, targets);
            }
        }
    }
    let mut targets = Vec::new();
    walk(root, Vec::new(), &mut targets);
    let (path, len) = {
        let r = &mut *r.borrow_mut();
        targets[r.below(targets.len())].clone()
    };
    let index = r.borrow_mut().below(len + 1);
    let node: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(None));
    let scratch = build(Structure::Off, vec![]);
    let (out, at_root) = (node.clone(), path.is_empty());
    // An update that throws here throws out of the test in JS too.
    scratch
        .update(
            move || {
                let r = &mut *r.borrow_mut();
                set_token(100000);
                let mut n = generate(r, 2, extended)?;
                while at_root && is_inline_node(&n) {
                    n = generate(r, 2, extended)?;
                }
                *out.borrow_mut() = Some(to_json(&n));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let node = node.take()?;
    let mut edited = root.clone();
    let mut at = &mut edited;
    for &i in &path {
        at = &mut json_children_mut(at).unwrap()[i];
    }
    json_children_mut(at).unwrap().insert(index, node.clone());
    Some(Edit {
        edited_children: json_children(&edited).unwrap().clone(),
        index,
        node,
        path,
    })
}

/// `JSON.stringify(value, replacer)` with a replacer that drops every key
/// in `keys`.
fn without_keys(value: &Value, keys: &[String]) -> Value {
    match value {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, _)| !keys.iter().any(|d| d == k))
                .map(|(k, v)| (k, without_keys(v, keys)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(|v| without_keys(v, keys)).collect()),
        _ => value.clone(),
    }
}

/// `withoutDerived(editor)`.
fn without_derived(editor: &Editor) -> String {
    let derived = get_declared_node_structure(&ListItemNode::default()).derived;
    without_keys(&editor.get_editor_state().to_json(false), &derived).to_string()
}

/// `stateJSON(editor)`.
fn state_json(editor: &Editor) -> String {
    editor.get_editor_state().to_json(false).to_string()
}

/// `child instanceof klass`, for the class of type `node_type`.
fn is_instance_of(child: &NodeRef, node_type: &str) -> bool {
    iter_static_node_config_chain(&*child.get_latest())
        .iter()
        .any(|link| link.node_type == node_type)
}

/// `allows(children, child)`.
fn allows(children: &NodeChildrenConfig, child: &NodeRef) -> bool {
    let inline =
        child.is_text() || is_line_break(child) || is_overflow_node(child) || { child.is_inline() };
    if is_overflow_node(child) {
        return true;
    }
    match children {
        NodeChildrenConfig::Inline => inline,
        NodeChildrenConfig::Block => !inline,
        NodeChildrenConfig::List(entries) => {
            if inline {
                entries.contains(&NodeChildEntry::Inline)
            } else {
                entries.iter().any(|e| match e {
                    NodeChildEntry::Inline => false,
                    NodeChildEntry::Class(klass) => is_instance_of(child, klass.node_type()),
                })
            }
        }
    }
}

/// `violations(editor)`: violations left in the document (it must be a
/// fixed point).
fn violations(editor: &Editor) -> Vec<String> {
    editor
        .read(|| {
            let mut out = Vec::new();
            let mut nodes = vec![get_root()];
            nodes.extend(all_nodes());
            for node in nodes {
                if !node.is_element() {
                    continue;
                }
                let root_or_shadow = is_root_or_shadow_root(&node);
                let children = if root_or_shadow {
                    Some(NodeChildrenConfig::Block)
                } else {
                    get_declared_node_structure(&*node.get_latest()).children
                };
                if node.is_empty()
                    && !root_or_shadow
                    && (is_overflow_node(&node) || (node.is_inline() && !node.can_be_empty()))
                {
                    out.push(format!("empty {}", node.get_type()));
                }
                for child in node.get_children() {
                    if let Some(children) = &children
                        && !allows(children, &child)
                    {
                        out.push(format!("{} holds {}", node.get_type(), child.get_type()));
                    }
                    let next = child.get_next_sibling();
                    let declared = get_declared_node_structure(&*child.get_latest());
                    if let Some(next) = next
                        && next.get_type() == child.get_type()
                        && let Some(merge) = &declared.merge_adjacent
                        && declared.declared_by.merge_adjacent == Some(child.get_type())
                    {
                        let a = child.export_json(false);
                        let b = next.export_json(false);
                        if merge.by.iter().all(|k| match k {
                            MergeAdjacentEntry::Field(k) => {
                                a.get(k).map(Value::to_string) == b.get(k).map(Value::to_string)
                            }
                            MergeAdjacentEntry::State(_) => false,
                        }) {
                            out.push(format!("equal adjacent {}", child.get_type()));
                        }
                    }
                }
            }
            out
        })
        .unwrap()
}

/// The failures of the run, by kind, in the order first seen.
#[derive(Default)]
struct Failures(RefCell<Vec<(String, Vec<String>)>>);

impl Failures {
    /// `fail(kind, failedSeed, msg)`.
    fn fail(&self, kind: &str, failed_seed: u32, msg: &str) {
        let entry: String = format!("{failed_seed} {msg}").chars().take(300).collect();
        let mut failures = self.0.borrow_mut();
        match failures.iter_mut().find(|(k, _)| k == kind) {
            Some((_, v)) => v.push(entry),
            None => failures.push((kind.to_owned(), vec![entry])),
        }
    }
}

/// `String(e)`.
fn error_string(e: &LexicalError) -> String {
    format!("Error: {e}")
}

/// `checkCarets(doc, seed, caretSeed, label, fail)`: three carets placed at
/// random in `doc` as it is (loaded without any transform) keep their
/// place in the content: after the normalization each is at the same
/// offset in document order, and valid.
fn check_carets(doc: &[Value], seed: u32, caret_seed: u64, label: &str, failures: &Failures) {
    let styled = with_distinct_styles(doc);
    let probe = build(Structure::Off, vec![]);
    type Points = Vec<(Vec<usize>, usize, PointType)>;
    let points: Rc<RefCell<Points>> = Rc::new(RefCell::new(Vec::new()));
    let out = points.clone();
    load_as_is(&probe, &styled, move || {
        fn walk(node: &NodeRef, path: Vec<usize>, out: &mut Points) {
            if node.is_text() {
                for i in 0..=node.get_text_content_size() {
                    out.push((path.clone(), i, PointType::Text));
                }
            } else if node.is_element() {
                for i in 0..=node.get_children_size() {
                    out.push((path.clone(), i, PointType::Element));
                }
                for (i, c) in node.get_children().iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(i);
                    walk(c, child_path, out);
                }
            }
        }
        walk(&get_root(), Vec::new(), &mut out.borrow_mut());
        Ok(())
    })
    .unwrap();
    let points = points.take();
    let mut r = Rng::new(caret_seed);
    let mut k = 0;
    while k < 3 && !points.is_empty() {
        k += 1;
        let (path, offset, point_type) = points[r.below(points.len())].clone();
        let editor = build(Structure::On, vec![]);
        let before = Rc::new(Cell::new(-1i64));
        let place = Rc::new(RefCell::new(String::new()));
        let (before_out, place_out) = (before.clone(), place.clone());
        let result = load_as_is(&editor, &styled, move || {
            let mut node = get_root();
            for &i in &path {
                node = node
                    .get_child_at_index(i)
                    .ok_or_else(|| LexicalError::new("no child"))?;
            }
            let selection = create_range_selection();
            selection.point_set(Which::Anchor, node.get_key(), offset, point_type)?;
            selection.point_set(Which::Focus, node.get_key(), offset, point_type)?;
            set_selection(Some(selection));
            before_out.set(content_offset(node.get_key(), offset));
            *place_out.borrow_mut() = format!(
                "{}[{}]@{offset}",
                node.get_type(),
                path.iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            Ok(())
        })
        .and_then(|()| mark_all_dirty(&editor, DirtyOrder::Forward, seed));
        if let Err(e) = result {
            failures.fail(&format!("throws (caret{label})"), seed, &error_string(&e));
            break;
        }
        let after: Result<i64, String> = editor
            .read(|| {
                let selection = get_selection().filter(crate::SelectionRef::is_range);
                let Some(selection) = selection else {
                    return Err("no collapsed range selection".to_owned());
                };
                let anchor = selection.point(Which::Anchor).unwrap();
                if selection.point(Which::Focus).as_ref() != Some(&anchor) {
                    return Err("no collapsed range selection".to_owned());
                }
                let node = anchor.get_node().unwrap();
                let valid = node.is_attached()
                    && if node.is_text() {
                        anchor.point_type == PointType::Text
                            && anchor.offset <= node.get_text_content_size()
                    } else {
                        node.is_element() && anchor.offset <= node.get_children_size()
                    };
                if valid {
                    Ok(content_offset(&anchor.key, anchor.offset))
                } else {
                    Err(format!(
                        "invalid {} point in {}",
                        anchor.point_type.as_str(),
                        node.get_type()
                    ))
                }
            })
            .unwrap();
        if after.as_ref() != Ok(&before.get()) {
            let after = match after {
                Ok(n) => n.to_string(),
                Err(s) => s,
            };
            failures.fail(
                &format!("caret moved{label}"),
                seed,
                &format!("{}: {} -> {after}", place.borrow(), before.get()),
            );
        }
    }
}

/// `checkLocalEdit(state, editSeed, label, fail, seed)`: a local edit that
/// breaks the normalized document `state` (inserting a random broken
/// subtree), where only the nodes it writes are dirty, gives what loading
/// the edited document gives, with every node dirty (as if it arrived
/// unrepaired), and leaves a fixed point.
fn check_local_edit(state: &str, edit_seed: u64, label: &str, failures: &Failures, seed: u32) {
    let parsed = crate::json::parse(state).unwrap();
    let root = parsed.get("root").unwrap();
    let children = json_children(root).unwrap().clone();
    let Some(edit) = random_edit(root, edit_seed, !label.is_empty()) else {
        return;
    };
    if lexical_rejects(&edit.edited_children) {
        return;
    }
    let local = build(Structure::On, vec![]);
    let fresh = build(Structure::On, vec![]);
    let (path, index, node) = (edit.path.clone(), edit.index, edit.node.clone());
    let edited_children = edit.edited_children.clone();
    let result = local
        .update(
            move || {
                let root = get_root();
                root.clear()?;
                let nodes = children
                    .iter()
                    .map(build_as_is)
                    .collect::<Result<Vec<_>, _>>()?;
                root.append(&nodes)
            },
            discrete(),
        )
        .and_then(|()| {
            local.update(
                move || {
                    let mut target = get_root();
                    for &i in &path {
                        target = target
                            .get_child_at_index(i)
                            .ok_or_else(|| LexicalError::new("no child at the edit's path"))?;
                    }
                    if !target.is_element() {
                        return Err(LexicalError::new("the edit's target is not an element"));
                    }
                    put(target.clone(), vec![])?;
                    element::splice(&target, index, 0, &[build_as_is(&node)?])
                },
                discrete(),
            )
        })
        .and_then(|()| {
            fresh.update(
                move || {
                    let root = get_root();
                    root.clear()?;
                    let nodes = edited_children
                        .iter()
                        .map(build_as_is)
                        .collect::<Result<Vec<_>, _>>()?;
                    root.append(&nodes)
                },
                discrete(),
            )
        });
    if let Err(e) = result {
        failures.fail(
            &format!("throws on an edit{label}"),
            seed,
            &error_string(&e),
        );
        return;
    }
    // Derived values (a list item's number) are recomputed by their class's
    // own transform when it runs, which this edit (as without the
    // extension) does not always trigger: compare the rest.
    if without_derived(&fresh) != without_derived(&local) {
        failures.fail(
            &format!("a local edit and a fresh load disagree{label}"),
            seed,
            "",
        );
    }
    let as_built = build(Structure::Off, vec![]);
    load_as_is(&as_built, &edit.edited_children, nothing).unwrap();
    if tokens(&local) != tokens(&as_built) {
        failures.fail(&format!("content lost by a local edit{label}"), seed, "");
    }
    let remaining = violations(&local);
    if !remaining.is_empty() {
        failures.fail(
            &format!("not a fixed point after a local edit{label}"),
            seed,
            &remaining.join(", "),
        );
    }
}

/// `getExtensionDependencyFromEditor(editor, NodeStructureExtension)
/// .output.disabled.value = false`, then `editor.read(() => {})`.
fn turn_on(editor: &Editor) {
    get_extension_dependency_from_editor(editor, &node_structure_extension())
        .unwrap()
        .output
        .unwrap()
        .disabled
        .set_value(false);
    editor.read(|| {}).unwrap();
}

/// `root.clear().append(...children.map($buildAsIs))` in a discrete update
/// (with transforms).
fn load_built_as_is(editor: &Editor, children: &[Value]) -> Result<(), LexicalError> {
    let children = children.to_vec();
    editor.update(
        move || {
            let root = get_root();
            root.clear()?;
            let nodes = children
                .iter()
                .map(build_as_is)
                .collect::<Result<Vec<_>, _>>()?;
            root.append(&nodes)
        },
        discrete(),
    )
}

/// The checks of one seed.
fn check_seed(seed: u32, failures: &Failures, checked: &mut usize) {
    let Input {
        input,
        plain,
        generated,
    } = make_input(seed);
    // The document as generated, before any transform ran: links,
    // autolinks and marks around blocks (which an editor without the
    // extension lifts blocks out of only in part, or throws on), and equal
    // adjacent lists whose items at the seam hold a nested list. Nothing is
    // lost, duplicated or reordered, every piece of text keeps the links,
    // autolinks and marks around it, and the result is a fixed point that
    // depends on the document only. (`INTERNAL_$normalizeNodeStructure` is
    // checked on the documents above only: in an update without transforms,
    // Lexical's own, such as the one that puts a list item outside a list
    // into a list, do not run.)
    {
        let as_built = build(Structure::Off, vec![]);
        load_as_is(&as_built, &generated, nothing).unwrap();
        let raw_tokens = tokens(&as_built);
        let raw_annotations = annotations(&as_built);
        let editor = build(Structure::On, vec![]);
        let reference = match load_built_as_is(&editor, &generated) {
            Ok(()) => Some(state_json(&editor)),
            Err(e) => {
                failures.fail("throws (generated)", seed, &error_string(&e));
                None
            }
        };
        if let Some(reference) = &reference {
            if tokens(&editor) != raw_tokens {
                failures.fail(
                    "content lost, duplicated or reordered (generated)",
                    seed,
                    "",
                );
            }
            let got = annotations(&editor);
            if got.join("\n") != raw_annotations.join("\n") {
                failures.fail(
                    "annotation lost (generated)",
                    seed,
                    &first_difference(&raw_annotations, &got),
                );
            }
            let remaining = violations(&editor);
            if !remaining.is_empty() {
                failures.fail("not a fixed point (generated)", seed, &remaining.join(", "));
            }
            mark_all_dirty(&editor, DirtyOrder::Forward, seed).unwrap();
            if &state_json(&editor) != reference {
                failures.fail("not idempotent (generated)", seed, "");
            }
            for (name, order) in ORDERS {
                let other = build(Structure::On, vec![]);
                if let Err(e) = load_in_order(&other, &generated, order, seed, true, false) {
                    failures.fail(
                        &format!("throws (generated, {name})"),
                        seed,
                        &error_string(&e),
                    );
                    continue;
                }
                if &state_json(&other) != reference {
                    failures.fail(
                        &format!("result depends on dirty order (generated, {name})"),
                        seed,
                        "",
                    );
                }
            }
            for (name, order) in DIRTY_ORDERS {
                let other = build(Structure::On, vec![]);
                let result = load_as_is(&other, &generated, nothing)
                    .and_then(|()| mark_all_dirty(&other, order, seed));
                if let Err(e) = result {
                    failures.fail(
                        &format!("throws (generated, dirty {name})"),
                        seed,
                        &error_string(&e),
                    );
                    continue;
                }
                if &state_json(&other) != reference {
                    failures.fail(
                        &format!("result depends on dirty order (generated, dirty {name})"),
                        seed,
                        "",
                    );
                }
            }
            let later = build(Structure::Disabled, vec![]);
            load_as_is(&later, &generated, nothing).unwrap();
            turn_on(&later);
            if &state_json(&later) != reference {
                failures.fail(
                    "result differs when enforcement is turned on (generated)",
                    seed,
                    "",
                );
            }
        }
        check_carets(
            &generated,
            seed,
            u64::from(seed) * 7741,
            " (generated)",
            failures,
        );
        if let Some(reference) = &reference {
            check_local_edit(
                reference,
                u64::from(seed) * 31 + 7,
                " (generated)",
                failures,
                seed,
            );
        }
    }
    let (Some(plain), Some(input)) = (plain, input) else {
        // Lexical's own transforms reject the document without the
        // extension (a list inside a link): there is no reference to
        // compare with.
        return;
    };
    // Loading with the extension: everything is created, outer nodes first.
    // A document Lexical's own transforms reject without the extension (a
    // list inside a link) is skipped.
    let off = build(Structure::Off, vec![]);
    if load(&off, &plain).is_err() {
        return;
    }
    *checked += 1;
    let loaded = build(Structure::On, vec![]);
    if let Err(e) = load(&loaded, &plain) {
        failures.fail("throws on load", seed, &error_string(&e));
        return;
    }
    if tokens(&loaded) != tokens(&off) {
        failures.fail("content lost, duplicated or reordered on load", seed, "");
    }
    // What a link, autolink or mark says about the text it holds stays on
    // it, as it does without the extension.
    if annotations(&loaded).join("\n") != annotations(&off).join("\n") {
        failures.fail(
            "annotation lost on load",
            seed,
            &first_difference(&annotations(&off), &annotations(&loaded)),
        );
    }
    let want = state_json(&loaded);
    let left = violations(&loaded);
    if !left.is_empty() {
        failures.fail("not a fixed point", seed, &left.join(", "));
    }
    // Idempotence: everything dirty again changes nothing.
    mark_all_dirty(&loaded, DirtyOrder::Forward, seed).unwrap();
    if state_json(&loaded) != want {
        failures.fail("not idempotent", seed, "");
    }
    // Idempotence of the whole normalization on its own result.
    loaded
        .update(
            || internal_normalize_node_structure(&get_root()),
            discrete(),
        )
        .unwrap();
    if state_json(&loaded) != want {
        failures.fail(
            "not idempotent (INTERNAL_$normalizeNodeStructure)",
            seed,
            "",
        );
    }
    // The same document with its nodes created in other orders.
    for (name, order) in ORDERS {
        let editor = build(Structure::On, vec![]);
        if let Err(e) = load_in_order(&editor, &plain, order, seed, false, false) {
            failures.fail(&format!("throws ({name})"), seed, &error_string(&e));
            continue;
        }
        if state_json(&editor) != want {
            failures.fail(&format!("result depends on dirty order ({name})"), seed, "");
        }
    }
    // The document with empty inline elements put in, loaded as it is (as
    // an update applied with skipTransforms leaves it), then every node
    // marked dirty, in several orders: the same result whichever transforms
    // run first (the normalization's, or Lexical's own, such as the removal
    // of an empty inline element), with the same content.
    let raw = build(Structure::Off, vec![]);
    load_as_is(&raw, &input, nothing).unwrap();
    let content = tokens(&raw);
    let mut as_is_reference: Option<Editor> = None;
    {
        let mut reference: Option<String> = None;
        for (name, order) in DIRTY_ORDERS {
            let editor = build(Structure::On, vec![]);
            let result = load_as_is(&editor, &input, nothing)
                .and_then(|()| mark_all_dirty(&editor, order, seed));
            if let Err(e) = result {
                failures.fail(&format!("throws (dirty {name})"), seed, &error_string(&e));
                break;
            }
            let result = state_json(&editor);
            match &reference {
                None => {
                    reference = Some(result);
                    if tokens(&editor) != content {
                        failures.fail("content lost, duplicated or reordered (as is)", seed, "");
                    }
                    let remaining = violations(&editor);
                    if !remaining.is_empty() {
                        failures.fail("not a fixed point (dirty)", seed, &remaining.join(", "));
                    }
                    as_is_reference = Some(editor);
                }
                Some(reference) => {
                    if &result != reference {
                        failures.fail(
                            &format!("result depends on dirty order (dirty {name})"),
                            seed,
                            "",
                        );
                    }
                }
            }
        }
    }
    // The same document created in every order, through the methods loading
    // uses: the same result, keeping the content loading builds (before any
    // transform runs) in its order, except what Lexical's own transforms
    // drop without the extension too (the rows of a table put in a list).
    {
        let as_built = build(Structure::Off, vec![]);
        let injected_off = build(Structure::Off, vec![]);
        let rejected = load_in_order(&as_built, &input, Order::OuterFirst, seed, false, true)
            .and_then(|()| load(&injected_off, &input))
            .is_err();
        let mut reference: Option<String> = None;
        let orders: &[(&str, Order)] = if rejected { &[] } else { &ORDERS };
        for &(name, order) in orders {
            let editor = build(Structure::On, vec![]);
            if let Err(e) = load_in_order(&editor, &input, order, seed, false, false) {
                failures.fail(
                    &format!("throws (empty inline, {name})"),
                    seed,
                    &error_string(&e),
                );
                break;
            }
            match &reference {
                None => {
                    reference = Some(state_json(&editor));
                    if !keeps_content(&tokens(&editor), &tokens(&as_built), &tokens(&injected_off))
                    {
                        failures.fail("content lost (empty inline)", seed, "");
                    }
                }
                Some(reference) => {
                    if &state_json(&editor) != reference {
                        failures.fail(
                            &format!("result depends on dirty order (empty inline, {name})"),
                            seed,
                            "",
                        );
                    }
                }
            }
        }
    }
    // A caret anywhere in the document keeps its place in the content.
    check_carets(&input, seed, u64::from(seed) * 7727, "", failures);
    // The document as an editor without enforcement leaves it, which is
    // then turned on: every leaf kept exactly once, in order, in a block of
    // the same kind, and the same result as loading it with enforcement.
    {
        let editor = build(Structure::Disabled, vec![]);
        load(&editor, &plain).unwrap();
        let before = leaves(&editor);
        turn_on(&editor);
        if state_json(&editor) != want {
            failures.fail(
                "result differs when enforcement is turned on later",
                seed,
                "",
            );
        }
        let after: Vec<Leaf> = leaves(&editor)
            .into_iter()
            .filter(|l| l.desc != "linebreak" || before.iter().any(|b| b.key == l.key))
            .collect();
        let describe_leaves = |ls: &[Leaf]| {
            ls.iter()
                .map(|l| format!("{}:{}", l.key, l.desc))
                .collect::<Vec<_>>()
                .join(" ")
        };
        if describe_leaves(&after) != describe_leaves(&before) {
            failures.fail("leaf lost, duplicated or reordered", seed, "");
        }
        let by_key: HashMap<&str, &Leaf> = after.iter().map(|l| (l.key.as_str(), l)).collect();
        for b in &before {
            let a = by_key.get(b.key.as_str());
            if let Some(a) = a
                && a.kind != b.kind
                && !(b.kind.as_deref() == Some("list") && a.kind.as_deref() == Some("listitem"))
            {
                let kind = |k: &Option<String>| k.clone().unwrap_or("null".to_owned());
                failures.fail(
                    "block kind lost",
                    seed,
                    &format!("{}: {} -> {}", b.desc, kind(&b.kind), kind(&a.kind)),
                );
                break;
            }
            if let Some(a) = a
                && b.in_table
                && !a.in_table
            {
                failures.fail("left its table", seed, &b.desc);
                break;
            }
        }
    }
    // The document as it is, in an editor that turns enforcement on before
    // any transform ran: the same result.
    {
        let editor = build(Structure::Disabled, vec![]);
        load_as_is(&editor, &input, nothing).unwrap();
        turn_on(&editor);
        if let Some(reference) = &as_is_reference
            && state_json(&editor) != state_json(reference)
        {
            failures.fail(
                "result differs when enforcement is turned on (as is)",
                seed,
                "",
            );
        }
    }
    // The internal entry point that normalizes a subtree now: on the root of
    // the document as an editor without enforcement leaves it, and on the
    // document as it is, in an update that runs no transform (list numbers,
    // which the list's own transform recomputes, are left out).
    {
        let editor = build(Structure::Off, vec![]);
        load(&editor, &plain).unwrap();
        editor
            .update(
                || internal_normalize_node_structure(&get_root()),
                discrete(),
            )
            .unwrap();
        if state_json(&editor) != want {
            failures.fail("INTERNAL_$normalizeNodeStructure differs", seed, "");
        }
        let as_is = build(Structure::Off, vec![]);
        load_as_is(&as_is, &input, || {
            internal_normalize_node_structure(&get_root())
        })
        .unwrap();
        if let Some(reference) = &as_is_reference
            && without_derived(&as_is) != without_derived(reference)
        {
            failures.fail("INTERNAL_$normalizeNodeStructure differs (as is)", seed, "");
        }
        let remaining = violations(&as_is);
        if !remaining.is_empty() {
            failures.fail(
                "not a fixed point (INTERNAL_$normalizeNodeStructure, as is)",
                seed,
                &remaining.join(", "),
            );
        }
    }
    // A local edit that breaks the normalized document, where only the nodes
    // it writes are dirty, gives what loading the edited document gives,
    // with every node dirty (as if it arrived unrepaired).
    check_local_edit(&want, u64::from(seed), "", failures, seed);
    let with_inline = build(
        Structure::On,
        vec![normalize_inline_elements_extension().into()],
    );
    match load(&with_inline, &plain) {
        Ok(()) => {
            if tokens(&with_inline) != tokens(&off) {
                failures.fail("content lost with NormalizeInlineElements", seed, "");
            }
        }
        Err(e) => failures.fail(
            "throws with NormalizeInlineElements",
            seed,
            &error_string(&e),
        ),
    }
}

fn env_number(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureProperty.test.ts::NodeStructureExtension properties > random broken documents (200 seeds)
#[test]
fn random_broken_documents_200_seeds() {
    let seeds = env_number("NODE_STRUCTURE_SEEDS", 200);
    let first_seed = env_number("NODE_STRUCTURE_FIRST_SEED", 1);
    let failures = Failures::default();
    let mut checked = 0;
    let mut generated_checked = 0;
    for seed in first_seed..first_seed + seeds {
        dispose_built();
        generated_checked += 1;
        check_seed(seed, &failures, &mut checked);
    }
    dispose_built();
    let summary: Vec<String> = failures
        .0
        .borrow()
        .iter()
        .map(|(k, v)| {
            format!(
                "{k}: {{count: {}, first: {:?}}}",
                v.len(),
                &v[..v.len().min(3)]
            )
        })
        .collect();
    if std::env::var("NODE_STRUCTURE_SEEDS").is_ok() {
        println!(
            "node structure property: {checked} documents checked, {generated_checked} generated, failures: {summary:#?}"
        );
    }
    assert!(
        checked as f64 > f64::from(seeds) * 0.75,
        "{checked} documents checked"
    );
    assert_eq!(summary, Vec::<String>::new());
}
