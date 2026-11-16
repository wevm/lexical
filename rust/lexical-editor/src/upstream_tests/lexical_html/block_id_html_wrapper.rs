//! Ports of `packages/lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts`
//! and `BlockIdHTMLWrapperResolve.test.ts`: `data-block-wrapper-id` (#9606)
//! on import. Upstream counts calls of `ElementNode.prototype.getParent`
//! and `getChildrenSize` with `vi.spyOn`, which is the work the resolve rule
//! does for each waiting id it looks at; a Rust method can't be spied on, so
//! these read `BlockIdHtmlRuntime`'s count of the waiting ids the resolver
//! looked at (as Swift's port does). The bounds are the same: a doubled
//! input costs less than three times as much.

#![cfg(all(feature = "extension", feature = "rich-text", feature = "list"))]

use super::super::support::update;
use super::support::with_large_stack;
use crate::dom::{DomNode, parse_from_string};
use crate::extension::{
    BlockIdConfigOverrides, GenerateId, LexicalEditorWithDispose, block_id_extension,
    build_editor_from_extensions, get_block_id, get_extension_dependency_from_editor,
};
use crate::html::block_id_html_extension;
use crate::html::import::{
    DomImportConfigOverrides, DomImportRule, define_import_rule, dom_import_extension,
    generate_nodes_from_dom_via_extension, sel,
};
use crate::list::{ListType, create_list_item_node, create_list_node, is_list_node};
use crate::{
    AnyLexicalExtensionArgument, Editor, InitialEditorStateType, LexicalError, LexicalExtension,
    NodeRef, config_extension, create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn parse(html: &str) -> DomNode {
    parse_from_string(&format!("<!doctype html><html><body>{html}</body></html>"))
}

fn generate(dom: &DomNode) -> Result<Vec<NodeRef>, LexicalError> {
    generate_nodes_from_dom_via_extension(dom, None)
}

fn fresh_ids() -> GenerateId {
    let n = Cell::new(0);
    GenerateId(Rc::new(move || {
        n.set(n.get() + 1);
        format!("fresh{}", n.get())
    }))
}

fn editor_with(
    name: &str,
    generate_id: Option<GenerateId>,
    rules: Vec<DomImportRule>,
) -> LexicalEditorWithDispose {
    let mut dependencies: Vec<AnyLexicalExtensionArgument> = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::list::list_extension().into(),
        config_extension(
            &block_id_extension(),
            [BlockIdConfigOverrides {
                generate_id,
                ..Default::default()
            }],
        ),
        block_id_html_extension().into(),
    ];
    if !rules.is_empty() {
        dependencies.push(config_extension(
            &dom_import_extension(),
            [DomImportConfigOverrides {
                rules: Some(rules.into_iter().map(Into::into).collect()),
                ..Default::default()
            }],
        ));
    }
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// How many waiting wrapper ids the editor's imports have looked at.
fn lookups(editor: &Editor) -> usize {
    get_extension_dependency_from_editor(editor, &block_id_html_extension())
        .unwrap()
        .init
        .unwrap()
        .wrapper_id_lookups()
}

/// Imports `dom` in a discrete update, checks the nodes with `check`, and
/// returns how many waiting wrapper ids the import looked at.
fn import_cost(editor: &Editor, dom: &DomNode, check: impl Fn(Vec<NodeRef>) + 'static) -> usize {
    let before = lookups(editor);
    let dom = dom.clone();
    update(editor, move || {
        check(generate(&dom)?);
        Ok(())
    });
    lookups(editor) - before
}

fn expect_linear(costs: &[usize]) {
    assert!(costs[0] > 0, "{costs:?}");
    assert!(costs[1] < costs[0] * 3, "{costs:?}");
    assert!(costs[2] < costs[1] * 3, "{costs:?}");
}

/// Runs an import of its own inside the outer one, for `<aside data-reenter>`.
fn reenter() -> DomImportRule {
    define_import_rule(Some("test/reenter"), sel::tag(&["aside"]), |_, el, _| {
        if el.has_attribute("data-reenter") {
            generate(&parse("<p>scratch</p>"))?;
        }
        Ok(vec![])
    })
}

/// `build()` of BlockIdHTMLWrapperImport.test.ts.
fn build_import() -> LexicalEditorWithDispose {
    let editor = editor_with(
        "block-wrapper-id-import",
        Some(fresh_ids()),
        vec![reenter()],
    );
    update(&editor, || get_root().append(&[create_paragraph_node()]));
    update(&editor, || get_root().clear());
    editor
}

fn append(editor: &Editor, html: &str) {
    let dom = parse(html);
    update(editor, move || get_root().append(&generate(&dom)?));
}

/// Every element (below the root, or with it when `root`) as
/// `type|text|id`, fresh ids shown as `<fresh>` when `masked`.
fn blocks(editor: &Editor, masked: bool, root: bool) -> Vec<String> {
    fn walk(node: &NodeRef, masked: bool, root: bool, out: &mut Vec<String>) {
        if !node.is_element() {
            return;
        }
        if root || node.get_type() != "root" {
            let id = get_block_id(node).unwrap();
            let id = match id {
                Some(id) if masked && id.starts_with("fresh") => "<fresh>".to_owned(),
                Some(id) => id,
                None => "null".to_owned(),
            };
            out.push(format!(
                "{}|{}|{id}",
                node.get_type(),
                node.get_text_content()
            ));
        }
        for child in node.get_children() {
            walk(&child, masked, root, out);
        }
    }
    editor
        .read(|| {
            let mut out = Vec::new();
            walk(&get_root(), masked, root, &mut out);
            out
        })
        .unwrap()
}

fn without_whitespace(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::wrapper ids that go on no block do not make import quadratic
#[test]
fn wrapper_ids_that_go_on_no_block_do_not_make_import_quadratic() {
    let mut costs = Vec::new();
    for n in [64, 128, 256] {
        let editor = build_import();
        let dom = parse(
            &(0..n)
                .map(|i| format!(r#"<ul data-block-wrapper-id="foreign{i}"><li>x</li></ul>"#))
                .collect::<String>(),
        );
        costs.push(import_cost(&editor, &dom, move |nodes| {
            assert_eq!(nodes.len(), n)
        }));
        editor.dispose();
    }
    // Counting operations, not timing: a doubled linear input costs ~2x,
    // whereas scanning every outstanding wrapper after every element costs
    // ~4x.
    expect_linear(&costs);
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::an import run inside another keeps the outer import's waiting wrapper id
#[test]
fn an_import_run_inside_another_keeps_the_outer_waiting_wrapper_id() {
    let plain = build_import();
    let nested = build_import();
    let fixture = |inner: &str| {
        format!(
            r#"<ul data-block-id="L"><li data-block-id="I">text<ul data-block-wrapper-id="W"><li data-block-id="N">inner</li></ul>{inner}</li></ul>"#
        )
    };
    append(&plain, &fixture("<aside></aside>"));
    append(&nested, &fixture("<aside data-reenter></aside>"));
    assert_eq!(blocks(&nested, true, false), blocks(&plain, true, false));
    assert!(
        blocks(&plain, false, false)
            .iter()
            .any(|b| b.ends_with("|W"))
    );
    plain.dispose();
    nested.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::HTML without wrapper ids reads the same with an import run inside it
#[test]
fn html_without_wrapper_ids_reads_the_same_with_an_import_run_inside_it() {
    let plain = build_import();
    let nested = build_import();
    let fixture = |inner: &str| {
        format!(
            r#"<ul data-block-id="L"><li data-block-id="I">text<ul><li data-block-id="N">inner</li></ul>{inner}</li></ul>"#
        )
    };
    append(&plain, &fixture("<aside></aside>"));
    append(&nested, &fixture("<aside data-reenter></aside>"));
    assert_eq!(blocks(&nested, false, false), blocks(&plain, false, false));
    plain.dispose();
    nested.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::an unknown element that hoists the same list again does not make import look at it again
#[test]
fn an_unknown_element_that_hoists_the_same_list_again_does_not_look_again() {
    // Nests {32, 64, 128} levels deep.
    let costs = with_large_stack(|| {
        let mut costs = Vec::new();
        for n in [32, 64, 128] {
            let editor = editor_with("block-wrapper-id-hoist", None, vec![]);
            // Each unrecognized wrapper hoists the same ListNode, whose n
            // child items have already been constructed. The valid
            // top-level wrapper id remains unresolved, because no wrapping
            // block is created.
            let dom = parse(&format!(
                r#"{}<ul data-block-wrapper-id="foreign">{}</ul>{}"#,
                "<x-transparent>".repeat(n),
                "<li>x</li>".repeat(n),
                "</x-transparent>".repeat(n)
            ));
            costs.push(import_cost(&editor, &dom, move |nodes| {
                assert_eq!(nodes.len(), 1);
                assert_eq!(
                    without_whitespace(&nodes[0].get_text_content()),
                    "x".repeat(n)
                );
            }));
            editor.dispose();
        }
        costs
    });
    expect_linear(&costs);
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::wrapper ids inside many unknown elements are looked at a bounded number of times each
#[test]
fn wrapper_ids_inside_many_unknown_elements_are_looked_at_a_bounded_number_of_times() {
    let costs = with_large_stack(|| {
        let mut costs = Vec::new();
        for n in [32, 64, 128] {
            let editor = editor_with("block-wrapper-id-deep", None, vec![]);
            let dom = parse(&format!(
                "{}{}{}",
                "<x-transparent>".repeat(n),
                (0..n)
                    .map(|i| format!(r#"<ul data-block-wrapper-id="w{i}"><li>x</li></ul>"#))
                    .collect::<String>(),
                "</x-transparent>".repeat(n)
            ));
            costs.push(import_cost(&editor, &dom, |_| {}));
            editor.dispose();
        }
        costs
    });
    expect_linear(&costs);
}

/// Imports its children and adds an item to each list among them.
fn grow_rule() -> DomImportRule {
    define_import_rule(Some("test/grow"), sel::tag(&["x-grow"]), |ctx, el, _| {
        let nodes = ctx.import_children(el, &Default::default())?;
        for node in nodes.iter().filter(|n| is_list_node(n)) {
            // Appending at the tail is an insertion-only splice: no scan of
            // old children is needed to construct each new child.
            let item = create_list_item_node(None);
            item.append(&[create_text_node("x")])?;
            node.append(&[item])?;
        }
        Ok(nodes)
    })
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLWrapperResolve.test.ts::a list that grows by one item per element around it is resolved in linear work
#[test]
fn a_list_that_grows_by_one_item_per_element_around_it_is_resolved_in_linear_work() {
    let costs = with_large_stack(|| {
        let mut costs = Vec::new();
        for n in [32, 64, 128] {
            let editor = editor_with("test-changing-count", None, vec![grow_rule()]);
            let dom = parse(&format!(
                r#"{}<ul data-block-wrapper-id="foreign"><li>x</li></ul>{}"#,
                "<x-grow>".repeat(n),
                "</x-grow>".repeat(n)
            ));
            costs.push(import_cost(&editor, &dom, move |nodes| {
                assert_eq!(nodes.len(), 1);
                assert_eq!(nodes[0].get_children().len(), n + 1);
                assert_eq!(
                    without_whitespace(&nodes[0].get_text_content()),
                    "x".repeat(n + 1)
                );
            }));
            editor.dispose();
        }
        costs
    });
    expect_linear(&costs);
}

#[derive(Clone, Copy, PartialEq)]
enum Growth {
    Parent,
    Holder,
}

#[derive(Default)]
struct Populate {
    inner: Option<NodeRef>,
    parent: Option<NodeRef>,
    holder: Option<NodeRef>,
}

fn build_populate(mode: Growth) -> LexicalEditorWithDispose {
    let built = Rc::new(RefCell::new(Populate::default()));
    let b = built.clone();
    let build_rule = define_import_rule(
        Some("test/build-empty"),
        sel::tag(&["x-build"]),
        move |ctx, el, _| {
            let inner = ctx.import_children(el, &Default::default())?[0].clone();
            let parent = create_list_node(ListType::Bullet, 1.0);
            let holder = create_list_item_node(None);
            if mode == Growth::Holder {
                parent.append(std::slice::from_ref(&holder))?;
            }
            if el.has_attribute("data-early") {
                holder.append(std::slice::from_ref(&inner))?;
                if mode == Growth::Parent {
                    parent.append(std::slice::from_ref(&holder))?;
                }
            }
            *b.borrow_mut() = Populate {
                inner: Some(inner),
                parent: Some(parent.clone()),
                holder: Some(holder),
            };
            Ok(vec![parent])
        },
    );
    let populate_rule = define_import_rule(
        Some("test/populate"),
        sel::tag(&["x-populate"]),
        move |ctx, el, _| {
            let nodes = ctx.import_children(el, &Default::default())?;
            let built = built.borrow();
            let holder = built.holder.clone().unwrap();
            if holder.get_children_size() == 0 {
                holder.append(&[built.inner.clone().unwrap()])?;
                if mode == Growth::Parent {
                    built.parent.clone().unwrap().append(&[holder])?;
                }
            }
            Ok(nodes)
        },
    );
    editor_with(
        "test-late-populate",
        Some(fresh_ids()),
        vec![build_rule, populate_rule],
    )
}

fn populate_result(mode: Growth, early: bool) -> Vec<String> {
    let editor = build_populate(mode);
    append(
        &editor,
        &format!(
            r#"<x-populate><x-build{}><ul data-block-wrapper-id="W"><li data-block-id="N">inner</li></ul></x-build></x-populate>"#,
            if early { " data-early" } else { "" }
        ),
    );
    let out = blocks(&editor, true, true);
    editor.dispose();
    out
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLWrapperResolve.test.ts::late population resolves ids exactly like early population (parent-growth)
#[test]
fn late_population_resolves_ids_like_early_population_parent_growth() {
    assert_eq!(
        populate_result(Growth::Parent, false),
        populate_result(Growth::Parent, true)
    );
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLWrapperResolve.test.ts::late population resolves ids exactly like early population (holder-growth)
#[test]
fn late_population_resolves_ids_like_early_population_holder_growth() {
    assert_eq!(
        populate_result(Growth::Holder, false),
        populate_result(Growth::Holder, true)
    );
}
