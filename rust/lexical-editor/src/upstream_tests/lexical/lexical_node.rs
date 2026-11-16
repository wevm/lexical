//! Ports of `packages/lexical/src/__tests__/unit/LexicalNode.test.ts`.
//!
//! Upstream checks the mounted editor's HTML after some updates; Rust has no
//! DOM reconciler (rust/PARITY.md), so those checks read the committed
//! document it renders instead ([`shape`]). A node upstream keeps as an
//! object is a [`NodeRef`] here, which resolves the latest version, so a
//! test that compares node objects compares keys, and one that reads a
//! stale version holds the `Rc` of that version.

use crate::nodes::element::ElementData;
use crate::nodes::text::TextData;
use crate::upstream_tests::support::{
    create_test_element_node, create_test_inline_element_node, initialize_unit_test, panics, shape,
    update, update_with,
};
use crate::{
    CreateEditorArgs, Editor, EditorReadMode, LexicalNode, NodeBase, NodeClass, NodeRef, PointType,
    SelectionRef, Which, create, create_editor, create_paragraph_node, create_range_selection,
    create_text_node, get_root, get_selection, set_selection,
};
use std::rc::Rc;

/// Stands in for constructing a bare `LexicalNode`, whose `getType` the
/// suite mocks to return `'node'`.
#[derive(Clone, Debug, Default)]
struct PlainNode {
    base: NodeBase,
}

impl LexicalNode for PlainNode {
    fn get_type(&self) -> &'static str {
        "node"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
}

/// `TestNode`.
#[derive(Clone, Debug, Default)]
struct TestNode {
    base: NodeBase,
}

impl LexicalNode for TestNode {
    fn get_type(&self) -> &'static str {
        "test"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
}

/// `InlineDecoratorNode`.
#[derive(Clone, Debug, Default)]
struct InlineDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for InlineDecoratorNode {
    fn get_type(&self) -> &'static str {
        "inline-decorator"
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
        true
    }
    fn is_parent_required(&self) -> bool {
        true
    }
    fn decorate(&self, _editor: &Editor) -> Option<Rc<dyn std::any::Any>> {
        Some(Rc::new("inline-decorator"))
    }
}

/// `BlockDecoratorNode`.
#[derive(Clone, Debug, Default)]
struct BlockDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for BlockDecoratorNode {
    fn get_type(&self) -> &'static str {
        "block-decorator"
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
    fn decorate(&self, _editor: &Editor) -> Option<Rc<dyn std::any::Any>> {
        Some(Rc::new("block-decorator"))
    }
}

/// The `LexicalNode tests` suite's editor and its `beforeEach`: a paragraph
/// holding the text node `foo`.
struct NodeTestEnv {
    editor: Editor,
    paragraph_node: NodeRef,
    text_node: NodeRef,
}

impl NodeTestEnv {
    fn new() -> Self {
        let editor = initialize_unit_test(CreateEditorArgs {
            nodes: vec![
                NodeClass::of::<PlainNode>(),
                NodeClass::of::<TestNode>(),
                NodeClass::of::<InlineDecoratorNode>(),
                NodeClass::of::<BlockDecoratorNode>(),
            ],
            ..CreateEditorArgs::default()
        });
        let (paragraph_node, text_node) = update_with(&editor, || {
            let paragraph_node = create_paragraph_node();
            let text_node = create_text_node("foo");
            paragraph_node.append(std::slice::from_ref(&text_node))?;
            get_root().append(std::slice::from_ref(&paragraph_node))?;
            Ok((paragraph_node, text_node))
        });
        Self {
            editor,
            paragraph_node,
            text_node,
        }
    }

    /// `editor.read('latest', fn)`.
    fn latest<R>(&self, f: impl FnOnce() -> R) -> R {
        self.editor
            .read_with_mode(EditorReadMode::Latest, f)
            .unwrap()
    }

    /// Unmergeable text nodes appended to the paragraph.
    fn append_unmergeable(&self, texts: &'static [&'static str]) -> Vec<NodeRef> {
        let paragraph_node = self.paragraph_node.clone();
        update_with(&self.editor, move || {
            let nodes: Vec<NodeRef> = texts
                .iter()
                .map(|t| {
                    let node = create_text_node(t);
                    node.toggle_unmergeable();
                    node
                })
                .collect();
            paragraph_node.append(&nodes)?;
            Ok(nodes)
        })
    }
}

fn keys(nodes: &[NodeRef]) -> Vec<String> {
    nodes.iter().map(|n| n.get_key().to_owned()).collect()
}

/// `VersionedTextNode`: a text node whose `afterCloneFrom` counts its
/// versions, which Rust's `Clone` is.
#[derive(Debug, Default)]
struct VersionedTextNode {
    base: NodeBase,
    text: TextData,
    version: u32,
    node_type: VersionedType,
}

#[derive(Clone, Copy, Debug, Default)]
enum VersionedType {
    #[default]
    VText,
    SncVText,
}

impl Clone for VersionedTextNode {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            text: self.text.clone(),
            version: self.version + 1,
            node_type: self.node_type,
        }
    }
}

impl LexicalNode for VersionedTextNode {
    fn get_type(&self) -> &'static str {
        match self.node_type {
            VersionedType::VText => "vtext",
            VersionedType::SncVText => "snc-vtext",
        }
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
}

/// `SNCVersionedTextNode`: the same, as another type.
#[derive(Clone, Debug)]
struct SncVersionedTextNode(VersionedTextNode);

impl Default for SncVersionedTextNode {
    fn default() -> Self {
        Self(VersionedTextNode {
            node_type: VersionedType::SncVText,
            ..VersionedTextNode::default()
        })
    }
}

impl LexicalNode for SncVersionedTextNode {
    fn get_type(&self) -> &'static str {
        self.0.get_type()
    }
    fn base(&self) -> &NodeBase {
        &self.0.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.0.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.0.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.0.text)
    }
}

/// A versioned node's version and mode.
fn version_of(node: &Rc<dyn LexicalNode>) -> u32 {
    if let Some(n) = node.downcast_ref::<VersionedTextNode>() {
        n.version
    } else {
        node.downcast_ref::<SncVersionedTextNode>()
            .unwrap()
            .0
            .version
    }
}

fn mode_of(node: &Rc<dyn LexicalNode>) -> u8 {
    node.text().unwrap().mode
}

/// `latest.getWritable()`: the writable version of the node.
fn get_writable(node: &NodeRef) -> Rc<dyn LexicalNode> {
    crate::lexical_utils::get_writable(node.get_key());
    node.get_latest()
}

/// The second and third updates of the `afterCloneFrom` tests, on the node
/// the first made, `versioned_text_node` (its key) and `stale` (that
/// version).
fn check_versions(editor: &Editor, versioned_text_node: NodeRef, stale: Rc<dyn LexicalNode>) {
    let (v, s) = (versioned_text_node.clone(), stale.clone());
    update(editor, move || {
        assert_eq!(version_of(&v.get_latest()), 0);
        v.set_text_content("update");
        v.set_mode("token")?;
        let latest = v.get_latest();
        assert_eq!(latest.text().unwrap().get_text(), "update");
        assert_eq!(version_of(&latest), 1);
        assert_eq!(s.text().unwrap().get_text(), "test");
        assert_eq!(version_of(&s), 0);
        Ok(())
    });
    update(editor, move || {
        let v = versioned_text_node;
        // Only the versions are read: an `Rc` of the writable version held
        // across a write would make the write clone it again.
        let latest = v.get_latest();
        assert_eq!(version_of(&stale), 0);
        assert_eq!(mode_of(&stale), 0);
        assert_eq!(v.text_data().get_mode(), "token");
        assert_eq!(version_of(&latest), 1);
        assert_eq!(mode_of(&latest), 1);
        drop(latest);
        v.set_text_content("another update");
        assert_eq!(version_of(&v.get_latest()), 2);
        assert_eq!(version_of(&get_writable(&v)), 2);
        assert_eq!(version_of(&get_writable(&v)), 2);
        assert_eq!(version_of(&v.get_latest()), 2);
        assert_eq!(mode_of(&stale), 0);
        assert_eq!(mode_of(&v.get_latest()), 1);
        assert_eq!(v.text_data().get_mode(), "token");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.afterCloneFrom()
#[test]
fn after_clone_from() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<VersionedTextNode>()],
        ..CreateEditorArgs::default()
    });
    let (node, stale) = update_with(&editor, || {
        let mut value = VersionedTextNode::default();
        value.text.text = "test".to_owned();
        let node = crate::lexical_utils::set_node_key(Box::new(value));
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&node))?;
        get_root().append(&[paragraph])?;
        assert_eq!(version_of(&node.get_latest()), 0);
        Ok((node.clone(), node.get_latest()))
    });
    check_versions(&editor, node, stale);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getType()
#[test]
fn get_type() {
    let env = NodeTestEnv::new();
    update(&env.editor, || {
        let node = create::<PlainNode>();
        assert_eq!(node.get_type(), node.get_latest().get_type());
        assert_eq!(node.get_type(), "node");
        Ok(())
    });
}

/// `new LexicalNode('__custom_key__')` makes a node outside the node map;
/// a [`NodeRef`] with a key the map doesn't have stands for it.
// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isAttached()
#[test]
fn is_attached() {
    let env = NodeTestEnv::new();
    let node = NodeRef::new("__custom_key__");
    update(&env.editor, || Ok(()));
    env.latest(|| {
        assert!(!node.is_attached());
        assert!(env.text_node.is_attached());
        assert!(env.paragraph_node.is_attached());
    });
    assert!(panics(|| env.text_node.is_attached()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected()
#[test]
fn is_selected() {
    let env = NodeTestEnv::new();
    let node = NodeRef::new("__custom_key__");
    update(&env.editor, || Ok(()));
    env.latest(|| {
        assert!(!node.is_selected(None).unwrap());
        assert!(!env.text_node.is_selected(None).unwrap());
        assert!(!env.paragraph_node.is_selected(None).unwrap());
    });
    let t = env.text_node.clone();
    update(&env.editor, move || t.select(Some(0), Some(0)).map(|_| ()));
    env.latest(|| assert!(env.text_node.is_selected(None).unwrap()));
    assert!(panics(|| env.text_node.is_selected(None)));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): selected text node
#[test]
fn is_selected_selected_text_node() {
    let env = NodeTestEnv::new();
    env.latest(|| {
        assert!(!env.paragraph_node.is_selected(None).unwrap());
        assert!(!env.text_node.is_selected(None).unwrap());
    });
    let t = env.text_node.clone();
    update(&env.editor, move || t.select(Some(0), Some(0)).map(|_| ()));
    env.latest(|| {
        assert!(env.text_node.is_selected(None).unwrap());
        assert!(!env.paragraph_node.is_selected(None).unwrap());
    });
}

/// The second paragraph, `bar`, of the `isSelected()` range tests.
fn insert_bar_paragraph(env: &NodeTestEnv) -> (NodeRef, NodeRef) {
    let (p, t) = (env.paragraph_node.clone(), env.text_node.clone());
    update_with(&env.editor, move || {
        assert!(!p.is_selected(None)?);
        assert!(!t.is_selected(None)?);
        let new_paragraph_node = create_paragraph_node();
        let new_text_node = create_text_node("bar");
        new_paragraph_node.append(std::slice::from_ref(&new_text_node))?;
        p.insert_after(&new_paragraph_node, true)?;
        assert!(!new_paragraph_node.is_selected(None)?);
        assert!(!new_text_node.is_selected(None)?);
        Ok((new_paragraph_node, new_text_node))
    })
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): selected block node range
#[test]
fn is_selected_selected_block_node_range() {
    let env = NodeTestEnv::new();
    let (new_paragraph_node, new_text_node) = insert_bar_paragraph(&env);
    let (t, nt) = (env.text_node.clone(), new_text_node.clone());
    update(&env.editor, move || {
        t.select(Some(0), Some(0))?;
        let selection = get_selection().expect("a selection");
        if !selection.is_range() {
            return Ok(());
        }
        selection.point_set(Which::Anchor, t.get_key(), 1, PointType::Text)?;
        selection.point_set(Which::Focus, nt.get_key(), 1, PointType::Text)
    });
    let (p, t) = (env.paragraph_node.clone(), env.text_node.clone());
    update(&env.editor, move || {
        let Some(selection) = get_selection().and_then(|s| s.range()) else {
            return Ok(());
        };
        assert_eq!(selection.anchor.key, t.get_key());
        assert_eq!(selection.focus.key, new_text_node.get_key());
        assert!(p.is_selected(None)?);
        assert!(t.is_selected(None)?);
        assert!(new_paragraph_node.is_selected(None)?);
        assert!(new_text_node.is_selected(None)?);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with custom range selection
#[test]
fn is_selected_with_custom_range_selection() {
    let env = NodeTestEnv::new();
    let (new_paragraph_node, new_text_node) = insert_bar_paragraph(&env);
    let (p, t) = (env.paragraph_node.clone(), env.text_node.clone());
    update(&env.editor, move || {
        let range_selection = create_range_selection();
        range_selection.point_set(Which::Anchor, t.get_key(), 1, PointType::Text)?;
        range_selection.point_set(Which::Focus, new_text_node.get_key(), 1, PointType::Text)?;
        let s = Some(&range_selection);
        assert!(p.is_selected(s)?);
        assert!(t.is_selected(s)?);
        assert!(new_paragraph_node.is_selected(s)?);
        assert!(new_text_node.is_selected(s)?);
        Ok(())
    });
}

/// The nodes of the `with inline decorator node` cases.
#[derive(Clone)]
struct InlineDecoratorCase {
    paragraph_node1: NodeRef,
    paragraph_node2: NodeRef,
    paragraph_node3: NodeRef,
    inline_decorator_node: NodeRef,
}

/// `LexicalNode.isSelected(): with inline decorator node`'s cases: the
/// selection `select` makes, forwards and then backwards, selects the
/// decorator (`is_selected`) or not.
fn inline_decorator_case(
    is_selected: bool,
    select: fn(&InlineDecoratorCase) -> Result<(), crate::LexicalError>,
) {
    let env = NodeTestEnv::new();
    let editor = env.editor.clone();
    let nodes = update_with(&editor, || {
        let inline_decorator_node = create::<InlineDecoratorNode>();
        let paragraph_node1 = create_paragraph_node();
        let paragraph_node2 = create_paragraph_node();
        paragraph_node2.append(std::slice::from_ref(&inline_decorator_node))?;
        let paragraph_node3 = create_paragraph_node();
        get_root().clear()?;
        get_root().append(&[
            paragraph_node1.clone(),
            paragraph_node2.clone(),
            paragraph_node3.clone(),
        ])?;
        Ok(InlineDecoratorCase {
            paragraph_node1,
            paragraph_node2,
            paragraph_node3,
            inline_decorator_node,
        })
    });
    let n = nodes.clone();
    update(&editor, move || select(&n));
    let names = {
        let n = &nodes;
        [
            (n.inline_decorator_node.get_key().to_owned(), "d"),
            (n.paragraph_node1.get_key().to_owned(), "p1"),
            (n.paragraph_node2.get_key().to_owned(), "p2"),
            (n.paragraph_node3.get_key().to_owned(), "p3"),
        ]
    };
    let name = move |key: &str| {
        names
            .iter()
            .find(|(k, _)| k == key)
            .map_or(key.to_owned(), |(_, v)| (*v).to_owned())
    };
    let verify = || {
        let selection = get_selection().expect("a selection");
        assert!(selection.is_range());
        let range = selection.range().unwrap();
        let dbg = [&range.anchor, &range.focus]
            .iter()
            .map(|p| format!("({}:{})", name(&p.key), p.offset))
            .collect::<Vec<_>>()
            .join(" ");
        let selected = selection
            .get_nodes()
            .unwrap()
            .iter()
            .map(|k| name(k.get_key()))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            nodes.inline_decorator_node.is_selected(None).unwrap(),
            is_selected,
            "{dbg} [{selected}]"
        );
    };
    editor.read(verify).unwrap();
    update(&editor, || {
        let selection = get_selection();
        if let Some(range) = selection.as_ref().and_then(SelectionRef::range) {
            let backwards = create_range_selection();
            backwards.point_set(
                Which::Anchor,
                &range.focus.key,
                range.focus.offset,
                range.focus.point_type,
            )?;
            backwards.point_set(
                Which::Focus,
                &range.anchor.key,
                range.anchor.offset,
                range.anchor.point_type,
            )?;
            set_selection(Some(backwards));
        }
        assert!(selection.is_some_and(|s| s.is_range()));
        Ok(())
    });
    editor.read(verify).unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected whole editor
#[test]
fn is_selected_whole_editor() {
    inline_decorator_case(true, |_| get_root().select(Some(0), None).map(|_| ()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected containing paragraph
#[test]
fn is_selected_containing_paragraph() {
    inline_decorator_case(true, |n| {
        n.paragraph_node2.select(Some(0), None).map(|_| ())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected before and containing
#[test]
fn is_selected_before_and_containing() {
    inline_decorator_case(true, |n| {
        n.paragraph_node2.select(Some(0), None)?.point_set(
            Which::Anchor,
            n.paragraph_node1.get_key(),
            0,
            PointType::Element,
        )
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected containing and after
#[test]
fn is_selected_containing_and_after() {
    inline_decorator_case(true, |n| {
        n.paragraph_node2.select(Some(0), None)?.point_set(
            Which::Focus,
            n.paragraph_node3.get_key(),
            0,
            PointType::Element,
        )
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected before and after
#[test]
fn is_selected_before_and_after() {
    inline_decorator_case(true, |n| {
        n.paragraph_node1.select(Some(0), None)?.point_set(
            Which::Focus,
            n.paragraph_node3.get_key(),
            0,
            PointType::Element,
        )
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected collapsed before
#[test]
fn is_not_selected_collapsed_before() {
    inline_decorator_case(false, |n| {
        n.paragraph_node2.select(Some(0), Some(0)).map(|_| ())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected in another element
#[test]
fn is_not_selected_in_another_element() {
    inline_decorator_case(false, |n| {
        n.paragraph_node1.select(Some(0), None).map(|_| ())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected before
#[test]
fn is_not_selected_before() {
    inline_decorator_case(false, |n| {
        n.paragraph_node1.select(Some(0), None)?.point_set(
            Which::Focus,
            n.paragraph_node2.get_key(),
            0,
            PointType::Element,
        )
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected collapsed after
#[test]
fn is_not_selected_collapsed_after() {
    inline_decorator_case(false, |n| n.paragraph_node2.select_end().map(|_| ()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected after
#[test]
fn is_not_selected_after() {
    inline_decorator_case(false, |n| {
        n.paragraph_node3.select(Some(0), None)?.point_set(
            Which::Anchor,
            n.paragraph_node2.get_key(),
            n.paragraph_node2.get_children_size(),
            PointType::Element,
        )
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getKey()
#[test]
fn get_key() {
    let env = NodeTestEnv::new();
    let key = env.latest(|| env.text_node.get_latest().get_key().to_owned());
    assert_eq!(env.text_node.get_key(), key);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getParent()
#[test]
fn get_parent() {
    let env = NodeTestEnv::new();
    update(&env.editor, || {
        assert_eq!(create::<PlainNode>().get_parent(), None);
        Ok(())
    });
    env.latest(|| {
        assert_eq!(env.text_node.get_parent(), Some(env.paragraph_node.clone()));
        assert_eq!(env.paragraph_node.get_parent(), Some(get_root()));
    });
    assert!(panics(|| env.text_node.get_parent()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getParentOrThrow()
#[test]
fn get_parent_or_throw() {
    let env = NodeTestEnv::new();
    update(&env.editor, || {
        assert!(create::<PlainNode>().get_parent_or_throw().is_err());
        Ok(())
    });
    env.latest(|| {
        assert_eq!(env.text_node.get_parent(), Some(env.paragraph_node.clone()));
        assert_eq!(env.paragraph_node.get_parent(), Some(get_root()));
    });
    assert!(panics(|| env.text_node.get_parent_or_throw()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getTopLevelElement()
#[test]
fn get_top_level_element() {
    let env = NodeTestEnv::new();
    update(&env.editor, || {
        assert_eq!(create::<PlainNode>().get_top_level_element()?, None);
        Ok(())
    });
    env.latest(|| {
        assert_eq!(
            env.text_node.get_top_level_element().unwrap(),
            Some(env.paragraph_node.clone())
        );
        assert_eq!(
            env.paragraph_node.get_top_level_element().unwrap(),
            Some(env.paragraph_node.clone())
        );
    });
    assert!(panics(|| env.text_node.get_top_level_element()));
    update(&env.editor, || {
        let node = create::<BlockDecoratorNode>();
        assert_eq!(node.get_top_level_element()?, None);
        get_root().append(std::slice::from_ref(&node))?;
        assert_eq!(node.get_top_level_element()?, Some(node));
        Ok(())
    });
    env.latest(|| {
        let (mut element_nodes, mut decorator_nodes) = (0, 0);
        for child in get_root().get_children() {
            assert_eq!(child.get_top_level_element().unwrap(), Some(child.clone()));
            if child.is_element() {
                element_nodes += 1;
            } else if child.is_decorator() {
                decorator_nodes += 1;
            } else {
                panic!("Expecting all children to be ElementNode or DecoratorNode");
            }
        }
        assert_eq!(decorator_nodes, 1);
        assert_eq!(element_nodes, 1);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getTopLevelElementOrThrow()
#[test]
fn get_top_level_element_or_throw() {
    let env = NodeTestEnv::new();
    update(&env.editor, || {
        assert!(
            create::<PlainNode>()
                .get_top_level_element_or_throw()
                .is_err()
        );
        Ok(())
    });
    env.latest(|| {
        assert_eq!(
            env.text_node.get_top_level_element_or_throw().unwrap(),
            env.paragraph_node
        );
        assert_eq!(
            env.paragraph_node.get_top_level_element_or_throw().unwrap(),
            env.paragraph_node
        );
    });
    assert!(panics(|| env.text_node.get_top_level_element_or_throw()));
    update(&env.editor, || {
        let node = create::<InlineDecoratorNode>();
        assert!(node.get_top_level_element_or_throw().is_err());
        get_root().append(std::slice::from_ref(&node))?;
        assert_eq!(node.get_top_level_element_or_throw()?, node);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getParents()
#[test]
fn get_parents() {
    let env = NodeTestEnv::new();
    update(&env.editor, || {
        assert!(create::<PlainNode>().get_parents().is_empty());
        Ok(())
    });
    assert_eq!(shape(&env.editor), "<paragraph>[foo]</paragraph>");
    env.latest(|| {
        assert_eq!(
            env.text_node.get_parents(),
            [env.paragraph_node.clone(), get_root()]
        );
        assert_eq!(env.paragraph_node.get_parents(), [get_root()]);
    });
    assert!(panics(|| env.text_node.get_parents()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getPreviousSibling()
#[test]
fn get_previous_sibling() {
    let env = NodeTestEnv::new();
    let bar = env.append_unmergeable(&["bar"]).remove(0);
    assert_eq!(shape(&env.editor), "<paragraph>[foo][bar]</paragraph>");
    env.latest(|| {
        let previous = bar.get_previous_sibling().unwrap();
        assert_eq!(previous, env.text_node);
        assert_eq!(previous.get_next_sibling(), Some(bar.clone()));
        assert_eq!(env.text_node.get_previous_sibling(), None);
    });
    assert!(panics(|| env.text_node.get_previous_sibling()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getPreviousSiblings()
#[test]
fn get_previous_siblings() {
    let env = NodeTestEnv::new();
    let nodes = env.append_unmergeable(&["bar", "baz"]);
    let (bar, baz) = (&nodes[0], &nodes[1]);
    assert_eq!(shape(&env.editor), "<paragraph>[foo][bar][baz]</paragraph>");
    env.latest(|| {
        assert_eq!(
            baz.get_previous_siblings(),
            [env.text_node.clone(), bar.clone()]
        );
        assert_eq!(
            bar.get_previous_siblings(),
            std::slice::from_ref(&env.text_node)
        );
        assert!(env.text_node.get_previous_siblings().is_empty());
    });
    assert!(panics(|| env.text_node.get_previous_siblings()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getNextSibling()
#[test]
fn get_next_sibling() {
    let env = NodeTestEnv::new();
    let bar = env.append_unmergeable(&["bar"]).remove(0);
    assert_eq!(shape(&env.editor), "<paragraph>[foo][bar]</paragraph>");
    env.latest(|| {
        assert_eq!(bar.get_next_sibling(), None);
        assert_eq!(env.text_node.get_next_sibling(), Some(bar.clone()));
    });
    assert!(panics(|| env.text_node.get_next_sibling()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getNextSiblings()
#[test]
fn get_next_siblings() {
    let env = NodeTestEnv::new();
    let nodes = env.append_unmergeable(&["bar", "baz"]);
    let (bar, baz) = (&nodes[0], &nodes[1]);
    assert_eq!(shape(&env.editor), "<paragraph>[foo][bar][baz]</paragraph>");
    env.latest(|| {
        assert!(baz.get_next_siblings().is_empty());
        assert_eq!(bar.get_next_siblings(), std::slice::from_ref(baz));
        assert_eq!(
            env.text_node.get_next_siblings(),
            [bar.clone(), baz.clone()]
        );
    });
    assert!(panics(|| env.text_node.get_next_siblings()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isBefore()
#[test]
fn is_before() {
    let env = NodeTestEnv::new();
    let nodes = env.append_unmergeable(&["bar", "baz"]);
    let (bar, baz) = (&nodes[0], &nodes[1]);
    assert_eq!(shape(&env.editor), "<paragraph>[foo][bar][baz]</paragraph>");
    env.latest(|| {
        let t = &env.text_node;
        assert!(!t.is_before(t));
        assert!(t.is_before(bar));
        assert!(t.is_before(baz));
        assert!(bar.is_before(baz));
        assert!(!baz.is_before(bar));
        assert!(!baz.is_before(t));
    });
    assert!(panics(|| env.text_node.is_before(bar)));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isParentOf()
#[test]
fn is_parent_of() {
    let env = NodeTestEnv::new();
    env.latest(|| {
        let root_node = get_root();
        let (p, t) = (&env.paragraph_node, &env.text_node);
        assert!(root_node.is_parent_of(t));
        assert!(root_node.is_parent_of(p));
        assert!(p.is_parent_of(t));
        assert!(!p.is_parent_of(&root_node));
        assert!(!t.is_parent_of(p));
        assert!(!t.is_parent_of(&root_node));
    });
    assert!(panics(|| env.paragraph_node.is_parent_of(&env.text_node)));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getNodesBetween()
#[test]
fn get_nodes_between() {
    let env = NodeTestEnv::new();
    let p = env.paragraph_node.clone();
    let (bar, baz, new_paragraph_node, qux) = update_with(&env.editor, move || {
        let bar = create_text_node("bar");
        bar.toggle_unmergeable();
        let baz = create_text_node("baz");
        baz.toggle_unmergeable();
        let new_paragraph_node = create_paragraph_node();
        let qux = create_text_node("qux");
        qux.toggle_unmergeable();
        get_root().append(std::slice::from_ref(&new_paragraph_node))?;
        p.append(&[bar.clone(), baz.clone()])?;
        new_paragraph_node.append(std::slice::from_ref(&qux))?;
        Ok((bar, baz, new_paragraph_node, qux))
    });
    assert_eq!(
        shape(&env.editor),
        "<paragraph>[foo][bar][baz]</paragraph><paragraph>[qux]</paragraph>"
    );
    env.latest(|| {
        let t = &env.text_node;
        assert_eq!(t.get_nodes_between(t), std::slice::from_ref(t));
        assert_eq!(t.get_nodes_between(&bar), [t.clone(), bar.clone()]);
        assert_eq!(
            t.get_nodes_between(&baz),
            [t.clone(), bar.clone(), baz.clone()]
        );
        assert_eq!(
            keys(&t.get_nodes_between(&qux)),
            keys(&[
                t.clone(),
                bar.clone(),
                baz.clone(),
                env.paragraph_node.clone(),
                new_paragraph_node.clone(),
                qux.clone(),
            ])
        );
    });
    assert!(panics(|| env.text_node.get_nodes_between(&baz)));
}

/// A text node in `mode`, appended to the paragraph.
fn append_with_mode(env: &NodeTestEnv, text: &'static str, mode: &'static str) -> NodeRef {
    let p = env.paragraph_node.clone();
    update_with(&env.editor, move || {
        let node = create_text_node(text);
        node.set_mode(mode)?;
        p.append(std::slice::from_ref(&node))?;
        Ok(node)
    })
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isToken()
#[test]
fn is_token() {
    let env = NodeTestEnv::new();
    let token_text_node = append_with_mode(&env, "token", "token");
    assert_eq!(shape(&env.editor), "<paragraph>[foo][token]</paragraph>");
    env.latest(|| {
        assert!(!env.text_node.is_token());
        assert!(token_text_node.is_token());
    });
    assert!(panics(|| env.text_node.is_token()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSegmented()
#[test]
fn is_segmented() {
    let env = NodeTestEnv::new();
    let segmented_text_node = append_with_mode(&env, "segmented", "segmented");
    assert_eq!(
        shape(&env.editor),
        "<paragraph>[foo][segmented]</paragraph>"
    );
    env.latest(|| {
        assert!(!env.text_node.is_segmented());
        assert!(segmented_text_node.is_segmented());
    });
    assert!(panics(|| env.text_node.is_segmented()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isDirectionless()
#[test]
fn is_directionless() {
    let env = NodeTestEnv::new();
    let p = env.paragraph_node.clone();
    let directionless_text_node = update_with(&env.editor, move || {
        let node = create_text_node("directionless");
        node.toggle_directionless();
        node.toggle_unmergeable();
        p.append(std::slice::from_ref(&node))?;
        Ok(node)
    });
    assert_eq!(
        shape(&env.editor),
        "<paragraph>[foo][directionless]</paragraph>"
    );
    env.latest(|| {
        assert!(!env.text_node.is_directionless());
        assert!(directionless_text_node.is_directionless());
    });
    assert!(panics(|| directionless_text_node.is_directionless()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getLatest()
#[test]
fn get_latest() {
    let env = NodeTestEnv::new();
    env.latest(|| {
        assert_eq!(
            env.text_node.get_latest().get_key(),
            env.text_node.get_key()
        )
    });
    assert!(panics(|| env.text_node.get_latest()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getLatest(): garbage collected node
#[test]
fn get_latest_garbage_collected_node() {
    let env = NodeTestEnv::new();
    let nodes = update_with(&env.editor, || {
        let node = create::<PlainNode>();
        node.get_latest();
        let text = create_text_node("");
        text.get_latest();
        let block = create_test_element_node();
        block.get_latest();
        Ok([node, text, block])
    });
    update(&env.editor, move || {
        for node in &nodes {
            assert!(panics(|| node.get_latest()));
        }
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getTextContent()
#[test]
fn get_text_content() {
    let env = NodeTestEnv::new();
    update(&env.editor, || {
        assert_eq!(create::<PlainNode>().get_text_content(), "");
        Ok(())
    });
    env.latest(|| assert_eq!(env.text_node.get_text_content(), "foo"));
    assert!(panics(|| env.text_node.get_text_content()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getTextContentSize()
#[test]
fn get_text_content_size() {
    let env = NodeTestEnv::new();
    env.latest(|| assert_eq!(env.text_node.get_text_content_size(), "foo".len()));
    assert!(panics(|| env.text_node.get_text_content_size()));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.createDOM()
#[cfg(feature = "html")]
#[test]
fn create_dom() {
    let env = NodeTestEnv::new();
    let e = env.editor.clone();
    update(&env.editor, move || {
        let node = create::<PlainNode>();
        assert!(
            node.get_latest()
                .create_dom(&node, &crate::EditorConfig::default(), &e)
                .is_err()
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.remove()
#[test]
fn remove() {
    let env = NodeTestEnv::new();
    env.latest(|| assert!(panics(|| env.text_node.remove(false))));
    assert_eq!(shape(&env.editor), "<paragraph>[foo]</paragraph>");
    let t = env.text_node.clone();
    update(&env.editor, move || {
        let node = create::<PlainNode>();
        node.remove(false)?;
        assert_eq!(node.get_parent(), None);
        t.remove(false)?;
        assert_eq!(t.get_parent(), None);
        Ok(())
    });
    assert_eq!(shape(&env.editor), "<paragraph></paragraph>");
    assert!(panics(|| env.text_node.remove(false)));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): from another parent
#[test]
fn replace_from_another_parent() {
    let env = NodeTestEnv::new();
    assert_eq!(shape(&env.editor), "<paragraph>[foo]</paragraph>");
    let bar = update_with(&env.editor, || {
        let bar_paragraph_node = create_paragraph_node();
        let bar = create_text_node("bar");
        bar_paragraph_node.append(std::slice::from_ref(&bar))?;
        get_root().append(&[bar_paragraph_node])?;
        Ok(bar)
    });
    assert_eq!(
        shape(&env.editor),
        "<paragraph>[foo]</paragraph><paragraph>[bar]</paragraph>"
    );
    let t = env.text_node.clone();
    update(&env.editor, move || t.replace(&bar, false).map(|_| ()));
    assert_eq!(
        shape(&env.editor),
        "<paragraph>[bar]</paragraph><paragraph></paragraph>"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): with a sibling in the same parent
#[test]
fn replace_with_a_sibling_in_the_same_parent() {
    let env = NodeTestEnv::new();
    let t = env.text_node.clone();
    update(&env.editor, move || {
        let paragraph = t.get_parent_or_throw()?;
        let bar = create_text_node("bar");
        paragraph.append(std::slice::from_ref(&bar))?;
        // Replacing a node with one of its own siblings must not leave the
        // parent's __size counting the sibling twice.
        t.replace(&bar, false)?;
        assert_eq!(paragraph.get_children_size(), 1);
        assert_eq!(paragraph.get_children().len(), 1);
        assert_eq!(
            paragraph.get_first_child().unwrap().get_text_content(),
            "bar"
        );
        Ok(())
    });
    assert_eq!(shape(&env.editor), "<paragraph>[bar]</paragraph>");
}

/// `textNode.<insert>(bar)`, with `bar` made by `make`, and the shape after.
fn insert_text(
    insert: fn(&NodeRef, &NodeRef) -> Result<(), crate::LexicalError>,
    make: fn() -> Result<NodeRef, crate::LexicalError>,
    expected: &str,
) {
    let env = NodeTestEnv::new();
    assert_eq!(shape(&env.editor), "<paragraph>[foo]</paragraph>");
    let t = env.text_node.clone();
    update(&env.editor, move || insert(&t, &make()?));
    assert_eq!(shape(&env.editor), expected);
}

fn replace_with(t: &NodeRef, n: &NodeRef) -> Result<(), crate::LexicalError> {
    t.replace(n, false).map(|_| ())
}
fn insert_after_with(t: &NodeRef, n: &NodeRef) -> Result<(), crate::LexicalError> {
    t.insert_after(n, true).map(|_| ())
}
fn insert_before_with(t: &NodeRef, n: &NodeRef) -> Result<(), crate::LexicalError> {
    t.insert_before(n, true).map(|_| ())
}
fn bar() -> Result<NodeRef, crate::LexicalError> {
    Ok(create_text_node("bar"))
}
fn bar_token() -> Result<NodeRef, crate::LexicalError> {
    let n = create_text_node("bar");
    n.set_mode("token")?;
    Ok(n)
}
fn bar_segmented() -> Result<NodeRef, crate::LexicalError> {
    let n = create_text_node("bar");
    n.set_mode("segmented")?;
    Ok(n)
}
fn bar_directionless() -> Result<NodeRef, crate::LexicalError> {
    let n = create_text_node("bar");
    n.toggle_directionless();
    Ok(n)
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): text
#[test]
fn replace_text() {
    insert_text(replace_with, bar, "<paragraph>[bar]</paragraph>");
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): token
#[test]
fn replace_token() {
    insert_text(replace_with, bar_token, "<paragraph>[bar]</paragraph>");
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): segmented
#[test]
fn replace_segmented() {
    insert_text(replace_with, bar_segmented, "<paragraph>[bar]</paragraph>");
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): directionless
#[test]
fn replace_directionless() {
    insert_text(
        replace_with,
        bar_directionless,
        "<paragraph>[bar]</paragraph>",
    );
}

/// Upstream spies `canBeEmpty` to return `false`, which
/// `TestInlineElementNode`'s does.
// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace() within canBeEmpty: false
#[test]
fn replace_within_can_be_empty_false() {
    let env = NodeTestEnv::new();
    update(&env.editor, || {
        let text_node = create_text_node("Hello");
        let inline = create_test_inline_element_node();
        inline.append(std::slice::from_ref(&text_node))?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[inline])?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        text_node.replace(&create_text_node("world"), false)?;
        Ok(())
    });
    assert_eq!(
        shape(&env.editor),
        "<paragraph><test_inline_block>[world]</test_inline_block></paragraph>"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter(): text
#[test]
fn insert_after_text() {
    insert_text(insert_after_with, bar, "<paragraph>[foobar]</paragraph>");
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter(): token
#[test]
fn insert_after_token() {
    insert_text(
        insert_after_with,
        bar_token,
        "<paragraph>[foo][bar]</paragraph>",
    );
}

/// Upstream inserts a token here too.
// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter(): segmented
#[test]
fn insert_after_segmented() {
    insert_text(
        insert_after_with,
        bar_token,
        "<paragraph>[foo][bar]</paragraph>",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter(): directionless
#[test]
fn insert_after_directionless() {
    insert_text(
        insert_after_with,
        bar_directionless,
        "<paragraph>[foobar]</paragraph>",
    );
}

/// Three paragraphs `A`, `B` and `C`, unmergeable or not.
fn three_blocks(env: &NodeTestEnv, unmergeable: bool) -> Vec<NodeRef> {
    update_with(&env.editor, move || {
        let root = get_root();
        root.clear()?;
        let mut nodes = Vec::new();
        let mut blocks = Vec::new();
        for text in ["A", "B", "C"] {
            let block = create_paragraph_node();
            let node = create_text_node(text);
            if unmergeable {
                node.toggle_unmergeable();
            }
            block.append(std::slice::from_ref(&node))?;
            blocks.push(block);
            nodes.push(node);
        }
        root.append(&blocks)?;
        blocks.extend(nodes);
        Ok(blocks)
    })
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter() move blocks around
#[test]
fn insert_after_move_blocks_around() {
    let env = NodeTestEnv::new();
    let nodes = three_blocks(&env, false);
    assert_eq!(
        shape(&env.editor),
        "<paragraph>[A]</paragraph><paragraph>[B]</paragraph><paragraph>[C]</paragraph>"
    );
    let (text1, block2) = (nodes[3].clone(), nodes[1].clone());
    update(&env.editor, move || {
        text1.insert_after(&block2, true).map(|_| ())
    });
    assert_eq!(
        shape(&env.editor),
        "<paragraph>[A]<paragraph>[B]</paragraph></paragraph><paragraph>[C]</paragraph>"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter() move blocks around #2
#[test]
fn insert_after_move_blocks_around_2() {
    let env = NodeTestEnv::new();
    let nodes = three_blocks(&env, true);
    assert_eq!(
        shape(&env.editor),
        "<paragraph>[A]</paragraph><paragraph>[B]</paragraph><paragraph>[C]</paragraph>"
    );
    let (text1, text2, text3) = (nodes[3].clone(), nodes[4].clone(), nodes[5].clone());
    update(&env.editor, move || {
        text3.insert_after(&text1, true)?;
        text3.insert_after(&text2, true)?;
        Ok(())
    });
    assert_eq!(
        shape(&env.editor),
        "<paragraph></paragraph><paragraph></paragraph><paragraph>[C][B][A]</paragraph>"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): from another parent
#[test]
fn insert_before_from_another_parent() {
    let env = NodeTestEnv::new();
    assert_eq!(shape(&env.editor), "<paragraph>[foo]</paragraph>");
    update(&env.editor, || {
        let bar_paragraph_node = create_paragraph_node();
        bar_paragraph_node.append(&[create_text_node("bar")])?;
        get_root().append(&[bar_paragraph_node])
    });
    assert_eq!(
        shape(&env.editor),
        "<paragraph>[foo]</paragraph><paragraph>[bar]</paragraph>"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): text
#[test]
fn insert_before_text() {
    insert_text(insert_before_with, bar, "<paragraph>[barfoo]</paragraph>");
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): token
#[test]
fn insert_before_token() {
    insert_text(
        insert_before_with,
        bar_token,
        "<paragraph>[bar][foo]</paragraph>",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): segmented
#[test]
fn insert_before_segmented() {
    insert_text(
        insert_before_with,
        bar_segmented,
        "<paragraph>[bar][foo]</paragraph>",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): directionless
#[test]
fn insert_before_directionless() {
    insert_text(
        insert_before_with,
        bar_directionless,
        "<paragraph>[barfoo]</paragraph>",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.selectNext()
#[test]
fn select_next() {
    let env = NodeTestEnv::new();
    let t = env.text_node.clone();
    update(&env.editor, move || {
        let bar = create_text_node("bar");
        t.insert_after(&bar, true)?;
        assert!(!bar.is_selected(None)?);
        t.select_next(None, None)?;
        assert!(bar.is_selected(None)?);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.selectNext(): no next sibling
#[test]
fn select_next_no_next_sibling() {
    let env = NodeTestEnv::new();
    let (t, p) = (env.text_node.clone(), env.paragraph_node.clone());
    update(&env.editor, move || {
        let selection = t.select_next(None, None)?;
        let anchor = selection.point(Which::Anchor).unwrap();
        assert_eq!(anchor.get_node()?, p);
        assert_eq!(anchor.offset, 1);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.selectNext(): non-text node
#[test]
fn select_next_non_text_node() {
    let env = NodeTestEnv::new();
    let t = env.text_node.clone();
    update(&env.editor, move || {
        let bar_node = create::<TestNode>();
        t.insert_after(&bar_node, true)?;
        let selection = t.select_next(None, None)?;
        let anchor = selection.point(Which::Anchor).unwrap();
        assert_eq!(Some(anchor.get_node()?), t.get_parent());
        assert_eq!(anchor.offset, 1);
        Ok(())
    });
}

/// `CustomTextNode` (`custom-text`): a `TextNode` subclass with no
/// boilerplate.
#[derive(Clone, Debug, Default)]
struct CustomTextNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for CustomTextNode {
    fn get_type(&self) -> &'static str {
        "custom-text"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > importJSON() with no boilerplate
#[test]
fn config_import_json_with_no_boilerplate() {
    let class = NodeClass::of::<CustomTextNode>();
    let import_json = class.import_json;
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![class],
        ..CreateEditorArgs::default()
    });
    update(&editor, move || {
        let json = crate::json::parse(
            r#"{"detail":0,"format":0,"mode":"normal","style":"","text":"codegen!","type":"custom-text","version":1}"#,
        )
        .unwrap();
        let node = import_json(json.as_object().unwrap())?;
        assert!(node.get_latest().is::<CustomTextNode>());
        assert_eq!(node.get_type(), "custom-text");
        assert_eq!(node.get_text_content(), "codegen!");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > clone() with no boilerplate
#[test]
fn config_clone_with_no_boilerplate() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<SncVersionedTextNode>()],
        ..CreateEditorArgs::default()
    });
    let (node, stale) = update_with(&editor, || {
        let node = create::<SncVersionedTextNode>();
        node.set_text_content("test");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&node))?;
        get_root().append(&[paragraph])?;
        assert_eq!(version_of(&node.get_latest()), 0);
        Ok((node.clone(), node.get_latest()))
    });
    check_versions(&editor, node, stale);
}

/// `ConfigTagNode`, `InnerTagNode` and `ConfigVersionNode`: elements with a
/// field their `afterCloneFrom` copies (or counts).
#[derive(Clone, Debug)]
struct TagNode {
    base: NodeBase,
    element: ElementData,
    tag: String,
    node_type: &'static str,
}

impl TagNode {
    fn new(node_type: &'static str) -> Self {
        Self {
            base: NodeBase::default(),
            element: ElementData::default(),
            tag: "default".to_owned(),
            node_type,
        }
    }
}

impl Default for TagNode {
    fn default() -> Self {
        Self::new("config-tag")
    }
}

impl LexicalNode for TagNode {
    fn get_type(&self) -> &'static str {
        self.node_type
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

/// `setTag(tag)`.
fn set_tag(node: &NodeRef, tag: &str) {
    crate::lexical_utils::with_writable(node.get_key(), |n| {
        n.downcast_mut::<TagNode>().unwrap().tag = tag.to_owned()
    });
}

fn tag_of(node: &dyn LexicalNode) -> String {
    node.downcast_ref::<TagNode>().unwrap().tag.clone()
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > direct clone() of an auto-synthesized node preserves properties
#[test]
fn config_direct_clone_of_an_auto_synthesized_node_preserves_properties() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<TagNode>()],
        ..CreateEditorArgs::default()
    });
    update(&editor, || {
        let source = create::<TagNode>();
        set_tag(&source, "custom");
        get_root().append(std::slice::from_ref(&source))?;
        assert_eq!(tag_of(&*source.get_latest()), "custom");
        let direct_clone = source.get_latest().clone_node();
        assert_eq!(tag_of(&*direct_clone), "custom");
        let wrapper_clone = crate::clone_with_properties(&source);
        assert_eq!(tag_of(&*wrapper_clone), "custom");
        Ok(())
    });
}

/// `ConfigVersionNode`.
#[derive(Debug, Default)]
struct ConfigVersionNode {
    base: NodeBase,
    element: ElementData,
    version: u32,
}

impl Clone for ConfigVersionNode {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            element: self.element.clone(),
            version: self.version + 1,
        }
    }
}

impl LexicalNode for ConfigVersionNode {
    fn get_type(&self) -> &'static str {
        "config-version"
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

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > afterCloneFrom runs exactly once for auto-synthesized clone
#[test]
fn config_after_clone_from_runs_exactly_once_for_auto_synthesized_clone() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<ConfigVersionNode>()],
        ..CreateEditorArgs::default()
    });
    update(&editor, || {
        let source = create::<ConfigVersionNode>();
        get_root().append(std::slice::from_ref(&source))?;
        let version = |n: &dyn LexicalNode| n.downcast_ref::<ConfigVersionNode>().unwrap().version;
        assert_eq!(version(&*source.get_latest()), 0);
        let cloned = crate::clone_with_properties(&source);
        assert_eq!(version(&*cloned), 1);
        Ok(())
    });
}

/// `OuterNode`: its `afterCloneFrom` directly clones `__templateSource`.
#[derive(Debug, Default)]
struct OuterNode {
    base: NodeBase,
    element: ElementData,
    reentrant_clone: Option<TagNode>,
    template_source: Option<TagNode>,
}

impl Clone for OuterNode {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            element: self.element.clone(),
            // A reentrant direct clone of a different node, made while this
            // one is cloned.
            reentrant_clone: self.template_source.clone(),
            template_source: self.template_source.clone(),
        }
    }
}

impl LexicalNode for OuterNode {
    fn get_type(&self) -> &'static str {
        "outer-reentrant"
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

/// `InnerTagNode`.
#[derive(Clone, Debug)]
struct InnerTagNode(TagNode);

impl Default for InnerTagNode {
    fn default() -> Self {
        Self(TagNode::new("inner-tag"))
    }
}

impl LexicalNode for InnerTagNode {
    fn get_type(&self) -> &'static str {
        self.0.get_type()
    }
    fn base(&self) -> &NodeBase {
        &self.0.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.0.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.0.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.0.element)
    }
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > reentrant direct clone during another clone preserves properties
#[test]
fn config_reentrant_direct_clone_during_another_clone_preserves_properties() {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<InnerTagNode>(),
            NodeClass::of::<OuterNode>(),
        ],
        ..CreateEditorArgs::default()
    });
    update(&editor, || {
        let inner = create::<InnerTagNode>();
        crate::lexical_utils::with_writable(inner.get_key(), |n| {
            n.downcast_mut::<InnerTagNode>().unwrap().0.tag = "custom".to_owned()
        });
        let outer = create::<OuterNode>();
        let source = inner
            .get_latest()
            .downcast_ref::<InnerTagNode>()
            .unwrap()
            .0
            .clone();
        crate::lexical_utils::with_writable(outer.get_key(), |n| {
            n.downcast_mut::<OuterNode>().unwrap().template_source = Some(source)
        });
        get_root().append(&[inner, outer.clone()])?;
        let cloned_outer = crate::clone_with_properties(&outer);
        let reentrant = cloned_outer
            .downcast_ref::<OuterNode>()
            .unwrap()
            .reentrant_clone
            .clone();
        assert!(reentrant.is_some());
        assert_eq!(reentrant.unwrap().tag, "custom");
        Ok(())
    });
}

/// The editor of the suites after `LexicalNode tests`: `initializeUnitTest`
/// with its default config.
fn unit_test_editor() -> Editor {
    initialize_unit_test(CreateEditorArgs {
        namespace: "test".to_owned(),
        ..CreateEditorArgs::default()
    })
}

/// The committed range selection's points are element points at `anchor`
/// and `focus` (key, offset).
fn expect_selection(editor: &Editor, anchor: (&str, usize), focus: (&str, usize)) {
    editor
        .read(|| {
            let sel = get_selection()
                .and_then(|s| s.range())
                .expect("a range selection");
            assert_eq!(
                (
                    sel.anchor.key.as_str(),
                    sel.anchor.offset,
                    sel.anchor.point_type
                ),
                (anchor.0, anchor.1, PointType::Element)
            );
            assert_eq!(
                (
                    sel.focus.key.as_str(),
                    sel.focus.offset,
                    sel.focus.point_type
                ),
                (focus.0, focus.1, PointType::Element)
            );
        })
        .unwrap();
}

/// The `#6031` suites' movers: `target.<method>(mover)`.
type Mover = fn(&NodeRef, &NodeRef) -> Result<(), crate::LexicalError>;

fn act_insert_before(target: &NodeRef, mover: &NodeRef) -> Result<(), crate::LexicalError> {
    target.insert_before(mover, true).map(|_| ())
}
fn act_insert_after(target: &NodeRef, mover: &NodeRef) -> Result<(), crate::LexicalError> {
    target.insert_after(mover, true).map(|_| ())
}
fn act_replace(target: &NodeRef, mover: &NodeRef) -> Result<(), crate::LexicalError> {
    target.replace(mover, false).map(|_| ())
}

/// `Refs` of the two-container setup: `source` holds `[mover, a2, a3]`,
/// `target` holds `[inTarget]`.
#[derive(Clone)]
struct Refs {
    source: NodeRef,
    target: NodeRef,
    mover: NodeRef,
    in_target: NodeRef,
}

/// `$setupTwoContainers`.
fn setup_two_containers() -> Result<Refs, crate::LexicalError> {
    let root = get_root();
    root.clear()?;
    let source = create_test_element_node();
    let target = create_test_element_node();
    let mover = create_paragraph_node();
    let in_target = create_paragraph_node();
    source.append(&[
        mover.clone(),
        create_paragraph_node(),
        create_paragraph_node(),
    ])?;
    target.append(std::slice::from_ref(&in_target))?;
    root.append(&[source.clone(), target.clone()])?;
    Ok(Refs {
        source,
        target,
        mover,
        in_target,
    })
}

/// A third container holding two paragraphs, appended to the root.
fn append_other() -> Result<NodeRef, crate::LexicalError> {
    let other = create_test_element_node();
    other.append(&[create_paragraph_node(), create_paragraph_node()])?;
    get_root().append(std::slice::from_ref(&other))?;
    Ok(other)
}

/// A cross-parent move with `source.select(anchor, focus)`.
fn cross_parent_move(act: Mover, anchor: usize, focus: usize) -> (Editor, String) {
    let editor = unit_test_editor();
    let source_key = update_with(&editor, move || {
        let refs = setup_two_containers()?;
        refs.source.select(Some(anchor), Some(focus))?;
        act(&refs.in_target, &refs.mover)?;
        Ok(refs.source.get_key().to_owned())
    });
    (editor, source_key)
}

fn shifts_offset_past_removed_index(act: Mover) {
    let (editor, source) = cross_parent_move(act, 3, 3);
    expect_selection(&editor, (&source, 2), (&source, 2));
}

fn does_not_shift_offset_at_removed_index(act: Mover) {
    let (editor, source) = cross_parent_move(act, 0, 0);
    expect_selection(&editor, (&source, 0), (&source, 0));
}

fn shifts_only_the_past_boundary_side(act: Mover) {
    let (editor, source) = cross_parent_move(act, 0, 3);
    expect_selection(&editor, (&source, 0), (&source, 2));
}

fn leaves_an_unrelated_third_parent_untouched(act: Mover) {
    let editor = unit_test_editor();
    let (source_key, other_key) = update_with(&editor, move || {
        let root = get_root();
        root.clear()?;
        let source = create_test_element_node();
        let target = create_test_element_node();
        let other = create_test_element_node();
        let mover = create_paragraph_node();
        let in_target = create_paragraph_node();
        source.append(&[
            mover.clone(),
            create_paragraph_node(),
            create_paragraph_node(),
        ])?;
        target.append(std::slice::from_ref(&in_target))?;
        other.append(&[create_paragraph_node()])?;
        root.append(&[source.clone(), target, other.clone()])?;
        // Cursor sits in `other`, which the move does not touch.
        other.select(Some(1), Some(1))?;
        act(&in_target, &mover)?;
        Ok((source.get_key().to_owned(), other.get_key().to_owned()))
    });
    expect_selection(&editor, (&other_key, 1), (&other_key, 1));
    assert_ne!(source_key, other_key);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertBefore shifts offset > removed index in source parent
#[test]
fn cross_parent_insert_before_shifts_offset_past_removed_index_in_source_parent() {
    shifts_offset_past_removed_index(act_insert_before);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertBefore does not shift offset at removed index (boundary)
#[test]
fn cross_parent_insert_before_does_not_shift_offset_at_removed_index() {
    does_not_shift_offset_at_removed_index(act_insert_before);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertBefore shifts only the past-boundary side of a non-collapsed selection
#[test]
fn cross_parent_insert_before_shifts_only_the_past_boundary_side() {
    shifts_only_the_past_boundary_side(act_insert_before);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertBefore leaves selection in an unrelated third parent untouched
#[test]
fn cross_parent_insert_before_leaves_selection_in_an_unrelated_third_parent_untouched() {
    leaves_an_unrelated_third_parent_untouched(act_insert_before);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertAfter shifts offset > removed index in source parent
#[test]
fn cross_parent_insert_after_shifts_offset_past_removed_index_in_source_parent() {
    shifts_offset_past_removed_index(act_insert_after);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertAfter does not shift offset at removed index (boundary)
#[test]
fn cross_parent_insert_after_does_not_shift_offset_at_removed_index() {
    does_not_shift_offset_at_removed_index(act_insert_after);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertAfter shifts only the past-boundary side of a non-collapsed selection
#[test]
fn cross_parent_insert_after_shifts_only_the_past_boundary_side() {
    shifts_only_the_past_boundary_side(act_insert_after);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertAfter leaves selection in an unrelated third parent untouched
#[test]
fn cross_parent_insert_after_leaves_selection_in_an_unrelated_third_parent_untouched() {
    leaves_an_unrelated_third_parent_untouched(act_insert_after);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > replace shifts offset > removed index in source parent
#[test]
fn cross_parent_replace_shifts_offset_past_removed_index_in_source_parent() {
    shifts_offset_past_removed_index(act_replace);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > replace does not shift offset at removed index (boundary)
#[test]
fn cross_parent_replace_does_not_shift_offset_at_removed_index() {
    does_not_shift_offset_at_removed_index(act_replace);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > replace shifts only the past-boundary side of a non-collapsed selection
#[test]
fn cross_parent_replace_shifts_only_the_past_boundary_side() {
    shifts_only_the_past_boundary_side(act_replace);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > replace leaves selection in an unrelated third parent untouched
#[test]
fn cross_parent_replace_leaves_selection_in_an_unrelated_third_parent_untouched() {
    leaves_an_unrelated_third_parent_untouched(act_replace);
}

/// A within-parent no-op move: `parent` holds `[a, b]`, collapsed at 2.
fn within_parent_move(insert_before: bool) {
    let editor = unit_test_editor();
    let parent_key = update_with(&editor, move || {
        let root = get_root();
        root.clear()?;
        let parent = create_test_element_node();
        let a = create_paragraph_node();
        let b = create_paragraph_node();
        parent.append(&[a.clone(), b.clone()])?;
        root.append(std::slice::from_ref(&parent))?;
        parent.select(Some(2), Some(2))?;
        if insert_before {
            b.insert_before(&a, true)?;
        } else {
            a.insert_after(&b, true)?;
        }
        Ok(parent.get_key().to_owned())
    });
    expect_selection(&editor, (&parent_key, 2), (&parent_key, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > within-parent move > insertBefore keeps an end-of-parent cursor in range for a no-op move
#[test]
fn within_parent_insert_before_keeps_an_end_of_parent_cursor_in_range() {
    within_parent_move(true);
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > within-parent move > insertAfter keeps an end-of-parent cursor in range for a no-op move
#[test]
fn within_parent_insert_after_keeps_an_end_of_parent_cursor_in_range() {
    within_parent_move(false);
}

/// `refs.source.select(anchor, focus)`, then `inTarget.insertAfter(mover)`;
/// the keys of the source and the target.
fn insert_after_cross_parent(anchor: usize, focus: usize) -> (Editor, String, String) {
    let editor = unit_test_editor();
    let (s, t) = update_with(&editor, move || {
        let refs = setup_two_containers()?;
        refs.source.select(Some(anchor), Some(focus))?;
        refs.in_target.insert_after(&refs.mover, true)?;
        Ok((
            refs.source.get_key().to_owned(),
            refs.target.get_key().to_owned(),
        ))
    });
    (editor, s, t)
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors a collapsed point that sat on the moved node
#[test]
fn insert_after_re_anchors_a_collapsed_point_on_the_moved_node() {
    let (editor, _, target) = insert_after_cross_parent(1, 1);
    expect_selection(&editor, (&target, 2), (&target, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors only the point that sat on the moved node
#[test]
fn insert_after_re_anchors_only_the_point_on_the_moved_node() {
    let (editor, source, target) = insert_after_cross_parent(0, 1);
    expect_selection(&editor, (&source, 0), (&target, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move shifts a source point past the removed index (times=-1)
#[test]
fn insert_after_shifts_a_source_point_past_the_removed_index() {
    let (editor, source, _) = insert_after_cross_parent(2, 2);
    expect_selection(&editor, (&source, 1), (&source, 1));
}

/// `parent` holds `[a, b]`, selected at `anchor` and `focus`; a fresh
/// paragraph is inserted after `a`.
fn insert_after_destination(
    anchor: usize,
    focus: usize,
    restore_selection: bool,
) -> (Editor, String) {
    let editor = unit_test_editor();
    let parent_key = update_with(&editor, move || {
        let root = get_root();
        root.clear()?;
        let parent = create_test_element_node();
        let a = create_paragraph_node();
        parent.append(&[a.clone(), create_paragraph_node()])?;
        root.append(std::slice::from_ref(&parent))?;
        parent.select(Some(anchor), Some(focus))?;
        a.insert_after(&create_paragraph_node(), restore_selection)?;
        Ok(parent.get_key().to_owned())
    });
    (editor, parent_key)
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > collapsed point on the destination parent shifts by one
#[test]
fn insert_after_collapsed_point_on_the_destination_parent_shifts_by_one() {
    let (editor, parent) = insert_after_destination(2, 2, true);
    expect_selection(&editor, (&parent, 3), (&parent, 3));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > non-collapsed selection on the destination parent shifts only the side past the insert
#[test]
fn insert_after_non_collapsed_selection_shifts_only_the_side_past_the_insert() {
    let (editor, parent) = insert_after_destination(0, 2, true);
    expect_selection(&editor, (&parent, 0), (&parent, 3));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > restoreSelection=false leaves the destination parent offset alone
#[test]
fn insert_after_restore_selection_false_leaves_the_destination_parent_offset_alone() {
    let (editor, parent) = insert_after_destination(2, 2, false);
    expect_selection(&editor, (&parent, 2), (&parent, 2));
}

/// `$setupMoverAtIndexOne`: source holds `[p, mover, p]`, target
/// `[inTarget]`; `source.select(offset, offset)` and the move.
fn insert_after_mover_at_index_one(offset: usize) -> (Editor, String, String) {
    let editor = unit_test_editor();
    let (s, t) = update_with(&editor, move || {
        let root = get_root();
        root.clear()?;
        let source = create_test_element_node();
        let target = create_test_element_node();
        let mover = create_paragraph_node();
        let in_target = create_paragraph_node();
        source.append(&[
            create_paragraph_node(),
            mover.clone(),
            create_paragraph_node(),
        ])?;
        target.append(std::slice::from_ref(&in_target))?;
        root.append(&[source.clone(), target.clone()])?;
        source.select(Some(offset), Some(offset))?;
        in_target.insert_after(&mover, true)?;
        Ok((source.get_key().to_owned(), target.get_key().to_owned()))
    });
    (editor, s, t)
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move leaves a point at the moved node index alone
#[test]
fn insert_after_leaves_a_point_at_the_moved_node_index_alone() {
    let (editor, source, _) = insert_after_mover_at_index_one(1);
    expect_selection(&editor, (&source, 1), (&source, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors onto a moved node that is not the first child
#[test]
fn insert_after_re_anchors_onto_a_moved_node_that_is_not_the_first_child() {
    let (editor, _, target) = insert_after_mover_at_index_one(2);
    expect_selection(&editor, (&target, 2), (&target, 2));
}

/// The selection `anchor` to `focus`, element points.
fn select_points(anchor: (&str, usize), focus: (&str, usize)) -> Result<(), crate::LexicalError> {
    let sel = create_range_selection();
    sel.point_set(Which::Anchor, anchor.0, anchor.1, PointType::Element)?;
    sel.point_set(Which::Focus, focus.0, focus.1, PointType::Element)?;
    set_selection(Some(sel));
    Ok(())
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors a focus-only point that sat on the moved node
#[test]
fn insert_after_re_anchors_a_focus_only_point_on_the_moved_node() {
    let editor = unit_test_editor();
    let (other, target) = update_with(&editor, || {
        let refs = setup_two_containers()?;
        let other = append_other()?;
        select_points((other.get_key(), 1), (refs.source.get_key(), 1))?;
        refs.in_target.insert_after(&refs.mover, true)?;
        Ok((other.get_key().to_owned(), refs.target.get_key().to_owned()))
    });
    expect_selection(&editor, (&other, 1), (&target, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > insert shifts a focus-only point on the destination parent
#[test]
fn insert_after_shifts_a_focus_only_point_on_the_destination_parent() {
    let editor = unit_test_editor();
    let (other, parent) = update_with(&editor, || {
        let root = get_root();
        root.clear()?;
        let parent = create_test_element_node();
        let a = create_paragraph_node();
        parent.append(&[a.clone(), create_paragraph_node()])?;
        let other = create_test_element_node();
        other.append(&[create_paragraph_node(), create_paragraph_node()])?;
        root.append(&[parent.clone(), other.clone()])?;
        select_points((other.get_key(), 1), (parent.get_key(), 2))?;
        a.insert_after(&create_paragraph_node(), true)?;
        Ok((other.get_key().to_owned(), parent.get_key().to_owned()))
    });
    expect_selection(&editor, (&other, 1), (&parent, 3));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors an anchor-only point that sat on the moved node
#[test]
fn insert_after_re_anchors_an_anchor_only_point_on_the_moved_node() {
    let editor = unit_test_editor();
    let (other, target) = update_with(&editor, || {
        let refs = setup_two_containers()?;
        let other = append_other()?;
        select_points((refs.source.get_key(), 1), (other.get_key(), 1))?;
        refs.in_target.insert_after(&refs.mover, true)?;
        Ok((other.get_key().to_owned(), refs.target.get_key().to_owned()))
    });
    expect_selection(&editor, (&target, 2), (&other, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > a point in an unrelated parent survives a cross-parent move untouched
#[test]
fn insert_after_a_point_in_an_unrelated_parent_survives_untouched() {
    let editor = unit_test_editor();
    let other = update_with(&editor, || {
        let refs = setup_two_containers()?;
        let other = append_other()?;
        other.select(Some(1), Some(1))?;
        refs.in_target.insert_after(&refs.mover, true)?;
        Ok(other.get_key().to_owned())
    });
    expect_selection(&editor, (&other, 1), (&other, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::selectPrevious and selectNext > both land on the sibling index
#[test]
fn select_previous_and_select_next_both_land_on_the_sibling_index() {
    let editor = unit_test_editor();
    update(&editor, || {
        use crate::nodes::line_break::create_line_break_node;
        let paragraph = create_paragraph_node();
        let first = create_line_break_node();
        let middle = create_line_break_node();
        let last = create_line_break_node();
        paragraph.append(&[first.clone(), middle, last.clone()])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        let key = paragraph.get_key();
        let sel = last.select_previous(None, None)?.range().unwrap();
        assert_eq!(
            (
                sel.anchor.key.as_str(),
                sel.anchor.point_type,
                sel.anchor.offset
            ),
            (key, PointType::Element, 2)
        );
        assert_eq!(sel.focus.offset, 2);
        let next_sel = first.select_next(None, None)?.range().unwrap();
        assert_eq!(
            (
                next_sel.anchor.key.as_str(),
                next_sel.anchor.point_type,
                next_sel.anchor.offset
            ),
            (key, PointType::Element, 1)
        );
        assert_eq!(next_sel.focus.offset, 1);
        Ok(())
    });
}

/// `$setup`: source holds `[s0, s1, s2]`, target `[t0, t1]`; then `f` with
/// them, which selects and edits, returning the keys to expect.
fn side_effects<R: 'static>(
    f: impl FnOnce(&NodeRef, &NodeRef) -> Result<R, crate::LexicalError> + 'static,
) -> (Editor, R) {
    let editor = unit_test_editor();
    let r = update_with(&editor, move || {
        let root = get_root();
        root.clear()?;
        let source = create_test_element_node();
        let target = create_test_element_node();
        source.append(&[
            create_paragraph_node(),
            create_paragraph_node(),
            create_paragraph_node(),
        ])?;
        target.append(&[create_paragraph_node(), create_paragraph_node()])?;
        root.append(&[source.clone(), target.clone()])?;
        f(&source, &target)
    });
    (editor, r)
}

fn key(node: &NodeRef) -> String {
    node.get_key().to_owned()
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts a point past the removed index in the old parent
#[test]
fn insert_before_shifts_a_point_past_the_removed_index_in_the_old_parent() {
    let (editor, s) = side_effects(|source, target| {
        source.select(Some(2), Some(2))?;
        target
            .get_last_child_or_throw()?
            .insert_before(&source.get_child_at_index(1).unwrap(), true)?;
        Ok(key(source))
    });
    expect_selection(&editor, (&s, 1), (&s, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore leaves a point at the removed index in the old parent alone
#[test]
fn insert_before_leaves_a_point_at_the_removed_index_in_the_old_parent_alone() {
    let (editor, s) = side_effects(|source, target| {
        source.select(Some(1), Some(1))?;
        target
            .get_last_child_or_throw()?
            .insert_before(&source.get_child_at_index(1).unwrap(), true)?;
        Ok(key(source))
    });
    expect_selection(&editor, (&s, 1), (&s, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts a point at or past the insert index in the new parent
#[test]
fn insert_before_shifts_a_point_at_or_past_the_insert_index_in_the_new_parent() {
    let (editor, t) = side_effects(|_, target| {
        target.select(Some(2), Some(2))?;
        target
            .get_last_child_or_throw()?
            .insert_before(&create_paragraph_node(), true)?;
        Ok(key(target))
    });
    expect_selection(&editor, (&t, 3), (&t, 3));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore leaves a point before the insert index alone
#[test]
fn insert_before_leaves_a_point_before_the_insert_index_alone() {
    let (editor, t) = side_effects(|_, target| {
        target.select(Some(0), Some(2))?;
        target
            .get_last_child_or_throw()?
            .insert_before(&create_paragraph_node(), true)?;
        Ok(key(target))
    });
    expect_selection(&editor, (&t, 0), (&t, 3));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore with restoreSelection=false leaves both parents alone
#[test]
fn insert_before_with_restore_selection_false_leaves_both_parents_alone() {
    let (editor, t) = side_effects(|_, target| {
        target.select(Some(2), Some(2))?;
        target
            .get_last_child_or_throw()?
            .insert_before(&create_paragraph_node(), false)?;
        Ok(key(target))
    });
    expect_selection(&editor, (&t, 2), (&t, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore leaves a point in an unrelated parent untouched
#[test]
fn insert_before_leaves_a_point_in_an_unrelated_parent_untouched() {
    let (editor, s) = side_effects(|source, target| {
        source.select(Some(1), Some(1))?;
        target
            .get_last_child_or_throw()?
            .insert_before(&create_paragraph_node(), true)?;
        Ok(key(source))
    });
    expect_selection(&editor, (&s, 1), (&s, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove shifts a point past the removed index
#[test]
fn remove_shifts_a_point_past_the_removed_index() {
    let (editor, s) = side_effects(|source, _| {
        source.select(Some(3), Some(3))?;
        source.get_child_at_index(1).unwrap().remove(false)?;
        Ok(key(source))
    });
    expect_selection(&editor, (&s, 2), (&s, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove leaves a point at or before the removed index alone
#[test]
fn remove_leaves_a_point_at_or_before_the_removed_index_alone() {
    let (editor, s) = side_effects(|source, _| {
        source.select(Some(1), Some(3))?;
        source.get_child_at_index(1).unwrap().remove(false)?;
        Ok(key(source))
    });
    expect_selection(&editor, (&s, 1), (&s, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove leaves a point in an unrelated parent untouched
#[test]
fn remove_leaves_a_point_in_an_unrelated_parent_untouched() {
    let (editor, t) = side_effects(|source, target| {
        target.select(Some(2), Some(2))?;
        source.get_child_at_index(1).unwrap().remove(false)?;
        Ok(key(target))
    });
    expect_selection(&editor, (&t, 2), (&t, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace shifts a point past the replacement's index in its old parent
#[test]
fn replace_shifts_a_point_past_the_replacements_index_in_its_old_parent() {
    let (editor, s) = side_effects(|source, target| {
        source.select(Some(2), Some(2))?;
        target
            .get_first_child_or_throw()?
            .replace(&source.get_child_at_index(1).unwrap(), false)?;
        Ok(key(source))
    });
    expect_selection(&editor, (&s, 1), (&s, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace leaves a point at the replacement's index in its old parent alone
#[test]
fn replace_leaves_a_point_at_the_replacements_index_in_its_old_parent_alone() {
    let (editor, s) = side_effects(|source, target| {
        source.select(Some(1), Some(1))?;
        target
            .get_first_child_or_throw()?
            .replace(&source.get_child_at_index(1).unwrap(), false)?;
        Ok(key(source))
    });
    expect_selection(&editor, (&s, 1), (&s, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace leaves a point in an unrelated parent untouched
#[test]
fn replace_leaves_a_point_in_an_unrelated_parent_untouched() {
    let (editor, t) = side_effects(|source, target| {
        target.select(Some(2), Some(2))?;
        target
            .get_first_child_or_throw()?
            .replace(&source.get_child_at_index(1).unwrap(), false)?;
        Ok(key(target))
    });
    expect_selection(&editor, (&t, 2), (&t, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts a focus-only point in the old parent
#[test]
fn insert_before_shifts_a_focus_only_point_in_the_old_parent() {
    let (editor, (o, s)) = side_effects(|source, target| {
        let other = append_other()?;
        select_points((other.get_key(), 1), (source.get_key(), 2))?;
        target
            .get_last_child_or_throw()?
            .insert_before(&source.get_child_at_index(1).unwrap(), true)?;
        Ok((key(&other), key(source)))
    });
    expect_selection(&editor, (&o, 1), (&s, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts a focus-only point in the new parent
#[test]
fn insert_before_shifts_a_focus_only_point_in_the_new_parent() {
    let (editor, (o, t)) = side_effects(|_, target| {
        let other = append_other()?;
        select_points((other.get_key(), 1), (target.get_key(), 2))?;
        target
            .get_last_child_or_throw()?
            .insert_before(&create_paragraph_node(), true)?;
        Ok((key(&other), key(target)))
    });
    expect_selection(&editor, (&o, 1), (&t, 3));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove shifts a focus-only point past the removed index
#[test]
fn remove_shifts_a_focus_only_point_past_the_removed_index() {
    let (editor, (o, s)) = side_effects(|source, _| {
        let other = append_other()?;
        select_points((other.get_key(), 1), (source.get_key(), 3))?;
        source.get_child_at_index(1).unwrap().remove(false)?;
        Ok((key(&other), key(source)))
    });
    expect_selection(&editor, (&o, 1), (&s, 2));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace shifts a focus-only point in the replacement's old parent
#[test]
fn replace_shifts_a_focus_only_point_in_the_replacements_old_parent() {
    let (editor, (o, s)) = side_effects(|source, target| {
        let other = append_other()?;
        select_points((other.get_key(), 1), (source.get_key(), 2))?;
        target
            .get_first_child_or_throw()?
            .replace(&source.get_child_at_index(1).unwrap(), false)?;
        Ok((key(&other), key(source)))
    });
    expect_selection(&editor, (&o, 1), (&s, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts an anchor-only point in the new parent
#[test]
fn insert_before_shifts_an_anchor_only_point_in_the_new_parent() {
    let (editor, (o, t)) = side_effects(|_, target| {
        let other = append_other()?;
        select_points((target.get_key(), 2), (other.get_key(), 1))?;
        target
            .get_last_child_or_throw()?
            .insert_before(&create_paragraph_node(), true)?;
        Ok((key(&other), key(target)))
    });
    expect_selection(&editor, (&t, 3), (&o, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove shifts an anchor-only point past the removed index
#[test]
fn remove_shifts_an_anchor_only_point_past_the_removed_index() {
    let (editor, (o, s)) = side_effects(|source, _| {
        let other = append_other()?;
        select_points((source.get_key(), 3), (other.get_key(), 1))?;
        source.get_child_at_index(1).unwrap().remove(false)?;
        Ok((key(&other), key(source)))
    });
    expect_selection(&editor, (&s, 2), (&o, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace shifts an anchor-only point in the replacement's old parent
#[test]
fn replace_shifts_an_anchor_only_point_in_the_replacements_old_parent() {
    let (editor, (o, s)) = side_effects(|source, target| {
        let other = append_other()?;
        select_points((source.get_key(), 2), (other.get_key(), 1))?;
        target
            .get_first_child_or_throw()?
            .replace(&source.get_child_at_index(1).unwrap(), false)?;
        Ok((key(&other), key(source)))
    });
    expect_selection(&editor, (&s, 1), (&o, 1));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace with a fresh node leaves the parent offsets alone
#[test]
fn replace_with_a_fresh_node_leaves_the_parent_offsets_alone() {
    let (editor, t) = side_effects(|_, target| {
        target.select(Some(2), Some(2))?;
        target
            .get_first_child_or_throw()?
            .replace(&create_paragraph_node(), false)?;
        Ok(key(target))
    });
    expect_selection(&editor, (&t, 2), (&t, 2));
}

/// A paragraph of `n` line breaks, the root's only child.
fn line_breaks(n: usize) -> Result<NodeRef, crate::LexicalError> {
    use crate::nodes::line_break::create_line_break_node;
    let parent = create_paragraph_node();
    let breaks: Vec<NodeRef> = (0..n).map(|_| create_line_break_node()).collect();
    parent.append(&breaks)?;
    get_root().clear()?;
    get_root().append(std::slice::from_ref(&parent))?;
    Ok(parent)
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > element-anchored point maps to prevSize + originalOffset (etrepum example)
#[test]
fn replace_include_children_element_point_maps_to_prev_size_plus_offset() {
    let editor = unit_test_editor();
    let (new_parent_key, prev_size) = update_with(&editor, || {
        let parent = line_breaks(3)?;
        parent.select(Some(1), Some(1))?;
        let new_parent = create_paragraph_node();
        new_parent.append(&[create_text_node("before")])?;
        let prev_size = new_parent.get_children_size();
        parent.replace(&new_parent, true)?;
        Ok((key(&new_parent), prev_size))
    });
    editor
        .read(|| {
            let sel = get_selection().unwrap();
            let range = sel.range().unwrap();
            assert!(range.is_collapsed());
            assert_eq!(
                (
                    range.anchor.key,
                    range.anchor.point_type,
                    range.anchor.offset
                ),
                (new_parent_key, PointType::Element, prev_size + 1)
            );
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > non-collapsed element-anchored anchor and focus both shift by prevSize
#[test]
fn replace_include_children_non_collapsed_points_shift_by_prev_size() {
    let editor = unit_test_editor();
    let new_parent_key = update_with(&editor, || {
        let parent = line_breaks(3)?;
        parent.select(Some(1), Some(3))?;
        let new_parent = create_paragraph_node();
        new_parent.append(&[
            crate::nodes::line_break::create_line_break_node(),
            crate::nodes::line_break::create_line_break_node(),
        ])?;
        parent.replace(&new_parent, true)?;
        Ok(key(&new_parent))
    });
    // prevSize = 2 (two line breaks pre-transfer)
    expect_selection(&editor, (&new_parent_key, 2 + 1), (&new_parent_key, 2 + 3));
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > text-anchored selection inside transferred children is unaffected
#[test]
fn replace_include_children_text_selection_is_unaffected() {
    let editor = unit_test_editor();
    let text_key = update_with(&editor, || {
        let parent = create_paragraph_node();
        let t1 = create_text_node("hello");
        parent.append(std::slice::from_ref(&t1))?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&parent))?;
        t1.select(Some(2), Some(2))?;
        let new_parent = create_paragraph_node();
        new_parent.append(&[crate::nodes::line_break::create_line_break_node()])?;
        parent.replace(&new_parent, true)?;
        Ok(key(&t1))
    });
    editor
        .read(|| {
            let range = get_selection().unwrap().range().unwrap();
            assert_eq!(
                (
                    range.anchor.key,
                    range.anchor.point_type,
                    range.anchor.offset
                ),
                (text_key, PointType::Text, 2)
            );
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > replace without includeChildren falls back to the legacy moveSelectionPointToEnd
#[test]
fn replace_without_include_children_falls_back_to_move_selection_point_to_end() {
    let editor = unit_test_editor();
    let (new_parent_key, prefix_key) = update_with(&editor, || {
        let parent = line_breaks(2)?;
        parent.select(Some(1), Some(1))?;
        let prefix = create_text_node("prefix");
        let new_parent = create_paragraph_node();
        new_parent.append(std::slice::from_ref(&prefix))?;
        parent.replace(&new_parent, false)?;
        Ok((key(&new_parent), key(&prefix)))
    });
    editor
        .read(|| {
            let range = get_selection().unwrap().range().unwrap();
            assert_eq!(range.anchor.point_type, PointType::Text);
            assert_eq!(range.anchor.key, prefix_key);
            assert_eq!(
                range
                    .anchor
                    .get_node()
                    .unwrap()
                    .get_parent_or_throw()
                    .unwrap()
                    .get_key(),
                new_parent_key
            );
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > $setBlocksType-style pattern (prevSize=0 fresh receiver) preserves original offset
#[test]
fn replace_include_children_into_a_fresh_receiver_preserves_the_offset() {
    let editor = unit_test_editor();
    let new_parent_key = update_with(&editor, || {
        let parent = line_breaks(3)?;
        parent.select(Some(2), Some(2))?;
        let new_parent = create_paragraph_node();
        parent.replace(&new_parent, true)?;
        Ok(key(&new_parent))
    });
    editor
        .read(|| {
            let range = get_selection().unwrap().range().unwrap();
            assert_eq!(
                (
                    range.anchor.key,
                    range.anchor.point_type,
                    range.anchor.offset
                ),
                (new_parent_key, PointType::Element, 2)
            );
        })
        .unwrap();
}

/// Upstream checks TypeScript's narrowing with `expectTypeOf`, which the
/// compiler stands for; the run-time assertions are these.
// upstream: lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode.$config() without registration > a structurally-identical subclass stays narrowable
#[test]
fn a_structurally_identical_subclass_stays_narrowable() {
    use crate::nodes::tab::{TabNode, create_tab_node};
    let is_tab_node = |node: &NodeRef| node.get_latest().is::<TabNode>();
    let editor = create_editor(CreateEditorArgs::default());
    update(&editor, move || {
        let narrow_from_text_node = |node: &NodeRef| {
            assert!(node.is_text());
            node.get_type()
        };
        let tab = create_tab_node();
        let text = create_text_node("x");
        assert!(is_tab_node(&tab));
        assert!(!is_tab_node(&text));
        assert_eq!(narrow_from_text_node(&text), "text");
        assert_eq!(narrow_from_text_node(&tab), "tab");
        Ok(())
    });
}
