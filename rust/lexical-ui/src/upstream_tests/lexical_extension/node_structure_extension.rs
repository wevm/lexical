//! Ports of `packages/lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts`.
//!
//! Upstream's editors also list `TableExtension`; the editors here have it
//! with the `table` feature, which the tests that make a table (in
//! [`tables`]) need.
//! JS's test classes become structs whose
//! [`LexicalNode::static_node_config_chain`] is their `$config()` chain (a
//! subclass lists its own link, then its parent's), and
//! `ElementNode.prototype.splice.call` is [`element::splice`].

#![cfg(all(
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "code",
    feature = "overflow",
    feature = "mark"
))]

use super::super::support::panic_message;
use crate::extension::{
    LexicalEditorWithDispose, NodeStructureConfigOverrides, build_editor_from_extensions,
    create_horizontal_rule_node, get_extension_dependency_from_editor, horizontal_rule_extension,
    node_structure_extension, register_node_structure,
};
use crate::json::Value;
use crate::link::{AutoLinkAttributes, LinkAttributes, create_auto_link_node, create_link_node};
use crate::list::{ListType, create_list_item_node, create_list_node, is_list_node};
use crate::nodes::element::{self, ElementData};
use crate::nodes::line_break::create_line_break_node;
use crate::rich_text::{HeadingTagType, create_heading_node, create_quote_node};
use crate::schema::{
    NodeSchema, NumberValueOptions, node_schema, number_value, string_value, with_field,
};
use crate::{
    AnyLexicalExtensionArgument, CreateEditorArgs, Editor, InitialEditorStateType, LexicalError,
    LexicalExtension, LexicalNode, MergeAdjacentDeclaration, MergeAdjacentEntry, NodeBase,
    NodeChildEntry, NodeChildrenConfig, NodeClass, NodeClassRef, NodeConfigLink, NodeRef,
    PointType, RequiredNodeStateConfig, StateConfig, StateValueConfig, StaticNodeConfig,
    UpdateOptions, Which, config_extension, create, create_editor, create_paragraph_node,
    create_range_selection, create_state, create_state_with, create_text_node, define_extension,
    get_declared_node_structure, get_root, get_selection, internal_is_node_structure_enforced,
    internal_normalize_node_structure, set_selection, set_state,
};
use std::rc::Rc;

/// An element class: its type, extra fields, and trait items.
macro_rules! element_class {
    ($(#[$doc:meta])* $name:ident, $type:literal, { $($field:ident: $ty:ty),* }, { $($item:item)* }) => {
        $(#[$doc])*
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            element: ElementData,
            $($field: $ty,)*
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
            fn element(&self) -> Option<&ElementData> {
                Some(&self.element)
            }
            fn element_mut(&mut self) -> Option<&mut ElementData> {
                Some(&mut self.element)
            }
            $($item)*
        }
    };
}

fn link(node_type: &'static str, config: StaticNodeConfig) -> NodeConfigLink {
    NodeConfigLink::new(node_type, config)
}

fn own(node_type: &'static str) -> NodeConfigLink {
    link(node_type, StaticNodeConfig::default())
}

fn j(text: &str) -> Value {
    crate::json::parse(text).unwrap()
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

fn upd(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

/// `shape(node)`.
fn shape(node: &NodeRef) -> Value {
    if node.is_element() {
        let mut out = vec![Value::from(node.get_type())];
        out.extend(node.get_children().iter().map(shape));
        Value::Array(out)
    } else if node.is_text() {
        Value::from(node.get_text_content())
    } else {
        Value::from(node.get_type())
    }
}

/// `rootShape(editor)`.
fn root_shape(editor: &Editor) -> Value {
    editor
        .read(|| Value::Array(get_root().get_children().iter().map(shape).collect()))
        .unwrap()
}

/// `listValues(editor)`.
fn list_values(editor: &Editor) -> Value {
    editor
        .read(|| {
            Value::Array(
                get_root()
                    .get_children()
                    .iter()
                    .map(|n| {
                        if is_list_node(n) {
                            Value::Array(
                                n.get_children()
                                    .iter()
                                    .map(|c| Value::from(c.get_list_item_value().unwrap()))
                                    .collect(),
                            )
                        } else {
                            Value::from(n.get_type())
                        }
                    })
                    .collect(),
            )
        })
        .unwrap()
}

/// `listStarts(editor)`.
fn list_starts(editor: &Editor) -> Value {
    editor
        .read(|| {
            Value::Array(
                get_root()
                    .get_children()
                    .iter()
                    .map(|n| {
                        if is_list_node(n) {
                            let mut out = vec![Value::from(n.get_list_start().unwrap())];
                            out.extend(
                                n.get_children()
                                    .iter()
                                    .map(|c| Value::from(c.get_text_content())),
                            );
                            Value::Array(out)
                        } else {
                            Value::from(n.get_type())
                        }
                    })
                    .collect(),
            )
        })
        .unwrap()
}

fn t(text: &str) -> NodeRef {
    create_text_node(text)
}

fn hr() -> NodeRef {
    create_horizontal_rule_node()
}

fn with(parent: NodeRef, children: Vec<NodeRef>) -> NodeRef {
    parent.append(&children).unwrap();
    parent
}

/// `put(parent, children)`: children put in as they are.
fn put(parent: NodeRef, children: Vec<NodeRef>) -> NodeRef {
    element::splice(&parent, parent.get_children_size(), 0, &children).unwrap();
    parent
}

fn p(children: Vec<NodeRef>) -> NodeRef {
    with(create_paragraph_node(), children)
}

fn li(children: Vec<NodeRef>) -> NodeRef {
    with(create_list_item_node(None), children)
}

fn a_link(url: &str) -> NodeRef {
    create_link_node(url, &LinkAttributes::default())
}

fn mark(id: &str) -> NodeRef {
    crate::mark::create_mark_node(&[id.to_owned()])
}

/// `$numbered(start, ...items)`.
fn numbered(start: f64, items: Vec<Vec<NodeRef>>) -> Result<(), LexicalError> {
    let list = create_list_node(ListType::Number, start);
    for item in items {
        list.append(&[li(item)])?;
    }
    get_root().append(&[list])?;
    Ok(())
}

/// `$paragraphWithRule()`.
fn paragraph_with_rule() -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(&[p(vec![t("a"), hr(), t("b")])])?;
    Ok(())
}

/// An editor built from `dependencies` and `nodes`, with
/// `$initialEditorState` `init` (none when `None`).
fn build_with(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
    nodes: Vec<NodeClass>,
    init: Option<Rc<dyn Fn() -> Result<(), LexicalError>>>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: init
            .map(|init| InitialEditorStateType::Update(Rc::new(move |_| init()))),
        dependencies,
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `build($init, withStructure)`.
fn build(
    init: impl Fn() -> Result<(), LexicalError> + 'static,
    with_structure: bool,
) -> LexicalEditorWithDispose {
    let mut deps = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::list::list_extension().into(),
        crate::link::link_extension().into(),
    ];
    #[cfg(feature = "table")]
    deps.push(crate::table::table_extension().into());
    deps.push(horizontal_rule_extension().into());
    if with_structure {
        deps.push(node_structure_extension().into());
    }
    build_with(
        "node-structure-test",
        deps,
        vec![
            NodeClass::of::<crate::code::CodeNode>(),
            NodeClass::of::<crate::overflow::OverflowNode>(),
        ],
        Some(Rc::new(init)),
    )
}

fn rich_and_structure() -> Vec<AnyLexicalExtensionArgument> {
    vec![
        crate::rich_text::rich_text_extension().into(),
        node_structure_extension().into(),
    ]
}

/// A case's children.
type Children = fn() -> Vec<NodeRef>;

/// A case's initial content.
type Init = fn() -> Result<(), LexicalError>;

/// The message `create_editor(nodes)` panics with.
fn create_editor_panic(nodes: Vec<NodeClass>) -> String {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        create_editor(CreateEditorArgs {
            nodes,
            ..CreateEditorArgs::default()
        })
    }));
    match result {
        Ok(_) => panic!("createEditor does not throw"),
        Err(payload) => panic_message(&*payload),
    }
}

// --- declarations

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::declarations > derived fields
#[test]
fn derived_fields() {
    let mut item = get_declared_node_structure(&crate::list::ListItemNode::default()).derived;
    item.sort();
    assert_eq!(item, vec!["indent".to_owned(), "value".to_owned()]);
    assert!(
        get_declared_node_structure(&crate::list::ListNode::default())
            .derived
            .is_empty()
    );
    let derived = create_state_with(
        "n",
        StateValueConfig {
            derived: true,
            ..StateValueConfig::new(|v: &Value| v.as_f64().unwrap_or(0.0))
        },
    );
    assert!(derived.derived());
    assert!(!create_state("n", |v: &Value| v.as_f64().unwrap_or(0.0)).derived());
}

element_class!(
    /// `Chip`: `inline: true`.
    ChipNode,
    "test-chip",
    {},
    {
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            vec![link(
                "test-chip",
                StaticNodeConfig {
                    inline: Some(true),
                    ..StaticNodeConfig::default()
                },
            )]
        }
    }
);
element_class!(
    /// `Box`: declares nothing.
    BoxNode,
    "test-box",
    {},
    {
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            vec![own("test-box")]
        }
    }
);

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::declarations > a class that declares inline gets an isInline() that returns it
#[test]
fn a_class_that_declares_inline_gets_an_is_inline_that_returns_it() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<ChipNode>(), NodeClass::of::<BoxNode>()],
        ..CreateEditorArgs::default()
    });
    upd(&editor, || {
        assert!(create::<ChipNode>().is_inline());
        assert!(!create::<BoxNode>().is_inline());
        Ok(())
    });
}

// --- validation at editor creation

element_class!(BadByNode, "test-bad-by", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![link(
            "test-bad-by",
            StaticNodeConfig {
                merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![
                    MergeAdjacentEntry::Field("nope"),
                ])),
                ..StaticNodeConfig::default()
            },
        )]
    }
});

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::validation at editor creation > mergeAdjacent.by names an unknown field
#[test]
fn merge_adjacent_by_names_an_unknown_field() {
    let message = create_editor_panic(vec![NodeClass::of::<BadByNode>()]);
    assert!(message.contains("not a schema field"), "{message}");
}

thread_local! {
    static DERIVED_D: StateConfig<f64> = create_state_with(
        "d",
        StateValueConfig {
            derived: true,
            ..StateValueConfig::new(|v: &Value| v.as_f64().unwrap_or(0.0))
        },
    );
    static COLOR: StateConfig<String> =
        create_state("color", |v: &Value| v.as_str().unwrap_or("").to_owned());
}

element_class!(BadDerivedNode, "test-bad-derived", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![link(
            "test-bad-derived",
            StaticNodeConfig {
                merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![
                    MergeAdjacentEntry::State(DERIVED_D.with(StateConfig::erased)),
                ])),
                ..StaticNodeConfig::default()
            },
        )]
    }
});

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::validation at editor creation > mergeAdjacent.by names a derived field or state
#[test]
fn merge_adjacent_by_names_a_derived_field_or_state() {
    let message = create_editor_panic(vec![NodeClass::of::<BadDerivedNode>()]);
    assert!(message.contains("derived"), "{message}");
}

element_class!(BadInlineNode, "test-bad-inline", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![link(
            "test-bad-inline",
            StaticNodeConfig {
                inline: Some(true),
                ..StaticNodeConfig::default()
            },
        )]
    }
    fn is_inline(&self) -> bool {
        false
    }
});

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::validation at editor creation > declared inline disagrees with isInline()
#[test]
fn declared_inline_disagrees_with_is_inline() {
    let message = create_editor_panic(vec![NodeClass::of::<BadInlineNode>()]);
    assert!(message.contains("isInline"), "{message}");
}

element_class!(ItemNode, "test-item", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![link(
            "test-item",
            StaticNodeConfig {
                children: Some(NodeChildrenConfig::Block),
                ..StaticNodeConfig::default()
            },
        )]
    }
});
element_class!(GroupNode, "test-group", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![link(
            "test-group",
            StaticNodeConfig {
                children: Some(NodeChildrenConfig::List(vec![NodeChildEntry::Class(
                    NodeClassRef::of::<ItemNode>(),
                )])),
                merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![
                    MergeAdjacentEntry::Field("format"),
                ])),
                ..StaticNodeConfig::default()
            },
        )]
    }
});

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::validation at editor creation > a merging group's children must hold inline content
#[test]
fn a_merging_groups_children_must_hold_inline_content() {
    let message = create_editor_panic(vec![
        NodeClass::of::<ItemNode>(),
        NodeClass::of::<GroupNode>(),
    ]);
    assert!(message.contains("hold inline content"), "{message}");
}

// --- existing subclasses that inherit a declaration

/// `LinkNode`'s chain, as a subclass inherits it.
fn link_chain() -> Vec<NodeConfigLink> {
    crate::link::LinkNode::default().static_node_config_chain()
}

element_class!(
    /// `InlineParagraphNode`: a `ParagraphNode` subclass (no `$config`)
    /// whose `isInline()` returns true.
    InlineParagraphNode,
    "test-inline-paragraph",
    {},
    {
        fn super_types(&self) -> Vec<&'static str> {
            vec!["paragraph"]
        }
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            let mut chain = vec![own("test-inline-paragraph")];
            chain.extend(
                crate::nodes::paragraph::ParagraphNode::default().static_node_config_chain(),
            );
            chain
        }
        fn is_inline(&self) -> bool {
            true
        }
    }
);

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::existing subclasses that inherit a declaration > a ParagraphNode subclass whose isInline() returns true
#[test]
fn a_paragraph_node_subclass_whose_is_inline_returns_true() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<crate::link::LinkNode>(),
            NodeClass::of::<crate::list::ListNode>(),
            NodeClass::of::<crate::list::ListItemNode>(),
            NodeClass::of::<InlineParagraphNode>(),
        ],
        ..CreateEditorArgs::default()
    });
    upd(&editor, || {
        let node = create::<InlineParagraphNode>();
        get_root().append(&[p(vec![node.clone()])])?;
        assert!(node.is_inline());
        Ok(())
    });
    assert_eq!(
        get_declared_node_structure(&InlineParagraphNode::default()).inline,
        None
    );
}

element_class!(
    /// `Bad extends LinkNode` that declares `inline: false` itself.
    BadOwnInlineNode,
    "test-bad-own-inline",
    {},
    {
        fn super_types(&self) -> Vec<&'static str> {
            vec!["link"]
        }
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            let mut chain = vec![link(
                "test-bad-own-inline",
                StaticNodeConfig {
                    inline: Some(false),
                    ..StaticNodeConfig::default()
                },
            )];
            chain.extend(link_chain());
            chain
        }
        fn is_inline(&self) -> bool {
            true
        }
    }
);

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::existing subclasses that inherit a declaration > a subclass that declares the key itself is still checked
#[test]
fn a_subclass_that_declares_the_key_itself_is_still_checked() {
    let message = create_editor_panic(vec![
        NodeClass::of::<crate::link::LinkNode>(),
        NodeClass::of::<BadOwnInlineNode>(),
    ]);
    assert!(message.contains("isInline"), "{message}");
}

// --- NodeStructureExtension

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > an OverflowNode stays in its paragraph
#[test]
fn an_overflow_node_stays_in_its_paragraph() {
    let editor = build(
        || {
            get_root().append(&[p(vec![
                t("within"),
                with(crate::overflow::create_overflow_node(), vec![t("over")]),
            ])])?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["paragraph","within",["overflow","over"]]]"#)
    );
    editor
        .read(|| {
            let overflow = get_root()
                .get_first_child_or_throw()
                .unwrap()
                .get_last_child()
                .unwrap();
            assert!(!overflow.is_inline());
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a shadow root quote keeps its blocks
#[test]
fn a_shadow_root_quote_keeps_its_blocks() {
    let editor = build(
        || {
            get_root().append(&[with(
                create_quote_node(true),
                vec![p(vec![t("one")]), p(vec![t("two")])],
            )])?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["quote",["paragraph","one"],["paragraph","two"]]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > paragraphs in a list item become lines of the item
#[test]
fn paragraphs_in_a_list_item_become_lines_of_the_item() {
    let editor = build(
        || {
            let x = t("x");
            get_root().append(&[with(
                create_list_node(ListType::Bullet, 1.0),
                vec![li(vec![x.clone()])],
            )])?;
            let para = p(vec![t("p")]);
            x.insert_after(&para, true)?;
            para.insert_after(&p(vec![t("q")]), true)?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["list",["listitem","x","linebreak","p","linebreak","q"]]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a heading inside a link becomes a heading holding the link
#[test]
fn a_heading_inside_a_link_becomes_a_heading_holding_the_link() {
    let editor = build(
        || {
            get_root().append(&[p(vec![with(
                a_link("https://a"),
                vec![with(
                    create_heading_node(HeadingTagType::H1),
                    vec![t("Lexical")],
                )],
            )])])?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["heading",["link","Lexical"]]]"#)
    );
    let url = editor
        .read(|| {
            get_root().get_all_text_nodes()[0]
                .get_parent()
                .unwrap()
                .get_url()
        })
        .unwrap();
    assert_eq!(url.as_deref(), Some("https://a"));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a quote holding paragraphs stays one quote, with line breaks
#[test]
fn a_quote_holding_paragraphs_stays_one_quote_with_line_breaks() {
    let editor = build(
        || {
            get_root().append(&[
                with(
                    create_quote_node(false),
                    vec![p(vec![t("a")]), p(vec![t("b")])],
                ),
                with(
                    create_quote_node(false),
                    vec![t("x"), p(vec![t("c")]), t("y")],
                ),
            ])?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["quote","a","linebreak","b"],["quote","x","linebreak","c","linebreak","y"]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > an empty paragraph in a quote keeps its line
#[test]
fn an_empty_paragraph_in_a_quote_keeps_its_line() {
    let editor = build(
        || {
            let empty = create_paragraph_node();
            get_root().append(&[with(
                create_quote_node(false),
                vec![create_paragraph_node(), p(vec![t("a")]), empty.clone()],
            )])?;
            empty.select(None, None)?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["quote","linebreak","a","linebreak"]]"#)
    );
    editor
        .read(|| {
            let selection = get_selection()
                .filter(crate::SelectionRef::is_range)
                .unwrap();
            let anchor = selection.point(Which::Anchor).unwrap();
            let quote = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(
                (anchor.key.clone(), anchor.offset),
                (quote.get_key().to_owned(), 3)
            );
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > typed blocks inside a quote are split around and keep their type
#[test]
fn typed_blocks_inside_a_quote_are_split_around_and_keep_their_type() {
    let editor = build(
        || {
            get_root().append(&[with(
                create_quote_node(false),
                vec![
                    p(vec![t("a")]),
                    with(create_heading_node(HeadingTagType::H2), vec![t("h")]),
                    p(vec![t("b")]),
                    with(crate::code::create_code_node(None, None), vec![t("c")]),
                    with(create_quote_node(false), vec![t("q")]),
                    p(vec![t("d")]),
                ],
            )])?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(
            r#"[["quote","a"],["heading","h"],["quote","b"],["code","c"],["quote","q"],["quote","d"]]"#
        )
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > paragraphs in other blocks that hold inline content become lines
#[test]
fn paragraphs_in_other_blocks_that_hold_inline_content_become_lines() {
    let editor = build(
        || {
            get_root().append(&[
                with(
                    crate::code::create_code_node(None, None),
                    vec![p(vec![t("one")]), p(vec![t("two")])],
                ),
                with(
                    create_heading_node(HeadingTagType::H1),
                    vec![t("x"), create_line_break_node(), p(vec![t("y")])],
                ),
            ])?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["code","one","linebreak","two"],["heading","x","linebreak","linebreak","y"]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a list item inside a paragraph is lifted without throwing
#[test]
fn a_list_item_inside_a_paragraph_is_lifted_without_throwing() {
    let editor = build(
        || {
            get_root().append(&[p(vec![t("a"), li(vec![t("item")]), t("b")])])?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["paragraph","a"],["list",["listitem","item"]],["paragraph","b"]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a caret between two merged links stays between their contents
#[test]
fn a_caret_between_two_merged_links_stays_between_their_contents() {
    let editor = build(
        || {
            let para = p(vec![
                with(a_link("https://a"), vec![t("one")]),
                with(a_link("https://a"), vec![t("two")]),
            ]);
            get_root().append(std::slice::from_ref(&para))?;
            para.select(Some(1), Some(1))?;
            Ok(())
        },
        true,
    );
    editor
        .read(|| {
            let selection = get_selection()
                .filter(crate::SelectionRef::is_range)
                .unwrap();
            let point = selection.point(Which::Anchor).unwrap();
            let node = point.get_node().unwrap();
            let text: String = crate::utf16::slice(&node.get_text_content(), 0, point.offset);
            let shown = if text == "one" || text == "onetwo" {
                text
            } else {
                format!("{:?}", point.point_type)
            };
            assert_ne!(shown, "onetwo");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > adjacent equal links merge; different ones do not
#[test]
fn adjacent_equal_links_merge_different_ones_do_not() {
    let editor = build(
        || {
            get_root().append(&[p(vec![
                with(a_link("https://a"), vec![t("1")]),
                with(a_link("https://a"), vec![t("2")]),
                with(a_link("https://b"), vec![t("3")]),
            ])])?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["paragraph",["link","12"],["link","3"]]]"#)
    );
    editor.dispose();
}

thread_local! {
    static TAG_SCHEMA: NodeSchema = node_schema(vec![(
        "tag",
        with_field(string_value(""), |n: &TagNode| &n.tag, |n: &mut TagNode| &mut n.tag).unwrap(),
    )])
    .unwrap();
    static SUB_TAG_SCHEMA: NodeSchema = node_schema(vec![(
        "tag",
        with_field(string_value(""), |n: &SubTagNode| &n.tag, |n: &mut SubTagNode| &mut n.tag).unwrap(),
    )])
    .unwrap();
}

fn tag_link() -> NodeConfigLink {
    link(
        "test-tag",
        StaticNodeConfig {
            inline: Some(true),
            children: Some(NodeChildrenConfig::Inline),
            merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![
                MergeAdjacentEntry::Field("tag"),
            ])),
        },
    )
}

element_class!(
    /// `TagNode`: inline, holds inline content, merges by `tag`.
    TagNode, "test-tag", { tag: String }, {
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            vec![tag_link()]
        }
        fn json_schema_chain(&self) -> Vec<NodeSchema> {
            vec![TAG_SCHEMA.with(Clone::clone), element::element_node_schema()]
        }
    }
);
element_class!(
    /// `SubTagNode extends TagNode`, declaring nothing of its own.
    SubTagNode, "test-subtag", { tag: String }, {
        fn super_types(&self) -> Vec<&'static str> {
            vec!["test-tag"]
        }
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            vec![own("test-subtag"), tag_link()]
        }
        fn json_schema_chain(&self) -> Vec<NodeSchema> {
            vec![SUB_TAG_SCHEMA.with(Clone::clone), element::element_node_schema()]
        }
    }
);

/// `$tag(klass, tag, text)`.
fn tag_of<T: LexicalNode + Default>(set: fn(&mut T, String), tag: &str, text: &str) -> NodeRef {
    let node = create::<T>();
    let tag = tag.to_owned();
    crate::lexical_utils::with_writable(node.get_key(), |n| {
        set(n.downcast_mut::<T>().unwrap(), tag)
    });
    with(node, vec![t(text)])
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a class that declares mergeAdjacent merges; a subclass that inherits it merges by its own code
#[test]
fn a_class_that_declares_merge_adjacent_merges_a_subclass_that_inherits_it_merges_by_its_own_code()
{
    let tag = |s: &str, text: &str| tag_of::<TagNode>(|n, v| n.tag = v, s, text);
    let sub = |s: &str, text: &str| tag_of::<SubTagNode>(|n, v| n.tag = v, s, text);
    let editor = build_with(
        "tags",
        rich_and_structure(),
        vec![NodeClass::of::<TagNode>(), NodeClass::of::<SubTagNode>()],
        Some(Rc::new(move || {
            get_root().append(&[
                p(vec![tag("a", "1"), tag("a", "2"), tag("b", "3")]),
                p(vec![sub("a", "4"), sub("a", "5")]),
            ])?;
            Ok(())
        })),
    );
    assert_eq!(
        root_shape(&editor),
        j(
            r#"[["paragraph",["test-tag","12"],["test-tag","3"]],["paragraph",["test-subtag","4"],["test-subtag","5"]]]"#
        )
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > adjacent lists of one type merge, whatever their start
#[test]
fn adjacent_lists_of_one_type_merge_whatever_their_start() {
    let editor = build(
        || {
            get_root().append(&[
                with(
                    create_list_node(ListType::Number, 1.0),
                    vec![li(vec![t("a")])],
                ),
                with(
                    create_list_node(ListType::Number, 5.0),
                    vec![li(vec![t("b")])],
                ),
                with(
                    create_list_node(ListType::Bullet, 1.0),
                    vec![li(vec![t("c")])],
                ),
            ])?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["list",["listitem","a"],["listitem","b"]],["list",["listitem","c"]]]"#)
    );
    editor.dispose();
}

/// `listValues` expected from `listStarts` expected: each list's start,
/// counting up.
fn values_from_starts(starts: &Value) -> Value {
    Value::Array(
        starts
            .as_array()
            .unwrap()
            .iter()
            .map(|e| match e.as_array() {
                Some(list) => {
                    let start = list[0].as_f64().unwrap();
                    Value::Array(
                        (0..list.len() - 1)
                            .map(|i| Value::from(start + i as f64))
                            .collect(),
                    )
                }
                None => e.clone(),
            })
            .collect(),
    )
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block decorator splits an ordered list wherever it sits in the item; numbering continues
#[test]
fn a_block_decorator_splits_an_ordered_list_wherever_it_sits_in_the_item() {
    let cases: [(&str, Children, &str); 4] = [
        (
            "first",
            || vec![hr(), t("b")],
            r#"[[5,"x"],"horizontalrule",[6,"b","y"]]"#,
        ),
        (
            "middle",
            || vec![t("a"), hr(), t("b")],
            r#"[[5,"x","a"],"horizontalrule",[7,"b","y"]]"#,
        ),
        (
            "last",
            || vec![t("a"), hr()],
            r#"[[5,"x","a"],"horizontalrule",[7,"y"]]"#,
        ),
        (
            "alone",
            || vec![hr()],
            r#"[[5,"x"],"horizontalrule",[6,"y"]]"#,
        ),
    ];
    for (name, item, expected) in cases {
        let editor = build(
            move || numbered(5.0, vec![vec![t("x")], item(), vec![t("y")]]),
            true,
        );
        let expected = j(expected);
        assert_eq!(list_starts(&editor), expected, "{name}");
        assert_eq!(
            list_values(&editor),
            values_from_starts(&expected),
            "{name}"
        );
        editor.dispose();
    }
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block decorator in the first or last item of an ordered list
#[test]
fn a_block_decorator_in_the_first_or_last_item_of_an_ordered_list() {
    let cases: [(Init, &str); 2] = [
        (
            || numbered(3.0, vec![vec![hr()], vec![t("y")]]),
            r#"["horizontalrule",[3,"y"]]"#,
        ),
        (
            || numbered(3.0, vec![vec![t("x")], vec![hr()]]),
            r#"[[3,"x"],"horizontalrule"]"#,
        ),
    ];
    for (init, expected) in cases {
        let editor = build(init, true);
        assert_eq!(list_starts(&editor), j(expected));
        editor.dispose();
    }
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block decorator put in a list item later splits the same way
#[test]
fn a_block_decorator_put_in_a_list_item_later_splits_the_same_way() {
    let editor = build(
        || numbered(5.0, vec![vec![t("x")], vec![t("a")], vec![t("y")]]),
        true,
    );
    upd(&editor, || {
        let list = get_root().get_first_child_or_throw()?;
        assert!(is_list_node(&list));
        let item = list.get_child_at_index(1).unwrap();
        item.get_first_child_or_throw()?
            .insert_before(&hr(), true)?;
        Ok(())
    });
    assert_eq!(
        list_starts(&editor),
        j(r#"[[5,"x"],"horizontalrule",[6,"a","y"]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block lifted out of a list item splits the list and keeps its type
#[test]
fn a_block_lifted_out_of_a_list_item_splits_the_list_and_keeps_its_type() {
    fn h() -> NodeRef {
        with(create_heading_node(HeadingTagType::H2), vec![t("h")])
    }
    let cases: [(Children, &str); 3] = [
        (|| vec![h(), t("a")], r#"[[1,"x"],"heading",[2,"a","y"]]"#),
        (|| vec![t("a"), h()], r#"[[1,"x","a"],"heading",[3,"y"]]"#),
        (
            || {
                vec![with(
                    crate::code::create_code_node(None, None),
                    vec![t("c")],
                )]
            },
            r#"[[1,"x"],"code",[2,"y"]]"#,
        ),
    ];
    for (item, expected) in cases {
        let editor = build(
            move || numbered(1.0, vec![vec![t("x")], item(), vec![t("y")]]),
            true,
        );
        assert_eq!(list_starts(&editor), j(expected));
        editor.dispose();
    }
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a paragraph in a block in a holder: the result does not depend on which is repaired first
#[test]
fn a_paragraph_in_a_block_in_a_holder_the_result_does_not_depend_on_which_is_repaired_first() {
    fn doc() -> Result<(), LexicalError> {
        get_root().append(&[with(
            crate::code::create_code_node(None, None),
            vec![p(vec![
                with(create_heading_node(HeadingTagType::H1), vec![t("h")]),
                t("a"),
            ])],
        )])?;
        Ok(())
    }
    let built = build(doc, true);
    let want = root_shape(&built);
    assert_eq!(want, j(r#"[["heading","h"],["code","a"]]"#));
    let editor = build_with(
        "order",
        vec![
            crate::rich_text::rich_text_extension().into(),
            config_extension(
                &node_structure_extension(),
                [NodeStructureConfigOverrides {
                    disabled: Some(true),
                }],
            ),
        ],
        vec![NodeClass::of::<crate::code::CodeNode>()],
        Some(Rc::new(doc)),
    );
    upd(&editor, || {
        fn walk(n: &NodeRef, all: &mut Vec<NodeRef>) {
            if n.is_element() {
                for c in n.get_children() {
                    walk(&c, all);
                }
            }
            all.push(n.clone());
        }
        let mut all = Vec::new();
        for c in get_root().get_children() {
            walk(&c, &mut all);
        }
        for n in all {
            n.mark_dirty();
        }
        Ok(())
    });
    get_extension_dependency_from_editor(&editor, &node_structure_extension())
        .unwrap()
        .output
        .unwrap()
        .disabled
        .set_value(false);
    assert_eq!(root_shape(&editor), want);
    built.dispose();
    editor.dispose();
}

/// What is before and after the caret in its element, and where that
/// element is.
fn caret_of(editor: &Editor) -> (String, String, usize) {
    editor
        .read(|| {
            let selection = get_selection()
                .filter(crate::SelectionRef::is_range)
                .unwrap();
            let anchor = selection.point(Which::Anchor).unwrap();
            let node = anchor.get_node().unwrap();
            if node.is_text() {
                let text = node.get_text_content();
                let len = crate::utf16::len(&text);
                return (
                    if anchor.offset > 0 {
                        crate::utf16::slice(&text, 0, anchor.offset)
                    } else {
                        node.get_previous_sibling()
                            .map_or("^".to_owned(), |n| n.get_text_content())
                    },
                    if anchor.offset < len {
                        crate::utf16::slice(&text, anchor.offset, len)
                    } else {
                        node.get_next_sibling()
                            .map_or("$".to_owned(), |n| n.get_text_content())
                    },
                    node.get_parent_or_throw()
                        .unwrap()
                        .get_index_within_parent()
                        .unwrap(),
                );
            }
            assert!(node.is_element());
            (
                anchor
                    .offset
                    .checked_sub(1)
                    .and_then(|i| node.get_child_at_index(i))
                    .map_or("^".to_owned(), |n| n.get_text_content()),
                node.get_child_at_index(anchor.offset)
                    .map_or("$".to_owned(), |n| n.get_text_content()),
                node.get_index_within_parent().unwrap(),
            )
        })
        .unwrap()
}

fn caret_tuple(before: &str, after: &str, index: usize) -> (String, String, usize) {
    (before.to_owned(), after.to_owned(), index)
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > an element caret after a lifted block stays after it
#[test]
fn an_element_caret_after_a_lifted_block_stays_after_it() {
    let rule_at = |offset: usize| {
        build(
            move || {
                let para = p(vec![t("a"), hr(), t("b")]);
                get_root().append(std::slice::from_ref(&para))?;
                para.select(Some(offset), Some(offset))?;
                Ok(())
            },
            true,
        )
    };
    let end = rule_at(3);
    assert_eq!(caret_of(&end), caret_tuple("b", "$", 2));
    let after = rule_at(2);
    assert_eq!(caret_of(&after), caret_tuple("^", "b", 2));
    let before = rule_at(1);
    assert_eq!(caret_of(&before), caret_tuple("a", "$", 0));
    let item = build(
        || {
            let item = create_list_item_node(None);
            get_root().append(&[with(
                create_list_node(ListType::Number, 1.0),
                vec![item.clone()],
            )])?;
            element::splice(
                &item,
                0,
                0,
                &[t("x"), hr(), with(a_link("https://a"), vec![t("y")])],
            )?;
            item.select(Some(3), Some(3))?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&item),
        j(r#"[["list",["listitem","x"]],"horizontalrule",["list",["listitem",["link","y"]]]]"#)
    );
    assert_eq!(caret_of(&item), caret_tuple("y", "$", 0));
    for editor in [end, after, before, item] {
        editor.dispose();
    }
}

fn insert_text_z(editor: &Editor, text: &'static str) {
    upd(editor, move || get_selection().unwrap().insert_text(text));
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a text caret survives the split of its node
#[test]
fn a_text_caret_survives_the_split_of_its_node() {
    let editor = build(|| numbered(1.0, vec![vec![t("x")], vec![t("y")]]), true);
    upd(&editor, || {
        let text = get_root().get_all_text_nodes()[0].clone();
        text.select(Some(1), Some(1))?;
        text.insert_after(&hr(), true)?;
        Ok(())
    });
    insert_text_z(&editor, "1");
    assert_eq!(
        list_starts(&editor),
        j(r#"[[1,"x1"],"horizontalrule",[2,"y"]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a list item holding a paragraph and a nested list gets no stray line break
#[test]
fn a_list_item_holding_a_paragraph_and_a_nested_list_gets_no_stray_line_break() {
    let editor = build(
        || {
            let item = create_list_item_node(None);
            get_root().append(&[with(
                create_list_node(ListType::Bullet, 1.0),
                vec![item.clone()],
            )])?;
            element::splice(
                &item,
                0,
                0,
                &[
                    p(vec![t("a")]),
                    with(
                        create_list_node(ListType::Bullet, 1.0),
                        vec![li(vec![t("b")])],
                    ),
                ],
            )?;
            Ok(())
        },
        true,
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["list",["listitem","a"],["listitem",["list",["listitem","b"]]]]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block in a mark keeps the mark on its content
#[test]
fn a_block_in_a_mark_keeps_the_mark_on_its_content() {
    let editor = build_with(
        "mark",
        rich_and_structure(),
        vec![NodeClass::of::<crate::mark::MarkNode>()],
        Some(Rc::new(|| {
            get_root().append(&[p(vec![
                t("a"),
                with(
                    mark("m"),
                    vec![
                        t("b"),
                        with(create_heading_node(HeadingTagType::H1), vec![t("h")]),
                    ],
                ),
            ])])?;
            Ok(())
        })),
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["paragraph","a",["mark","b"]],["heading",["mark","h"]]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > the importers read whether an editor enforces the structure
#[test]
fn the_importers_read_whether_an_editor_enforces_the_structure() {
    let on = build(|| Ok(()), true);
    let off = build(|| Ok(()), false);
    assert!(internal_is_node_structure_enforced(&on));
    assert!(!internal_is_node_structure_enforced(&off));
    assert!(!internal_is_node_structure_enforced(&create_editor(
        CreateEditorArgs::default()
    )));
    on.dispose();
    off.dispose();
}

// --- merging

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > ListNode: the extension and the list transform merge the same lists
#[test]
fn list_node_the_extension_and_the_list_transform_merge_the_same_lists() {
    let editor = build(
        || {
            get_root().append(&[with(
                create_list_node(ListType::Number, 2.0),
                vec![li(vec![t("a")])],
            )])?;
            let para = create_paragraph_node();
            get_root().append(std::slice::from_ref(&para))?;
            element::splice(
                &para,
                0,
                0,
                &[with(
                    create_list_node(ListType::Number, 7.0),
                    vec![li(vec![t("b")])],
                )],
            )?;
            get_root().append(&[
                with(
                    create_list_node(ListType::Number, 9.0),
                    vec![li(vec![t("c")])],
                ),
                with(
                    create_list_node(ListType::Bullet, 1.0),
                    vec![li(vec![t("d")])],
                ),
            ])?;
            Ok(())
        },
        true,
    );
    assert_eq!(list_starts(&editor), j(r#"[[2,"a","b","c"],[1,"d"]]"#));
    assert_eq!(list_values(&editor), j("[[2,3,4],[1]]"));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > ListNode: the extension merges lists exactly as the list transform does, nested lists at the seam included
#[test]
fn list_node_the_extension_merges_lists_exactly_as_the_list_transform_does() {
    fn doc() -> Result<(), LexicalError> {
        let nested = |text: &str| {
            li(vec![with(
                create_list_node(ListType::Bullet, 1.0),
                vec![li(vec![t(text)])],
            )])
        };
        get_root().append(&[
            with(
                create_list_node(ListType::Number, 3.0),
                vec![li(vec![t("x")]), nested("a")],
            ),
            with(
                create_list_node(ListType::Number, 8.0),
                vec![nested("b"), li(vec![t("y")])],
            ),
        ])?;
        Ok(())
    }
    let on = build(doc, true);
    let off = build(doc, false);
    assert_eq!(
        root_shape(&on),
        j(
            r#"[["list",["listitem","x"],["listitem",["list",["listitem","a"],["listitem","b"]]],["listitem","y"]]]"#
        )
    );
    assert_eq!(root_shape(&off), root_shape(&on));
    assert_eq!(
        crate::json::stringify(&on.get_editor_state().to_json(false)),
        crate::json::stringify(&off.get_editor_state().to_json(false))
    );
    on.dispose();
    off.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > AutoLinkNode never merges
#[test]
fn auto_link_node_never_merges() {
    let editor = build_with(
        "autolink",
        vec![
            crate::rich_text::rich_text_extension().into(),
            crate::link::link_extension().into(),
            node_structure_extension().into(),
        ],
        vec![NodeClass::of::<crate::link::AutoLinkNode>()],
        Some(Rc::new(|| {
            let auto = || create_auto_link_node("https://a", &AutoLinkAttributes::default());
            get_root().append(&[p(vec![
                with(auto(), vec![t("1")]),
                with(auto(), vec![t("2")]),
            ])])?;
            Ok(())
        })),
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["paragraph",["autolink","1"],["autolink","2"]]]"#)
    );
    editor.dispose();
}

thread_local! {
    static CLEAN_TAG_SCHEMA: NodeSchema = node_schema(vec![(
        "tag",
        with_field(string_value(""), |n: &CleanTagNode| &n.tag, |n: &mut CleanTagNode| &mut n.tag).unwrap(),
    )])
    .unwrap();
}

element_class!(
    /// `CleanTagNode`: a transform that removes it when empty; holds
    /// paragraphs; merges by `tag`.
    CleanTagNode, "test-clean-tag", { tag: String }, {
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            vec![link("test-clean-tag", StaticNodeConfig {
                children: Some(NodeChildrenConfig::List(vec![NodeChildEntry::Class(
                    NodeClassRef::of::<crate::nodes::paragraph::ParagraphNode>(),
                )])),
                merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![MergeAdjacentEntry::Field("tag")])),
                ..StaticNodeConfig::default()
            })]
        }
        fn json_schema_chain(&self) -> Vec<NodeSchema> {
            vec![CLEAN_TAG_SCHEMA.with(Clone::clone), element::element_node_schema()]
        }
        fn transform(&self, this: &NodeRef) -> Result<(), LexicalError> {
            if this.is_empty() {
                this.remove(false)?;
            }
            Ok(())
        }
    }
);

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > a class with an unrelated transform of its own is still merged
#[test]
fn a_class_with_an_unrelated_transform_of_its_own_is_still_merged() {
    let editor = build_with(
        "clean-tag",
        rich_and_structure(),
        vec![NodeClass::of::<CleanTagNode>()],
        Some(Rc::new(|| {
            for text in ["a", "b"] {
                let node = create::<CleanTagNode>();
                crate::lexical_utils::with_writable(node.get_key(), |n| {
                    n.downcast_mut::<CleanTagNode>().unwrap().tag = "t".to_owned()
                });
                get_root().append(&[with(node, vec![p(vec![t(text)])])])?;
            }
            Ok(())
        })),
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["test-clean-tag",["paragraph","a"],["paragraph","b"]]]"#)
    );
    editor.dispose();
}

element_class!(
    /// `ColorNode`: merges by the `color` NodeState, stored flat.
    ColorNode,
    "test-color",
    {},
    {
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            vec![link(
                "test-color",
                StaticNodeConfig {
                    inline: Some(true),
                    children: Some(NodeChildrenConfig::Inline),
                    merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![
                        MergeAdjacentEntry::State(COLOR.with(StateConfig::erased)),
                    ])),
                },
            )]
        }
        fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
            vec![COLOR.with(|c| RequiredNodeStateConfig::new(c, true))]
        }
    }
);

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > mergeAdjacent.by naming a NodeState
#[test]
fn merge_adjacent_by_naming_a_node_state() {
    let editor = build_with(
        "color",
        rich_and_structure(),
        vec![NodeClass::of::<ColorNode>()],
        Some(Rc::new(|| {
            let color = |value: &str, text: &str| {
                let node = create::<ColorNode>();
                COLOR
                    .with(|c| set_state(&node, c, value.to_owned()))
                    .unwrap();
                with(node, vec![t(text)])
            };
            get_root().append(&[p(vec![
                color("red", "1"),
                color("red", "2"),
                color("blue", "3"),
            ])])?;
            Ok(())
        })),
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["paragraph",["test-color","12"],["test-color","3"]]]"#)
    );
    editor.dispose();
}

element_class!(FitItemNode, "test-fit-item", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![link(
            "test-fit-item",
            StaticNodeConfig {
                children: Some(NodeChildrenConfig::Inline),
                ..StaticNodeConfig::default()
            },
        )]
    }
});
element_class!(FitBlockItemNode, "test-fit-block-item", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![link(
            "test-fit-block-item",
            StaticNodeConfig {
                children: Some(NodeChildrenConfig::Block),
                ..StaticNodeConfig::default()
            },
        )]
    }
});

fn fit_group_link() -> NodeConfigLink {
    link(
        "test-fit-group",
        StaticNodeConfig {
            children: Some(NodeChildrenConfig::List(vec![NodeChildEntry::Class(
                NodeClassRef::of::<FitItemNode>(),
            )])),
            merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![
                MergeAdjacentEntry::Field("format"),
            ])),
            ..StaticNodeConfig::default()
        },
    )
}

element_class!(FitGroupNode, "test-fit-group", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![fit_group_link()]
    }
});
element_class!(FitBlockGroupNode, "test-fit-block-group", {}, {
    fn super_types(&self) -> Vec<&'static str> {
        vec!["test-fit-group"]
    }
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![
            link(
                "test-fit-block-group",
                StaticNodeConfig {
                    children: Some(NodeChildrenConfig::List(vec![NodeChildEntry::Class(
                        NodeClassRef::of::<FitBlockItemNode>(),
                    )])),
                    ..StaticNodeConfig::default()
                },
            ),
            fit_group_link(),
        ]
    }
});

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > an inherited mergeAdjacent that does not fit the subclass is dropped
#[test]
fn an_inherited_merge_adjacent_that_does_not_fit_the_subclass_is_dropped() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        create_editor(CreateEditorArgs {
            nodes: vec![
                NodeClass::of::<FitItemNode>(),
                NodeClass::of::<FitBlockItemNode>(),
                NodeClass::of::<FitGroupNode>(),
                NodeClass::of::<FitBlockGroupNode>(),
            ],
            ..CreateEditorArgs::default()
        });
    }));
    assert!(result.is_ok());
    let group = get_declared_node_structure(&FitGroupNode::default()).merge_adjacent;
    let by: Vec<String> = group
        .unwrap()
        .by
        .iter()
        .map(|e| match e {
            MergeAdjacentEntry::Field(name) => (*name).to_owned(),
            MergeAdjacentEntry::State(_) => "state".to_owned(),
        })
        .collect();
    assert_eq!(by, vec!["format".to_owned()]);
    assert!(
        get_declared_node_structure(&FitBlockGroupNode::default())
            .merge_adjacent
            .is_none()
    );
}

// --- registration

/// `sh(n)`: the types of elements and leaves.
fn types_shape(n: &NodeRef) -> Value {
    if n.is_element() {
        let mut out = vec![Value::from(n.get_type())];
        out.extend(n.get_children().iter().map(types_shape));
        Value::Array(out)
    } else {
        Value::from(n.get_type())
    }
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > registerNodeStructure: idempotent, and disposed by its last registration
#[test]
fn register_node_structure_idempotent_and_disposed_by_its_last_registration() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<crate::extension::HorizontalRuleNode>()],
        ..CreateEditorArgs::default()
    });
    let broken = || {
        let root = get_root();
        root.clear()?;
        root.append(&[p(vec![t("a"), hr()])])?;
        Ok(())
    };
    let root_types = |editor: &Editor| {
        editor
            .read(|| Value::Array(get_root().get_children().iter().map(types_shape).collect()))
            .unwrap()
    };
    let first = register_node_structure(&editor);
    let second = register_node_structure(&editor);
    upd(&editor, broken);
    assert_eq!(
        root_types(&editor),
        j(r#"[["paragraph","text"],"horizontalrule"]"#)
    );
    // `first(); first();`: an `Unregister` is `FnOnce`, so the type rules
    // out the second call, which does nothing in JS.
    first();
    upd(&editor, broken);
    assert_eq!(
        root_types(&editor),
        j(r#"[["paragraph","text"],"horizontalrule"]"#)
    );
    second();
    assert!(!internal_is_node_structure_enforced(&editor));
    upd(&editor, broken);
    assert_eq!(
        root_types(&editor),
        j(r#"[["paragraph","text","horizontalrule"]]"#)
    );
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > disabled: the declarations are only read; turning it on normalizes the document
#[test]
fn disabled_the_declarations_are_only_read_turning_it_on_normalizes_the_document() {
    let editor = build_with(
        "disabled",
        vec![
            crate::rich_text::rich_text_extension().into(),
            horizontal_rule_extension().into(),
            config_extension(
                &node_structure_extension(),
                [NodeStructureConfigOverrides {
                    disabled: Some(true),
                }],
            ),
        ],
        vec![],
        Some(Rc::new(paragraph_with_rule)),
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["paragraph","a","horizontalrule","b"]]"#)
    );
    assert!(!internal_is_node_structure_enforced(&editor));
    get_extension_dependency_from_editor(&editor, &node_structure_extension())
        .unwrap()
        .output
        .unwrap()
        .disabled
        .set_value(false);
    assert_eq!(
        root_shape(&editor),
        j(r#"[["paragraph","a"],"horizontalrule",["paragraph","b"]]"#)
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > each editor enforces its own registration (nested editors too)
#[test]
fn each_editor_enforces_its_own_registration_nested_editors_too() {
    let parent = build(|| Ok(()), true);
    let nested = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<crate::extension::HorizontalRuleNode>()],
        parent_editor: Some((*parent).clone()),
        ..CreateEditorArgs::default()
    });
    upd(&nested, paragraph_with_rule);
    let nested_types = |e: &Editor| {
        e.read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|n| n.get_type().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap()
    };
    assert_eq!(nested_types(&nested), vec!["paragraph"]);
    let dispose = register_node_structure(&nested);
    assert_eq!(
        nested_types(&nested),
        vec!["paragraph", "horizontalrule", "paragraph"]
    );
    dispose();
    parent.dispose();
}

element_class!(
    /// `MyOverflowNode extends OverflowNode`.
    MyOverflowNode,
    "test-my-overflow",
    {},
    {
        fn super_types(&self) -> Vec<&'static str> {
            vec!["overflow"]
        }
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            vec![own("test-my-overflow"), own("overflow")]
        }
    }
);
element_class!(
    /// `FakeOverflowNode extends ParagraphNode`, of type `overflow`.
    FakeOverflowNode,
    "overflow",
    {},
    {
        fn super_types(&self) -> Vec<&'static str> {
            vec!["paragraph"]
        }
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            let mut chain = vec![own("overflow")];
            chain.extend(
                crate::nodes::paragraph::ParagraphNode::default().static_node_config_chain(),
            );
            chain
        }
    }
);

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > an OverflowNode subclass is exempt too; a class that only shares the type name is not
#[test]
fn an_overflow_node_subclass_is_exempt_too_a_class_that_only_shares_the_type_name_is_not() {
    let editor = build_with(
        "overflow",
        rich_and_structure(),
        vec![NodeClass::of::<MyOverflowNode>()],
        Some(Rc::new(|| {
            get_root().append(&[p(vec![
                t("within"),
                with(create::<MyOverflowNode>(), vec![t("over")]),
            ])])?;
            Ok(())
        })),
    );
    assert_eq!(
        root_shape(&editor),
        j(r#"[["paragraph","within",["test-my-overflow","over"]]]"#)
    );
    let fake = build_with(
        "fake-overflow",
        rich_and_structure(),
        vec![NodeClass::of::<FakeOverflowNode>()],
        Some(Rc::new(|| {
            let para = create_paragraph_node();
            get_root().append(std::slice::from_ref(&para))?;
            element::splice(
                &para,
                0,
                0,
                &[t("a"), with(create::<FakeOverflowNode>(), vec![t("b")])],
            )?;
            Ok(())
        })),
    );
    assert_eq!(
        root_shape(&fake),
        j(r#"[["paragraph","a","linebreak","b"]]"#)
    );
    editor.dispose();
    fake.dispose();
}

thread_local! {
    static BLOCK_TAG_SCHEMA: NodeSchema = node_schema(vec![(
        "tag",
        with_field(string_value(""), |n: &BlockTagNode| &n.tag, |n: &mut BlockTagNode| &mut n.tag).unwrap(),
    )])
    .unwrap();
}

element_class!(
    /// `BlockTagNode`: a block holding inline content, merged by `tag`.
    BlockTagNode, "test-block-tag", { tag: String }, {
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            vec![link("test-block-tag", StaticNodeConfig {
                children: Some(NodeChildrenConfig::Inline),
                merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![MergeAdjacentEntry::Field("tag")])),
                ..StaticNodeConfig::default()
            })]
        }
        fn json_schema_chain(&self) -> Vec<NodeSchema> {
            vec![BLOCK_TAG_SCHEMA.with(Clone::clone), element::element_node_schema()]
        }
    }
);

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > INTERNAL_$normalizeNodeStructure normalizes a subtree now, without the extension
#[test]
fn internal_normalize_node_structure_normalizes_a_subtree_now_without_the_extension() {
    let editor = build_with(
        "internal",
        vec![
            crate::rich_text::rich_text_extension().into(),
            horizontal_rule_extension().into(),
        ],
        vec![NodeClass::of::<BlockTagNode>()],
        None,
    );
    upd(&editor, || {
        let tag = |children: Vec<NodeRef>| {
            let node = create::<BlockTagNode>();
            crate::lexical_utils::with_writable(node.get_key(), |n| {
                n.downcast_mut::<BlockTagNode>().unwrap().tag = "t".to_owned()
            });
            with(node, children)
        };
        let root = get_root();
        root.clear()?;
        root.append(&[tag(vec![t("a")]), tag(vec![t("b"), hr()])])?;
        Ok(())
    });
    assert_eq!(
        root_shape(&editor),
        j(r#"[["test-block-tag","a"],["test-block-tag","b","horizontalrule"]]"#)
    );
    upd(&editor, || {
        internal_normalize_node_structure(&get_root().get_last_child_or_throw()?)
    });
    let normalized = j(r#"[["test-block-tag","ab"],"horizontalrule"]"#);
    assert_eq!(root_shape(&editor), normalized);
    upd(&editor, || internal_normalize_node_structure(&get_root()));
    assert_eq!(root_shape(&editor), normalized);
    editor.dispose();
}

// --- getDeclaredNodeStructure

thread_local! {
    static COUNTED_SCHEMA: NodeSchema = node_schema(vec![(
        "count",
        with_field(
            number_value(0.0, NumberValueOptions::default()).unwrap(),
            |n: &CountedNode| &n.count,
            |n: &mut CountedNode| &mut n.count,
        )
        .unwrap()
        .derived(),
    )])
    .unwrap();
}

element_class!(
    /// `CountedNode`: a derived `count` field named in `mergeAdjacent.by`.
    CountedNode, "test-counted", { count: f64 }, {
        fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
            vec![link("test-counted", StaticNodeConfig {
                merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![MergeAdjacentEntry::Field("count")])),
                ..StaticNodeConfig::default()
            })]
        }
        fn json_schema_chain(&self) -> Vec<NodeSchema> {
            vec![COUNTED_SCHEMA.with(Clone::clone), element::element_node_schema()]
        }
    }
);

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::getDeclaredNodeStructure > a derived schema field in mergeAdjacent.by is refused with its own message
#[test]
fn a_derived_schema_field_in_merge_adjacent_by_is_refused_with_its_own_message() {
    assert_eq!(
        get_declared_node_structure(&CountedNode::default()).derived,
        vec!["count".to_owned()]
    );
    let message = create_editor_panic(vec![NodeClass::of::<CountedNode>()]);
    assert!(
        message.contains("names count, which is derived"),
        "{message}"
    );
}

element_class!(WrapItemNode, "test-wrap-item", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![link(
            "test-wrap-item",
            StaticNodeConfig {
                children: Some(NodeChildrenConfig::Inline),
                ..StaticNodeConfig::default()
            },
        )]
    }
});
element_class!(WrapGroupNode, "test-wrap-group", {}, {
    fn static_node_config_chain(&self) -> Vec<NodeConfigLink> {
        vec![link(
            "test-wrap-group",
            StaticNodeConfig {
                children: Some(NodeChildrenConfig::List(vec![
                    NodeChildEntry::Class(NodeClassRef::of::<WrapItemNode>()),
                    NodeChildEntry::Class(
                        NodeClassRef::of::<crate::nodes::paragraph::ParagraphNode>(),
                    ),
                ])),
                ..StaticNodeConfig::default()
            },
        )]
    }
});

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::getDeclaredNodeStructure > a custom group wraps a stray child in the first class it lists
#[test]
fn a_custom_group_wraps_a_stray_child_in_the_first_class_it_lists() {
    let editor = build_with(
        "group",
        rich_and_structure(),
        vec![
            NodeClass::of::<WrapItemNode>(),
            NodeClass::of::<WrapGroupNode>(),
        ],
        Some(Rc::new(|| {
            let group = create::<WrapGroupNode>();
            get_root().append(std::slice::from_ref(&group))?;
            element::splice(
                &group,
                0,
                0,
                &[t("a"), create_line_break_node(), p(vec![t("p")]), t("b")],
            )?;
            Ok(())
        })),
    );
    assert_eq!(
        root_shape(&editor),
        j(
            r#"[["test-wrap-group",["test-wrap-item","a","linebreak"],["paragraph","p"],["test-wrap-item","b"]]]"#
        )
    );
    editor.dispose();
}

// --- carets, empty inline elements and dirty order

/// `buildEditor()` of `carets, empty inline elements and dirty order`, and
/// of `formatting, ...` (with or without the extension).
fn caret_editor(with_structure: bool) -> LexicalEditorWithDispose {
    let mut deps = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::list::list_extension().into(),
        crate::link::link_extension().into(),
    ];
    // `configExtension(TableExtension, {hasHorizontalScroll: false})`.
    #[cfg(feature = "table")]
    deps.push(config_extension(
        &crate::table::table_extension(),
        [crate::table::TableConfigOverrides {
            has_horizontal_scroll: Some(false),
            ..Default::default()
        }],
    ));
    deps.push(horizontal_rule_extension().into());
    if with_structure {
        deps.push(node_structure_extension().into());
    }
    build_with(
        "node-structure-carets",
        deps,
        vec![
            NodeClass::of::<crate::mark::MarkNode>(),
            NodeClass::of::<crate::overflow::OverflowNode>(),
        ],
        None,
    )
}

/// `loadThenNormalize(editor, $build, order, $select)`.
fn load_then_normalize(
    editor: &Editor,
    build: fn() -> Vec<NodeRef>,
    inner_first: bool,
    select: impl FnOnce() -> Result<(), LexicalError> + 'static,
) {
    editor
        .update(
            move || {
                let root = get_root();
                root.clear()?;
                root.append(&build())?;
                select()
            },
            skip_transforms(),
        )
        .unwrap();
    upd(editor, move || {
        fn walk(node: &NodeRef, inner_first: bool, out: &mut Vec<NodeRef>) {
            if !inner_first {
                out.push(node.clone());
            }
            if node.is_element() {
                for c in node.get_children() {
                    walk(&c, inner_first, out);
                }
            }
            if inner_first {
                out.push(node.clone());
            }
        }
        let mut nodes = Vec::new();
        for c in get_root().get_children() {
            walk(&c, inner_first, &mut nodes);
        }
        for n in nodes {
            n.mark_dirty();
        }
        Ok(())
    });
}

/// `content(editor)`: the text, `#` for a rule, line breaks left out.
fn content(editor: &Editor) -> String {
    editor
        .read(|| {
            fn walk(node: &NodeRef, out: &mut String) {
                if node.is_element() {
                    for c in node.get_children() {
                        walk(&c, out);
                    }
                } else if node.get_type() == "horizontalrule" {
                    out.push('#');
                } else if node.is_text() {
                    out.push_str(&node.get_text_content());
                }
            }
            let mut out = String::new();
            for c in get_root().get_children() {
                walk(&c, &mut out);
            }
            out
        })
        .unwrap()
}

fn pp(children: Vec<NodeRef>) -> NodeRef {
    put(create_paragraph_node(), children)
}

fn element_caret_case(
    build: fn() -> Vec<NodeRef>,
    path: &'static [usize],
    offset: usize,
    want: &str,
) {
    let editor = caret_editor(true);
    load_then_normalize(&editor, build, false, move || {
        let mut node = get_root();
        for &i in path {
            node = node.get_child_at_index(i).unwrap();
        }
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, node.get_key(), offset, PointType::Element)?;
        selection.point_set(Which::Focus, node.get_key(), offset, PointType::Element)?;
        set_selection(Some(selection));
        Ok(())
    });
    insert_text_z(&editor, "Z");
    assert_eq!(content(&editor), want);
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an element caret after a mark split around a paragraph keeps its place in the content
#[test]
fn an_element_caret_after_a_mark_split_around_a_paragraph_keeps_its_place_in_the_content() {
    element_caret_case(
        || {
            vec![pp(vec![put(
                mark("m"),
                vec![t("a"), pp(vec![t("b")]), t("c")],
            )])]
        },
        &[0],
        1,
        "abcZ",
    );
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an element caret after a quote lifted out of a list item and split around a rule keeps its place in the content
#[test]
fn an_element_caret_after_a_quote_lifted_out_of_a_list_item_and_split_around_a_rule() {
    element_caret_case(
        || {
            vec![put(
                create_list_node(ListType::Bullet, 1.0),
                vec![put(
                    create_list_item_node(None),
                    vec![
                        t("x"),
                        put(
                            create_quote_node(false),
                            vec![pp(vec![t("a")]), hr(), t("b")],
                        ),
                    ],
                )],
            )]
        },
        &[0, 0],
        2,
        "xa#bZ",
    );
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an element caret after a paragraph split around a rule keeps its place in the content
#[test]
fn an_element_caret_after_a_paragraph_split_around_a_rule_keeps_its_place_in_the_content() {
    element_caret_case(|| vec![pp(vec![t("a"), hr(), t("b")])], &[], 1, "a#bZ");
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an element caret before a list whose first item starts with a rule keeps its place in the content
#[test]
fn an_element_caret_before_a_list_whose_first_item_starts_with_a_rule() {
    element_caret_case(
        || {
            vec![put(
                create_list_node(ListType::Number, 1.0),
                vec![put(create_list_item_node(None), vec![hr(), t("y")])],
            )]
        },
        &[],
        0,
        "Z#y",
    );
}

fn removed_before_holder_case(build: fn() -> Vec<NodeRef>, want: &str) {
    for inner_first in [false, true] {
        let editor = caret_editor(true);
        load_then_normalize(&editor, build, inner_first, || Ok(()));
        assert_eq!(root_shape(&editor), j(want), "inner first: {inner_first}");
        editor.dispose();
    }
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an empty link in an OverflowNode before a rule in a list item is removed before its holder is repaired, whatever the dirty order
#[test]
fn an_empty_link_in_an_overflow_node_before_a_rule_in_a_list_item_is_removed_first() {
    removed_before_holder_case(
        || {
            vec![put(
                create_list_node(ListType::Bullet, 1.0),
                vec![put(
                    create_list_item_node(None),
                    vec![
                        put(
                            crate::overflow::create_overflow_node(),
                            vec![a_link("https://a")],
                        ),
                        hr(),
                        t("y"),
                    ],
                )],
            )]
        },
        r#"["horizontalrule",["list",["listitem","y"]]]"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an empty link alone in a list is removed before its holder is repaired, whatever the dirty order
#[test]
fn an_empty_link_alone_in_a_list_is_removed_before_its_holder_is_repaired() {
    removed_before_holder_case(
        || {
            vec![
                put(
                    create_list_node(ListType::Bullet, 1.0),
                    vec![a_link("https://a")],
                ),
                pp(vec![t("z")]),
            ]
        },
        r#"[["paragraph","z"]]"#,
    );
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > a node whose rule changes is checked again, though no child was written
#[test]
fn a_node_whose_rule_changes_is_checked_again_though_no_child_was_written() {
    let editor = caret_editor(true);
    upd(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[with(create_quote_node(false), vec![t("a")])])?;
        Ok(())
    });
    upd(&editor, || {
        get_root()
            .get_first_child()
            .unwrap()
            .set_quote_is_shadow_root(true)
    });
    assert_eq!(root_shape(&editor), j(r#"[["quote",["paragraph","a"]]]"#));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > items a list renumbers are not visited; what changed still is
#[test]
fn items_a_list_renumbers_are_not_visited_what_changed_still_is() {
    let editor = caret_editor(true);
    upd(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[with(
            create_list_node(ListType::Number, 1.0),
            ["a", "b", "c", "d"]
                .iter()
                .map(|s| li(vec![t(s)]))
                .collect(),
        )])?;
        Ok(())
    });
    upd(&editor, || {
        get_root().get_all_text_nodes()[0].select(Some(1), Some(1))?;
        get_selection()
            .filter(crate::SelectionRef::is_range)
            .unwrap()
            .insert_paragraph()?;
        get_root().get_all_text_nodes()[1].insert_after(&hr(), true)?;
        Ok(())
    });
    assert_eq!(
        list_starts(&editor),
        j(r#"[[1,"a","","b"],"horizontalrule",[4,"c","d"]]"#)
    );
    editor.dispose();
}

// --- formatting, list seams, orphan list items and the root

/// `annotated(editor)`: the shape with each link's URL and each mark's ids.
fn annotated(editor: &Editor) -> Value {
    fn walk(node: &NodeRef) -> Value {
        let head = if let Some(ids) = node.get_ids() {
            Some(format!("mark:{}", ids.join(",")))
        } else {
            node.get_url().map(|url| format!("link:{url}"))
        };
        match head {
            Some(head) => {
                let mut out = vec![Value::from(head)];
                out.extend(node.get_children().iter().map(walk));
                Value::Array(out)
            }
            None if node.is_element() => {
                let mut out = vec![Value::from(node.get_type())];
                out.extend(node.get_children().iter().map(walk));
                Value::Array(out)
            }
            None => shape(node),
        }
    }
    editor
        .read(|| Value::Array(get_root().get_children().iter().map(walk).collect()))
        .unwrap()
}

/// `load(editor, $build)`.
fn load(editor: &Editor, build: fn() -> Vec<NodeRef>) {
    upd(editor, move || {
        let root = get_root();
        root.clear()?;
        put(root, build());
        Ok(())
    });
}

fn ul(children: Vec<NodeRef>) -> NodeRef {
    put(create_list_node(ListType::Bullet, 1.0), children)
}

fn pli(children: Vec<NodeRef>) -> NodeRef {
    put(create_list_item_node(None), children)
}

/// `$nested(text, ...rest)`.
fn nested(text: &str, rest: Vec<NodeRef>) -> NodeRef {
    let mut children = vec![put(
        create_list_node(ListType::Number, 1.0),
        vec![pli(vec![t(text)])],
    )];
    children.extend(rest);
    pli(children)
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > a list item outside a list, in a link, keeps the link as it does without the extension
#[test]
fn a_list_item_outside_a_list_in_a_link_keeps_the_link_as_it_does_without_the_extension() {
    fn doc() -> Vec<NodeRef> {
        vec![with(
            create_paragraph_node(),
            vec![put(a_link("https://a"), vec![t("l"), pli(vec![t("x")])])],
        )]
    }
    let on = caret_editor(true);
    let off = caret_editor(false);
    load(&on, doc);
    load(&off, doc);
    assert_eq!(
        annotated(&on),
        j(r#"[["paragraph",["link:https://a","l"]],["list",["listitem",["link:https://a","x"]]]]"#)
    );
    assert_eq!(annotated(&on), annotated(&off));
    on.dispose();
    off.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > lists that merge keep what follows or precedes a nested list in an item at the seam
#[test]
fn lists_that_merge_keep_what_follows_or_precedes_a_nested_list_in_an_item_at_the_seam() {
    {
        let editor = caret_editor(true);
        load(&editor, || {
            vec![
                ul(vec![nested("a", vec![])]),
                with(create_paragraph_node(), vec![t("sep")]),
                ul(vec![nested("b", vec![t("tail")])]),
            ]
        });
        upd(&editor, || {
            get_root().get_child_at_index(1).unwrap().remove(false)
        });
        assert_eq!(
            root_shape(&editor),
            j(
                r#"[["list",["listitem",["list",["listitem","a"],["listitem","b"]]],["listitem","tail"]]]"#
            )
        );
        editor.dispose();
    }
    {
        let editor = caret_editor(true);
        load(&editor, || {
            vec![
                ul(vec![nested("a", vec![t("x")])]),
                ul(vec![nested("b", vec![])]),
            ]
        });
        assert_eq!(
            root_shape(&editor),
            j(
                r#"[["list",["listitem",["list",["listitem","a"]]],["listitem","x"],["listitem",["list",["listitem","b"]]]]]"#
            )
        );
        editor.dispose();
    }
    {
        let editor = caret_editor(true);
        load(&editor, || {
            vec![
                with(
                    create_paragraph_node(),
                    vec![put(
                        a_link("https://a"),
                        vec![ul(vec![nested("a", vec![])])],
                    )],
                ),
                ul(vec![nested("b", vec![t("tail")])]),
            ]
        });
        assert_eq!(
            annotated(&editor),
            j(
                r#"[["list",["listitem",["list",["listitem",["link:https://a","a"]],["listitem","b"]]],["listitem","tail"]]]"#
            )
        );
        editor.dispose();
    }
    fn chain() -> Vec<NodeRef> {
        vec![
            ul(vec![nested("a", vec![])]),
            ul(vec![
                nested("b", vec![]),
                nested("c", vec![]),
                pli(vec![t("d")]),
            ]),
        ]
    }
    let on = caret_editor(true);
    let off = caret_editor(false);
    load(&on, chain);
    load(&off, chain);
    assert_eq!(
        root_shape(&on),
        j(
            r#"[["list",["listitem",["list",["listitem","a"],["listitem","b"]]],["listitem",["list",["listitem","c"]]],["listitem","d"]]]"#
        )
    );
    assert_eq!(
        crate::json::stringify(&on.get_editor_state().to_json(false)),
        crate::json::stringify(&off.get_editor_state().to_json(false))
    );
    on.dispose();
    off.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > items the seam joins are still normalized: the result is a fixed point
#[test]
fn items_the_seam_joins_are_still_normalized_the_result_is_a_fixed_point() {
    fn doc() -> Vec<NodeRef> {
        vec![
            ul(vec![nested("a", vec![])]),
            ul(vec![pli(vec![put(
                create_list_node(ListType::Number, 1.0),
                vec![pli(vec![
                    with(create_paragraph_node(), vec![t("p")]),
                    with(create_paragraph_node(), vec![t("q")]),
                ])],
            )])]),
        ]
    }
    let loaded = caret_editor(true);
    load(&loaded, doc);
    let normalized = caret_editor(false);
    normalized
        .update(
            || {
                let root = get_root();
                root.clear()?;
                put(root.clone(), doc());
                internal_normalize_node_structure(&root)
            },
            skip_transforms(),
        )
        .unwrap();
    let expected =
        j(r#"[["list",["listitem",["list",["listitem","a"],["listitem","p","linebreak","q"]]]]]"#);
    assert_eq!(root_shape(&loaded), expected);
    assert_eq!(root_shape(&normalized), expected);
    loaded.dispose();
    normalized.dispose();
}

/// `typeAt($build, offset)`.
fn type_at(build: fn() -> NodeRef, offset: usize) -> (Value, bool) {
    let editor = caret_editor(true);
    upd(&editor, move || {
        let holder = build();
        let root = get_root();
        root.clear()?;
        put(root, vec![holder.clone()]);
        holder.select(Some(offset), Some(offset))?;
        Ok(())
    });
    insert_text_z(&editor, "Z");
    let attached = editor
        .read(|| {
            get_selection()
                .filter(crate::SelectionRef::is_range)
                .unwrap()
                .point(Which::Anchor)
                .unwrap()
                .get_node()
                .unwrap()
                .is_attached()
        })
        .unwrap();
    let out = (root_shape(&editor), attached);
    editor.dispose();
    out
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > a caret next to a list item outside a list keeps its place
#[test]
fn a_caret_next_to_a_list_item_outside_a_list_keeps_its_place() {
    assert_eq!(
        type_at(
            || put(create_paragraph_node(), vec![t("a"), pli(vec![])]),
            2
        ),
        (j(r#"[["paragraph","aZ"],["list",["listitem"]]]"#), true)
    );
    assert_eq!(
        type_at(
            || put(create_quote_node(false), vec![t("a"), pli(vec![])]),
            2
        ),
        (j(r#"[["quote","aZ"],["list",["listitem"]]]"#), true)
    );
    assert_eq!(
        type_at(
            || put(create_paragraph_node(), vec![t("a"), pli(vec![t("x")])]),
            1
        ),
        (j(r#"[["paragraph","aZ"],["list",["listitem","x"]]]"#), true)
    );
    assert_eq!(
        type_at(
            || put(create_paragraph_node(), vec![t("a"), pli(vec![t("x")])]),
            2
        ),
        (
            j(r#"[["paragraph","a"],["list",["listitem","x"]],["paragraph","Z"]]"#),
            true
        )
    );
    assert_eq!(
        type_at(|| put(create_paragraph_node(), vec![pli(vec![])]), 1),
        (j(r#"[["list",["listitem"]],["paragraph","Z"]]"#), true)
    );
}

// upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > inline content at the root is wrapped whenever core would wrap it
#[test]
fn inline_content_at_the_root_is_wrapped_whenever_core_would_wrap_it() {
    let run = |with_structure: bool, edit: fn() -> Result<(), LexicalError>| {
        let editor = caret_editor(with_structure);
        load(&editor, || {
            vec![with(create_paragraph_node(), vec![t("p")])]
        });
        editor
            .update(
                || {
                    put(get_root(), vec![t("stray")]);
                    Ok(())
                },
                skip_transforms(),
            )
            .unwrap();
        upd(&editor, edit);
        let out = root_shape(&editor);
        editor.dispose();
        out
    };
    let edits: [fn() -> Result<(), LexicalError>; 2] = [
        || {
            let text = get_root().get_first_descendant().unwrap();
            assert!(text.is_text());
            text.set_text_content("pp");
            Ok(())
        },
        || {
            get_root().mark_dirty();
            Ok(())
        },
    ];
    for edit in edits {
        assert_eq!(run(true, edit), run(false, edit));
    }
    assert_eq!(
        run(true, edits[0]),
        j(r#"[["paragraph","pp"],["paragraph","stray"]]"#)
    );
}

/// The tests that make a table.
#[cfg(feature = "table")]
mod tables {
    use super::*;
    use crate::table::{
        TableCellNode, TableNode, TableRowNode, create_table_cell_node, create_table_node,
        create_table_row_node,
    };

    fn table_of(cell_children: Vec<NodeRef>) -> NodeRef {
        with(
            create_table_node(),
            vec![with(
                create_table_row_node(None),
                vec![with(create_table_cell_node(0, 1.0, None), cell_children)],
            )],
        )
    }

    fn children_of(node: &dyn LexicalNode) -> Option<NodeChildrenConfig> {
        get_declared_node_structure(node).children
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::declarations > Lexical's own nodes
    #[test]
    fn lexical_s_own_nodes() {
        use crate::code::CodeNode;
        use crate::link::{AutoLinkNode, LinkNode};
        use crate::list::{ListItemNode, ListNode};
        use crate::rich_text::{HeadingNode, QuoteNode};
        let inline = Some(NodeChildrenConfig::Inline);
        assert_eq!(
            children_of(&crate::nodes::paragraph::ParagraphNode::default()),
            inline
        );
        assert_eq!(children_of(&HeadingNode::default()), inline);
        assert_eq!(children_of(&QuoteNode::default()), inline);
        assert_eq!(children_of(&CodeNode::default()), inline);
        assert_eq!(
            children_of(&TableCellNode::default()),
            Some(NodeChildrenConfig::Block)
        );
        assert_eq!(
            children_of(&ListNode::default()),
            Some(NodeChildrenConfig::List(vec![NodeChildEntry::Class(
                NodeClassRef::of::<ListItemNode>()
            )]))
        );
        let fields = |merge: Option<crate::MergeAdjacentConfig>| -> Vec<&'static str> {
            merge
                .expect("a mergeAdjacent")
                .by
                .iter()
                .map(|entry| match entry {
                    MergeAdjacentEntry::Field(name) => *name,
                    MergeAdjacentEntry::State(_) => panic!("a state"),
                })
                .collect()
        };
        assert_eq!(
            fields(get_declared_node_structure(&ListNode::default()).merge_adjacent),
            ["listType"]
        );
        assert_eq!(
            children_of(&ListItemNode::default()),
            Some(NodeChildrenConfig::List(vec![
                NodeChildEntry::Inline,
                NodeChildEntry::Class(NodeClassRef::of::<ListNode>()),
            ]))
        );
        let link = get_declared_node_structure(&LinkNode::default());
        assert_eq!(link.inline, Some(true));
        assert_eq!(
            fields(link.merge_adjacent),
            ["url", "target", "rel", "title"]
        );
        // AutoLinkNode inherits inline/children but turns the merge off.
        let auto = get_declared_node_structure(&AutoLinkNode::default());
        assert_eq!(auto.inline, Some(true));
        assert_eq!(auto.children, inline);
        assert!(auto.merge_adjacent.is_none());
        assert_eq!(
            get_declared_node_structure(&crate::mark::MarkNode::default()).inline,
            Some(true)
        );
        // OverflowNode declares nothing (the normalization knows it, see
        // below).
        let overflow = get_declared_node_structure(&crate::overflow::OverflowNode::default());
        assert!(overflow.inline.is_none());
        assert!(overflow.children.is_none());
        assert!(overflow.merge_adjacent.is_none());
        assert!(overflow.derived.is_empty());
        assert_eq!(overflow.declared_by, crate::DeclaredBy::default());
    }

    /// `$paraWithTable()`.
    fn para_with_table() -> Result<(), LexicalError> {
        let table = table_of(vec![p(vec![t("cell")])]);
        get_root().append(&[p(vec![t("before"), table, t("after")])])?;
        Ok(())
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > off by default: a block in a paragraph stays
    #[test]
    fn off_by_default_a_block_in_a_paragraph_stays() {
        let editor = build(para_with_table, false);
        assert_eq!(
            root_shape(&editor),
            j(
                r#"[["paragraph","before",["table",["tablerow",["tablecell",["paragraph","cell"]]]],"after"]]"#
            )
        );
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block in a paragraph is lifted out (paragraph split)
    #[test]
    fn a_block_in_a_paragraph_is_lifted_out_paragraph_split() {
        let editor = build(para_with_table, true);
        assert_eq!(
            root_shape(&editor),
            j(
                r#"[["paragraph","before"],["table",["tablerow",["tablecell",["paragraph","cell"]]]],["paragraph","after"]]"#
            )
        );
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > inline content in a table cell is wrapped in a paragraph
    #[test]
    fn inline_content_in_a_table_cell_is_wrapped_in_a_paragraph() {
        let editor = build(
            || {
                get_root().append(&[table_of(vec![t("a"), p(vec![t("b")]), t("c")])])?;
                Ok(())
            },
            true,
        );
        assert_eq!(
            root_shape(&editor),
            j(
                r#"[["table",["tablerow",["tablecell",["paragraph","a"],["paragraph","b"],["paragraph","c"]]]]]"#
            )
        );
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a table in a list item is lifted out of the list
    #[test]
    fn a_table_in_a_list_item_is_lifted_out_of_the_list() {
        let editor = build(
            || {
                get_root().append(&[with(
                    create_list_node(ListType::Bullet, 1.0),
                    vec![
                        li(vec![t("x")]),
                        li(vec![table_of(vec![p(vec![t("c")])])]),
                        li(vec![t("y")]),
                    ],
                )])?;
                Ok(())
            },
            true,
        );
        let shapes = root_shape(&editor);
        // No text lost or duplicated, and the table is no longer in the list.
        let json = shapes.to_string();
        let texts: Vec<&str> = ["\"x\"", "\"y\"", "\"c\""]
            .iter()
            .flat_map(|needle| json.match_indices(needle))
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_values()
            .collect();
        assert_eq!(texts, ["\"x\"", "\"c\"", "\"y\""]);
        let Value::Array(blocks) = &shapes else {
            panic!("an array")
        };
        assert!(blocks.iter().any(|b| match b {
            Value::Array(items) => items.first() == Some(&Value::from("table")),
            _ => false,
        }));
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > no table, row or text is lost when blocks are lifted out of a list
    #[test]
    fn no_table_row_or_text_is_lost_when_blocks_are_lifted_out_of_a_list() {
        let table = |text: &str| table_of(vec![p(vec![t(text)])]);
        let editor = build(
            move || {
                get_root().append(&[with(
                    create_list_node(ListType::Bullet, 1.0),
                    vec![
                        li(vec![hr(), t("x")]),
                        li(vec![table("t1"), table("t2"), t("y")]),
                    ],
                )])?;
                Ok(())
            },
            true,
        );
        assert_eq!(
            root_shape(&editor),
            j(
                r#"["horizontalrule",["list",["listitem","x"]],["table",["tablerow",["tablecell",["paragraph","t1"]]]],["table",["tablerow",["tablecell",["paragraph","t2"]]]],["list",["listitem","y"]]]"#
            )
        );
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > normalization is a pure function of the document
    #[test]
    fn normalization_is_a_pure_function_of_the_document() {
        let run = || {
            let editor = build(para_with_table, true);
            let json = editor.get_editor_state().to_json(false).to_string();
            editor.dispose();
            json
        };
        assert_eq!(run(), run());
    }

    /// `ChipNode`: a decorator that turns inline while its cell is not
    /// written.
    #[derive(Clone, Debug, Default)]
    struct ChipNode {
        base: NodeBase,
        inline: bool,
    }

    impl LexicalNode for ChipNode {
        fn get_type(&self) -> &'static str {
            "test-chip-decorator"
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
            self.inline
        }
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > a table cell's declared children: inline content is wrapped in a paragraph
    #[test]
    fn a_table_cell_s_declared_children_inline_content_is_wrapped_in_a_paragraph() {
        // A decorator that turns inline while its cell is not written:
        // core's shadow root transform never sees it, the extension does.
        let run = |with_structure: bool| {
            let mut deps = vec![crate::rich_text::rich_text_extension().into()];
            if with_structure {
                deps.push(node_structure_extension().into());
            }
            // The table nodes alone: TableExtension's own transforms write
            // the cell, so core's transform would see it too.
            let editor = build_with(
                "cell",
                deps,
                vec![
                    NodeClass::of::<ChipNode>(),
                    NodeClass::of::<TableNode>(),
                    NodeClass::of::<TableRowNode>(),
                    NodeClass::of::<TableCellNode>(),
                ],
                Some(Rc::new(|| {
                    get_root().append(&[table_of(vec![create::<ChipNode>()])])?;
                    Ok(())
                })),
            );
            // Commit the initial state, in which every node is dirty.
            editor.read(|| {}).unwrap();
            upd(&editor, || {
                let table = get_root().get_first_child_or_throw()?;
                let row = table.get_first_child_or_throw()?;
                let cell = row.get_first_child_or_throw()?;
                let chip = cell.get_first_child_or_throw()?;
                assert!(chip.get_latest().is::<ChipNode>());
                crate::lexical_utils::with_writable(chip.get_key(), |n| {
                    n.downcast_mut::<ChipNode>().unwrap().inline = true;
                });
                Ok(())
            });
            let shape = root_shape(&editor);
            editor.dispose();
            shape
        };
        assert_eq!(
            run(false),
            j(r#"[["table",["tablerow",["tablecell","test-chip-decorator"]]]]"#)
        );
        assert_eq!(
            run(true),
            j(r#"[["table",["tablerow",["tablecell",["paragraph","test-chip-decorator"]]]]]"#)
        );
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an empty mark before a quote in a table cell is removed before its holder is repaired, whatever the dirty order
    #[test]
    fn an_empty_mark_before_a_quote_in_a_table_cell_is_removed_before_its_holder_is_repaired() {
        removed_before_holder_case(
            || {
                vec![with(
                    create_table_node(),
                    vec![with(
                        create_table_row_node(None),
                        vec![put(
                            create_table_cell_node(0, 1.0, None),
                            vec![mark("m"), put(create_quote_node(false), vec![t("x")])],
                        )],
                    )],
                )]
            },
            r#"[["table",["tablerow",["tablecell",["quote","x"]]]]]"#,
        );
    }

    // upstream: lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > a list, table or shadow root lifted out of a link or mark keeps them on all its text
    #[test]
    fn a_list_table_or_shadow_root_lifted_out_of_a_link_or_mark_keeps_them_on_all_its_text() {
        let cases: [(Children, &str); 6] = [
            (
                || vec![p(vec![put(mark("c1"), vec![ul(vec![pli(vec![t("x")])])])])],
                r#"[["list",["listitem",["mark:c1","x"]]]]"#,
            ),
            (
                || {
                    vec![p(vec![put(
                        mark("c1"),
                        vec![ul(vec![pli(vec![t("x"), ul(vec![pli(vec![t("y")])])])])],
                    )])]
                },
                r#"[["list",["listitem",["mark:c1","x"]],["listitem",["list",["listitem",["mark:c1","y"]]]]]]"#,
            ),
            (
                || {
                    vec![p(vec![put(
                        a_link("https://a"),
                        vec![put(mark("c1"), vec![ul(vec![pli(vec![t("x")])])])],
                    )])]
                },
                r#"[["list",["listitem",["link:https://a",["mark:c1","x"]]]]]"#,
            ),
            (
                || {
                    vec![p(vec![put(
                        mark("c1"),
                        vec![t("l"), table_of(vec![p(vec![t("cell")])])],
                    )])]
                },
                r#"[["paragraph",["mark:c1","l"]],["table",["tablerow",["tablecell",["paragraph",["mark:c1","cell"]]]]]]"#,
            ),
            (
                || {
                    vec![p(vec![put(
                        mark("c1"),
                        vec![put(create_quote_node(true), vec![p(vec![t("s")])])],
                    )])]
                },
                r#"[["quote",["paragraph",["mark:c1","s"]]]]"#,
            ),
            // An OverflowNode is wrapped inside, as LinkNode's own transform
            // leaves it.
            (
                || {
                    vec![p(vec![put(
                        mark("c1"),
                        vec![ul(vec![pli(vec![
                            t("x"),
                            put(crate::overflow::create_overflow_node(), vec![t("y")]),
                        ])])],
                    )])]
                },
                r#"[["list",["listitem",["mark:c1","x"],["overflow",["mark:c1","y"]]]]]"#,
            ),
        ];
        for (build, expected) in cases {
            let editor = caret_editor(true);
            load(&editor, build);
            assert_eq!(annotated(&editor), j(expected));
            editor.dispose();
        }
    }
}
