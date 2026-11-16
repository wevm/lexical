//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockContainers.test.ts`.
//!
//! What each structural command does in each kind of block container, with
//! and without NodeStructureExtension. The structural-command table
//! (`EXPECTED`) and its markers are generated from the upstream file, one
//! Rust test per command carrying one marker per case.

#![cfg(all(
    feature = "extension",
    feature = "markdown",
    feature = "overflow",
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "code"
))]

use super::super::support::{mount_in_jsdom, update};
use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, create_horizontal_rule_node,
    horizontal_rule_extension, node_structure_extension,
};
use crate::json::Value;
use crate::list::{ListType, create_list_item_node, create_list_node, insert_list, remove_list};
use crate::rich_text::{HeadingTagType, create_heading_node, create_quote_node};
use crate::selection::{set_blocks_type, wrap_nodes};
use crate::{
    DELETE_CHARACTER_COMMAND, Editor, ElementData, INDENT_CONTENT_COMMAND,
    INSERT_PARAGRAPH_COMMAND, LexicalError, LexicalExtension, LexicalNode, NodeBase,
    NodeChildrenConfig, NodeClass, NodeConfigLink, NodeRef, OUTDENT_CONTENT_COMMAND, PointType,
    StaticNodeConfig, Which, config_extension, create, create_node_selection,
    create_paragraph_node, create_range_selection, create_text_node, define_extension, get_root,
    get_selection, get_slot, get_slot_names, is_block_container, set_selection, set_slot,
};
use std::rc::Rc;

macro_rules! element_node {
    ($(#[$doc:meta])* $name:ident, $type:literal, [$($super:literal),*], $chain:expr, shadow: $shadow:expr, inline: $inline:expr) => {
        $(#[$doc])*
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            element: ElementData,
        }

        impl LexicalNode for $name {
            fn get_type(&self) -> &'static str {
                $type
            }
            fn super_types(&self) -> Vec<&'static str> {
                vec![$($super),*]
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
            fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
                $chain
            }
            fn is_shadow_root(&self) -> bool {
                $shadow
            }
            fn is_inline(&self) -> bool {
                $inline
            }
            #[cfg(feature = "html")]
            fn create_dom(
                &self,
                _this: &NodeRef,
                _config: &crate::EditorConfig,
                _editor: &Editor,
            ) -> Result<crate::dom::DomNode, LexicalError> {
                Ok(crate::lexical_dom::get_document().create_element("div"))
            }
        }
    };
}

/// `DeclaredBoxNode`'s own `$config()`: `children: 'block'`.
fn declared_link() -> NodeConfigLink {
    NodeConfigLink::new(
        "test_declared_box",
        StaticNodeConfig {
            children: Some(NodeChildrenConfig::Block),
            ..StaticNodeConfig::default()
        },
    )
}

element_node!(
    /// `PlainBoxNode`: a container that declares nothing.
    PlainBoxNode, "test_plain_box", [], Vec::new(), shadow: false, inline: false
);
element_node!(
    /// `DeclaredBoxNode`: `children: 'block'`.
    DeclaredBoxNode, "test_declared_box", [], vec![declared_link()], shadow: false, inline: false
);
element_node!(
    /// `ShadowBoxNode`: a shadow root.
    ShadowBoxNode, "test_shadow_box", [], Vec::new(), shadow: true, inline: false
);
element_node!(
    /// `DeclaredBoxSubclassNode`: extends `DeclaredBoxNode`, declares nothing.
    DeclaredBoxSubclassNode,
    "test_declared_box_subclass",
    ["test_declared_box"],
    vec![
        NodeConfigLink::new("test_declared_box_subclass", StaticNodeConfig::default()),
        declared_link()
    ],
    shadow: false,
    inline: false
);
element_node!(
    /// `InlineDeclaredBoxNode`: extends `DeclaredBoxNode`, and is inline.
    InlineDeclaredBoxNode,
    "test_inline_declared_box",
    ["test_declared_box"],
    vec![
        NodeConfigLink::new("test_inline_declared_box", StaticNodeConfig::default()),
        declared_link()
    ],
    shadow: false,
    inline: true
);

/// `ComponentNode`: a block decorator (a slot host here).
#[derive(Clone, Debug, Default)]
struct ComponentNode {
    base: NodeBase,
}

impl LexicalNode for ComponentNode {
    fn get_type(&self) -> &'static str {
        "test_component"
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
        false
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("div"))
    }
}

type R = Result<(), LexicalError>;

/// `$getRoot().clear()`.
fn cleared_root() -> Result<NodeRef, LexicalError> {
    let root = get_root();
    root.clear()?;
    Ok(root)
}

fn append(parent: NodeRef, children: Vec<NodeRef>) -> NodeRef {
    parent.append(&children).unwrap();
    parent
}

fn make(kind: &str) -> NodeRef {
    match kind {
        "plain" => create::<PlainBoxNode>(),
        "declared" => create::<DeclaredBoxNode>(),
        _ => create::<ShadowBoxNode>(),
    }
}

/// `$box(kind, depth, inner)`.
fn make_box(kind: &str, depth: usize, inner: Vec<NodeRef>) -> NodeRef {
    let mut node = append(make(kind), inner);
    for _ in 1..depth {
        node = append(make(kind), vec![node]);
    }
    node
}

/// `$p(t)`.
fn p(t: &str) -> NodeRef {
    append(create_paragraph_node(), vec![create_text_node(t)])
}

/// `$build(ctx, inner)`: root > p(before) > CONTAINER(inner) > p(after).
fn build(ctx: &str, inner: impl FnOnce() -> Vec<NodeRef>) -> R {
    let root = get_root();
    root.clear()?;
    root.append(&[p("before")])?;
    match ctx {
        "root" => {
            root.append(&inner())?;
        }
        "slot-shadow" => {
            let c = create::<ComponentNode>();
            set_slot(&c, "body", &append(create::<ShadowBoxNode>(), inner()))?;
            root.append(&[c])?;
        }
        "slot-bare" => {
            let c = create::<ComponentNode>();
            let first = inner().into_iter().next().unwrap();
            set_slot(&c, "body", &first)?;
            root.append(&[c])?;
        }
        _ => {
            let kind = ctx.replace('2', "");
            let depth = if ctx.ends_with('2') { 2 } else { 1 };
            root.append(&[make_box(&kind, depth, inner())])?;
        }
    }
    root.append(&[p("after")])?;
    Ok(())
}

fn short_name(node_type: &str) -> &str {
    match node_type {
        "heading" => "h",
        "listitem" => "li",
        "paragraph" => "p",
        "test_component" => "component",
        "test_declared_box" => "dbox",
        "test_plain_box" => "box",
        "test_shadow_box" => "sbox",
        other => other,
    }
}

/// `$describeNode(n)`.
fn describe_node(n: &NodeRef) -> String {
    let node_type = n.get_type();
    if n.is_text() {
        return crate::json::stringify(&Value::from(n.get_text_content())).unwrap();
    }
    if node_type == "linebreak" {
        return "br".to_owned();
    }
    let mut extra = Vec::new();
    if n.is_element() && node_type != "listitem" {
        let indent = n.get_indent().unwrap();
        if indent > 0.0 {
            extra.push(format!("indent{}", crate::json::number_to_string(indent)));
        }
    }
    if node_type == "list" {
        extra.push(n.get_list_type().unwrap().as_str().to_owned());
    }
    let slots = get_slot_names(n)
        .iter()
        .map(|s| format!("{s}:{}", describe_node(&get_slot(n, s).unwrap())))
        .collect::<Vec<_>>()
        .join(" ");
    let kids = if n.is_element() {
        n.get_children()
            .iter()
            .map(describe_node)
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        String::new()
    };
    format!(
        "{}{}{}({kids})",
        short_name(node_type),
        if extra.is_empty() {
            String::new()
        } else {
            format!("[{}]", extra.join(","))
        },
        if slots.is_empty() {
            String::new()
        } else {
            format!("{{{slots}}}")
        },
    )
}

/// `$shape()`.
fn shape() -> String {
    get_root()
        .get_children()
        .iter()
        .map(describe_node)
        .collect::<Vec<_>>()
        .join(" / ")
}

fn read_shape(editor: &Editor) -> String {
    editor.read(shape).unwrap()
}

/// `$findText(text)`.
fn find_text(text: &str) -> NodeRef {
    fn visit(n: &NodeRef, text: &str) -> Option<NodeRef> {
        if n.is_text() && n.get_text_content() == text {
            return Some(n.clone());
        }
        for s in get_slot_names(n) {
            if let Some(r) = visit(&get_slot(n, &s).unwrap(), text) {
                return Some(r);
            }
        }
        if n.is_element() {
            for c in n.get_children() {
                if let Some(r) = visit(&c, text) {
                    return Some(r);
                }
            }
        }
        None
    }
    visit(&get_root(), text).unwrap_or_else(|| panic!("no text {text}"))
}

/// `$selectRange(a, ao, b, bo)`.
fn select_range(a: &NodeRef, ao: usize, b: &NodeRef, bo: usize) -> R {
    let sel = create_range_selection();
    sel.point_set(Which::Anchor, a.get_key(), ao, PointType::Text)?;
    sel.point_set(Which::Focus, b.get_key(), bo, PointType::Text)?;
    set_selection(Some(sel));
    Ok(())
}

fn selection() -> crate::SelectionRef {
    get_selection().expect("a selection")
}

fn heading() -> NodeRef {
    create_heading_node(HeadingTagType::H1)
}

fn quote() -> NodeRef {
    create_quote_node(false)
}

fn bullet_list(items: Vec<NodeRef>) -> NodeRef {
    append(create_list_node(ListType::Bullet, 1.0), items)
}

fn item(children: Vec<NodeRef>) -> NodeRef {
    append(create_list_item_node(None), children)
}

/// The command names of `CMDS`.
const CMDS: [&str; 20] = [
    "backspace-after-container",
    "backspace-at-container-start",
    "delete-at-container-end",
    "enter-empty-last-listitem",
    "indent",
    "insertList-collapsed",
    "insertList-quote-in-container",
    "insertList-range",
    "insertNodeToNearestRoot-hr-end",
    "insertNodeToNearestRoot-hr-mid",
    "insertNodes-blocks",
    "insertNodes-heading",
    "markdown-heading",
    "markdown-list",
    "outdent",
    "removeList",
    "setBlocksType-code-range",
    "setBlocksType-heading",
    "setBlocksType-quote",
    "wrapNodes-quote",
];

/// `CMDS[cmd].inner`.
fn cmd_inner(cmd: &str) -> Vec<NodeRef> {
    match cmd {
        "enter-empty-last-listitem" => vec![bullet_list(vec![
            item(vec![create_text_node("abc")]),
            create_list_item_node(None),
        ])],
        "insertList-quote-in-container" => {
            vec![append(quote(), vec![create_text_node("abc")]), p("def")]
        }
        "markdown-heading" | "markdown-list" => vec![create_paragraph_node(), p("def")],
        "outdent" => {
            let first = p("abc");
            first.set_indent(1.0).unwrap();
            vec![first, p("def")]
        }
        "removeList" => vec![bullet_list(vec![
            item(vec![create_text_node("abc")]),
            item(vec![create_text_node("def")]),
        ])],
        _ => vec![p("abc"), p("def")],
    }
}

/// `CMDS[cmd].run`.
fn cmd_run(cmd: &str, editor: &Editor) -> R {
    let caret = |text: &str, offset: usize| -> R {
        find_text(text).select(Some(offset), Some(offset))?;
        Ok(())
    };
    match cmd {
        "backspace-after-container" => {
            caret("after", 0)?;
            editor.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
        }
        "backspace-at-container-start" => {
            caret("abc", 0)?;
            editor.dispatch_command(&DELETE_CHARACTER_COMMAND, true)?;
        }
        "delete-at-container-end" => {
            caret("def", 3)?;
            editor.dispatch_command(&DELETE_CHARACTER_COMMAND, false)?;
        }
        "enter-empty-last-listitem" => {
            let list = find_text("abc")
                .get_parent_or_throw()?
                .get_parent_or_throw()?;
            list.get_last_child_or_throw()?.select(None, None)?;
            editor.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())?;
        }
        "indent" => {
            caret("abc", 1)?;
            editor.dispatch_command(&INDENT_CONTENT_COMMAND, ())?;
        }
        "insertList-collapsed" | "insertList-quote-in-container" => {
            caret("abc", 1)?;
            insert_list(ListType::Bullet)?;
        }
        "insertList-range" => {
            select_range(&find_text("abc"), 1, &find_text("def"), 1)?;
            insert_list(ListType::Number)?;
        }
        "insertNodeToNearestRoot-hr-end" => {
            caret("def", 3)?;
            crate::utils::insert_node_to_nearest_root(&create_horizontal_rule_node())?;
        }
        "insertNodeToNearestRoot-hr-mid" => {
            caret("abc", 1)?;
            crate::utils::insert_node_to_nearest_root(&create_horizontal_rule_node())?;
        }
        "insertNodes-blocks" => {
            caret("abc", 1)?;
            selection().insert_nodes(vec![p("X"), p("Y")])?;
        }
        "insertNodes-heading" => {
            caret("abc", 1)?;
            selection().insert_nodes(vec![append(
                create_heading_node(HeadingTagType::H2),
                vec![create_text_node("H")],
            )])?;
        }
        "markdown-heading" | "markdown-list" => {
            // caret in the empty first paragraph of the container
            let def = find_text("def");
            let first = def.get_parent_or_throw()?.get_previous_sibling().unwrap();
            first.select(None, None)?;
        }
        "outdent" => {
            caret("abc", 1)?;
            editor.dispatch_command(&OUTDENT_CONTENT_COMMAND, ())?;
        }
        "removeList" => {
            caret("abc", 1)?;
            remove_list()?;
        }
        "setBlocksType-code-range" => {
            select_range(&find_text("abc"), 1, &find_text("def"), 1)?;
            set_blocks_type(
                get_selection().as_ref(),
                || crate::code::create_code_node(None, None),
                None,
            )?;
        }
        "setBlocksType-heading" => {
            caret("abc", 1)?;
            set_blocks_type(get_selection().as_ref(), heading, None)?;
        }
        "setBlocksType-quote" => {
            caret("abc", 1)?;
            set_blocks_type(get_selection().as_ref(), quote, None)?;
        }
        "wrapNodes-quote" => {
            caret("abc", 1)?;
            wrap_nodes(&selection(), quote, None)?;
        }
        other => panic!("no command {other}"),
    }
    Ok(())
}

/// `CMDS[cmd].type`.
fn cmd_type(cmd: &str) -> &'static str {
    match cmd {
        "markdown-heading" => "# x",
        "markdown-list" => "- x",
        _ => "",
    }
}

/// `$strip(shape)`.
fn strip(shape: &str) -> String {
    let shape = shape.strip_prefix("p(\"before\") / ").unwrap_or(shape);
    shape
        .strip_suffix(" / p(\"after\")")
        .unwrap_or(shape)
        .to_owned()
}

/// `buildEditor(structure)`.
fn build_editor(structure: bool) -> LexicalEditorWithDispose {
    let mut dependencies = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::list::list_extension().into(),
        crate::link::link_extension().into(),
        crate::code::code_extension().into(),
        horizontal_rule_extension().into(),
    ];
    if structure {
        dependencies.push(node_structure_extension().into());
    }
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        nodes: vec![
            NodeClass::of::<PlainBoxNode>(),
            NodeClass::of::<DeclaredBoxNode>(),
            NodeClass::of::<ShadowBoxNode>(),
            NodeClass::of::<ComponentNode>(),
        ],
        register: Some(Rc::new(|editor: &Editor, _, _| {
            crate::markdown::register_markdown_shortcuts(editor, &crate::markdown::transformers())
        })),
        ..LexicalExtension::<(), (), ()>::new("block-containers", ())
    })
    .into()])
    .unwrap()
}

/// `$run(cmd, ctx, structure)`.
fn run(cmd: &'static str, ctx: &'static str, structure: bool) -> String {
    let editor = build_editor(structure);
    mount_in_jsdom(&editor);
    update(&editor, move || build(ctx, || cmd_inner(cmd)));
    let e = (*editor).clone();
    update(&editor, move || cmd_run(cmd, &e));
    for ch in cmd_type(cmd).chars() {
        update(&editor, move || {
            if let Some(selection) = get_selection().filter(crate::SelectionRef::is_range) {
                selection.insert_text(&ch.to_string())?;
            }
            Ok(())
        });
    }
    let out = strip(&read_shape(&editor));
    editor.dispose();
    out
}

/// Runs each case of one command and fails with every mismatch.
fn check_command(cmd: &'static str, cases: &[(&'static str, bool, &str)]) {
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|&(ctx, structure, expected)| {
            let got = run(cmd, ctx, structure);
            (got != expected).then(|| {
                format!("{cmd} in {ctx} (NodeStructureExtension: {structure}):\n  expected {expected}\n  got      {got}")
            })
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A command's name and its cases' `(name, before, after)`.
type CommandCases = (
    &'static str,
    &'static [(&'static str, &'static str, &'static str)],
);

const EXPECTED: &[CommandCases] = &[
    (
        "backspace-after-container",
        &[
            (
                "root",
                "p(\"abc\") / p(\"defafter\")",
                "p(\"abc\") / p(\"defafter\")",
            ),
            (
                "plain",
                "box(p(\"abc\") p(\"defafter\"))",
                "box(p(\"abc\") p(\"defafter\"))",
            ),
            (
                "plain2",
                "box(box(p(\"abc\") p(\"defafter\")))",
                "box(box(p(\"abc\") p(\"defafter\")))",
            ),
            (
                "declared",
                "dbox(p(\"abc\") p(\"defafter\"))",
                "dbox(p(\"abc\") p(\"defafter\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"abc\") p(\"defafter\")))",
                "dbox(dbox(p(\"abc\") p(\"defafter\")))",
            ),
            (
                "shadow",
                "sbox(p(\"abc\") p(\"def\"))",
                "sbox(p(\"abc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
            ),
            ("slot-shadow", "p(\"after\")", "p(\"after\")"),
            ("slot-bare", "p(\"after\")", "p(\"after\")"),
        ],
    ),
    (
        "backspace-at-container-start",
        &[
            (
                "root",
                "p(\"beforeabc\") / p(\"def\")",
                "p(\"beforeabc\") / p(\"def\")",
            ),
            (
                "plain",
                "p(\"beforeabc\") / box(p(\"def\"))",
                "p(\"beforeabc\") / box(p(\"def\"))",
            ),
            (
                "plain2",
                "p(\"beforeabc\") / box(box(p(\"def\")))",
                "p(\"beforeabc\") / box(box(p(\"def\")))",
            ),
            (
                "declared",
                "p(\"beforeabc\") / dbox(p(\"def\"))",
                "p(\"beforeabc\") / dbox(p(\"def\"))",
            ),
            (
                "declared2",
                "p(\"beforeabc\") / dbox(dbox(p(\"def\")))",
                "p(\"beforeabc\") / dbox(dbox(p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(p(\"abc\") p(\"def\"))",
                "sbox(p(\"abc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p(\"abc\")}()",
                "component{body:p(\"abc\")}()",
            ),
        ],
    ),
    (
        "delete-at-container-end",
        &[
            (
                "root",
                "p(\"abc\") / p(\"defafter\")",
                "p(\"abc\") / p(\"defafter\")",
            ),
            (
                "plain",
                "box(p(\"abc\") p(\"defafter\"))",
                "box(p(\"abc\") p(\"defafter\"))",
            ),
            (
                "plain2",
                "box(box(p(\"abc\") p(\"defafter\")))",
                "box(box(p(\"abc\") p(\"defafter\")))",
            ),
            (
                "declared",
                "dbox(p(\"abc\") p(\"defafter\"))",
                "dbox(p(\"abc\") p(\"defafter\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"abc\") p(\"defafter\")))",
                "dbox(dbox(p(\"abc\") p(\"defafter\")))",
            ),
            (
                "shadow",
                "sbox(p(\"abc\") p(\"def\"))",
                "sbox(p(\"abc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
        ],
    ),
    (
        "enter-empty-last-listitem",
        &[
            (
                "root",
                "list[bullet](li(\"abc\")) / p()",
                "list[bullet](li(\"abc\")) / p()",
            ),
            (
                "plain",
                "box(list[bullet](li(\"abc\") li() li()))",
                "box(list[bullet](li(\"abc\") li() li()))",
            ),
            (
                "plain2",
                "box(box(list[bullet](li(\"abc\") li() li())))",
                "box(box(list[bullet](li(\"abc\") li() li())))",
            ),
            (
                "declared",
                "dbox(list[bullet](li(\"abc\") li() li()))",
                "dbox(list[bullet](li(\"abc\")) p())",
            ),
            (
                "declared2",
                "dbox(dbox(list[bullet](li(\"abc\") li() li())))",
                "dbox(dbox(list[bullet](li(\"abc\")) p()))",
            ),
            (
                "shadow",
                "sbox(list[bullet](li(\"abc\")) p())",
                "sbox(list[bullet](li(\"abc\")) p())",
            ),
            (
                "shadow2",
                "sbox(sbox(list[bullet](li(\"abc\")) p()))",
                "sbox(sbox(list[bullet](li(\"abc\")) p()))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(list[bullet](li(\"abc\")) p())}()",
                "component{body:sbox(list[bullet](li(\"abc\")) p())}()",
            ),
            (
                "slot-bare",
                "component{body:list[bullet](li(\"abc\") li() li())}()",
                "component{body:list[bullet](li(\"abc\") li() li())}()",
            ),
        ],
    ),
    (
        "indent",
        &[
            (
                "root",
                "p[indent1](\"abc\") / p(\"def\")",
                "p[indent1](\"abc\") / p(\"def\")",
            ),
            (
                "plain",
                "box(p[indent1](\"abc\") p(\"def\"))",
                "box(p[indent1](\"abc\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(p[indent1](\"abc\") p(\"def\")))",
                "box(box(p[indent1](\"abc\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(p[indent1](\"abc\") p(\"def\"))",
                "dbox(p[indent1](\"abc\") p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p[indent1](\"abc\") p(\"def\")))",
                "dbox(dbox(p[indent1](\"abc\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(p[indent1](\"abc\") p(\"def\"))",
                "sbox(p[indent1](\"abc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(p[indent1](\"abc\") p(\"def\")))",
                "sbox(sbox(p[indent1](\"abc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(p[indent1](\"abc\") p(\"def\"))}()",
                "component{body:sbox(p[indent1](\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p[indent1](\"abc\")}()",
                "component{body:p[indent1](\"abc\")}()",
            ),
        ],
    ),
    (
        "insertList-collapsed",
        &[
            (
                "root",
                "list[bullet](li(\"abc\")) / p(\"def\")",
                "list[bullet](li(\"abc\")) / p(\"def\")",
            ),
            (
                "plain",
                "list[bullet](li(p(\"abc\") p(\"def\")))",
                "list[bullet](li(\"abc\" br \"def\"))",
            ),
            (
                "plain2",
                "list[bullet](li(box(p(\"abc\") p(\"def\"))))",
                "box(box(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "declared",
                "list[bullet](li(p(\"abc\") p(\"def\")))",
                "dbox(list[bullet](li(\"abc\")) p(\"def\"))",
            ),
            (
                "declared2",
                "list[bullet](li(dbox(p(\"abc\") p(\"def\"))))",
                "dbox(dbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(list[bullet](li(\"abc\")) p(\"def\"))",
                "sbox(list[bullet](li(\"abc\")) p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(list[bullet](li(\"abc\")) p(\"def\")))",
                "sbox(sbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(list[bullet](li(\"abc\")) p(\"def\"))}()",
                "component{body:sbox(list[bullet](li(\"abc\")) p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p(\"abc\")}()",
                "component{body:p(\"abc\")}()",
            ),
        ],
    ),
    (
        "insertList-quote-in-container",
        &[
            (
                "root",
                "list[bullet](li(\"abc\")) / p(\"def\")",
                "list[bullet](li(\"abc\")) / p(\"def\")",
            ),
            (
                "plain",
                "list[bullet](li(quote(\"abc\") p(\"def\")))",
                "box(list[bullet](li(\"abc\")) p(\"def\"))",
            ),
            (
                "plain2",
                "list[bullet](li(box(quote(\"abc\") p(\"def\"))))",
                "box(box(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "declared",
                "list[bullet](li(quote(\"abc\") p(\"def\")))",
                "dbox(list[bullet](li(\"abc\")) p(\"def\"))",
            ),
            (
                "declared2",
                "list[bullet](li(dbox(quote(\"abc\") p(\"def\"))))",
                "dbox(dbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(list[bullet](li(\"abc\")) p(\"def\"))",
                "sbox(list[bullet](li(\"abc\")) p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(list[bullet](li(\"abc\")) p(\"def\")))",
                "sbox(sbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(list[bullet](li(\"abc\")) p(\"def\"))}()",
                "component{body:sbox(list[bullet](li(\"abc\")) p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:quote(\"abc\")}()",
                "component{body:quote(\"abc\")}()",
            ),
        ],
    ),
    (
        "insertList-range",
        &[
            (
                "root",
                "list[number](li(\"abc\") li(\"def\"))",
                "list[number](li(\"abc\") li(\"def\"))",
            ),
            (
                "plain",
                "list[number](li(p(\"abc\") p(\"def\")))",
                "list[number](li(\"abc\" br \"def\"))",
            ),
            (
                "plain2",
                "list[number](li(box(p(\"abc\") p(\"def\"))))",
                "box(box(list[number](li(\"abc\") li(\"def\"))))",
            ),
            (
                "declared",
                "list[number](li(p(\"abc\") p(\"def\")))",
                "dbox(list[number](li(\"abc\") li(\"def\")))",
            ),
            (
                "declared2",
                "list[number](li(dbox(p(\"abc\") p(\"def\"))))",
                "dbox(dbox(list[number](li(\"abc\") li(\"def\"))))",
            ),
            (
                "shadow",
                "sbox(list[number](li(\"abc\") li(\"def\")))",
                "sbox(list[number](li(\"abc\") li(\"def\")))",
            ),
            (
                "shadow2",
                "sbox(sbox(list[number](li(\"abc\") li(\"def\"))))",
                "sbox(sbox(list[number](li(\"abc\") li(\"def\"))))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(list[number](li(\"abc\") li(\"def\")))}()",
                "component{body:sbox(list[number](li(\"abc\") li(\"def\")))}()",
            ),
        ],
    ),
    (
        "insertNodeToNearestRoot-hr-end",
        &[
            (
                "root",
                "p(\"abc\") / p(\"def\") / horizontalrule()",
                "p(\"abc\") / p(\"def\") / horizontalrule()",
            ),
            (
                "plain",
                "box(p(\"abc\") p(\"def\")) / horizontalrule()",
                "box(p(\"abc\") p(\"def\")) / horizontalrule()",
            ),
            (
                "plain2",
                "box(box(p(\"abc\") p(\"def\"))) / horizontalrule()",
                "box(box(p(\"abc\") p(\"def\"))) / horizontalrule()",
            ),
            (
                "declared",
                "dbox(p(\"abc\") p(\"def\")) / horizontalrule()",
                "dbox(p(\"abc\") p(\"def\") horizontalrule() p())",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"abc\") p(\"def\"))) / horizontalrule()",
                "dbox(dbox(p(\"abc\") p(\"def\") horizontalrule() p()))",
            ),
            (
                "shadow",
                "sbox(p(\"abc\") p(\"def\") horizontalrule() p())",
                "sbox(p(\"abc\") p(\"def\") horizontalrule() p())",
            ),
            (
                "shadow2",
                "sbox(sbox(p(\"abc\") p(\"def\") horizontalrule() p()))",
                "sbox(sbox(p(\"abc\") p(\"def\") horizontalrule() p()))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(p(\"abc\") p(\"def\") horizontalrule() p())}()",
                "component{body:sbox(p(\"abc\") p(\"def\") horizontalrule() p())}()",
            ),
        ],
    ),
    (
        "insertNodeToNearestRoot-hr-mid",
        &[
            (
                "root",
                "p(\"a\") / horizontalrule() / p(\"bc\") / p(\"def\")",
                "p(\"a\") / horizontalrule() / p(\"bc\") / p(\"def\")",
            ),
            (
                "plain",
                "box(p(\"a\")) / horizontalrule() / box(p(\"bc\") p(\"def\"))",
                "box(p(\"a\")) / horizontalrule() / box(p(\"bc\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(p(\"a\"))) / horizontalrule() / box(box(p(\"bc\") p(\"def\")))",
                "box(box(p(\"a\"))) / horizontalrule() / box(box(p(\"bc\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(p(\"a\")) / horizontalrule() / dbox(p(\"bc\") p(\"def\"))",
                "dbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"a\"))) / horizontalrule() / dbox(dbox(p(\"bc\") p(\"def\")))",
                "dbox(dbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))",
                "sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\")))",
                "sbox(sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))}()",
                "component{body:sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p(\"a\" horizontalrule() \"bc\")}()",
                "component{body:p(\"a\" horizontalrule() \"bc\")}()",
            ),
        ],
    ),
    (
        "insertNodes-blocks",
        &[
            (
                "root",
                "p(\"aX\") / p(\"Ybc\") / p(\"def\")",
                "p(\"aX\") / p(\"Ybc\") / p(\"def\")",
            ),
            (
                "plain",
                "box(p(\"aXYbc\") p(\"def\"))",
                "box(p(\"aXYbc\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(p(\"aXYbc\") p(\"def\")))",
                "box(box(p(\"aXYbc\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(p(\"aXYbc\") p(\"def\"))",
                "dbox(p(\"aX\") p(\"Ybc\") p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"aXYbc\") p(\"def\")))",
                "dbox(dbox(p(\"aX\") p(\"Ybc\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(p(\"aX\") p(\"Ybc\") p(\"def\"))",
                "sbox(p(\"aX\") p(\"Ybc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(p(\"aX\") p(\"Ybc\") p(\"def\")))",
                "sbox(sbox(p(\"aX\") p(\"Ybc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(p(\"aX\") p(\"Ybc\") p(\"def\"))}()",
                "component{body:sbox(p(\"aX\") p(\"Ybc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p(\"aXYbc\")}()",
                "component{body:p(\"aXYbc\")}()",
            ),
        ],
    ),
    (
        "insertNodes-heading",
        &[
            (
                "root",
                "p(\"aHbc\") / p(\"def\")",
                "p(\"aHbc\") / p(\"def\")",
            ),
            (
                "plain",
                "box(p(\"aHbc\") p(\"def\"))",
                "box(p(\"aHbc\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(p(\"aHbc\") p(\"def\")))",
                "box(box(p(\"aHbc\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(p(\"aHbc\") p(\"def\"))",
                "dbox(p(\"aHbc\") p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"aHbc\") p(\"def\")))",
                "dbox(dbox(p(\"aHbc\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(p(\"aHbc\") p(\"def\"))",
                "sbox(p(\"aHbc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(p(\"aHbc\") p(\"def\")))",
                "sbox(sbox(p(\"aHbc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(p(\"aHbc\") p(\"def\"))}()",
                "component{body:sbox(p(\"aHbc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p(\"aHbc\")}()",
                "component{body:p(\"aHbc\")}()",
            ),
        ],
    ),
    (
        "markdown-heading",
        &[
            ("root", "h(\"x\") / p(\"def\")", "h(\"x\") / p(\"def\")"),
            (
                "plain",
                "box(p(\"# x\") p(\"def\"))",
                "box(p(\"# x\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(p(\"# x\") p(\"def\")))",
                "box(box(p(\"# x\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(p(\"# x\") p(\"def\"))",
                "dbox(h(\"x\") p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"# x\") p(\"def\")))",
                "dbox(dbox(h(\"x\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(h(\"x\") p(\"def\"))",
                "sbox(h(\"x\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(h(\"x\") p(\"def\")))",
                "sbox(sbox(h(\"x\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(h(\"x\") p(\"def\"))}()",
                "component{body:sbox(h(\"x\") p(\"def\"))}()",
            ),
        ],
    ),
    (
        "markdown-list",
        &[
            (
                "root",
                "list[bullet](li(\"x\")) / p(\"def\")",
                "list[bullet](li(\"x\")) / p(\"def\")",
            ),
            (
                "plain",
                "box(p(\"- x\") p(\"def\"))",
                "box(p(\"- x\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(p(\"- x\") p(\"def\")))",
                "box(box(p(\"- x\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(p(\"- x\") p(\"def\"))",
                "dbox(list[bullet](li(\"x\")) p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"- x\") p(\"def\")))",
                "dbox(dbox(list[bullet](li(\"x\")) p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(list[bullet](li(\"x\")) p(\"def\"))",
                "sbox(list[bullet](li(\"x\")) p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(list[bullet](li(\"x\")) p(\"def\")))",
                "sbox(sbox(list[bullet](li(\"x\")) p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(list[bullet](li(\"x\")) p(\"def\"))}()",
                "component{body:sbox(list[bullet](li(\"x\")) p(\"def\"))}()",
            ),
        ],
    ),
    (
        "outdent",
        &[
            ("root", "p(\"abc\") / p(\"def\")", "p(\"abc\") / p(\"def\")"),
            (
                "plain",
                "box(p(\"abc\") p(\"def\"))",
                "box(p(\"abc\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(p(\"abc\") p(\"def\")))",
                "box(box(p(\"abc\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(p(\"abc\") p(\"def\"))",
                "dbox(p(\"abc\") p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"abc\") p(\"def\")))",
                "dbox(dbox(p(\"abc\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(p(\"abc\") p(\"def\"))",
                "sbox(p(\"abc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p(\"abc\")}()",
                "component{body:p(\"abc\")}()",
            ),
        ],
    ),
    (
        "removeList",
        &[
            ("root", "p(\"abc\") / p(\"def\")", "p(\"abc\") / p(\"def\")"),
            (
                "plain",
                "box(p(\"abc\") p(\"def\"))",
                "box(p(\"abc\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(p(\"abc\") p(\"def\")))",
                "box(box(p(\"abc\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(p(\"abc\") p(\"def\"))",
                "dbox(p(\"abc\") p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(p(\"abc\") p(\"def\")))",
                "dbox(dbox(p(\"abc\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(p(\"abc\") p(\"def\"))",
                "sbox(p(\"abc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
                "sbox(sbox(p(\"abc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:list[bullet](li(\"abc\") li(\"def\"))}()",
                "component{body:list[bullet](li(\"abc\") li(\"def\"))}()",
            ),
        ],
    ),
    (
        "setBlocksType-code-range",
        &[
            (
                "root",
                "code(\"abc\") / code(\"def\")",
                "code(\"abc\") / code(\"def\")",
            ),
            (
                "plain",
                "box(code(\"abc\") code(\"def\"))",
                "box(code(\"abc\") code(\"def\"))",
            ),
            (
                "plain2",
                "box(box(code(\"abc\") code(\"def\")))",
                "box(box(code(\"abc\") code(\"def\")))",
            ),
            (
                "declared",
                "dbox(code(\"abc\") code(\"def\"))",
                "dbox(code(\"abc\") code(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(code(\"abc\") code(\"def\")))",
                "dbox(dbox(code(\"abc\") code(\"def\")))",
            ),
            (
                "shadow",
                "sbox(code(\"abc\") code(\"def\"))",
                "sbox(code(\"abc\") code(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(code(\"abc\") code(\"def\")))",
                "sbox(sbox(code(\"abc\") code(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(code(\"abc\") code(\"def\"))}()",
                "component{body:sbox(code(\"abc\") code(\"def\"))}()",
            ),
        ],
    ),
    (
        "setBlocksType-heading",
        &[
            ("root", "h(\"abc\") / p(\"def\")", "h(\"abc\") / p(\"def\")"),
            (
                "plain",
                "box(h(\"abc\") p(\"def\"))",
                "box(h(\"abc\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(h(\"abc\") p(\"def\")))",
                "box(box(h(\"abc\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(h(\"abc\") p(\"def\"))",
                "dbox(h(\"abc\") p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(h(\"abc\") p(\"def\")))",
                "dbox(dbox(h(\"abc\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(h(\"abc\") p(\"def\"))",
                "sbox(h(\"abc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(h(\"abc\") p(\"def\")))",
                "sbox(sbox(h(\"abc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(h(\"abc\") p(\"def\"))}()",
                "component{body:sbox(h(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p(\"abc\")}()",
                "component{body:p(\"abc\")}()",
            ),
        ],
    ),
    (
        "setBlocksType-quote",
        &[
            (
                "root",
                "quote(\"abc\") / p(\"def\")",
                "quote(\"abc\") / p(\"def\")",
            ),
            (
                "plain",
                "box(quote(\"abc\") p(\"def\"))",
                "box(quote(\"abc\") p(\"def\"))",
            ),
            (
                "plain2",
                "box(box(quote(\"abc\") p(\"def\")))",
                "box(box(quote(\"abc\") p(\"def\")))",
            ),
            (
                "declared",
                "dbox(quote(\"abc\") p(\"def\"))",
                "dbox(quote(\"abc\") p(\"def\"))",
            ),
            (
                "declared2",
                "dbox(dbox(quote(\"abc\") p(\"def\")))",
                "dbox(dbox(quote(\"abc\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(quote(\"abc\") p(\"def\"))",
                "sbox(quote(\"abc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(quote(\"abc\") p(\"def\")))",
                "sbox(sbox(quote(\"abc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(quote(\"abc\") p(\"def\"))}()",
                "component{body:sbox(quote(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p(\"abc\")}()",
                "component{body:p(\"abc\")}()",
            ),
        ],
    ),
    (
        "wrapNodes-quote",
        &[
            (
                "root",
                "quote(\"abc\") / p(\"def\")",
                "quote(\"abc\") / p(\"def\")",
            ),
            (
                "plain",
                "quote(\"abc\") / box(p(\"def\"))",
                "quote(\"abc\") / box(p(\"def\"))",
            ),
            (
                "plain2",
                "quote(\"abc\") / box(box(p(\"def\")))",
                "quote(\"abc\") / box(box(p(\"def\")))",
            ),
            (
                "declared",
                "quote(\"abc\") / dbox(p(\"def\"))",
                "dbox(quote(\"abc\") p(\"def\"))",
            ),
            (
                "declared2",
                "quote(\"abc\") / dbox(dbox(p(\"def\")))",
                "dbox(dbox(quote(\"abc\") p(\"def\")))",
            ),
            (
                "shadow",
                "sbox(quote(\"abc\") p(\"def\"))",
                "sbox(quote(\"abc\") p(\"def\"))",
            ),
            (
                "shadow2",
                "sbox(sbox(quote(\"abc\") p(\"def\")))",
                "sbox(sbox(quote(\"abc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                "component{body:sbox(quote(\"abc\") p(\"def\"))}()",
                "component{body:sbox(quote(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                "component{body:p(\"abc\")}()",
                "component{body:p(\"abc\")}()",
            ),
        ],
    ),
];

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in slot-bare (NodeStructureExtension: true)
#[test]
fn backspace_after_container() {
    check_command(
        "backspace-after-container",
        &[
            ("root", false, "p(\"abc\") / p(\"defafter\")"),
            ("root", true, "p(\"abc\") / p(\"defafter\")"),
            ("plain", false, "box(p(\"abc\") p(\"defafter\"))"),
            ("plain", true, "box(p(\"abc\") p(\"defafter\"))"),
            ("plain2", false, "box(box(p(\"abc\") p(\"defafter\")))"),
            ("plain2", true, "box(box(p(\"abc\") p(\"defafter\")))"),
            ("declared", false, "dbox(p(\"abc\") p(\"defafter\"))"),
            ("declared", true, "dbox(p(\"abc\") p(\"defafter\"))"),
            ("declared2", false, "dbox(dbox(p(\"abc\") p(\"defafter\")))"),
            ("declared2", true, "dbox(dbox(p(\"abc\") p(\"defafter\")))"),
            ("shadow", false, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow", true, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            ("slot-shadow", false, "p(\"after\")"),
            ("slot-shadow", true, "p(\"after\")"),
            ("slot-bare", false, "p(\"after\")"),
            ("slot-bare", true, "p(\"after\")"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in slot-bare (NodeStructureExtension: true)
#[test]
fn backspace_at_container_start() {
    check_command(
        "backspace-at-container-start",
        &[
            ("root", false, "p(\"beforeabc\") / p(\"def\")"),
            ("root", true, "p(\"beforeabc\") / p(\"def\")"),
            ("plain", false, "p(\"beforeabc\") / box(p(\"def\"))"),
            ("plain", true, "p(\"beforeabc\") / box(p(\"def\"))"),
            ("plain2", false, "p(\"beforeabc\") / box(box(p(\"def\")))"),
            ("plain2", true, "p(\"beforeabc\") / box(box(p(\"def\")))"),
            ("declared", false, "p(\"beforeabc\") / dbox(p(\"def\"))"),
            ("declared", true, "p(\"beforeabc\") / dbox(p(\"def\"))"),
            (
                "declared2",
                false,
                "p(\"beforeabc\") / dbox(dbox(p(\"def\")))",
            ),
            (
                "declared2",
                true,
                "p(\"beforeabc\") / dbox(dbox(p(\"def\")))",
            ),
            ("shadow", false, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow", true, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:p(\"abc\")}()"),
            ("slot-bare", true, "component{body:p(\"abc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in slot-shadow (NodeStructureExtension: true)
#[test]
fn delete_at_container_end() {
    check_command(
        "delete-at-container-end",
        &[
            ("root", false, "p(\"abc\") / p(\"defafter\")"),
            ("root", true, "p(\"abc\") / p(\"defafter\")"),
            ("plain", false, "box(p(\"abc\") p(\"defafter\"))"),
            ("plain", true, "box(p(\"abc\") p(\"defafter\"))"),
            ("plain2", false, "box(box(p(\"abc\") p(\"defafter\")))"),
            ("plain2", true, "box(box(p(\"abc\") p(\"defafter\")))"),
            ("declared", false, "dbox(p(\"abc\") p(\"defafter\"))"),
            ("declared", true, "dbox(p(\"abc\") p(\"defafter\"))"),
            ("declared2", false, "dbox(dbox(p(\"abc\") p(\"defafter\")))"),
            ("declared2", true, "dbox(dbox(p(\"abc\") p(\"defafter\")))"),
            ("shadow", false, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow", true, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in slot-bare (NodeStructureExtension: true)
#[test]
fn enter_empty_last_listitem() {
    check_command(
        "enter-empty-last-listitem",
        &[
            ("root", false, "list[bullet](li(\"abc\")) / p()"),
            ("root", true, "list[bullet](li(\"abc\")) / p()"),
            ("plain", false, "box(list[bullet](li(\"abc\") li() li()))"),
            ("plain", true, "box(list[bullet](li(\"abc\") li() li()))"),
            (
                "plain2",
                false,
                "box(box(list[bullet](li(\"abc\") li() li())))",
            ),
            (
                "plain2",
                true,
                "box(box(list[bullet](li(\"abc\") li() li())))",
            ),
            (
                "declared",
                false,
                "dbox(list[bullet](li(\"abc\") li() li()))",
            ),
            ("declared", true, "dbox(list[bullet](li(\"abc\")) p())"),
            (
                "declared2",
                false,
                "dbox(dbox(list[bullet](li(\"abc\") li() li())))",
            ),
            (
                "declared2",
                true,
                "dbox(dbox(list[bullet](li(\"abc\")) p()))",
            ),
            ("shadow", false, "sbox(list[bullet](li(\"abc\")) p())"),
            ("shadow", true, "sbox(list[bullet](li(\"abc\")) p())"),
            (
                "shadow2",
                false,
                "sbox(sbox(list[bullet](li(\"abc\")) p()))",
            ),
            ("shadow2", true, "sbox(sbox(list[bullet](li(\"abc\")) p()))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(list[bullet](li(\"abc\")) p())}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(list[bullet](li(\"abc\")) p())}()",
            ),
            (
                "slot-bare",
                false,
                "component{body:list[bullet](li(\"abc\") li() li())}()",
            ),
            (
                "slot-bare",
                true,
                "component{body:list[bullet](li(\"abc\") li() li())}()",
            ),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in slot-bare (NodeStructureExtension: true)
#[test]
fn indent_command() {
    check_command(
        "indent",
        &[
            ("root", false, "p[indent1](\"abc\") / p(\"def\")"),
            ("root", true, "p[indent1](\"abc\") / p(\"def\")"),
            ("plain", false, "box(p[indent1](\"abc\") p(\"def\"))"),
            ("plain", true, "box(p[indent1](\"abc\") p(\"def\"))"),
            ("plain2", false, "box(box(p[indent1](\"abc\") p(\"def\")))"),
            ("plain2", true, "box(box(p[indent1](\"abc\") p(\"def\")))"),
            ("declared", false, "dbox(p[indent1](\"abc\") p(\"def\"))"),
            ("declared", true, "dbox(p[indent1](\"abc\") p(\"def\"))"),
            (
                "declared2",
                false,
                "dbox(dbox(p[indent1](\"abc\") p(\"def\")))",
            ),
            (
                "declared2",
                true,
                "dbox(dbox(p[indent1](\"abc\") p(\"def\")))",
            ),
            ("shadow", false, "sbox(p[indent1](\"abc\") p(\"def\"))"),
            ("shadow", true, "sbox(p[indent1](\"abc\") p(\"def\"))"),
            (
                "shadow2",
                false,
                "sbox(sbox(p[indent1](\"abc\") p(\"def\")))",
            ),
            (
                "shadow2",
                true,
                "sbox(sbox(p[indent1](\"abc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                false,
                "component{body:sbox(p[indent1](\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(p[indent1](\"abc\") p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:p[indent1](\"abc\")}()"),
            ("slot-bare", true, "component{body:p[indent1](\"abc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in slot-bare (NodeStructureExtension: true)
#[test]
fn insert_list_collapsed() {
    check_command(
        "insertList-collapsed",
        &[
            ("root", false, "list[bullet](li(\"abc\")) / p(\"def\")"),
            ("root", true, "list[bullet](li(\"abc\")) / p(\"def\")"),
            ("plain", false, "list[bullet](li(p(\"abc\") p(\"def\")))"),
            ("plain", true, "list[bullet](li(\"abc\" br \"def\"))"),
            (
                "plain2",
                false,
                "list[bullet](li(box(p(\"abc\") p(\"def\"))))",
            ),
            (
                "plain2",
                true,
                "box(box(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            ("declared", false, "list[bullet](li(p(\"abc\") p(\"def\")))"),
            (
                "declared",
                true,
                "dbox(list[bullet](li(\"abc\")) p(\"def\"))",
            ),
            (
                "declared2",
                false,
                "list[bullet](li(dbox(p(\"abc\") p(\"def\"))))",
            ),
            (
                "declared2",
                true,
                "dbox(dbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "shadow",
                false,
                "sbox(list[bullet](li(\"abc\")) p(\"def\"))",
            ),
            ("shadow", true, "sbox(list[bullet](li(\"abc\")) p(\"def\"))"),
            (
                "shadow2",
                false,
                "sbox(sbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "shadow2",
                true,
                "sbox(sbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "slot-shadow",
                false,
                "component{body:sbox(list[bullet](li(\"abc\")) p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(list[bullet](li(\"abc\")) p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:p(\"abc\")}()"),
            ("slot-bare", true, "component{body:p(\"abc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in slot-bare (NodeStructureExtension: true)
#[test]
fn insert_list_quote_in_container() {
    check_command(
        "insertList-quote-in-container",
        &[
            ("root", false, "list[bullet](li(\"abc\")) / p(\"def\")"),
            ("root", true, "list[bullet](li(\"abc\")) / p(\"def\")"),
            (
                "plain",
                false,
                "list[bullet](li(quote(\"abc\") p(\"def\")))",
            ),
            ("plain", true, "box(list[bullet](li(\"abc\")) p(\"def\"))"),
            (
                "plain2",
                false,
                "list[bullet](li(box(quote(\"abc\") p(\"def\"))))",
            ),
            (
                "plain2",
                true,
                "box(box(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "declared",
                false,
                "list[bullet](li(quote(\"abc\") p(\"def\")))",
            ),
            (
                "declared",
                true,
                "dbox(list[bullet](li(\"abc\")) p(\"def\"))",
            ),
            (
                "declared2",
                false,
                "list[bullet](li(dbox(quote(\"abc\") p(\"def\"))))",
            ),
            (
                "declared2",
                true,
                "dbox(dbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "shadow",
                false,
                "sbox(list[bullet](li(\"abc\")) p(\"def\"))",
            ),
            ("shadow", true, "sbox(list[bullet](li(\"abc\")) p(\"def\"))"),
            (
                "shadow2",
                false,
                "sbox(sbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "shadow2",
                true,
                "sbox(sbox(list[bullet](li(\"abc\")) p(\"def\")))",
            ),
            (
                "slot-shadow",
                false,
                "component{body:sbox(list[bullet](li(\"abc\")) p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(list[bullet](li(\"abc\")) p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:quote(\"abc\")}()"),
            ("slot-bare", true, "component{body:quote(\"abc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in slot-shadow (NodeStructureExtension: true)
#[test]
fn insert_list_range() {
    check_command(
        "insertList-range",
        &[
            ("root", false, "list[number](li(\"abc\") li(\"def\"))"),
            ("root", true, "list[number](li(\"abc\") li(\"def\"))"),
            ("plain", false, "list[number](li(p(\"abc\") p(\"def\")))"),
            ("plain", true, "list[number](li(\"abc\" br \"def\"))"),
            (
                "plain2",
                false,
                "list[number](li(box(p(\"abc\") p(\"def\"))))",
            ),
            (
                "plain2",
                true,
                "box(box(list[number](li(\"abc\") li(\"def\"))))",
            ),
            ("declared", false, "list[number](li(p(\"abc\") p(\"def\")))"),
            (
                "declared",
                true,
                "dbox(list[number](li(\"abc\") li(\"def\")))",
            ),
            (
                "declared2",
                false,
                "list[number](li(dbox(p(\"abc\") p(\"def\"))))",
            ),
            (
                "declared2",
                true,
                "dbox(dbox(list[number](li(\"abc\") li(\"def\"))))",
            ),
            (
                "shadow",
                false,
                "sbox(list[number](li(\"abc\") li(\"def\")))",
            ),
            (
                "shadow",
                true,
                "sbox(list[number](li(\"abc\") li(\"def\")))",
            ),
            (
                "shadow2",
                false,
                "sbox(sbox(list[number](li(\"abc\") li(\"def\"))))",
            ),
            (
                "shadow2",
                true,
                "sbox(sbox(list[number](li(\"abc\") li(\"def\"))))",
            ),
            (
                "slot-shadow",
                false,
                "component{body:sbox(list[number](li(\"abc\") li(\"def\")))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(list[number](li(\"abc\") li(\"def\")))}()",
            ),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in slot-shadow (NodeStructureExtension: true)
#[test]
fn insert_node_to_nearest_root_hr_end() {
    check_command(
        "insertNodeToNearestRoot-hr-end",
        &[
            ("root", false, "p(\"abc\") / p(\"def\") / horizontalrule()"),
            ("root", true, "p(\"abc\") / p(\"def\") / horizontalrule()"),
            (
                "plain",
                false,
                "box(p(\"abc\") p(\"def\")) / horizontalrule()",
            ),
            (
                "plain",
                true,
                "box(p(\"abc\") p(\"def\")) / horizontalrule()",
            ),
            (
                "plain2",
                false,
                "box(box(p(\"abc\") p(\"def\"))) / horizontalrule()",
            ),
            (
                "plain2",
                true,
                "box(box(p(\"abc\") p(\"def\"))) / horizontalrule()",
            ),
            (
                "declared",
                false,
                "dbox(p(\"abc\") p(\"def\")) / horizontalrule()",
            ),
            (
                "declared",
                true,
                "dbox(p(\"abc\") p(\"def\") horizontalrule() p())",
            ),
            (
                "declared2",
                false,
                "dbox(dbox(p(\"abc\") p(\"def\"))) / horizontalrule()",
            ),
            (
                "declared2",
                true,
                "dbox(dbox(p(\"abc\") p(\"def\") horizontalrule() p()))",
            ),
            (
                "shadow",
                false,
                "sbox(p(\"abc\") p(\"def\") horizontalrule() p())",
            ),
            (
                "shadow",
                true,
                "sbox(p(\"abc\") p(\"def\") horizontalrule() p())",
            ),
            (
                "shadow2",
                false,
                "sbox(sbox(p(\"abc\") p(\"def\") horizontalrule() p()))",
            ),
            (
                "shadow2",
                true,
                "sbox(sbox(p(\"abc\") p(\"def\") horizontalrule() p()))",
            ),
            (
                "slot-shadow",
                false,
                "component{body:sbox(p(\"abc\") p(\"def\") horizontalrule() p())}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(p(\"abc\") p(\"def\") horizontalrule() p())}()",
            ),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in slot-bare (NodeStructureExtension: true)
#[test]
fn insert_node_to_nearest_root_hr_mid() {
    check_command(
        "insertNodeToNearestRoot-hr-mid",
        &[
            (
                "root",
                false,
                "p(\"a\") / horizontalrule() / p(\"bc\") / p(\"def\")",
            ),
            (
                "root",
                true,
                "p(\"a\") / horizontalrule() / p(\"bc\") / p(\"def\")",
            ),
            (
                "plain",
                false,
                "box(p(\"a\")) / horizontalrule() / box(p(\"bc\") p(\"def\"))",
            ),
            (
                "plain",
                true,
                "box(p(\"a\")) / horizontalrule() / box(p(\"bc\") p(\"def\"))",
            ),
            (
                "plain2",
                false,
                "box(box(p(\"a\"))) / horizontalrule() / box(box(p(\"bc\") p(\"def\")))",
            ),
            (
                "plain2",
                true,
                "box(box(p(\"a\"))) / horizontalrule() / box(box(p(\"bc\") p(\"def\")))",
            ),
            (
                "declared",
                false,
                "dbox(p(\"a\")) / horizontalrule() / dbox(p(\"bc\") p(\"def\"))",
            ),
            (
                "declared",
                true,
                "dbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))",
            ),
            (
                "declared2",
                false,
                "dbox(dbox(p(\"a\"))) / horizontalrule() / dbox(dbox(p(\"bc\") p(\"def\")))",
            ),
            (
                "declared2",
                true,
                "dbox(dbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\")))",
            ),
            (
                "shadow",
                false,
                "sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))",
            ),
            (
                "shadow",
                true,
                "sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))",
            ),
            (
                "shadow2",
                false,
                "sbox(sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\")))",
            ),
            (
                "shadow2",
                true,
                "sbox(sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                false,
                "component{body:sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(p(\"a\") horizontalrule() p(\"bc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                false,
                "component{body:p(\"a\" horizontalrule() \"bc\")}()",
            ),
            (
                "slot-bare",
                true,
                "component{body:p(\"a\" horizontalrule() \"bc\")}()",
            ),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in slot-bare (NodeStructureExtension: true)
#[test]
fn insert_nodes_blocks() {
    check_command(
        "insertNodes-blocks",
        &[
            ("root", false, "p(\"aX\") / p(\"Ybc\") / p(\"def\")"),
            ("root", true, "p(\"aX\") / p(\"Ybc\") / p(\"def\")"),
            ("plain", false, "box(p(\"aXYbc\") p(\"def\"))"),
            ("plain", true, "box(p(\"aXYbc\") p(\"def\"))"),
            ("plain2", false, "box(box(p(\"aXYbc\") p(\"def\")))"),
            ("plain2", true, "box(box(p(\"aXYbc\") p(\"def\")))"),
            ("declared", false, "dbox(p(\"aXYbc\") p(\"def\"))"),
            ("declared", true, "dbox(p(\"aX\") p(\"Ybc\") p(\"def\"))"),
            ("declared2", false, "dbox(dbox(p(\"aXYbc\") p(\"def\")))"),
            (
                "declared2",
                true,
                "dbox(dbox(p(\"aX\") p(\"Ybc\") p(\"def\")))",
            ),
            ("shadow", false, "sbox(p(\"aX\") p(\"Ybc\") p(\"def\"))"),
            ("shadow", true, "sbox(p(\"aX\") p(\"Ybc\") p(\"def\"))"),
            (
                "shadow2",
                false,
                "sbox(sbox(p(\"aX\") p(\"Ybc\") p(\"def\")))",
            ),
            (
                "shadow2",
                true,
                "sbox(sbox(p(\"aX\") p(\"Ybc\") p(\"def\")))",
            ),
            (
                "slot-shadow",
                false,
                "component{body:sbox(p(\"aX\") p(\"Ybc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(p(\"aX\") p(\"Ybc\") p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:p(\"aXYbc\")}()"),
            ("slot-bare", true, "component{body:p(\"aXYbc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in slot-bare (NodeStructureExtension: true)
#[test]
fn insert_nodes_heading() {
    check_command(
        "insertNodes-heading",
        &[
            ("root", false, "p(\"aHbc\") / p(\"def\")"),
            ("root", true, "p(\"aHbc\") / p(\"def\")"),
            ("plain", false, "box(p(\"aHbc\") p(\"def\"))"),
            ("plain", true, "box(p(\"aHbc\") p(\"def\"))"),
            ("plain2", false, "box(box(p(\"aHbc\") p(\"def\")))"),
            ("plain2", true, "box(box(p(\"aHbc\") p(\"def\")))"),
            ("declared", false, "dbox(p(\"aHbc\") p(\"def\"))"),
            ("declared", true, "dbox(p(\"aHbc\") p(\"def\"))"),
            ("declared2", false, "dbox(dbox(p(\"aHbc\") p(\"def\")))"),
            ("declared2", true, "dbox(dbox(p(\"aHbc\") p(\"def\")))"),
            ("shadow", false, "sbox(p(\"aHbc\") p(\"def\"))"),
            ("shadow", true, "sbox(p(\"aHbc\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(p(\"aHbc\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(p(\"aHbc\") p(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(p(\"aHbc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(p(\"aHbc\") p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:p(\"aHbc\")}()"),
            ("slot-bare", true, "component{body:p(\"aHbc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in slot-shadow (NodeStructureExtension: true)
#[test]
fn markdown_heading() {
    check_command(
        "markdown-heading",
        &[
            ("root", false, "h(\"x\") / p(\"def\")"),
            ("root", true, "h(\"x\") / p(\"def\")"),
            ("plain", false, "box(p(\"# x\") p(\"def\"))"),
            ("plain", true, "box(p(\"# x\") p(\"def\"))"),
            ("plain2", false, "box(box(p(\"# x\") p(\"def\")))"),
            ("plain2", true, "box(box(p(\"# x\") p(\"def\")))"),
            ("declared", false, "dbox(p(\"# x\") p(\"def\"))"),
            ("declared", true, "dbox(h(\"x\") p(\"def\"))"),
            ("declared2", false, "dbox(dbox(p(\"# x\") p(\"def\")))"),
            ("declared2", true, "dbox(dbox(h(\"x\") p(\"def\")))"),
            ("shadow", false, "sbox(h(\"x\") p(\"def\"))"),
            ("shadow", true, "sbox(h(\"x\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(h(\"x\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(h(\"x\") p(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(h(\"x\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(h(\"x\") p(\"def\"))}()",
            ),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in slot-shadow (NodeStructureExtension: true)
#[test]
fn markdown_list() {
    check_command(
        "markdown-list",
        &[
            ("root", false, "list[bullet](li(\"x\")) / p(\"def\")"),
            ("root", true, "list[bullet](li(\"x\")) / p(\"def\")"),
            ("plain", false, "box(p(\"- x\") p(\"def\"))"),
            ("plain", true, "box(p(\"- x\") p(\"def\"))"),
            ("plain2", false, "box(box(p(\"- x\") p(\"def\")))"),
            ("plain2", true, "box(box(p(\"- x\") p(\"def\")))"),
            ("declared", false, "dbox(p(\"- x\") p(\"def\"))"),
            ("declared", true, "dbox(list[bullet](li(\"x\")) p(\"def\"))"),
            ("declared2", false, "dbox(dbox(p(\"- x\") p(\"def\")))"),
            (
                "declared2",
                true,
                "dbox(dbox(list[bullet](li(\"x\")) p(\"def\")))",
            ),
            ("shadow", false, "sbox(list[bullet](li(\"x\")) p(\"def\"))"),
            ("shadow", true, "sbox(list[bullet](li(\"x\")) p(\"def\"))"),
            (
                "shadow2",
                false,
                "sbox(sbox(list[bullet](li(\"x\")) p(\"def\")))",
            ),
            (
                "shadow2",
                true,
                "sbox(sbox(list[bullet](li(\"x\")) p(\"def\")))",
            ),
            (
                "slot-shadow",
                false,
                "component{body:sbox(list[bullet](li(\"x\")) p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(list[bullet](li(\"x\")) p(\"def\"))}()",
            ),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in slot-bare (NodeStructureExtension: true)
#[test]
fn outdent_command() {
    check_command(
        "outdent",
        &[
            ("root", false, "p(\"abc\") / p(\"def\")"),
            ("root", true, "p(\"abc\") / p(\"def\")"),
            ("plain", false, "box(p(\"abc\") p(\"def\"))"),
            ("plain", true, "box(p(\"abc\") p(\"def\"))"),
            ("plain2", false, "box(box(p(\"abc\") p(\"def\")))"),
            ("plain2", true, "box(box(p(\"abc\") p(\"def\")))"),
            ("declared", false, "dbox(p(\"abc\") p(\"def\"))"),
            ("declared", true, "dbox(p(\"abc\") p(\"def\"))"),
            ("declared2", false, "dbox(dbox(p(\"abc\") p(\"def\")))"),
            ("declared2", true, "dbox(dbox(p(\"abc\") p(\"def\")))"),
            ("shadow", false, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow", true, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:p(\"abc\")}()"),
            ("slot-bare", true, "component{body:p(\"abc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in slot-bare (NodeStructureExtension: true)
#[test]
fn remove_list_command() {
    check_command(
        "removeList",
        &[
            ("root", false, "p(\"abc\") / p(\"def\")"),
            ("root", true, "p(\"abc\") / p(\"def\")"),
            ("plain", false, "box(p(\"abc\") p(\"def\"))"),
            ("plain", true, "box(p(\"abc\") p(\"def\"))"),
            ("plain2", false, "box(box(p(\"abc\") p(\"def\")))"),
            ("plain2", true, "box(box(p(\"abc\") p(\"def\")))"),
            ("declared", false, "dbox(p(\"abc\") p(\"def\"))"),
            ("declared", true, "dbox(p(\"abc\") p(\"def\"))"),
            ("declared2", false, "dbox(dbox(p(\"abc\") p(\"def\")))"),
            ("declared2", true, "dbox(dbox(p(\"abc\") p(\"def\")))"),
            ("shadow", false, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow", true, "sbox(p(\"abc\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(p(\"abc\") p(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(p(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-bare",
                false,
                "component{body:list[bullet](li(\"abc\") li(\"def\"))}()",
            ),
            (
                "slot-bare",
                true,
                "component{body:list[bullet](li(\"abc\") li(\"def\"))}()",
            ),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in slot-shadow (NodeStructureExtension: true)
#[test]
fn set_blocks_type_code_range() {
    check_command(
        "setBlocksType-code-range",
        &[
            ("root", false, "code(\"abc\") / code(\"def\")"),
            ("root", true, "code(\"abc\") / code(\"def\")"),
            ("plain", false, "box(code(\"abc\") code(\"def\"))"),
            ("plain", true, "box(code(\"abc\") code(\"def\"))"),
            ("plain2", false, "box(box(code(\"abc\") code(\"def\")))"),
            ("plain2", true, "box(box(code(\"abc\") code(\"def\")))"),
            ("declared", false, "dbox(code(\"abc\") code(\"def\"))"),
            ("declared", true, "dbox(code(\"abc\") code(\"def\"))"),
            (
                "declared2",
                false,
                "dbox(dbox(code(\"abc\") code(\"def\")))",
            ),
            ("declared2", true, "dbox(dbox(code(\"abc\") code(\"def\")))"),
            ("shadow", false, "sbox(code(\"abc\") code(\"def\"))"),
            ("shadow", true, "sbox(code(\"abc\") code(\"def\"))"),
            ("shadow2", false, "sbox(sbox(code(\"abc\") code(\"def\")))"),
            ("shadow2", true, "sbox(sbox(code(\"abc\") code(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(code(\"abc\") code(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(code(\"abc\") code(\"def\"))}()",
            ),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in slot-bare (NodeStructureExtension: true)
#[test]
fn set_blocks_type_heading() {
    check_command(
        "setBlocksType-heading",
        &[
            ("root", false, "h(\"abc\") / p(\"def\")"),
            ("root", true, "h(\"abc\") / p(\"def\")"),
            ("plain", false, "box(h(\"abc\") p(\"def\"))"),
            ("plain", true, "box(h(\"abc\") p(\"def\"))"),
            ("plain2", false, "box(box(h(\"abc\") p(\"def\")))"),
            ("plain2", true, "box(box(h(\"abc\") p(\"def\")))"),
            ("declared", false, "dbox(h(\"abc\") p(\"def\"))"),
            ("declared", true, "dbox(h(\"abc\") p(\"def\"))"),
            ("declared2", false, "dbox(dbox(h(\"abc\") p(\"def\")))"),
            ("declared2", true, "dbox(dbox(h(\"abc\") p(\"def\")))"),
            ("shadow", false, "sbox(h(\"abc\") p(\"def\"))"),
            ("shadow", true, "sbox(h(\"abc\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(h(\"abc\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(h(\"abc\") p(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(h(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(h(\"abc\") p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:p(\"abc\")}()"),
            ("slot-bare", true, "component{body:p(\"abc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in slot-bare (NodeStructureExtension: true)
#[test]
fn set_blocks_type_quote() {
    check_command(
        "setBlocksType-quote",
        &[
            ("root", false, "quote(\"abc\") / p(\"def\")"),
            ("root", true, "quote(\"abc\") / p(\"def\")"),
            ("plain", false, "box(quote(\"abc\") p(\"def\"))"),
            ("plain", true, "box(quote(\"abc\") p(\"def\"))"),
            ("plain2", false, "box(box(quote(\"abc\") p(\"def\")))"),
            ("plain2", true, "box(box(quote(\"abc\") p(\"def\")))"),
            ("declared", false, "dbox(quote(\"abc\") p(\"def\"))"),
            ("declared", true, "dbox(quote(\"abc\") p(\"def\"))"),
            ("declared2", false, "dbox(dbox(quote(\"abc\") p(\"def\")))"),
            ("declared2", true, "dbox(dbox(quote(\"abc\") p(\"def\")))"),
            ("shadow", false, "sbox(quote(\"abc\") p(\"def\"))"),
            ("shadow", true, "sbox(quote(\"abc\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(quote(\"abc\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(quote(\"abc\") p(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(quote(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(quote(\"abc\") p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:p(\"abc\")}()"),
            ("slot-bare", true, "component{body:p(\"abc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in root (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in plain (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in plain (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in plain2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in plain2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in declared (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in declared (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in declared2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in declared2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in shadow2 (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in shadow2 (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in slot-shadow (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in slot-shadow (NodeStructureExtension: true)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in slot-bare (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in slot-bare (NodeStructureExtension: true)
#[test]
fn wrap_nodes_quote() {
    check_command(
        "wrapNodes-quote",
        &[
            ("root", false, "quote(\"abc\") / p(\"def\")"),
            ("root", true, "quote(\"abc\") / p(\"def\")"),
            ("plain", false, "quote(\"abc\") / box(p(\"def\"))"),
            ("plain", true, "quote(\"abc\") / box(p(\"def\"))"),
            ("plain2", false, "quote(\"abc\") / box(box(p(\"def\")))"),
            ("plain2", true, "quote(\"abc\") / box(box(p(\"def\")))"),
            ("declared", false, "quote(\"abc\") / dbox(p(\"def\"))"),
            ("declared", true, "dbox(quote(\"abc\") p(\"def\"))"),
            (
                "declared2",
                false,
                "quote(\"abc\") / dbox(dbox(p(\"def\")))",
            ),
            ("declared2", true, "dbox(dbox(quote(\"abc\") p(\"def\")))"),
            ("shadow", false, "sbox(quote(\"abc\") p(\"def\"))"),
            ("shadow", true, "sbox(quote(\"abc\") p(\"def\"))"),
            ("shadow2", false, "sbox(sbox(quote(\"abc\") p(\"def\")))"),
            ("shadow2", true, "sbox(sbox(quote(\"abc\") p(\"def\")))"),
            (
                "slot-shadow",
                false,
                "component{body:sbox(quote(\"abc\") p(\"def\"))}()",
            ),
            (
                "slot-shadow",
                true,
                "component{body:sbox(quote(\"abc\") p(\"def\"))}()",
            ),
            ("slot-bare", false, "component{body:p(\"abc\")}()"),
            ("slot-bare", true, "component{body:p(\"abc\")}()"),
        ],
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > every command has a row of cases
#[test]
fn every_command_has_a_row_of_cases() {
    let mut rows: Vec<&str> = EXPECTED.iter().map(|(cmd, _)| *cmd).collect();
    rows.sort_unstable();
    let mut cmds = CMDS.to_vec();
    cmds.sort_unstable();
    assert_eq!(rows, cmds);
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > a declared container behaves like a shadow root for every command but the joins at its edges
#[test]
fn a_declared_container_behaves_like_a_shadow_root_for_every_command_but_the_joins_at_its_edges() {
    for (cmd, by_ctx) in EXPECTED {
        let at = |ctx: &str| {
            let (_, _, with) = by_ctx.iter().find(|(c, _, _)| *c == ctx).unwrap();
            with.replace("dbox", "B").replace("sbox", "B")
        };
        let joins = cmd.starts_with("backspace") || cmd.starts_with("delete");
        for (declared, shadow) in [("declared", "shadow"), ("declared2", "shadow2")] {
            if joins {
                assert_ne!(at(declared), at(shadow), "{cmd}");
            } else {
                assert_eq!(at(declared), at(shadow), "{cmd}");
            }
        }
    }
}

/// `$isBlockContainer`'s case: `mode` is absent, disabled or enforced.
fn is_block_container_case(mode: &str, structure: bool) {
    let mut dependencies = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::list::list_extension().into(),
    ];
    if mode != "absent" {
        dependencies.push(config_extension(
            &node_structure_extension(),
            [crate::extension::NodeStructureConfigOverrides {
                disabled: Some(mode == "disabled"),
            }],
        ));
    }
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        nodes: vec![
            NodeClass::of::<PlainBoxNode>(),
            NodeClass::of::<DeclaredBoxNode>(),
            NodeClass::of::<DeclaredBoxSubclassNode>(),
            NodeClass::of::<InlineDeclaredBoxNode>(),
            NodeClass::of::<ShadowBoxNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("is-block-container", ())
    })
    .into()])
    .unwrap();
    update(&editor, move || {
        let list = bullet_list(vec![item(vec![create_text_node("x")])]);
        let nodes: Vec<(&str, NodeRef, bool)> = vec![
            ("declared", create::<DeclaredBoxNode>(), structure),
            ("inherited", create::<DeclaredBoxSubclassNode>(), structure),
            ("inline", create::<InlineDeclaredBoxNode>(), false),
            ("listItem", list.get_first_child_or_throw()?, false),
            ("list", list, false),
            ("paragraph", create_paragraph_node(), false),
            ("plain", create::<PlainBoxNode>(), false),
            ("quote", create_quote_node(false), false),
            ("root", get_root(), true),
            ("shadow", create::<ShadowBoxNode>(), true),
            ("shadowQuote", create_quote_node(true), true),
            ("text", create_text_node("x"), false),
        ];
        for (name, node, expected) in nodes {
            assert_eq!(is_block_container(&node), expected, "{name}");
        }
        // `$isBlockContainer(null)` is false: Rust's takes a node.
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::$isBlockContainer > reads a children: block declaration only where NodeStructureExtension enforces it (absent)
#[test]
fn reads_a_children_block_declaration_only_where_node_structure_extension_enforces_it_absent() {
    is_block_container_case("absent", false);
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::$isBlockContainer > reads a children: block declaration only where NodeStructureExtension enforces it (disabled)
#[test]
fn reads_a_children_block_declaration_only_where_node_structure_extension_enforces_it_disabled() {
    is_block_container_case("disabled", false);
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::$isBlockContainer > reads a children: block declaration only where NodeStructureExtension enforces it (enforced)
#[test]
fn reads_a_children_block_declaration_only_where_node_structure_extension_enforces_it_enforced() {
    is_block_container_case("enforced", true);
}

/// `$range(empty)`: p("before") / empty / p("after"), from "b|efore" to
/// "a|fter".
fn range_around(empty: NodeRef) -> R {
    let a = create_text_node("before");
    let b = create_text_node("after");
    cleared_root()?.append(&[
        append(create_paragraph_node(), vec![a.clone()]),
        empty,
        append(create_paragraph_node(), vec![b.clone()]),
    ])?;
    select_range(&a, 1, &b, 1)
}

/// `$inBox(offset)`.
fn in_box(offset: usize) -> R {
    let b = append(create::<DeclaredBoxNode>(), vec![p("abc")]);
    cleared_root()?.append(std::slice::from_ref(&b))?;
    b.select(Some(offset), Some(offset))?;
    Ok(())
}

/// Selects `nodes` in a node selection.
fn select_nodes(nodes: &[&NodeRef]) {
    let selection = create_node_selection();
    for node in nodes {
        selection.add(node.get_key());
    }
    set_selection(Some(selection));
}

/// The case of `selections that pass over or sit on a container`.
fn selection_case(act: impl FnOnce() -> R + 'static, expected: &str) {
    let editor = build_editor(true);
    update(&editor, act);
    assert_eq!(read_shape(&editor), expected);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty declared box in a range: $insertList (NodeStructureExtension)
#[test]
fn an_empty_declared_box_in_a_range_insert_list() {
    selection_case(
        || {
            range_around(create::<DeclaredBoxNode>())?;
            insert_list(ListType::Bullet)
        },
        r#"list[bullet](li("before")) / dbox() / list[bullet](li("after"))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty shadow root in a range: $insertList (NodeStructureExtension)
#[test]
fn an_empty_shadow_root_in_a_range_insert_list() {
    selection_case(
        || {
            range_around(create::<ShadowBoxNode>())?;
            insert_list(ListType::Bullet)
        },
        r#"list[bullet](li("before")) / sbox() / list[bullet](li("after"))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty declared box in a range: $setBlocksType (NodeStructureExtension)
#[test]
fn an_empty_declared_box_in_a_range_set_blocks_type() {
    selection_case(
        || {
            range_around(create::<DeclaredBoxNode>())?;
            set_blocks_type(get_selection().as_ref(), heading, None)
        },
        r#"h("before") / dbox() / h("after")"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an element point on a declared box: insertParagraph (NodeStructureExtension)
#[test]
fn an_element_point_on_a_declared_box_insert_paragraph() {
    selection_case(
        || {
            in_box(1)?;
            selection().insert_paragraph()?;
            Ok(())
        },
        r#"dbox(p("abc") p())"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an element point on a declared box: insertNodes (NodeStructureExtension)
#[test]
fn an_element_point_on_a_declared_box_insert_nodes() {
    selection_case(
        || {
            in_box(1)?;
            selection().insert_nodes(vec![p("X")])
        },
        r#"dbox(p("abc") p("X"))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an element point on a declared box: $insertList (NodeStructureExtension)
#[test]
fn an_element_point_on_a_declared_box_insert_list() {
    selection_case(
        || {
            in_box(0)?;
            insert_list(ListType::Bullet)
        },
        r#"dbox(list[bullet](li("abc")))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range from before a declared box into it: $insertList (NodeStructureExtension)
#[test]
fn a_range_from_before_a_declared_box_into_it_insert_list() {
    selection_case(
        || {
            let a = create_text_node("before");
            let b = create_text_node("in");
            cleared_root()?.append(&[
                append(create_paragraph_node(), vec![a.clone()]),
                append(
                    create::<DeclaredBoxNode>(),
                    vec![append(create_paragraph_node(), vec![b.clone()])],
                ),
            ])?;
            select_range(&a, 1, &b, 1)?;
            insert_list(ListType::Bullet)
        },
        r#"list[bullet](li("before")) / dbox(list[bullet](li("in")))"#,
    );
}

/// A horizontal rule after p("abc") in a container of `kind`, selected.
fn rule_selected_in(kind: &str) -> R {
    let hr = create_horizontal_rule_node();
    cleared_root()?.append(&[append(make(kind), vec![p("abc"), hr.clone()])])?;
    select_nodes(&[&hr]);
    Ok(())
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a horizontal rule selected in a declared box: $insertList (NodeStructureExtension)
#[test]
fn a_horizontal_rule_selected_in_a_declared_box_insert_list() {
    selection_case(
        || {
            rule_selected_in("declared")?;
            insert_list(ListType::Bullet)
        },
        r#"dbox(p("abc") horizontalrule())"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a horizontal rule selected in a shadow root: $insertList (NodeStructureExtension)
#[test]
fn a_horizontal_rule_selected_in_a_shadow_root_insert_list() {
    selection_case(
        || {
            rule_selected_in("shadow")?;
            insert_list(ListType::Bullet)
        },
        r#"sbox(p("abc") horizontalrule())"#,
    );
}

/// p("abc") / hr / p("def") in a declared box, from "a|bc" to "d|ef".
fn range_over_rule_in_declared_box() -> R {
    cleared_root()?.append(&[append(
        create::<DeclaredBoxNode>(),
        vec![p("abc"), create_horizontal_rule_node(), p("def")],
    )])?;
    select_range(&find_text("abc"), 1, &find_text("def"), 1)
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range over a horizontal rule in a declared box: $insertList (NodeStructureExtension)
#[test]
fn a_range_over_a_horizontal_rule_in_a_declared_box_insert_list() {
    selection_case(
        || {
            range_over_rule_in_declared_box()?;
            insert_list(ListType::Bullet)
        },
        r#"dbox(list[bullet](li("abc")) horizontalrule() list[bullet](li("def")))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range over a horizontal rule in a declared box: $wrapNodes (NodeStructureExtension)
#[test]
fn a_range_over_a_horizontal_rule_in_a_declared_box_wrap_nodes() {
    selection_case(
        || {
            range_over_rule_in_declared_box()?;
            wrap_nodes(&selection(), quote, None)
        },
        r#"dbox(quote("abc") horizontalrule() quote("def"))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range over a horizontal rule at the root: $wrapNodes (NodeStructureExtension)
#[test]
fn a_range_over_a_horizontal_rule_at_the_root_wrap_nodes() {
    selection_case(
        || {
            cleared_root()?.append(&[p("abc"), create_horizontal_rule_node(), p("def")])?;
            select_range(&find_text("abc"), 1, &find_text("def"), 1)?;
            wrap_nodes(&selection(), quote, None)
        },
        r#"quote("abc") / horizontalrule() / quote("def")"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range over a quote and a paragraph in a box that declares nothing: $insertList (NodeStructureExtension)
#[test]
fn a_range_over_a_quote_and_a_paragraph_in_a_box_that_declares_nothing_insert_list() {
    selection_case(
        || {
            cleared_root()?.append(&[append(
                create::<PlainBoxNode>(),
                vec![append(quote(), vec![create_text_node("y")]), p("x")],
            )])?;
            select_range(&find_text("y"), 0, &find_text("x"), 1)?;
            insert_list(ListType::Bullet)
        },
        r#"box(list[bullet](li("y") li("x")))"#,
    );
}

/// An empty declared box with p("slot") in its `aside` slot, between
/// p("before") and p("after").
fn empty_declared_box_with_slot() -> Result<NodeRef, LexicalError> {
    let b = create::<DeclaredBoxNode>();
    set_slot(&b, "aside", &p("slot"))?;
    cleared_root()?.append(&[p("before"), b.clone(), p("after")])?;
    Ok(b)
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty declared box with a slot, element point: $wrapNodes (NodeStructureExtension)
#[test]
fn an_empty_declared_box_with_a_slot_element_point_wrap_nodes() {
    selection_case(
        || {
            let b = empty_declared_box_with_slot()?;
            b.select(Some(0), Some(0))?;
            wrap_nodes(&selection(), quote, None)
        },
        r#"p("before") / dbox{aside:p("slot")}(quote()) / p("after")"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty declared box with a slot, node selected: $insertList (NodeStructureExtension)
#[test]
fn an_empty_declared_box_with_a_slot_node_selected_insert_list() {
    selection_case(
        || {
            let b = empty_declared_box_with_slot()?;
            select_nodes(&[&b]);
            insert_list(ListType::Bullet)
        },
        r#"p("before") / dbox{aside:p("slot")}() / p("after")"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a declared box holding only a block decorator, element point: $insertList (NodeStructureExtension)
#[test]
fn a_declared_box_holding_only_a_block_decorator_element_point_insert_list() {
    selection_case(
        || {
            let b = append(create::<DeclaredBoxNode>(), vec![create::<ComponentNode>()]);
            cleared_root()?.append(&[p("before"), b.clone(), p("after")])?;
            b.select(Some(0), Some(0))?;
            insert_list(ListType::Bullet)
        },
        r#"p("before") / dbox(component()) / p("after")"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a declared box as a named-slot value: $wrapNodes (NodeStructureExtension)
#[test]
fn a_declared_box_as_a_named_slot_value_wrap_nodes() {
    selection_case(
        || {
            let host = create::<ComponentNode>();
            let body = append(create::<DeclaredBoxNode>(), vec![p("abc")]);
            set_slot(&host, "body", &body)?;
            cleared_root()?.append(&[host])?;
            find_text("abc").select(Some(1), Some(1))?;
            wrap_nodes(&selection(), quote, None)
        },
        r#"component{body:dbox(quote("abc"))}()"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty quote and a paragraph of a box that declares nothing, node selected: $insertList (NodeStructureExtension)
#[test]
fn an_empty_quote_and_a_paragraph_of_a_box_that_declares_nothing_node_selected_insert_list() {
    selection_case(
        || {
            let q = quote();
            let text = create_text_node("x");
            let b = append(
                create::<PlainBoxNode>(),
                vec![
                    q.clone(),
                    append(create_paragraph_node(), vec![text.clone()]),
                ],
            );
            set_slot(&b, "aside", &p("slot"))?;
            cleared_root()?.append(&[b])?;
            select_nodes(&[&q, &text]);
            insert_list(ListType::Bullet)
        },
        r#"box{aside:p("slot")}(list[bullet](li() li("x")))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > removing the only, empty, declared box of a shadow root (NodeStructureExtension)
#[test]
fn removing_the_only_empty_declared_box_of_a_shadow_root() {
    selection_case(
        || {
            let shadow = append(create::<ShadowBoxNode>(), vec![create::<DeclaredBoxNode>()]);
            cleared_root()?.append(std::slice::from_ref(&shadow))?;
            shadow.select(Some(0), Some(1))?;
            selection().remove_text()
        },
        "sbox(p())",
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > the caret in an empty declared box: $setBlocksType (NodeStructureExtension)
#[test]
fn the_caret_in_an_empty_declared_box_set_blocks_type() {
    selection_case(
        || {
            let b = create::<DeclaredBoxNode>();
            cleared_root()?.append(&[p("before"), b.clone(), p("after")])?;
            b.select(Some(0), Some(0))?;
            set_blocks_type(get_selection().as_ref(), heading, None)
        },
        r#"p("before") / dbox(h()) / p("after")"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > with no active editor, a declared box is not a container
#[test]
fn with_no_active_editor_a_declared_box_is_not_a_container() {
    let editor = build_editor(true);
    update(&editor, || {
        cleared_root()?.append(&[append(create::<DeclaredBoxNode>(), vec![p("x")])])?;
        Ok(())
    });
    let first = || is_block_container(&get_root().get_first_child().unwrap());
    assert!(!editor.get_editor_state().read(first));
    assert!(editor.read(first).unwrap());
    editor.dispose();
}

/// The case of `fixes every editor gets`, with and without the extension.
fn fix_case(act: fn() -> R, expected: &str) {
    for structure in [false, true] {
        let editor = build_editor(structure);
        update(&editor, act);
        assert_eq!(read_shape(&editor), expected, "structure: {structure}");
        editor.dispose();
    }
}

fn wrap_nodes_in_empty_root() -> R {
    cleared_root()?.select(Some(0), Some(0))?;
    wrap_nodes(&selection(), quote, None)
}

fn wrap_nodes_in_empty_shadow_root() -> R {
    let shadow = create::<ShadowBoxNode>();
    cleared_root()?.append(&[p("before"), shadow.clone(), p("after")])?;
    shadow.select(Some(0), Some(0))?;
    wrap_nodes(&selection(), quote, None)
}

fn insert_list_over_empty_shadow_root() -> R {
    range_around(create::<ShadowBoxNode>())?;
    insert_list(ListType::Bullet)
}

fn insert_list_with_rule_selected_in_shadow_root() -> R {
    rule_selected_in("shadow")?;
    insert_list(ListType::Bullet)
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $wrapNodes in an empty root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $wrapNodes in an empty root (NodeStructureExtension: true)
#[test]
fn wrap_nodes_in_an_empty_root() {
    fix_case(wrap_nodes_in_empty_root, "quote()");
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $wrapNodes in an empty shadow root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $wrapNodes in an empty shadow root (NodeStructureExtension: true)
#[test]
fn wrap_nodes_in_an_empty_shadow_root() {
    fix_case(
        wrap_nodes_in_empty_shadow_root,
        r#"p("before") / sbox(quote()) / p("after")"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $insertList over an empty shadow root in a range (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $insertList over an empty shadow root in a range (NodeStructureExtension: true)
#[test]
fn insert_list_over_an_empty_shadow_root_in_a_range() {
    fix_case(
        insert_list_over_empty_shadow_root,
        r#"list[bullet](li("before")) / sbox() / list[bullet](li("after"))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $insertList with a horizontal rule selected in a shadow root (NodeStructureExtension: false)
// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $insertList with a horizontal rule selected in a shadow root (NodeStructureExtension: true)
#[test]
fn insert_list_with_a_horizontal_rule_selected_in_a_shadow_root() {
    fix_case(
        insert_list_with_rule_selected_in_shadow_root,
        r#"sbox(p("abc") horizontalrule())"#,
    );
}

/// `$emptyBox()`: an empty declared box, indented and centered, between
/// p("before") and p("after").
fn empty_box() -> Result<NodeRef, LexicalError> {
    let b = create::<DeclaredBoxNode>();
    b.set_indent(1.0)?;
    b.set_element_format("center");
    cleared_root()?.append(&[p("before"), b.clone(), p("after")])?;
    Ok(b)
}

/// The block `$emptyBox()`'s box holds: its format type.
fn new_block_format_type(editor: &Editor) -> String {
    editor
        .read(|| {
            get_root()
                .get_child_at_index(1)
                .unwrap()
                .get_first_child()
                .unwrap()
                .element_data()
                .get_format_type()
                .to_owned()
        })
        .unwrap()
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::a new block put in an empty declared container > $setBlocksType does not take the container's indent and format
#[test]
fn set_blocks_type_does_not_take_the_containers_indent_and_format() {
    let editor = build_editor(true);
    update(&editor, || {
        empty_box()?.select(Some(0), Some(0))?;
        set_blocks_type(get_selection().as_ref(), heading, None)
    });
    assert_eq!(
        read_shape(&editor),
        r#"p("before") / dbox[indent1](h()) / p("after")"#
    );
    assert_eq!(new_block_format_type(&editor), "");
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::a new block put in an empty declared container > $wrapNodes does not take the container's indent and format
#[test]
fn wrap_nodes_does_not_take_the_containers_indent_and_format() {
    let editor = build_editor(true);
    update(&editor, || {
        empty_box()?.select(Some(0), Some(0))?;
        wrap_nodes(&selection(), quote, None)
    });
    assert_eq!(
        read_shape(&editor),
        r#"p("before") / dbox[indent1](quote()) / p("after")"#
    );
    assert_eq!(new_block_format_type(&editor), "");
    editor.dispose();
}

/// `${type}:${offset}` of each point of the selection.
fn points(editor: &Editor) -> [String; 2] {
    editor
        .read(|| {
            let selection = selection();
            let at = |which| {
                let point = selection.point(which).unwrap();
                format!("{}:{}", point.get_node().unwrap().get_type(), point.offset)
            };
            [at(Which::Anchor), at(Which::Focus)]
        })
        .unwrap()
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::a new block put in an empty declared container > $setBlocksType keeps a range that starts in the empty container
#[test]
fn set_blocks_type_keeps_a_range_that_starts_in_the_empty_container() {
    let editor = build_editor(true);
    update(&editor, || {
        let b = empty_box()?;
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, b.get_key(), 0, PointType::Element)?;
        selection.point_set(
            Which::Focus,
            find_text("after").get_key(),
            2,
            PointType::Text,
        )?;
        set_selection(Some(selection));
        set_blocks_type(get_selection().as_ref(), heading, None)
    });
    assert_eq!(
        read_shape(&editor),
        r#"p("before") / dbox[indent1](h()) / h("after")"#
    );
    assert_eq!(points(&editor), ["heading:0", "text:2"]);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::a new block put in an empty declared container > $setBlocksType keeps a backward range that ends in the empty container
#[test]
fn set_blocks_type_keeps_a_backward_range_that_ends_in_the_empty_container() {
    let editor = build_editor(true);
    update(&editor, || {
        let b = empty_box()?;
        let selection = create_range_selection();
        selection.point_set(
            Which::Anchor,
            find_text("after").get_key(),
            2,
            PointType::Text,
        )?;
        selection.point_set(Which::Focus, b.get_key(), 0, PointType::Element)?;
        set_selection(Some(selection));
        set_blocks_type(get_selection().as_ref(), heading, None)
    });
    assert_eq!(
        read_shape(&editor),
        r#"p("before") / dbox[indent1](h()) / h("after")"#
    );
    assert_eq!(points(&editor), ["text:2", "heading:0"]);
    editor.dispose();
}

/// `buildOverflowEditor(structure)`.
fn build_overflow_editor(structure: bool) -> LexicalEditorWithDispose {
    let mut dependencies = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::list::list_extension().into(),
    ];
    if structure {
        dependencies.push(node_structure_extension().into());
    }
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        nodes: vec![
            NodeClass::of::<crate::overflow::OverflowNode>(),
            NodeClass::of::<PlainBoxNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("block-containers-overflow", ())
    })
    .into()])
    .unwrap()
}

fn overflow(text: &str) -> NodeRef {
    append(
        crate::overflow::create_overflow_node(),
        vec![create_text_node(text)],
    )
}

/// `$insertList with the caret in an OverflowNode %s`.
fn overflow_case(name: &str, inner: fn() -> Vec<NodeRef>, text: &'static str, expected: &str) {
    for structure in [false, true] {
        if name == "in a box that holds a quote" && !structure {
            continue;
        }
        let editor = build_overflow_editor(structure);
        update(&editor, move || {
            cleared_root()?.append(&inner())?;
            find_text(text).select(Some(1), Some(1))?;
            insert_list(ListType::Bullet)
        });
        assert_eq!(read_shape(&editor), expected, "structure: {structure}");
        editor.dispose();
    }
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::the character limit's OverflowNode > $insertList with the caret in an OverflowNode in a paragraph
#[test]
fn insert_list_with_the_caret_in_an_overflow_node_in_a_paragraph() {
    overflow_case(
        "in a paragraph",
        || {
            vec![append(
                create_paragraph_node(),
                vec![create_text_node("abc"), overflow("def")],
            )]
        },
        "def",
        r#"list[bullet](li("abc" overflow("def")))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::the character limit's OverflowNode > $insertList with the caret in an OverflowNode alone in a paragraph after another
#[test]
fn insert_list_with_the_caret_in_an_overflow_node_alone_in_a_paragraph_after_another() {
    overflow_case(
        "alone in a paragraph after another",
        || {
            vec![
                p("abc"),
                append(create_paragraph_node(), vec![overflow("ghi")]),
            ]
        },
        "ghi",
        r#"p("abc") / list[bullet](li(overflow("ghi")))"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::the character limit's OverflowNode > $insertList with the caret in an OverflowNode in a box that holds a quote
#[test]
fn insert_list_with_the_caret_in_an_overflow_node_in_a_box_that_holds_a_quote() {
    overflow_case(
        "in a box that holds a quote",
        || {
            vec![append(
                create::<PlainBoxNode>(),
                vec![
                    append(quote(), vec![create_text_node("q")]),
                    append(
                        create_paragraph_node(),
                        vec![create_text_node("abc"), overflow("def")],
                    ),
                ],
            )]
        },
        "def",
        r#"box(quote("q") list[bullet](li("abc" overflow("def"))))"#,
    );
}

/// `#9570: $insertList in a box inside a box`.
fn box_in_box_case(kind: &'static str, structure: bool, expected: &str) {
    let editor = build_editor(structure);
    update(&editor, move || {
        let mk = || {
            if kind == "box" {
                create::<PlainBoxNode>()
            } else {
                create::<DeclaredBoxNode>()
            }
        };
        let text = create_text_node("abc");
        cleared_root()?.append(&[append(
            mk(),
            vec![append(
                mk(),
                vec![append(create_paragraph_node(), vec![text.clone()])],
            )],
        )])?;
        text.select(Some(1), Some(1))?;
        insert_list(ListType::Bullet)
    });
    assert_eq!(read_shape(&editor), expected);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::#9570: $insertList in a box inside a box > box (NodeStructureExtension: false) makes a list and keeps both boxes where the structure is enforced
#[test]
fn box_without_structure_makes_a_list() {
    box_in_box_case("box", false, r#"list[bullet](li(box(p("abc"))))"#);
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::#9570: $insertList in a box inside a box > box (NodeStructureExtension: true) makes a list and keeps both boxes where the structure is enforced
#[test]
fn box_with_structure_makes_a_list_and_keeps_both_boxes() {
    box_in_box_case("box", true, r#"box(box(list[bullet](li("abc"))))"#);
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::#9570: $insertList in a box inside a box > dbox (NodeStructureExtension: false) makes a list and keeps both boxes where the structure is enforced
#[test]
fn dbox_without_structure_makes_a_list() {
    box_in_box_case("dbox", false, r#"list[bullet](li(dbox(p("abc"))))"#);
}

// upstream: lexical-extension/src/__tests__/unit/BlockContainers.test.ts::#9570: $insertList in a box inside a box > dbox (NodeStructureExtension: true) makes a list and keeps both boxes where the structure is enforced
#[test]
fn dbox_with_structure_makes_a_list_and_keeps_both_boxes() {
    box_in_box_case("dbox", true, r#"dbox(dbox(list[bullet](li("abc"))))"#);
}
