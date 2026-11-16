//! Ports of
//! `packages/lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts`.
//!
//! Upstream guards against a parent cycle (an update that never returns)
//! by counting `getParent` calls; a Rust method can't be patched, so the
//! property tests run each document's update on a thread of its own and
//! count one that hasn't returned after [`HANG`] as never returning.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, create_horizontal_rule_node,
    horizontal_rule_extension, is_horizontal_rule_node,
};
use crate::nodes::element::ElementData;
use crate::rich_text::{QuoteNode, create_quote_node};
use crate::selection::wrap_nodes;
use crate::upstream_tests::support::{panic_message, update};
use crate::{
    LexicalExtension, LexicalNode, NodeBase, NodeClass, NodeRef, PointType, UpdateOptions, Which,
    create_paragraph_node, create_range_selection, create_text_node, define_extension, get_root,
    set_selection,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// How long a document's update may take before it counts as never
/// returning (one takes milliseconds). The thread of one that never
/// returns spins until the test binary exits; with `direct_text`, 3 of the
/// 500 documents do, as upstream's comment says of JS.
const HANG: Duration = Duration::from_secs(3);

/// `BoxNode` (`test_wrap_box`): a plain block element that holds blocks,
/// with no declared structure.
#[derive(Clone, Debug, Default)]
struct BoxNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for BoxNode {
    fn get_type(&self) -> &'static str {
        "test_wrap_box"
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

fn build() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![horizontal_rule_extension().into()],
        nodes: vec![NodeClass::of::<BoxNode>(), NodeClass::of::<QuoteNode>()],
        ..LexicalExtension::<(), (), ()>::new("wrap-block-leaf", ())
    })
    .into()])
    .unwrap()
}

fn element_of(element: NodeRef, children: &[NodeRef]) -> NodeRef {
    element.append(children).unwrap();
    element
}

fn box_of(children: &[NodeRef]) -> NodeRef {
    element_of(crate::create::<BoxNode>(), children)
}

fn paragraph_of(text: &NodeRef) -> NodeRef {
    element_of(create_paragraph_node(), std::slice::from_ref(text))
}

fn quote() -> NodeRef {
    create_quote_node(false)
}

/// `outline(editor)`: the texts and rules in document order, and the
/// quotes' texts.
fn outline(editor: &LexicalEditorWithDispose) -> (Vec<String>, Vec<String>) {
    editor
        .read(|| {
            fn walk(
                node: &NodeRef,
                in_quote: bool,
                all: &mut Vec<String>,
                quoted: &mut Vec<String>,
            ) {
                if node.is_text() {
                    all.push(node.get_text_content());
                    if in_quote {
                        quoted.push(node.get_text_content());
                    }
                } else if is_horizontal_rule_node(node) {
                    all.push("<hr>".to_owned());
                    if in_quote {
                        quoted.push("<hr>".to_owned());
                    }
                } else if node.is_element() {
                    let quote = in_quote || node.get_type() == "quote";
                    for child in node.get_children() {
                        walk(&child, quote, all, quoted);
                    }
                }
            }
            let (mut all, mut quoted) = (Vec::new(), Vec::new());
            walk(&get_root(), false, &mut all, &mut quoted);
            (all, quoted)
        })
        .unwrap()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).to_owned()).collect()
}

/// The JS LCG in doubles (the product exceeds 2^53 and rounds as in JS).
struct Random(Cell<f64>);

impl Random {
    fn next(&self) -> f64 {
        let seed = (self.0.get() * 1_103_515_245.0 + 12345.0) % 2_147_483_648.0;
        self.0.set(seed);
        seed / 2_147_483_648.0
    }

    fn pick(&self, count: usize) -> usize {
        (self.next() * count as f64).floor() as usize
    }
}

/// `$children(depth)`.
fn random_children(
    random: &Random,
    n: &Cell<usize>,
    direct_text: bool,
    depth: usize,
) -> Vec<NodeRef> {
    let mut out = Vec::new();
    let count = 1 + random.pick(4);
    for _ in 0..count {
        let r = random.next();
        if r < 0.25 {
            out.push(create_horizontal_rule_node());
        } else if r < 0.4 && depth < 2 {
            out.push(box_of(&random_children(random, n, direct_text, depth + 1)));
        } else if direct_text && r < 0.55 && depth > 0 {
            // Text directly in a box, next to its blocks.
            out.push(create_text_node(&format!("t{}", n.get())));
            n.set(n.get() + 1);
        } else {
            out.push(paragraph_of(&create_text_node(&format!("t{}", n.get()))));
            n.set(n.get() + 1);
        }
    }
    out
}

/// One document of `wrapFailures`: builds it from `seed`, sends the seed
/// after its random picks on `seed_out`, and returns the failure, if any.
fn wrap_run(
    run: usize,
    seed: f64,
    direct_text: bool,
    seed_out: std::sync::mpsc::Sender<f64>,
) -> Option<String> {
    let editor = build();
    let before = Rc::new(RefCell::new(Vec::<String>::new()));
    let before_in = before.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        editor.update(
            move || {
                let random = Random(Cell::new(seed));
                let n = Cell::new(0);
                let children = random_children(&random, &n, direct_text, 0);
                get_root().clear()?;
                get_root().append(&children)?;
                *before_in.borrow_mut() = get_root()
                    .get_all_text_nodes()
                    .iter()
                    .map(NodeRef::get_text_content)
                    .collect();
                let mut points: Vec<(String, usize, PointType)> = Vec::new();
                fn collect(node: &NodeRef, points: &mut Vec<(String, usize, PointType)>) {
                    if node.is_text() {
                        for o in 0..=node.get_text_content_size() {
                            points.push((node.get_key().to_owned(), o, PointType::Text));
                        }
                    } else if node.is_element() {
                        for o in 0..=node.get_children_size() {
                            points.push((node.get_key().to_owned(), o, PointType::Element));
                        }
                        for child in node.get_children() {
                            collect(&child, points);
                        }
                    }
                }
                collect(&get_root(), &mut points);
                let a = points[random.pick(points.len())].clone();
                let b = points[random.pick(points.len())].clone();
                let _ = seed_out.send(random.0.get());
                let selection = create_range_selection();
                selection.point_set(Which::Anchor, &a.0, a.1, a.2)?;
                selection.point_set(Which::Focus, &b.0, b.1, b.2)?;
                set_selection(Some(selection.clone()));
                wrap_nodes(&selection, quote, None)
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
    }));
    let error = match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error.to_string()),
        Err(payload) => Some(panic_message(&*payload)),
    };
    let after: Vec<String> = outline(&editor)
        .0
        .into_iter()
        .filter(|t| t != "<hr>")
        .collect();
    // Adjacent texts may merge: compare the text in order.
    let before = before.borrow().concat();
    editor.dispose();
    if error.is_some() || after.concat() != before {
        Some(format!(
            "run {run}: {}",
            error.unwrap_or_else(|| after.join(","))
        ))
    } else {
        None
    }
}

/// `wrapFailures(directText)`: random documents of paragraphs, rules
/// (adjacent ones too), boxes (nested up to twice) and, with
/// `direct_text`, text directly in a box, and random ranges between text
/// and element points: the documents where `wrap_nodes` fails (or never
/// returns) or does not keep every text in order.
fn wrap_failures(direct_text: bool) -> Vec<String> {
    let mut seed = 9638.0;
    let mut failures = Vec::new();
    for run in 0..500 {
        let (seed_in, seed_out) = std::sync::mpsc::channel();
        let (done_in, done_out) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done_in.send(wrap_run(run, seed, direct_text, seed_in));
        });
        seed = seed_out.recv().expect("the document was built");
        match done_out.recv_timeout(HANG) {
            Ok(failure) => failures.extend(failure),
            Err(_) => failures.push(format!("run {run}: never returned")),
        }
    }
    failures
}

/// Runs `setup` in an update, then wraps from the `[anchor, focus]` it
/// returns.
fn wrap(
    editor: &LexicalEditorWithDispose,
    setup: impl FnOnce() -> [(NodeRef, usize); 2] + 'static,
) {
    update(editor, move || {
        let [anchor, focus] = setup();
        let selection = create_range_selection();
        for (which, (node, offset)) in [(Which::Anchor, anchor), (Which::Focus, focus)] {
            let point_type = if node.is_element() {
                PointType::Element
            } else {
                PointType::Text
            };
            selection.point_set(which, node.get_key(), offset, point_type)?;
        }
        set_selection(Some(selection.clone()));
        wrap_nodes(&selection, quote, None)
    });
}

fn set_root(children: &[NodeRef]) {
    get_root().clear().unwrap();
    get_root().append(children).unwrap();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > a range across a horizontal rule in a box keeps every paragraph
#[test]
fn a_range_across_a_horizontal_rule_in_a_box_keeps_every_paragraph() {
    let editor = build();
    wrap(&editor, || {
        let bb = create_text_node("bb");
        let cc = create_text_node("cc");
        set_root(&[box_of(&[
            paragraph_of(&create_text_node("aa")),
            paragraph_of(&bb),
            create_horizontal_rule_node(),
            paragraph_of(&cc),
        ])]);
        [(bb, 1), (cc, 2)]
    });
    assert_eq!(
        outline(&editor),
        (
            strings(&["aa", "bb", "<hr>", "cc"]),
            strings(&["bb", "<hr>", "cc"])
        )
    );
    // The rule stays in the box, next to the paragraphs.
    let types = editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(NodeRef::get_type)
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(types, ["test_wrap_box"]);
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > a range that starts at the rule
#[test]
fn a_range_that_starts_at_the_rule() {
    let editor = build();
    wrap(&editor, || {
        let cc = create_text_node("cc");
        let b = box_of(&[
            paragraph_of(&create_text_node("aa")),
            create_horizontal_rule_node(),
            paragraph_of(&cc),
        ]);
        set_root(std::slice::from_ref(&b));
        [(b, 1), (cc, 2)]
    });
    assert_eq!(
        outline(&editor),
        (strings(&["aa", "<hr>", "cc"]), strings(&["<hr>", "cc"]))
    );
    editor.dispose();
}

/// `a box with text and two rules in it, from the second rule (...)`.
fn a_box_with_text_and_two_rules_in_it_from_the_second_rule(backward: bool) {
    let editor = build();
    wrap(&editor, move || {
        let cc = create_text_node("cc");
        let b = box_of(&[
            create_text_node("aa"),
            create_horizontal_rule_node(),
            create_horizontal_rule_node(),
            cc.clone(),
        ]);
        set_root(std::slice::from_ref(&b));
        let (from, to) = ((b, 2), (cc, 2));
        if backward { [to, from] } else { [from, to] }
    });
    assert_eq!(outline(&editor).0, strings(&["aa", "<hr>", "<hr>", "cc"]));
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > a box with text and two rules in it, from the second rule (forward)
#[test]
fn a_box_with_text_and_two_rules_in_it_from_the_second_rule_forward() {
    a_box_with_text_and_two_rules_in_it_from_the_second_rule(false);
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > a box with text and two rules in it, from the second rule (backward)
#[test]
fn a_box_with_text_and_two_rules_in_it_from_the_second_rule_backward() {
    a_box_with_text_and_two_rules_in_it_from_the_second_rule(true);
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > control: the same range without the rule
#[test]
fn control_the_same_range_without_the_rule() {
    let editor = build();
    wrap(&editor, || {
        let bb = create_text_node("bb");
        let cc = create_text_node("cc");
        set_root(&[box_of(&[
            paragraph_of(&create_text_node("aa")),
            paragraph_of(&bb),
            paragraph_of(&cc),
        ])]);
        [(bb, 1), (cc, 2)]
    });
    assert_eq!(
        outline(&editor),
        (strings(&["aa", "bb", "cc"]), strings(&["bb", "cc"]))
    );
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > control: a rule at the root
#[test]
fn control_a_rule_at_the_root() {
    let editor = build();
    wrap(&editor, || {
        let bb = create_text_node("bb");
        let cc = create_text_node("cc");
        set_root(&[
            paragraph_of(&create_text_node("aa")),
            paragraph_of(&bb),
            create_horizontal_rule_node(),
            paragraph_of(&cc),
        ]);
        [(bb, 1), (cc, 2)]
    });
    assert_eq!(
        outline(&editor),
        (
            strings(&["aa", "bb", "<hr>", "cc"]),
            strings(&["bb", "<hr>", "cc"])
        )
    );
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > property: random box, rule and paragraph mixes
#[test]
fn property_random_box_rule_and_paragraph_mixes() {
    assert_eq!(wrap_failures(false), Vec::<String>::new());
}

// With text directly in a box next to its blocks, $wrapNodes reorders text
// or never returns in some documents on main too (#9665): pinned until that
// is fixed (upstream's `test.fails`).
// upstream: lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > property: with text directly in a box (#9665)
#[test]
fn property_with_text_directly_in_a_box() {
    assert_ne!(wrap_failures(true), Vec::<String>::new());
}
