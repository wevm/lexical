//! Ports of
//! `packages/lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts`.
//!
//! Upstream generates its tests with nested loops; each `describe` here is
//! one table-driven test that runs the same loops, in the same order, and
//! carries one marker per case. The markers come from
//! `swift/parity/upstream-tests.json`, and each test checks that the ids it
//! computes are exactly its markers ([`check_markers`]).

#![cfg(all(
    feature = "extension",
    feature = "link",
    feature = "list",
    feature = "mark",
    feature = "overflow",
    feature = "rich-text"
))]

use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, node_structure_extension,
};
use crate::link::{LinkAttributes, LinkNode, create_link_node};
use crate::list::{ListItemNode, ListNode, ListType, create_list_item_node, create_list_node};
use crate::mark::{MarkNode, create_mark_node};
use crate::nodes::element::ElementData;
use crate::overflow::{OverflowNode, create_overflow_node};
use crate::rich_text::{QuoteNode, create_quote_node};
use crate::selection::wrap_nodes;
use crate::upstream_tests::support::{
    TestShadowRootNode, create_test_shadow_root_node, panic_message, update, update_with,
};
use crate::{
    LexicalExtension, LexicalNode, NodeBase, NodeClass, NodeRef, PointType, Which,
    create_paragraph_node, create_range_selection, create_text_node, define_extension, get_root,
    get_selection, get_slot, set_selection, set_slot,
};

const FILE: &str = "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts";

/// The ids marked above the test named `test_fn` in this file.
fn markers_of(test_fn: &str) -> Vec<String> {
    let source = include_str!("wrap_nodes_inline_content.rs");
    let lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .position(|l| *l == format!("fn {test_fn}() {{"))
        .expect("the test function");
    let mut start = at;
    while start > 0 && !lines[start - 1].is_empty() {
        start -= 1;
    }
    lines[start..at]
        .iter()
        .filter_map(|l| l.strip_prefix("// upstream: "))
        .map(str::to_owned)
        .collect()
}

/// Checks that the ids a table-driven test ran are exactly its markers.
fn check_markers(test_fn: &str, describe: &str, names: &[String]) {
    let ids: Vec<String> = names
        .iter()
        .map(|name| format!("{FILE}::{describe} > {name}"))
        .collect();
    assert_eq!(markers_of(test_fn), ids);
}

/// Runs each case, then fails listing every case that failed.
fn run_cases(cases: Vec<(String, Box<dyn FnOnce()>)>) -> Vec<String> {
    let mut names = Vec::new();
    let mut failed = Vec::new();
    for (name, case) in cases {
        if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(case)) {
            failed.push(format!("{name}: {}", panic_message(&*payload)));
        }
        names.push(name);
    }
    assert!(
        failed.is_empty(),
        "{} of {} cases failed:\n{}",
        failed.len(),
        names.len(),
        failed.join("\n")
    );
    names
}

fn build(name: &str, structure: bool, nodes: Vec<NodeClass>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: if structure {
            vec![node_structure_extension().into()]
        } else {
            Vec::new()
        },
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn structure_suffix(structure: bool) -> &'static str {
    if structure {
        ", NodeStructureExtension"
    } else {
        ""
    }
}

fn backward_suffix(backward: bool) -> &'static str {
    if backward { ", backward" } else { "" }
}

fn quote() -> NodeRef {
    create_quote_node(false)
}

fn paragraph_of(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

fn text(text: &str) -> NodeRef {
    create_text_node(text)
}

fn token(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.set_mode("token").unwrap();
    node
}

fn set_point(selection: &crate::SelectionRef, which: Which, node: &NodeRef, offset: usize) {
    let point_type = if node.is_element() {
        PointType::Element
    } else {
        PointType::Text
    };
    selection
        .point_set(which, node.get_key(), offset, point_type)
        .unwrap();
}

/// `$content(node)`: a block's content as a string: types, nesting and text.
fn content(node: &NodeRef) -> String {
    if node.is_element() {
        let children: Vec<String> = node.get_children().iter().map(content).collect();
        format!("{}({})", node.get_type(), children.join(","))
    } else if node.is_text() {
        crate::json::Value::from(node.get_text_content()).to_string()
    } else {
        node.get_type().to_owned()
    }
}

/// `$outside(node, scopeKey)`: [`content`] with the scope's own content left
/// out.
fn outside(node: &NodeRef, scope_key: &str) -> String {
    if node.get_key() == scope_key {
        return "scope".to_owned();
    }
    if node.is_element() {
        let children: Vec<String> = node
            .get_children()
            .iter()
            .map(|child| outside(child, scope_key))
            .collect();
        return format!("{}({})", node.get_type(), children.join(","));
    }
    content(node)
}

/// `$textOf(parent, text)`.
fn text_of(parent: &NodeRef, text: &str) -> NodeRef {
    parent
        .get_all_text_nodes()
        .into_iter()
        .find(|node| node.get_text_content() == text)
        .unwrap_or_else(|| panic!("no text node {text}"))
}

/// `$texts(node)`: the text nodes under `node`, in document order, as
/// key:text.
fn texts(node: &NodeRef) -> Vec<String> {
    if node.is_element() {
        node.get_children().iter().flat_map(texts).collect()
    } else if node.is_text() {
        vec![format!("{}:{}", node.get_key(), node.get_text_content())]
    } else {
        Vec::new()
    }
}

/// The texts of the root's text nodes.
fn root_texts(editor: &LexicalEditorWithDispose) -> Vec<String> {
    editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(NodeRef::get_text_content)
                .collect()
        })
        .unwrap()
}

/// Whether `point` is attached and within its node's size.
fn point_in_range(point: &crate::Point) -> bool {
    let node = point.get_node().unwrap();
    node.is_attached()
        && point.offset
            <= if node.is_element() {
                node.get_children_size()
            } else {
                node.get_text_content_size()
            }
}

/// `InlineShadowNode` (`test_wrap_inline_shadow`): an inline shadow root
/// that can be empty.
#[derive(Clone, Debug, Default)]
struct InlineShadowNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for InlineShadowNode {
    fn get_type(&self) -> &'static str {
        "test_wrap_inline_shadow"
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
    fn is_inline(&self) -> bool {
        true
    }
    fn is_shadow_root(&self) -> bool {
        true
    }
    fn can_be_empty(&self) -> bool {
        true
    }
}

// $wrapNodes keeps the inline content of the blocks it wraps (#9612)

const WRAPPERS: [&str; 3] = ["link", "mark", "overflow"];

/// `nestings(depth)`.
fn nestings(depth: usize) -> Vec<Vec<&'static str>> {
    if depth == 0 {
        return vec![Vec::new()];
    }
    nestings(depth - 1)
        .into_iter()
        .flat_map(|rest| {
            WRAPPERS.map(|w| {
                let mut shape = vec![w];
                shape.extend(rest.iter().copied());
                shape
            })
        })
        .collect()
}

fn wrapper(kind: &str) -> NodeRef {
    match kind {
        "link" => create_link_node("https://example.net", &LinkAttributes::default()),
        "mark" => create_mark_node(&["c1".to_owned()]),
        _ => create_overflow_node(),
    }
}

/// `$paragraph(shape)`.
fn paragraph_with(shape: &[&str]) -> NodeRef {
    let mut inner = text("inner");
    for kind in shape.iter().rev() {
        let w = wrapper(kind);
        w.append(&[inner]).unwrap();
        inner = w;
    }
    paragraph_of(&[text("abc "), inner, text(" end")])
}

const WHERE: [&str; 5] = [
    "caret before the wrappers",
    "caret inside the inner text",
    "the whole paragraph",
    "from the inner text to the end",
    "this paragraph and the next",
];

fn inline_content_case(
    structure: bool,
    wrapping: bool,
    context: &'static str,
    at: &'static str,
    shape: Vec<&'static str>,
) {
    let editor = build(
        "@wrapNodes-inline-content-test",
        structure,
        vec![
            NodeClass::of::<LinkNode>(),
            NodeClass::of::<MarkNode>(),
            NodeClass::of::<OverflowNode>(),
            NodeClass::of::<QuoteNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
    );
    let in_root = context == "root";
    update(&editor, move || {
        get_root().clear()?;
        let first = paragraph_with(&shape);
        let second = paragraph_of(&[text("next")]);
        let holder = if in_root {
            get_root()
        } else {
            create_test_shadow_root_node()
        };
        holder.append(&[first, second])?;
        if !in_root {
            get_root().append(&[holder])?;
        }
        Ok(())
    });
    let expected = update_with(&editor, move || {
        let holder = if in_root {
            get_root()
        } else {
            get_root().get_first_child_or_throw()?
        };
        let first = holder.get_first_child_or_throw()?;
        let second = first.get_next_sibling().unwrap();
        let inner = text_of(&first, "inner");
        let abc = text_of(&first, "abc ");
        let end = text_of(&first, " end");
        let selection = create_range_selection();
        let (anchor, focus) = match at {
            "caret before the wrappers" => ((abc.clone(), 2), (abc, 2)),
            "caret inside the inner text" => ((inner.clone(), 2), (inner, 2)),
            "the whole paragraph" => ((abc, 0), (end, 4)),
            "from the inner text to the end" => ((inner, 2), (end, 4)),
            _ => ((inner, 2), (text_of(&second, "next"), 2)),
        };
        set_point(&selection, Which::Anchor, &anchor.0, anchor.1);
        set_point(&selection, Which::Focus, &focus.0, focus.1);
        set_selection(Some(selection.clone()));
        let wraps = if at == "this paragraph and the next" {
            vec![first.clone(), second.clone()]
        } else {
            vec![first.clone()]
        };
        let quoted = wraps
            .iter()
            .map(|block| {
                let children: Vec<String> = block.get_children().iter().map(content).collect();
                format!("quote({})", children.join(","))
            })
            .collect::<Vec<_>>()
            .join(",");
        let rest = if wraps.len() == 1 {
            vec![content(&second)]
        } else {
            Vec::new()
        };
        let mut expected = vec![if wrapping {
            format!("quote({quoted})")
        } else {
            quoted
        }];
        expected.extend(rest);
        let wrapping_element = wrapping.then(quote);
        wrap_nodes(&selection, quote, wrapping_element.as_ref())?;
        Ok(expected.join(","))
    });
    let actual = editor
        .read(|| {
            let holder = if in_root {
                get_root()
            } else {
                get_root().get_first_child_or_throw().unwrap()
            };
            holder
                .get_children()
                .iter()
                .map(content)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap();
    assert_eq!(actual, expected);
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret before the wrappers, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret inside the inner text, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, the whole paragraph, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, from the inner text to the end, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, this paragraph and the next, in the root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret before the wrappers, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret inside the inner text, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, the whole paragraph, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, from the inner text to the end, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, this paragraph and the next, in the shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret before the wrappers, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret inside the inner text, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, the whole paragraph, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, from the inner text to the end, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, this paragraph and the next, in the root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret before the wrappers, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret inside the inner text, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, the whole paragraph, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, from the inner text to the end, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, this paragraph and the next, in the shadow root, with a wrapping element
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret before the wrappers, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret inside the inner text, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, the whole paragraph, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, from the inner text to the end, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, this paragraph and the next, in the root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret before the wrappers, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, caret inside the inner text, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, the whole paragraph, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, from the inner text to the end, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > link, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > mark, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > link > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > link > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > link > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > mark > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > mark > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > mark > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > link > overflow > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > mark > overflow > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps the inline content of the blocks it wraps (#9612) > overflow > overflow > overflow, this paragraph and the next, in the shadow root, NodeStructureExtension
#[test]
fn wrap_nodes_keeps_the_inline_content_of_the_blocks_it_wraps() {
    let shapes: Vec<Vec<&str>> = (1..=3).flat_map(nestings).collect();
    let mut cases: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for structure in [false, true] {
        let wrappings: &[bool] = if structure { &[false] } else { &[false, true] };
        for &wrapping in wrappings {
            for context in ["root", "shadow root"] {
                for at in WHERE {
                    for shape in &shapes {
                        let name = format!(
                            "{}, {at}, in the {context}{}{}",
                            shape.join(" > "),
                            structure_suffix(structure),
                            if wrapping {
                                ", with a wrapping element"
                            } else {
                                ""
                            }
                        );
                        let shape = shape.clone();
                        cases.push((
                            name,
                            Box::new(move || {
                                inline_content_case(structure, wrapping, context, at, shape);
                            }),
                        ));
                    }
                }
            }
        }
    }
    let names = run_cases(cases);
    check_markers(
        "wrap_nodes_keeps_the_inline_content_of_the_blocks_it_wraps",
        "$wrapNodes keeps the inline content of the blocks it wraps (#9612)",
        &names,
    );
}

// $wrapNodes keeps a selection in a shadow root inside that scope

const SCOPES: [&str; 4] = [
    "inline, in the paragraph",
    "inline, in a mark in the paragraph",
    "block, at the root",
    "block, the paragraph's slot value",
];

fn shadow_scope_case(structure: bool, scope: &'static str, held: &'static str, at: &'static str) {
    let editor = build(
        "@wrapNodes-shadow-scope-test",
        structure,
        vec![
            NodeClass::of::<InlineShadowNode>(),
            NodeClass::of::<MarkNode>(),
            NodeClass::of::<QuoteNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
    );
    let filled = held != "empty";
    let (outside_before, parent_key, scope_key) = update_with(&editor, move || {
        get_root().clear()?;
        let shadow = if scope.starts_with("inline") {
            crate::create::<InlineShadowNode>()
        } else {
            create_test_shadow_root_node()
        };
        if held == "holding a paragraph" {
            shadow.append(&[paragraph_of(&[text("inside")])])?;
        } else if held == "holding text" {
            shadow.append(&[text("inside")])?;
        }
        // Token mode keeps the two outside texts apart when nothing is
        // left between them.
        let parent = paragraph_of(&[token("outside before")]);
        if scope == "inline, in the paragraph" {
            parent.append(std::slice::from_ref(&shadow))?;
        } else if scope == "inline, in a mark in the paragraph" {
            let mark = create_mark_node(&["c1".to_owned()]);
            mark.append(std::slice::from_ref(&shadow))?;
            parent.append(&[mark])?;
        } else if scope.ends_with("slot value") {
            set_slot(&parent, "scope", &shadow)?;
        }
        parent.append(&[token("outside after")])?;
        get_root().append(std::slice::from_ref(&parent))?;
        if scope == "block, at the root" {
            get_root().append(std::slice::from_ref(&shadow))?;
        }
        let scope_key = shadow.get_key().to_owned();
        Ok((
            outside(&parent, &scope_key),
            parent.get_key().to_owned(),
            scope_key,
        ))
    });
    let key = scope_key.clone();
    let expected_slot = scope_key.clone();
    update(&editor, move || {
        let shadow = crate::get_node_by_key(&key).unwrap();
        let selection = create_range_selection();
        if !filled {
            set_point(&selection, Which::Anchor, &shadow, 0);
            set_point(&selection, Which::Focus, &shadow, 0);
        } else {
            let inside = text_of(&shadow, "inside");
            let caret = at == "caret in the inner text";
            set_point(
                &selection,
                Which::Anchor,
                &inside,
                if caret { 3 } else { 0 },
            );
            set_point(&selection, Which::Focus, &inside, if caret { 3 } else { 6 });
        }
        set_selection(Some(selection.clone()));
        wrap_nodes(&selection, quote, None)
    });
    let (attached, outside_after, slot, shadow_content, shadow_type) = editor
        .read(move || {
            let parent = crate::get_node_by_key(&parent_key).unwrap();
            let shadow = crate::get_node_by_key(&scope_key).unwrap();
            (
                parent.is_attached(),
                outside(&parent, &scope_key),
                get_slot(&parent, "scope").map(|n| n.get_key().to_owned()),
                content(&shadow),
                shadow.get_type(),
            )
        })
        .unwrap();
    assert!(attached);
    assert_eq!(outside_after, outside_before);
    if scope.ends_with("slot value") {
        assert_eq!(slot, Some(expected_slot));
    }
    assert_eq!(
        shadow_content,
        format!(
            "{shadow_type}(quote({}))",
            if filled { "\"inside\"" } else { "" }
        )
    );
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, empty, caret in the empty scope
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding a paragraph, caret in the inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding a paragraph, the whole inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding text, caret in the inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding text, the whole inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, empty, caret in the empty scope
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, holding a paragraph, caret in the inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, holding a paragraph, the whole inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, holding text, caret in the inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, holding text, the whole inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, empty, caret in the empty scope
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding a paragraph, caret in the inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding a paragraph, the whole inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding text, caret in the inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding text, the whole inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, empty, caret in the empty scope
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding a paragraph, caret in the inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding a paragraph, the whole inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding text, caret in the inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding text, the whole inner text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, empty, caret in the empty scope, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding a paragraph, caret in the inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding a paragraph, the whole inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding text, caret in the inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding text, the whole inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, empty, caret in the empty scope, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, holding a paragraph, caret in the inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, holding a paragraph, the whole inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, holding text, caret in the inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in a mark in the paragraph, holding text, the whole inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, empty, caret in the empty scope, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding a paragraph, caret in the inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding a paragraph, the whole inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding text, caret in the inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding text, the whole inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, empty, caret in the empty scope, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding a paragraph, caret in the inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding a paragraph, the whole inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding text, caret in the inner text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding text, the whole inner text, NodeStructureExtension
#[test]
fn wrap_nodes_keeps_a_selection_in_a_shadow_root_inside_that_scope() {
    let mut cases: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for structure in [false, true] {
        for scope in SCOPES {
            for held in ["empty", "holding a paragraph", "holding text"] {
                let places: &[&'static str] = if held != "empty" {
                    &["caret in the inner text", "the whole inner text"]
                } else {
                    &["caret in the empty scope"]
                };
                for &at in places {
                    cases.push((
                        format!("{scope}, {held}, {at}{}", structure_suffix(structure)),
                        Box::new(move || shadow_scope_case(structure, scope, held, at)),
                    ));
                }
            }
        }
    }
    let names = run_cases(cases);
    check_markers(
        "wrap_nodes_keeps_a_selection_in_a_shadow_root_inside_that_scope",
        "$wrapNodes keeps a selection in a shadow root inside that scope",
        &names,
    );
}

// $wrapNodes over a block container that mixes inline content and blocks

/// A part of a mix: direct text, or a paragraph holding it.
#[derive(Clone, Copy)]
enum Part {
    Text(&'static str),
    Paragraph(&'static str),
}

use Part::{Paragraph as P, Text as T};

const MIXES: [&[Part]; 6] = [
    &[T("AA"), P("BB"), T("CC")],
    &[P("AA"), T("BB"), P("CC")],
    &[T("AA"), T("BB"), P("CC"), T("DD")],
    &[P("AA"), P("BB"), T("CC")],
    &[T("AA"), T("BB"), T("CC")],
    &[P("AA"), P("BB"), P("CC")],
];

const CONTAINERS: [&str; 3] = [
    "block shadow root at the root",
    "block shadow root, a paragraph's slot value",
    "inline shadow root in a paragraph",
];

const SPANS: [&str; 3] = [
    "all of it",
    "first text to the middle",
    "middle to the last text",
];

fn mixed_container_case(
    structure: bool,
    container: &'static str,
    mix: &'static [Part],
    span: &'static str,
    backward: bool,
) {
    let editor = build(
        "@wrapNodes-mixed-container-test",
        structure,
        vec![
            NodeClass::of::<InlineShadowNode>(),
            NodeClass::of::<MarkNode>(),
            NodeClass::of::<QuoteNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
    );
    let scope_key = update_with(&editor, move || {
        get_root().clear()?;
        let scope = if container.starts_with("inline") {
            crate::create::<InlineShadowNode>()
        } else {
            create_test_shadow_root_node()
        };
        for (i, part) in mix.iter().enumerate() {
            match *part {
                Part::Text(t) => scope.append(&[token(t)])?,
                Part::Paragraph(t) => {
                    let t = token(t);
                    let child = if i == 1 {
                        let mark = create_mark_node(&["c1".to_owned()]);
                        mark.append(&[t])?;
                        mark
                    } else {
                        t
                    };
                    scope.append(&[paragraph_of(&[child])])?;
                }
            }
        }
        let host = paragraph_of(&[token("outside")]);
        if container == "block shadow root at the root" {
            get_root().append(&[host, scope.clone()])?;
        } else if container.starts_with("inline") {
            host.append(std::slice::from_ref(&scope))?;
            get_root().append(&[host])?;
        } else {
            set_slot(&host, "body", &scope)?;
            get_root().append(&[host])?;
        }
        Ok(scope.get_key().to_owned())
    });
    let key = scope_key.clone();
    let before = update_with(&editor, move || {
        let scope = crate::get_node_by_key(&key).unwrap();
        let before = texts(&scope);
        let all = scope.get_all_text_nodes();
        let middle = all[all.len() / 2].clone();
        let last = all[all.len() - 1].clone();
        let (start, end) = match span {
            "all of it" => (all[0].clone(), last),
            "first text to the middle" => (all[0].clone(), middle),
            _ => (middle, last),
        };
        let selection = create_range_selection();
        let (anchor, focus) = if backward { (end, start) } else { (start, end) };
        let anchor_offset = if backward {
            anchor.get_text_content_size()
        } else {
            0
        };
        let focus_offset = if backward {
            0
        } else {
            focus.get_text_content_size()
        };
        set_point(&selection, Which::Anchor, &anchor, anchor_offset);
        set_point(&selection, Which::Focus, &focus, focus_offset);
        set_selection(Some(selection.clone()));
        wrap_nodes(&selection, quote, None)?;
        Ok(before)
    });
    let (attached, after) = editor
        .read(move || {
            let scope = crate::get_node_by_key(&scope_key).unwrap();
            (scope.is_attached(), texts(&scope))
        })
        .unwrap();
    assert!(attached);
    assert_eq!(after, before);
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, all of it
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, all of it, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, first text to the middle
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, first text to the middle, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, middle to the last text
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, middle to the last text, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 0, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 3, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 5, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 0, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 3, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 5, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 0, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 3, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, middle to the last text, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, all of it, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, all of it, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, first text to the middle, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, first text to the middle, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, middle to the last text, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 5, middle to the last text, backward, NodeStructureExtension
#[test]
fn wrap_nodes_over_a_block_container_that_mixes_inline_content_and_blocks() {
    let mut cases: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for structure in [false, true] {
        for container in CONTAINERS {
            for (m, mix) in MIXES.into_iter().enumerate() {
                for span in SPANS {
                    for backward in [false, true] {
                        cases.push((
                            format!(
                                "{container}, mix {m}, {span}{}{}",
                                backward_suffix(backward),
                                structure_suffix(structure)
                            ),
                            Box::new(move || {
                                mixed_container_case(structure, container, mix, span, backward);
                            }),
                        ));
                    }
                }
            }
        }
    }
    let names = run_cases(cases);
    check_markers(
        "wrap_nodes_over_a_block_container_that_mixes_inline_content_and_blocks",
        "$wrapNodes over a block container that mixes inline content and blocks",
        &names,
    );
}

// $wrapNodes over a range from a list item to an element point on its list

fn list_point_case(structure: bool, item: usize, offset: usize, backward: bool) {
    let editor = build(
        "@wrapNodes-list-point-test",
        structure,
        vec![
            NodeClass::of::<ListNode>(),
            NodeClass::of::<ListItemNode>(),
            NodeClass::of::<QuoteNode>(),
        ],
    );
    update(&editor, move || {
        let list = create_list_node(ListType::Bullet, 1.0);
        for t in ["aa", "bb", "cc"] {
            let list_item = create_list_item_node(None);
            list_item.append(&[text(t)])?;
            list.append(&[list_item])?;
        }
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[text("before")]), list.clone()])?;
        let text_node = list
            .get_child_at_index(item)
            .unwrap()
            .get_first_descendant()
            .unwrap();
        let selection = create_range_selection();
        let (text_point, list_point) = if backward {
            (Which::Focus, Which::Anchor)
        } else {
            (Which::Anchor, Which::Focus)
        };
        set_point(&selection, text_point, &text_node, 1);
        set_point(&selection, list_point, &list, offset);
        set_selection(Some(selection.clone()));
        wrap_nodes(&selection, quote, None)
    });
    assert_eq!(root_texts(&editor), ["before", "aa", "bb", "cc"]);
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 0
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 0, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 1
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 1, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 2
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 2, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 3
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 3, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 0
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 0, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 1
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 1, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 2
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 2, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 3
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 3, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 0
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 0, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 1
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 1, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 2
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 2, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 3
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 3, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 0, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 0, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 1, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 1, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 2, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 2, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 3, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 3, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 0, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 0, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 1, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 1, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 2, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 2, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 3, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 3, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 0, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 0, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 1, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 1, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 2, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 2, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 3, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 3, backward, NodeStructureExtension
#[test]
fn wrap_nodes_over_a_range_from_a_list_item_to_an_element_point_on_its_list() {
    let mut cases: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for structure in [false, true] {
        for item in [0, 1, 2] {
            for offset in [0, 1, 2, 3] {
                for backward in [false, true] {
                    cases.push((
                        format!(
                            "item {item}, list offset {offset}{}{}",
                            backward_suffix(backward),
                            structure_suffix(structure)
                        ),
                        Box::new(move || list_point_case(structure, item, offset, backward)),
                    ));
                }
            }
        }
    }
    let names = run_cases(cases);
    check_markers(
        "wrap_nodes_over_a_range_from_a_list_item_to_an_element_point_on_its_list",
        "$wrapNodes over a range from a list item to an element point on its list",
        &names,
    );
}

// $wrapNodes over a range that starts at the end of a block

const ENDS: [&str; 4] = [
    "text in a paragraph",
    "an empty paragraph",
    "an empty paragraph in a shadow root",
    "text in a shadow root",
];

fn start_at_block_end_case(
    structure: bool,
    first: &'static str,
    end: &'static str,
    backward: bool,
) {
    let editor = build(
        "@wrapNodes-start-at-block-end-test",
        structure,
        vec![
            NodeClass::of::<ListNode>(),
            NodeClass::of::<ListItemNode>(),
            NodeClass::of::<QuoteNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
    );
    let (first_key, before) = update_with(&editor, move || {
        let block = if first == "list" {
            let list = create_list_node(ListType::Bullet, 1.0);
            for t in ["aa", "bb"] {
                let list_item = create_list_item_node(None);
                list_item.append(&[text(t)])?;
                list.append(&[list_item])?;
            }
            list
        } else {
            paragraph_of(&[text("aa bb")])
        };
        let empty = create_paragraph_node();
        let ee = text("ee");
        let mut children = vec![
            block.clone(),
            paragraph_of(&[text("cc")]),
            paragraph_of(&[text("dd")]),
        ];
        if end.ends_with("shadow root") {
            let shadow = create_test_shadow_root_node();
            shadow.append(&[empty.clone(), paragraph_of(std::slice::from_ref(&ee))])?;
            children.push(shadow);
        } else {
            children.extend([empty.clone(), paragraph_of(std::slice::from_ref(&ee))]);
        }
        children.push(paragraph_of(&[text("ff")]));
        get_root().clear()?;
        get_root().append(&children)?;
        let before = texts(&get_root());
        let selection = create_range_selection();
        let (at_end, other) = if backward {
            (Which::Focus, Which::Anchor)
        } else {
            (Which::Anchor, Which::Focus)
        };
        set_point(&selection, at_end, &block, block.get_children_size());
        if end.starts_with("text") {
            set_point(&selection, other, &ee, 1);
        } else {
            set_point(&selection, other, &empty, 0);
        }
        set_selection(Some(selection.clone()));
        wrap_nodes(&selection, quote, None)?;
        Ok((block.get_key().to_owned(), before))
    });
    let (after, block_key, block_type) = editor
        .read(|| {
            let block = get_root().get_first_child_or_throw().unwrap();
            (
                texts(&get_root()),
                block.get_key().to_owned(),
                block.get_type(),
            )
        })
        .unwrap();
    assert_eq!(after, before);
    assert_eq!(block_key, first_key);
    assert_eq!(block_type, first);
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a paragraph
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a paragraph, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph in a shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph in a shadow root, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a shadow root, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a paragraph
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a paragraph, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph in a shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph in a shadow root, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a shadow root
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a shadow root, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a paragraph, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a paragraph, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph in a shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph in a shadow root, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a shadow root, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a paragraph, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a paragraph, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph in a shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph in a shadow root, backward, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a shadow root, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a shadow root, backward, NodeStructureExtension
#[test]
fn wrap_nodes_over_a_range_that_starts_at_the_end_of_a_block() {
    let mut cases: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for structure in [false, true] {
        for first in ["list", "paragraph"] {
            for end in ENDS {
                for backward in [false, true] {
                    cases.push((
                        format!(
                            "{first}, to {end}{}{}",
                            backward_suffix(backward),
                            structure_suffix(structure)
                        ),
                        Box::new(move || start_at_block_end_case(structure, first, end, backward)),
                    ));
                }
            }
        }
    }
    let names = run_cases(cases);
    check_markers(
        "wrap_nodes_over_a_range_that_starts_at_the_end_of_a_block",
        "$wrapNodes over a range that starts at the end of a block",
        &names,
    );
}

// $wrapNodes leaves the selection inside the new tree

fn selection_restore_case(structure: bool, backward: bool) {
    let editor = build(
        "@wrapNodes-selection-restore-test",
        structure,
        vec![NodeClass::of::<QuoteNode>()],
    );
    let points = update_with(&editor, move || {
        let paragraphs: Vec<NodeRef> = ["aa", "bb", "cc"]
            .iter()
            .map(|t| paragraph_of(&[text(t)]))
            .collect();
        get_root().clear()?;
        get_root().append(&paragraphs)?;
        let selection = create_range_selection();
        let (text_point, root_point) = if backward {
            (Which::Focus, Which::Anchor)
        } else {
            (Which::Anchor, Which::Focus)
        };
        set_point(
            &selection,
            text_point,
            &paragraphs[0].get_first_child_or_throw()?,
            0,
        );
        set_point(&selection, root_point, &get_root(), 3);
        set_selection(Some(selection.clone()));
        wrap_nodes(&selection, quote, None)?;
        let after = get_selection();
        let is_range = after.as_ref().is_some_and(crate::SelectionRef::is_range);
        let points: Vec<bool> = after
            .filter(crate::SelectionRef::is_range)
            .map(|after| {
                [Which::Anchor, Which::Focus]
                    .map(|which| point_in_range(&after.point(which).unwrap()))
                    .to_vec()
            })
            .unwrap_or_default();
        Ok((is_range, points))
    });
    assert!(points.0);
    assert_eq!(points.1, [true, true]);
    assert_eq!(root_texts(&editor), ["aa", "bb", "cc"]);
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes leaves the selection inside the new tree > three root paragraphs, from the first text to the root's end
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes leaves the selection inside the new tree > three root paragraphs, from the first text to the root's end, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes leaves the selection inside the new tree > three root paragraphs, from the first text to the root's end, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes leaves the selection inside the new tree > three root paragraphs, from the first text to the root's end, backward, NodeStructureExtension
#[test]
fn wrap_nodes_leaves_the_selection_inside_the_new_tree() {
    let mut cases: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for structure in [false, true] {
        for backward in [false, true] {
            cases.push((
                format!(
                    "three root paragraphs, from the first text to the root's end{}{}",
                    backward_suffix(backward),
                    structure_suffix(structure)
                ),
                Box::new(move || selection_restore_case(structure, backward)),
            ));
        }
    }
    let names = run_cases(cases);
    check_markers(
        "wrap_nodes_leaves_the_selection_inside_the_new_tree",
        "$wrapNodes leaves the selection inside the new tree",
        &names,
    );
}

// $wrapNodes over the whole root, with a wrapping element

fn root_points_case(structure: bool, backward: bool) {
    let editor = build(
        "@wrapNodes-root-points-test",
        structure,
        vec![
            NodeClass::of::<QuoteNode>(),
            NodeClass::of::<TestShadowRootNode>(),
        ],
    );
    let selected = update_with(&editor, move || {
        get_root().clear()?;
        let paragraphs: Vec<NodeRef> = ["aa", "bb", "cc"]
            .iter()
            .map(|t| paragraph_of(&[text(t)]))
            .collect();
        get_root().append(&paragraphs)?;
        let root = get_root();
        let selection = create_range_selection();
        set_point(
            &selection,
            Which::Anchor,
            &root,
            if backward { 3 } else { 0 },
        );
        set_point(
            &selection,
            Which::Focus,
            &root,
            if backward { 0 } else { 3 },
        );
        set_selection(Some(selection.clone()));
        let wrapping = create_test_shadow_root_node();
        wrap_nodes(&selection, quote, Some(&wrapping))?;
        let mut selected = String::new();
        if let Some(after) = get_selection().filter(crate::SelectionRef::is_range) {
            let in_range = [Which::Anchor, Which::Focus]
                .iter()
                .all(|&which| point_in_range(&after.point(which).unwrap()));
            selected = format!(
                "{} {}",
                if after.range().unwrap().is_collapsed() {
                    "collapsed"
                } else {
                    "range"
                },
                if in_range { "in range" } else { "out of range" }
            );
            after.insert_text("Z")?;
        }
        Ok(selected)
    });
    // As on main: a collapsed selection, so typing keeps the wrapped text.
    assert_eq!(selected, "collapsed in range");
    // The typed text goes next to the wrapped text, not over it.
    let all = root_texts(&editor);
    let wrapped: Vec<&String> = all.iter().filter(|t| *t != "Z").collect();
    assert_eq!(wrapped, ["aa", "bb", "cc"]);
    assert!(all.iter().any(|t| t == "Z"));
    editor.dispose();
}

// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over the whole root, with a wrapping element > root element points 0 to 3
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over the whole root, with a wrapping element > root element points 0 to 3, backward
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over the whole root, with a wrapping element > root element points 0 to 3, NodeStructureExtension
// upstream: lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over the whole root, with a wrapping element > root element points 0 to 3, backward, NodeStructureExtension
#[test]
fn wrap_nodes_over_the_whole_root_with_a_wrapping_element() {
    let mut cases: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for structure in [false, true] {
        for backward in [false, true] {
            cases.push((
                format!(
                    "root element points 0 to 3{}{}",
                    backward_suffix(backward),
                    structure_suffix(structure)
                ),
                Box::new(move || root_points_case(structure, backward)),
            ));
        }
    }
    let names = run_cases(cases);
    check_markers(
        "wrap_nodes_over_the_whole_root_with_a_wrapping_element",
        "$wrapNodes over the whole root, with a wrapping element",
        &names,
    );
}
