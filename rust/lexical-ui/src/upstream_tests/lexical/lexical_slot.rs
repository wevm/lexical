//! Ports of `packages/lexical/src/__tests__/unit/LexicalSlot.test.ts`, on
//! editors with a no-op root element, so commits reconcile. The tests of
//! the rendered slot DOM (containers, placeholders, managed line breaks,
//! DOM reuse) are skipped in `rust/parity/skips.json`; where a test reads
//! only whether a node has a keyed DOM (`getElementByKey`), this reads the
//! reconciler's key map, as Swift's port does.

#![cfg(feature = "extension")]

use crate::caret::{CaretDirection, RootMode, get_child_caret};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::lexical_utils::with_writable;
use crate::nodes::element::ElementData;
use crate::nodes::line_break::create_line_break_node;
use crate::selection_deletion::get_nearest_root_or_shadow_root;
use crate::upstream_tests::support::{
    TestInlineElementNode, TestShadowRootNode, create_test_inline_element_node,
    create_test_shadow_root_node, panic_message, set_test_root_element, update, update_with,
};
use crate::{
    Editor, InitialEditorStateType, LexicalError, LexicalExtension, LexicalNode, NodeBase,
    NodeClass, NodeRef, PointType, SelectionRef, Which, create, create_node_selection,
    create_paragraph_node, create_range_selection, create_text_node, define_extension,
    get_node_by_key, get_root, get_selection, get_selection_slot_frame, get_slot, get_slot_frame,
    get_slot_host, get_slot_name_within_host, get_slot_names, is_slot_host, remove_slot,
    select_all, select_all_in, set_selection, set_slot,
};
use std::rc::Rc;

/// `TestDecoratorNode` (`__tests__/utils`): a decorator whose text is
/// "Hello world", inline unless `setIsInline(false)`.
#[derive(Clone, Debug, Default)]
struct TestDecoratorNode {
    base: NodeBase,
    block: bool,
}

impl LexicalNode for TestDecoratorNode {
    fn get_type(&self) -> &'static str {
        "test_decorator"
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
        !self.block
    }
    fn get_text_content(&self) -> String {
        "Hello world".to_owned()
    }
}

/// `$createTestDecoratorNode()`, then `setIsInline(inline)`.
fn create_test_decorator_node(inline: bool) -> NodeRef {
    let node = create::<TestDecoratorNode>();
    with_writable(node.get_key(), |n| {
        n.downcast_mut::<TestDecoratorNode>().unwrap().block = !inline;
    });
    node
}

/// `SlotTextHostNode`: a decorator host that keeps the base
/// `getTextContent` (its slots' text), inline unless `setIsInline(false)`.
#[derive(Clone, Debug, Default)]
struct SlotTextHostNode {
    base: NodeBase,
    block: bool,
}

impl LexicalNode for SlotTextHostNode {
    fn get_type(&self) -> &'static str {
        "slot_text_host"
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
        !self.block
    }
}

/// `$create(SlotTextHostNode)`, then `setIsInline(inline)`.
fn create_slot_text_host(inline: bool) -> NodeRef {
    let node = create::<SlotTextHostNode>();
    with_writable(node.get_key(), |n| {
        n.downcast_mut::<SlotTextHostNode>().unwrap().block = !inline;
    });
    node
}

macro_rules! host_class {
    ($(#[$doc:meta])* $name:ident, $type:literal, [$($slot:literal),*], [$($super:literal),*]) => {
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
            fn declared_slots(&self) -> &'static [&'static str] {
                &[$($slot),*]
            }
        }
    };
}

host_class!(
    /// `DeclaredHostNode`: declares `title` before `body`.
    DeclaredHostNode,
    "declared_slot_host",
    ["title", "body"],
    []
);
host_class!(
    /// `ReorderedHostNode`: a subclass that redeclares the order.
    ReorderedHostNode,
    "reordered_slot_host",
    ["body", "title"],
    ["declared_slot_host"]
);
host_class!(
    DupDeclaredHostNode,
    "dup_declared_slot_host",
    ["x", "x"],
    []
);
host_class!(
    ReservedDeclaredHostNode,
    "reserved_declared_slot_host",
    ["__proto__"],
    []
);

/// `createSlotEditor()`: an empty root, on a root element.
fn create_slot_editor() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            get_root().clear()
        }))),
        nodes: vec![
            NodeClass::of::<TestShadowRootNode>(),
            NodeClass::of::<TestDecoratorNode>(),
            NodeClass::of::<TestInlineElementNode>(),
            NodeClass::of::<DeclaredHostNode>(),
            NodeClass::of::<ReorderedHostNode>(),
            NodeClass::of::<DupDeclaredHostNode>(),
            NodeClass::of::<ReservedDeclaredHostNode>(),
            NodeClass::of::<SlotTextHostNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[slot-core]", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

/// `$slotContainer(...texts)`: a shadow root with a paragraph per text.
fn slot_container(texts: &[&str]) -> NodeRef {
    let container = create_test_shadow_root_node();
    for text in texts {
        container
            .append(&[para(&[create_text_node(text)])])
            .unwrap();
    }
    container
}

fn para(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

fn node(key: &str) -> NodeRef {
    get_node_by_key(key).expect("a node")
}

fn read<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read(f).unwrap()
}

/// `expect(f).toThrow()`: the message `f` fails or panics with.
fn throws<R>(f: impl FnOnce() -> Result<R, LexicalError>) -> Option<String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(Ok(_)) => None,
        Ok(Err(e)) => Some(e.to_string()),
        Err(payload) => Some(panic_message(&*payload)),
    }
}

fn throws_matching<R>(f: impl FnOnce() -> Result<R, LexicalError>, fragment: &str) {
    let message = throws(f).expect("expected an error");
    assert!(message.contains(fragment), "{message}");
}

fn key_has_dom(editor: &Editor, key: &str) -> bool {
    editor.0.key_to_dom_map.borrow().contains_key(key)
}

fn names(node: &NodeRef) -> Vec<String> {
    get_slot_names(node)
}

fn range_point(selection: &SelectionRef, which: Which) -> crate::Point {
    selection.point(which).unwrap()
}

/// A collapsed range selection at `key`, set as the selection.
fn caret_at(key: &str, offset: usize, point_type: PointType) -> SelectionRef {
    let selection = create_range_selection();
    selection
        .point_set(Which::Anchor, key, offset, point_type)
        .unwrap();
    selection
        .point_set(Which::Focus, key, offset, point_type)
        .unwrap();
    set_selection(Some(selection.clone()));
    selection
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a slotted node is reachable, parentless, and attached
#[test]
fn a_slotted_node_is_reachable_parentless_and_attached() {
    let editor = create_slot_editor();
    let (host_key, slot_key) = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = slot_container(&[]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        Ok((host.get_key().to_owned(), slot.get_key().to_owned()))
    });
    read(&editor, || {
        let host = node(&host_key);
        let slot = node(&slot_key);
        assert_eq!(get_slot(&host, "title"), Some(slot.clone()));
        assert_eq!(names(&host), ["title"]);
        assert_eq!(get_slot_host(&slot), Some(host.clone()));
        assert_eq!(slot.get_parent(), None);
        assert_eq!(slot.get_latest().base().parent, None);
        assert_eq!(
            slot.get_latest().base().slot_host.as_deref(),
            Some(host.get_key())
        );
        assert!(slot.is_attached());
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getTopLevelElement stops at the slot boundary
#[test]
fn get_top_level_element_stops_at_the_slot_boundary() {
    let editor = create_slot_editor();
    let (slot, para_node, text) = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = create_test_shadow_root_node();
        let text = create_text_node("Title");
        let p = para(std::slice::from_ref(&text));
        slot.append(std::slice::from_ref(&p))?;
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        Ok((slot, p, text))
    });
    read(&editor, || {
        assert_eq!(
            text.get_top_level_element().unwrap(),
            Some(para_node.clone())
        );
        assert_eq!(text.get_top_level_element_or_throw().unwrap(), para_node);
        assert_eq!(slot.get_top_level_element().unwrap(), Some(slot.clone()));
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > slot map survives a host mutation (clone)
#[test]
fn slot_map_survives_a_host_mutation() {
    let editor = create_slot_editor();
    let (host, slot) = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = slot_container(&[]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "body", &slot)?;
        Ok((host, slot))
    });
    let h = host.clone();
    update(&editor, move || {
        h.set_element_style("color: red");
        Ok(())
    });
    read(&editor, || {
        assert_eq!(host.element_data().get_style(), "color: red");
        assert_eq!(get_slot(&host, "body"), Some(slot.clone()));
        assert!(slot.is_attached());
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot detaches a node that already has a parent
#[test]
fn set_slot_detaches_a_node_that_already_has_a_parent() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        let parented = create_test_shadow_root_node();
        get_root().append(&[host.clone(), parented.clone()])?;
        assert_eq!(parented.get_parent(), Some(get_root()));
        set_slot(&host, "title", &parented)?;
        assert_eq!(parented.get_parent(), None);
        assert_eq!(get_root().get_children_size(), 1);
        assert_eq!(get_slot(&host, "title"), Some(parented));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > $setSlot moves a node already slotted elsewhere (move semantics)
#[test]
fn set_slot_moves_a_node_already_slotted_elsewhere() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host_a = create_paragraph_node();
        let host_b = create_paragraph_node();
        let slot = create_test_shadow_root_node();
        get_root().append(&[host_a.clone(), host_b.clone()])?;
        set_slot(&host_a, "title", &slot)?;
        set_slot(&host_b, "title", &slot)?;
        assert!(names(&host_a).is_empty());
        assert_eq!(get_slot(&host_b, "title"), Some(slot.clone()));
        assert_eq!(get_slot_host(&slot), Some(host_b.clone()));
        set_slot(&host_b, "subtitle", &slot)?;
        assert_eq!(names(&host_b), ["subtitle"]);
        assert_eq!(get_slot(&host_b, "subtitle"), Some(slot.clone()));
        assert_eq!(get_slot_host(&slot), Some(host_b));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot rejects hosting itself or an ancestor (cycle guard)
#[test]
fn set_slot_rejects_hosting_itself_or_an_ancestor() {
    let editor = create_slot_editor();
    update(&editor, || {
        let outer = create_test_shadow_root_node();
        let inner = create_test_shadow_root_node();
        get_root().append(std::slice::from_ref(&outer))?;
        outer.append(std::slice::from_ref(&inner))?;
        throws_matching(|| set_slot(&outer, "x", &outer), "cycle");
        throws_matching(|| set_slot(&inner, "x", &outer), "cycle");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot rejects an ancestor reachable through a slot up-link
#[test]
fn set_slot_rejects_an_ancestor_reachable_through_a_slot_up_link() {
    let editor = create_slot_editor();
    update(&editor, || {
        let a = create_test_shadow_root_node();
        let b = create_test_shadow_root_node();
        get_root().append(std::slice::from_ref(&a))?;
        set_slot(&a, "x", &b)?;
        throws_matching(|| set_slot(&b, "y", &a), "cycle");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > moving a slotted node into a child list throws (reverse guard)
#[test]
fn moving_a_slotted_node_into_a_child_list_throws() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        let sibling = create_paragraph_node();
        get_root().append(&[host.clone(), sibling.clone()])?;
        let slot = create_test_shadow_root_node();
        set_slot(&host, "title", &slot)?;
        assert!(throws(|| host.append(std::slice::from_ref(&slot))).is_some());
        assert!(throws(|| sibling.insert_after(&slot, true)).is_some());
        assert!(throws(|| sibling.insert_before(&slot, true)).is_some());
        assert!(throws(|| sibling.replace(&slot, false)).is_some());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > replace() on a slotted node throws (use $setSlot)
#[test]
fn replace_on_a_slotted_node_throws() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let slot = create_test_shadow_root_node();
        set_slot(&host, "title", &slot)?;
        let message = throws(|| slot.replace(&create_paragraph_node(), false)).unwrap();
        // `/\(type paragraph\)[\s\S]*\$setSlot/`
        let at = message.find("(type paragraph)").expect(&message);
        assert!(message[at..].contains("$setSlot"), "{message}");
        assert_eq!(get_slot(&host, "title"), Some(slot));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > remove() on a slotted node throws (use $removeSlot)
#[test]
fn remove_on_a_slotted_node_throws() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let slot = create_test_shadow_root_node();
        set_slot(&host, "title", &slot)?;
        assert!(throws(|| slot.remove(false)).is_some());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot rejects reserved names (__proto__, constructor, prototype)
#[test]
fn set_slot_rejects_reserved_names() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        for reserved in ["__proto__", "constructor", "prototype"] {
            let slot = create_test_shadow_root_node();
            assert!(throws(|| set_slot(&host, reserved, &slot)).is_some());
        }
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot enforces non-inline element or decorator values
#[test]
fn set_slot_enforces_non_inline_element_or_decorator_values() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let invalid = "not a valid slot value";
        throws_matching(|| set_slot(&host, "title", &create_text_node("x")), invalid);
        throws_matching(
            || set_slot(&host, "title", &create_test_inline_element_node()),
            invalid,
        );
        throws_matching(
            || set_slot(&host, "title", &create_test_decorator_node(true)),
            invalid,
        );
        let line = create_paragraph_node();
        assert_eq!(throws(|| set_slot(&host, "line", &line)), None);
        assert_eq!(get_slot(&host, "line"), Some(line));
        remove_slot(&host, "line")?;
        let shadow = create_test_shadow_root_node();
        assert_eq!(throws(|| set_slot(&host, "title", &shadow)), None);
        assert_eq!(get_slot(&host, "title"), Some(shadow));
        let block_decorator = create_test_decorator_node(false);
        assert_eq!(throws(|| set_slot(&host, "body", &block_decorator)), None);
        assert_eq!(get_slot(&host, "body"), Some(block_decorator));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > overwriting a slot name orphans the previous occupant (no leak)
#[test]
fn overwriting_a_slot_name_orphans_the_previous_occupant() {
    let editor = create_slot_editor();
    let (old_key, new_key) = update_with(&editor, || {
        let host = create_paragraph_node();
        let old_slot = create_test_shadow_root_node();
        let new_slot = create_test_shadow_root_node();
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &old_slot)?;
        set_slot(&host, "title", &new_slot)?;
        Ok((old_slot.get_key().to_owned(), new_slot.get_key().to_owned()))
    });
    read(&editor, || {
        let host = get_root().get_first_child().unwrap();
        assert_eq!(host.get_type(), "paragraph");
        assert_eq!(get_slot(&host, "title").unwrap().get_key(), new_key);
        assert!(get_node_by_key(&old_key).is_none());
        assert!(node(&new_key).is_attached());
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > slots round-trip through serialize -> parse, alongside a normal child
#[test]
fn slots_round_trip_through_serialize_parse_alongside_a_normal_child() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        let title = slot_container(&["Heading"]);
        let normal_child = para(&[create_text_node("Body text")]);
        get_root().append(std::slice::from_ref(&host))?;
        host.append(&[normal_child])?;
        set_slot(&host, "title", &title)?;
        Ok(())
    });
    let stringified = editor.get_editor_state().to_json(false).to_string();
    editor.parse_editor_state(&stringified).unwrap().read(|| {
        let host = get_root().get_first_child().unwrap();
        assert_eq!(host.get_type(), "paragraph");
        assert_eq!(host.get_children().len(), 1);
        assert!(host.get_text_content().contains("Body text"));
        let title = get_slot(&host, "title").expect("a title");
        assert_eq!(title.get_text_content(), "Heading");
        assert_eq!(get_slot_host(&title), Some(host));
        assert_eq!(title.get_parent(), None);
        assert!(title.is_attached());
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a nested host (slot value contains another host with its own slot) round-trips
#[test]
fn a_nested_host_round_trips() {
    let editor = create_slot_editor();
    update(&editor, || {
        let outer_host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&outer_host))?;
        let container = create_test_shadow_root_node();
        let inner_host = para(&[create_text_node("inner body")]);
        container.append(std::slice::from_ref(&inner_host))?;
        set_slot(&inner_host, "inner", &slot_container(&["InnerSlot"]))?;
        set_slot(&outer_host, "outer", &container)?;
        Ok(())
    });
    let stringified = editor.get_editor_state().to_json(false).to_string();
    editor.parse_editor_state(&stringified).unwrap().read(|| {
        let outer_host = get_root().get_first_child().unwrap();
        assert_eq!(outer_host.get_type(), "paragraph");
        let outer_slot = get_slot(&outer_host, "outer").unwrap();
        assert!(outer_slot.is_element());
        assert_eq!(get_slot_host(&outer_slot), Some(outer_host));
        let inner_host = outer_slot.get_first_child().unwrap();
        assert!(inner_host.is_element());
        assert_eq!(
            inner_host.get_children()[0].get_text_content(),
            "inner body"
        );
        let inner_slot = get_slot(&inner_host, "inner").unwrap();
        assert_eq!(inner_slot.get_text_content(), "InnerSlot");
        assert_eq!(get_slot_host(&inner_slot), Some(inner_host));
        assert_eq!(inner_slot.get_parent(), None);
        assert!(inner_slot.is_attached());
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > restoring a prior editor state reverts a slot move (undo-style)
#[test]
fn restoring_a_prior_editor_state_reverts_a_slot_move() {
    let editor = create_slot_editor();
    let slot_key = update_with(&editor, || {
        let host_a = create_paragraph_node();
        let host_b = create_paragraph_node();
        get_root().append(&[host_a.clone(), host_b])?;
        let slot = slot_container(&["Moved"]);
        set_slot(&host_a, "title", &slot)?;
        Ok(slot.get_key().to_owned())
    });
    let before_move = editor.get_editor_state();
    update(&editor, || {
        let host_a = get_root().get_first_child().unwrap();
        let host_b = get_root().get_last_child().unwrap();
        set_slot(&host_b, "title", &get_slot(&host_a, "title").unwrap())?;
        Ok(())
    });
    read(&editor, || {
        let host_b = get_root().get_last_child().unwrap();
        assert_eq!(get_slot(&host_b, "title").unwrap().get_key(), slot_key);
    });
    editor.set_editor_state(&before_move, None).unwrap();
    read(&editor, || {
        let host_a = get_root().get_first_child().unwrap();
        let host_b = get_root().get_last_child().unwrap();
        assert!(names(&host_b).is_empty());
        assert_eq!(get_slot(&host_a, "title").unwrap().get_key(), slot_key);
        assert_eq!(get_slot_host(&node(&slot_key)), Some(host_a));
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > export throws when a slot key resolves to no node
#[test]
fn export_throws_when_a_slot_key_resolves_to_no_node() {
    let headless = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            get_root().clear()
        }))),
        ..LexicalExtension::<(), (), ()>::new("[slot-poc-headless]", ())
    })
    .into()])
    .unwrap();
    update(&headless, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        with_writable(host.get_key(), |n| {
            let slots = n.base_mut().slots.get_or_insert_default();
            Rc::make_mut(slots).set("ghost", "nonexistent-key".to_owned());
        });
        Ok(())
    });
    assert!(throws(|| Ok::<_, LexicalError>(headless.get_editor_state().to_json(false))).is_some());
}

/// A host paragraph with a `title` slot (`Title`) and a body paragraph
/// (`Body`), appended to the root.
fn title_and_body_host() -> Result<NodeRef, LexicalError> {
    let host = create_paragraph_node();
    let title = slot_container(&["Title"]);
    let body = para(&[create_text_node("Body")]);
    get_root().append(std::slice::from_ref(&host))?;
    host.append(&[body])?;
    set_slot(&host, "title", &title)?;
    Ok(host)
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getTextContent reads slots-first, ahead of children
#[test]
fn get_text_content_reads_slots_first_ahead_of_children() {
    let editor = create_slot_editor();
    update(&editor, || title_and_body_host().map(drop));
    read(&editor, || {
        let host = get_root().get_first_child().unwrap();
        assert_eq!(host.get_text_content(), "TitleBody");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > detaching a host garbage-collects its slot node
#[test]
fn detaching_a_host_garbage_collects_its_slot_node() {
    let editor = create_slot_editor();
    let slot_key = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = slot_container(&[]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        Ok(slot.get_key().to_owned())
    });
    assert!(read(&editor, || get_node_by_key(&slot_key)).is_some());
    update(&editor, || {
        get_root().get_first_child().unwrap().remove(false)
    });
    assert!(read(&editor, || get_node_by_key(&slot_key)).is_none());
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a slot value moved to another host survives the old host being removed in the same commit
#[test]
fn a_slot_value_moved_to_another_host_survives_the_old_host_being_removed() {
    let editor = create_slot_editor();
    let slot_key = update_with(&editor, || {
        let host_a = create_paragraph_node();
        let host_b = create_paragraph_node();
        get_root().append(&[host_a.clone(), host_b])?;
        let slot = slot_container(&["Moved"]);
        set_slot(&host_a, "title", &slot)?;
        Ok(slot.get_key().to_owned())
    });
    update(&editor, || {
        let host_a = get_root().get_first_child().unwrap();
        let host_b = get_root().get_last_child().unwrap();
        set_slot(&host_b, "title", &get_slot(&host_a, "title").unwrap())?;
        host_a.remove(false)
    });
    read(&editor, || {
        assert!(get_node_by_key(&slot_key).is_some());
        let host_b = get_root().get_first_child().unwrap();
        assert_eq!(get_slot(&host_b, "title").unwrap().get_key(), slot_key);
        assert!(node(&slot_key).is_attached());
    });
    update(&editor, || {
        get_root().get_first_child().unwrap().remove(false)
    });
    assert!(read(&editor, || get_node_by_key(&slot_key)).is_none());
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > detaching a decorator host garbage-collects its slot node
#[test]
fn detaching_a_decorator_host_garbage_collects_its_slot_node() {
    let editor = create_slot_editor();
    let (slot_key, text_key) = update_with(&editor, || {
        let host = create_test_decorator_node(false);
        let slot = create_test_shadow_root_node();
        let text = create_text_node("Slotted");
        slot.append(&[para(std::slice::from_ref(&text))])?;
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        Ok((slot.get_key().to_owned(), text.get_key().to_owned()))
    });
    assert!(read(&editor, || get_node_by_key(&slot_key)).is_some());
    assert!(read(&editor, || get_node_by_key(&text_key)).is_some());
    update(&editor, || {
        get_root().get_first_child().unwrap().remove(false)
    });
    assert!(read(&editor, || get_node_by_key(&slot_key)).is_none());
    assert!(read(&editor, || get_node_by_key(&text_key)).is_none());
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getTextContentSize counts slots-first, like getTextContent
#[test]
fn get_text_content_size_counts_slots_first_like_get_text_content() {
    let editor = create_slot_editor();
    update(&editor, || title_and_body_host().map(drop));
    read(&editor, || {
        let host = get_root().get_first_child().unwrap();
        assert_eq!(
            host.get_text_content_size(),
            crate::utf16::len(&host.get_text_content())
        );
        assert_eq!(host.get_text_content_size(), 9);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getAllTextNodes includes slot text, slots-first
#[test]
fn get_all_text_nodes_includes_slot_text_slots_first() {
    let editor = create_slot_editor();
    update(&editor, || title_and_body_host().map(drop));
    read(&editor, || {
        let host = get_root().get_first_child().unwrap();
        let texts: Vec<String> = host
            .get_all_text_nodes()
            .iter()
            .map(NodeRef::get_text_content)
            .collect();
        assert_eq!(texts, ["Title", "Body"]);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a host with slots but no children is not empty
#[test]
fn a_host_with_slots_but_no_children_is_not_empty() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot_container(&["Title"]))?;
        Ok(())
    });
    read(&editor, || {
        let host = get_root().get_first_child().unwrap();
        assert_eq!(host.get_children_size(), 0);
        assert!(!host.is_empty());
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > reconciler text cache folds slot text in slots-first (RootNode.__cachedText)
#[test]
fn reconciler_text_cache_folds_slot_text_in_slots_first() {
    let editor = create_slot_editor();
    update(&editor, || title_and_body_host().map(drop));
    read(&editor, || {
        let root = get_root();
        let host = root.get_first_child().unwrap();
        assert_eq!(host.get_text_content(), "TitleBody");
        assert_eq!(root.get_text_content(), "TitleBody");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a host with only a slot renders the slot and caches its text
#[test]
fn a_host_with_only_a_slot_renders_the_slot_and_caches_its_text() {
    let editor = create_slot_editor();
    let title_key = update_with(&editor, || {
        let host = create_paragraph_node();
        let title = slot_container(&["Title"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &title)?;
        Ok(title.get_key().to_owned())
    });
    assert!(key_has_dom(&editor, &title_key));
    read(&editor, || {
        let root = get_root();
        assert_eq!(root.get_first_child().unwrap().get_text_content(), "Title");
        assert_eq!(root.get_text_content(), "Title");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > descendant navigation stays children-only (slots stay out of selection)
#[test]
fn descendant_navigation_stays_children_only() {
    let editor = create_slot_editor();
    let body_text_key = update_with(&editor, || {
        let host = title_and_body_host()?;
        Ok(host.get_first_descendant().unwrap().get_key().to_owned())
    });
    read(&editor, || {
        let host = get_root().get_first_child().unwrap();
        assert_eq!(
            host.get_first_descendant().unwrap().get_key(),
            body_text_key
        );
        assert_eq!(host.get_last_descendant().unwrap().get_key(), body_text_key);
        assert_eq!(node(&body_text_key).get_text_content(), "Body");
    });
}

/// A host with a `title` slot holding `titleText` and five child
/// paragraphs `c0`..`c4`; the host and the text keys.
fn suffix_host(title_text: &str) -> Result<(String, String, Vec<String>), LexicalError> {
    let host = create_paragraph_node();
    let title = create_test_shadow_root_node();
    let text = create_text_node(title_text);
    title.append(&[para(std::slice::from_ref(&text))])?;
    get_root().append(std::slice::from_ref(&host))?;
    let mut child_keys = Vec::new();
    for i in 0..5 {
        let child_text = create_text_node(&format!("c{i}"));
        host.append(&[para(std::slice::from_ref(&child_text))])?;
        child_keys.push(child_text.get_key().to_owned());
    }
    set_slot(&host, "title", &title)?;
    Ok((
        host.get_key().to_owned(),
        text.get_key().to_owned(),
        child_keys,
    ))
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > suffix fast path keeps slot text when a slot and a suffix child are edited together
#[test]
fn suffix_fast_path_keeps_slot_text_when_a_slot_and_a_suffix_child_are_edited_together() {
    let editor = create_slot_editor();
    let (host_key, title_text_key, child_keys) = update_with(&editor, || suffix_host("Title"));
    let child = child_keys[4].clone();
    update(&editor, move || {
        node(&title_text_key).set_text_content("Header");
        node(&child).set_text_content("c4!");
        Ok(())
    });
    read(&editor, || {
        let host = node(&host_key);
        assert_eq!(get_root().get_text_content(), host.get_text_content());
        assert!(host.get_text_content().starts_with("Header"));
        assert!(host.get_text_content().contains("c4!"));
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > suffix fast path keeps slot text when only a suffix child is edited
#[test]
fn suffix_fast_path_keeps_slot_text_when_only_a_suffix_child_is_edited() {
    let editor = create_slot_editor();
    let (host_key, child_keys) = update_with(&editor, || {
        let host = create_paragraph_node();
        let title = slot_container(&["Title"]);
        get_root().append(std::slice::from_ref(&host))?;
        let mut child_keys = Vec::new();
        for i in 0..5 {
            let child_text = create_text_node(&format!("c{i}"));
            host.append(&[para(std::slice::from_ref(&child_text))])?;
            child_keys.push(child_text.get_key().to_owned());
        }
        set_slot(&host, "title", &title)?;
        Ok((host.get_key().to_owned(), child_keys))
    });
    let child = child_keys[4].clone();
    update(&editor, move || {
        node(&child).set_text_content("c4!");
        Ok(())
    });
    read(&editor, || {
        let host = node(&host_key);
        assert_eq!(get_root().get_text_content(), host.get_text_content());
        assert!(host.get_text_content().starts_with("Title"));
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removing a slot drops its container and its text from the cache
#[test]
fn removing_a_slot_drops_its_container_and_its_text_from_the_cache() {
    let editor = create_slot_editor();
    let (host_key, title_key) = update_with(&editor, || {
        let host = title_and_body_host()?;
        let title = get_slot(&host, "title").unwrap();
        Ok((host.get_key().to_owned(), title.get_key().to_owned()))
    });
    let h = host_key.clone();
    update(&editor, move || remove_slot(&node(&h), "title").map(drop));
    // `hostDom.querySelector('[data-lexical-slot="title"]')`: the slot's
    // keyed DOM.
    assert!(!key_has_dom(&editor, &title_key));
    read(&editor, || {
        assert_eq!(node(&host_key).get_text_content(), "Body");
        assert_eq!(get_root().get_text_content(), "Body");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removing the host last child keeps its slot containers in the DOM
#[test]
fn removing_the_host_last_child_keeps_its_slot_containers_in_the_dom() {
    let editor = create_slot_editor();
    let (child_key, host_key, title_key) = update_with(&editor, || {
        let host = create_paragraph_node();
        let child = create_text_node("A");
        host.append(std::slice::from_ref(&child))?;
        let title = slot_container(&["TT"]);
        set_slot(&host, "title", &title)?;
        get_root().append(std::slice::from_ref(&host))?;
        Ok((
            child.get_key().to_owned(),
            host.get_key().to_owned(),
            title.get_key().to_owned(),
        ))
    });
    update(&editor, move || node(&child_key).remove(false));
    read(&editor, || {
        let host = node(&host_key);
        assert_eq!(names(&host), ["title"]);
        assert_eq!(host.get_text_content(), "TT");
    });
    assert!(key_has_dom(&editor, &title_key));
    assert_eq!(read(&editor, || node(&title_key).get_text_content()), "TT");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a document-wide RangeSelection carries slot text
#[test]
fn a_document_wide_range_selection_carries_slot_text() {
    let editor = create_slot_editor();
    update(&editor, || {
        let before = para(&[create_text_node("BEFORE")]);
        let host = create_paragraph_node();
        let slot = slot_container(&["SLOTTEXT"]);
        let after = para(&[create_text_node("AFTER")]);
        get_root().append(&[before, host.clone(), after])?;
        set_slot(&host, "title", &slot)?;
        Ok(())
    });
    let text = update_with(&editor, || {
        let root = get_root();
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, root.get_key(), 0, PointType::Element)?;
        selection.point_set(
            Which::Focus,
            root.get_key(),
            root.get_children_size(),
            PointType::Element,
        )?;
        set_selection(Some(selection.clone()));
        selection.get_text_content()
    });
    assert!(text.contains("SLOTTEXT"), "{text}");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > replace(includeChildren) does not transfer slots; they GC with the replaced host
#[test]
fn replace_include_children_does_not_transfer_slots() {
    let editor = create_slot_editor();
    let (new_host_key, old_host_key, slot_key) = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = slot_container(&["SLOTTEXT"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        let new_host = create_paragraph_node();
        host.replace(&new_host, true)?;
        assert_eq!(get_slot(&new_host, "title"), None);
        assert!(names(&new_host).is_empty());
        assert_eq!(get_slot(&host, "title"), Some(slot.clone()));
        assert!(!host.is_attached());
        Ok((
            new_host.get_key().to_owned(),
            host.get_key().to_owned(),
            slot.get_key().to_owned(),
        ))
    });
    read(&editor, || {
        assert!(get_node_by_key(&old_host_key).is_none());
        assert!(get_node_by_key(&slot_key).is_none());
        assert!(node(&new_host_key).is_attached());
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > replace on a decorator host does not transfer slots; they GC with it
#[test]
fn replace_on_a_decorator_host_does_not_transfer_slots() {
    let editor = create_slot_editor();
    let (old_host_key, slot_key) = update_with(&editor, || {
        let host = create_test_decorator_node(false);
        let slot = slot_container(&["SLOTTEXT"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "media", &slot)?;
        let new_host = create_test_decorator_node(false);
        host.replace(&new_host, false)?;
        assert_eq!(get_slot(&new_host, "media"), None);
        assert_eq!(get_slot(&host, "media"), Some(slot.clone()));
        assert!(!host.is_attached());
        Ok((host.get_key().to_owned(), slot.get_key().to_owned()))
    });
    read(&editor, || {
        assert!(get_node_by_key(&old_host_key).is_none());
        assert!(get_node_by_key(&slot_key).is_none());
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a replaced host reattached in the same update keeps its slots (wrap pattern)
#[test]
fn a_replaced_host_reattached_in_the_same_update_keeps_its_slots() {
    let editor = create_slot_editor();
    let (host_key, slot_key) = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = slot_container(&["SLOTTEXT"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        let wrapper = create_test_shadow_root_node();
        host.replace(&wrapper, false)?;
        wrapper.append(std::slice::from_ref(&host))?;
        assert_eq!(get_slot(&host, "title"), Some(slot.clone()));
        assert!(names(&wrapper).is_empty());
        Ok((host.get_key().to_owned(), slot.get_key().to_owned()))
    });
    read(&editor, || {
        let host = node(&host_key);
        assert!(host.is_attached());
        let slot = get_slot(&host, "title").expect("a slot");
        assert_eq!(slot.get_key(), slot_key);
        assert_eq!(slot.get_text_content(), "SLOTTEXT");
        assert_eq!(get_slot_host(&slot), Some(host));
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removing an element host clears its slot subtree from the DOM map
#[test]
fn removing_an_element_host_clears_its_slot_subtree_from_the_dom_map() {
    let editor = create_slot_editor();
    let (host_key, slot_key, inner_key) = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = slot_container(&["SLOTTEXT"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        Ok((
            host.get_key().to_owned(),
            slot.get_key().to_owned(),
            slot.get_first_child_or_throw()?.get_key().to_owned(),
        ))
    });
    assert!(key_has_dom(&editor, &slot_key));
    update(&editor, move || node(&host_key).remove(false));
    assert!(!key_has_dom(&editor, &slot_key));
    assert!(!key_has_dom(&editor, &inner_key));
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removing a decorator host clears its slot subtree from the DOM map
#[test]
fn removing_a_decorator_host_clears_its_slot_subtree_from_the_dom_map() {
    let editor = create_slot_editor();
    let (host_key, slot_key) = update_with(&editor, || {
        let host = create_test_decorator_node(false);
        let slot = slot_container(&["SLOTTEXT"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "media", &slot)?;
        Ok((host.get_key().to_owned(), slot.get_key().to_owned()))
    });
    assert!(key_has_dom(&editor, &slot_key));
    update(&editor, move || node(&host_key).remove(false));
    assert!(!key_has_dom(&editor, &slot_key));
}

/// A host paragraph with a `title` slot holding `SLOTTEXT`; the slot key.
fn slottext_host() -> Result<String, LexicalError> {
    let host = create_paragraph_node();
    let slot = slot_container(&["SLOTTEXT"]);
    get_root().append(std::slice::from_ref(&host))?;
    set_slot(&host, "title", &slot)?;
    Ok(slot.get_key().to_owned())
}

fn first_host_title_text(editor: &Editor) -> Option<String> {
    read(editor, || {
        let host = get_root().get_first_child().unwrap();
        assert_eq!(host.get_type(), "paragraph");
        get_slot(&host, "title").map(|slot| slot.get_text_content())
    })
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > NodeSelection.insertNodes leaves a slotted node intact
#[test]
fn node_selection_insert_nodes_leaves_a_slotted_node_intact() {
    let editor = create_slot_editor();
    let slot_key = update_with(&editor, slottext_host);
    let caught = update_with(&editor, move || {
        let ns = create_node_selection();
        ns.add(&slot_key);
        set_selection(Some(ns.clone()));
        Ok(throws(|| ns.insert_nodes(vec![create_paragraph_node()])))
    });
    assert_eq!(caught, None);
    assert_eq!(first_host_title_text(&editor).as_deref(), Some("SLOTTEXT"));
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > NodeSelection.deleteNodes leaves a slotted node intact
#[test]
fn node_selection_delete_nodes_leaves_a_slotted_node_intact() {
    let editor = create_slot_editor();
    let slot_key = update_with(&editor, slottext_host);
    let caught = update_with(&editor, move || {
        let ns = create_node_selection();
        ns.add(&slot_key);
        set_selection(Some(ns.clone()));
        Ok(throws(|| ns.delete_nodes()))
    });
    assert_eq!(caught, None);
    assert_eq!(first_host_title_text(&editor).as_deref(), Some("SLOTTEXT"));
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removeText inside a slot stays scoped and does not walk past the slot boundary
#[test]
fn remove_text_inside_a_slot_stays_scoped() {
    let editor = create_slot_editor();
    let text_key = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = create_test_shadow_root_node();
        let text = create_text_node("SLOTTEXT");
        slot.append(&[para(std::slice::from_ref(&text))])?;
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        Ok(text.get_key().to_owned())
    });
    let caught = update_with(&editor, move || {
        assert!(node(&text_key).is_text());
        let sel = create_range_selection();
        sel.point_set(Which::Anchor, &text_key, 0, PointType::Text)?;
        sel.point_set(Which::Focus, &text_key, 4, PointType::Text)?;
        set_selection(Some(sel.clone()));
        Ok(throws(|| sel.remove_text()))
    });
    assert_eq!(caught, None);
    assert_eq!(first_host_title_text(&editor).as_deref(), Some("TEXT"));
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > backspace after a slot-bearing decorator host removes it (#8904)
#[test]
fn backspace_after_a_slot_bearing_decorator_host_removes_it() {
    let editor = create_slot_editor();
    let (host_key, text_key) = update_with(&editor, || {
        let host = create_test_decorator_node(false);
        set_slot(&host, "quote", &slot_container(&["QUOTE"]))?;
        let text = create_text_node("after");
        get_root().append(&[host.clone(), para(std::slice::from_ref(&text))])?;
        Ok((host.get_key().to_owned(), text.get_key().to_owned()))
    });
    update(&editor, move || {
        caret_at(&text_key, 0, PointType::Text).delete_character(true)
    });
    read(&editor, || {
        assert!(get_node_by_key(&host_key).is_none());
        assert_eq!(get_root().get_children_size(), 1);
        assert_eq!(get_root().get_text_content(), "after");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > backspace in an empty paragraph after a slot-bearing decorator host selects it (#8904)
#[test]
fn backspace_in_an_empty_paragraph_after_a_slot_bearing_decorator_host_selects_it() {
    let editor = create_slot_editor();
    let (host_key, paragraph_key) = update_with(&editor, || {
        let host = create_test_decorator_node(false);
        set_slot(&host, "quote", &slot_container(&["QUOTE"]))?;
        let paragraph = create_paragraph_node();
        get_root().append(&[host.clone(), paragraph.clone()])?;
        Ok((host.get_key().to_owned(), paragraph.get_key().to_owned()))
    });
    let p = paragraph_key.clone();
    update(&editor, move || {
        caret_at(&p, 0, PointType::Element).delete_character(true)
    });
    read(&editor, || {
        assert!(get_node_by_key(&paragraph_key).is_none());
        assert!(get_node_by_key(&host_key).is_some());
        let sel = get_selection().unwrap();
        assert!(sel.is_node());
        let keys: Vec<String> = sel
            .get_nodes()
            .unwrap()
            .iter()
            .map(|n| n.get_key().to_owned())
            .collect();
        assert_eq!(keys, std::slice::from_ref(&host_key));
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > backspace at the start of a host child does not merge the slot-bearing host away
#[test]
fn backspace_at_the_start_of_a_host_child_does_not_merge_the_slot_bearing_host_away() {
    let editor = create_slot_editor();
    let (host_key, child_key) = update_with(&editor, || {
        get_root().append(&[para(&[create_text_node("AAA")])])?;
        let host = create_paragraph_node();
        set_slot(&host, "title", &slot_container(&["TITLE"]))?;
        let child = create_text_node("BBB");
        host.append(std::slice::from_ref(&child))?;
        get_root().append(std::slice::from_ref(&host))?;
        Ok((host.get_key().to_owned(), child.get_key().to_owned()))
    });
    let c = child_key.clone();
    update(&editor, move || {
        caret_at(&c, 0, PointType::Text).delete_character(true)
    });
    read(&editor, || {
        let host = node(&host_key);
        assert_eq!(get_root().get_children_size(), 2);
        assert_eq!(names(&host), ["title"]);
        assert_eq!(
            get_slot(&host, "title").unwrap().get_text_content(),
            "TITLE"
        );
        assert_eq!(host.get_text_content(), "TITLEBBB");
        let sel = get_selection().unwrap();
        assert!(sel.is_range());
        let anchor = range_point(&sel, Which::Anchor);
        assert_eq!(anchor.key, child_key);
        assert_eq!(anchor.offset, 0);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getParentCaret stops at a slot value in every mode
#[test]
fn get_parent_caret_stops_at_a_slot_value_in_every_mode() {
    let editor = create_slot_editor();
    let slot_key = update_with(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot_container(&["TITLE"]))?;
        Ok(get_slot(&host, "title").unwrap().get_key().to_owned())
    });
    read(&editor, || {
        let slot = node(&slot_key);
        let child_caret = get_child_caret(&slot, CaretDirection::Next).unwrap();
        assert!(child_caret.get_parent_caret(RootMode::Root).is_none());
        assert!(child_caret.get_parent_caret(RootMode::ShadowRoot).is_none());
    });
}

fn frame_key(editor: &Editor) -> Option<String> {
    read(editor, || {
        get_selection_slot_frame(get_selection().as_ref())
            .unwrap()
            .map(|n| n.get_key().to_owned())
    })
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns null for a null selection
#[test]
fn get_selection_slot_frame_returns_null_for_a_null_selection() {
    let editor = create_slot_editor();
    assert_eq!(
        read(&editor, || get_selection_slot_frame(None).unwrap()),
        None
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns null for a selection outside any slot
#[test]
fn get_selection_slot_frame_returns_null_for_a_selection_outside_any_slot() {
    let editor = create_slot_editor();
    update(&editor, || {
        let text = create_text_node("plain");
        get_root().append(&[para(std::slice::from_ref(&text))])?;
        text.select(Some(0), Some(5)).map(drop)
    });
    assert_eq!(frame_key(&editor), None);
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns the slot frame for a RangeSelection inside a slot
#[test]
fn get_selection_slot_frame_returns_the_slot_frame_for_a_range_selection_inside_a_slot() {
    let editor = create_slot_editor();
    let container_key = update_with(&editor, || {
        let host = create_paragraph_node();
        let container = slot_container(&["inside"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &container)?;
        let first = container.get_first_child_or_throw()?;
        assert_eq!(first.get_type(), "paragraph");
        first.select_start()?;
        Ok(container.get_key().to_owned())
    });
    assert_eq!(frame_key(&editor), Some(container_key));
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns the slot frame for a NodeSelection inside a slot
#[test]
fn get_selection_slot_frame_returns_the_slot_frame_for_a_node_selection_inside_a_slot() {
    let editor = create_slot_editor();
    let container_key = update_with(&editor, || {
        let host = create_paragraph_node();
        let container = slot_container(&["inside"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &container)?;
        let node_selection = create_node_selection();
        node_selection.add(container.get_first_child_or_throw()?.get_key());
        set_selection(Some(node_selection));
        Ok(container.get_key().to_owned())
    });
    assert_eq!(frame_key(&editor), Some(container_key));
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns null for an empty NodeSelection
#[test]
fn get_selection_slot_frame_returns_null_for_an_empty_node_selection() {
    let editor = create_slot_editor();
    update(&editor, || {
        get_root().append(&[create_paragraph_node()])?;
        set_selection(Some(create_node_selection()));
        Ok(())
    });
    assert_eq!(frame_key(&editor), None);
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > returns the slot name for a node sitting in a named slot
#[test]
fn get_slot_name_within_host_returns_the_slot_name_for_a_node_sitting_in_a_named_slot() {
    let editor = create_slot_editor();
    let title_key = update_with(&editor, || {
        let host = create_paragraph_node();
        let title = slot_container(&[]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &title)?;
        Ok(title.get_key().to_owned())
    });
    assert_eq!(
        read(&editor, || get_slot_name_within_host(&node(&title_key))).as_deref(),
        Some("title")
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > disambiguates between multiple slots on the same host
#[test]
fn get_slot_name_within_host_disambiguates_between_multiple_slots_on_the_same_host() {
    let editor = create_slot_editor();
    let (title_key, body_key) = update_with(&editor, || {
        let host = create_paragraph_node();
        let title = slot_container(&[]);
        let body = slot_container(&[]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &title)?;
        set_slot(&host, "body", &body)?;
        Ok((title.get_key().to_owned(), body.get_key().to_owned()))
    });
    read(&editor, || {
        assert_eq!(
            get_slot_name_within_host(&node(&title_key)).as_deref(),
            Some("title")
        );
        assert_eq!(
            get_slot_name_within_host(&node(&body_key)).as_deref(),
            Some("body")
        );
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > returns null for a regular child (not a slot value)
#[test]
fn get_slot_name_within_host_returns_null_for_a_regular_child() {
    let editor = create_slot_editor();
    let child_key = update_with(&editor, || {
        let host = create_paragraph_node();
        let child = create_text_node("plain");
        host.append(std::slice::from_ref(&child))?;
        get_root().append(&[host])?;
        Ok(child.get_key().to_owned())
    });
    assert_eq!(
        read(&editor, || get_slot_name_within_host(&node(&child_key))),
        None
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > returns null for the host itself
#[test]
fn get_slot_name_within_host_returns_null_for_the_host_itself() {
    let editor = create_slot_editor();
    let host_key = update_with(&editor, || {
        let host = create_paragraph_node();
        set_slot(&host, "title", &slot_container(&[]))?;
        get_root().append(std::slice::from_ref(&host))?;
        Ok(host.get_key().to_owned())
    });
    assert_eq!(
        read(&editor, || get_slot_name_within_host(&node(&host_key))),
        None
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > returns the immediate slot name when slots are nested
#[test]
fn get_slot_name_within_host_returns_the_immediate_slot_name_when_slots_are_nested() {
    let editor = create_slot_editor();
    let inner_slot_key = update_with(&editor, || {
        let outer_host = create_paragraph_node();
        let inner_host = slot_container(&[]);
        let inner_slot = slot_container(&[]);
        get_root().append(std::slice::from_ref(&outer_host))?;
        set_slot(&outer_host, "outer", &inner_host)?;
        set_slot(&inner_host, "inner", &inner_slot)?;
        Ok(inner_slot.get_key().to_owned())
    });
    assert_eq!(
        read(&editor, || get_slot_name_within_host(&node(
            &inner_slot_key
        )))
        .as_deref(),
        Some("inner")
    );
}

/// `selection.removeText()` or `selection[kind](true)`.
fn delete_with(selection: &SelectionRef, kind: &str) -> Result<(), LexicalError> {
    match kind {
        "removeText" => selection.remove_text(),
        "deleteCharacter" => selection.delete_character(true),
        "deleteWord" => selection.delete_word(true),
        "deleteLine" => selection.delete_line(true),
        _ => unreachable!(),
    }
}

/// `deleting a %s selection inside a root slot keeps the selection in that
/// slot`.
fn deleting_inside_a_root_slot(kind: &'static str) {
    let editor = create_slot_editor();
    update(&editor, move || {
        let root = get_root();
        let slot = slot_container(&["Footnote"]);
        set_slot(&root, "footnotes", &slot)?;
        if kind == "node" {
            let selection = create_node_selection();
            selection.add(slot.get_first_child_or_throw()?.get_key());
            set_selection(Some(selection.clone()));
            selection.delete_nodes()?;
        } else {
            let selection = select_all_in(&slot.select_start()?)?;
            delete_with(&selection, kind)?;
        }
        assert_eq!(
            get_selection_slot_frame(get_selection().as_ref())?,
            Some(slot)
        );
        assert_eq!(root.get_children_size(), 0);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a deleteCharacter selection inside a root slot keeps the selection in that slot
#[test]
fn deleting_a_delete_character_selection_inside_a_root_slot() {
    deleting_inside_a_root_slot("deleteCharacter");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a deleteWord selection inside a root slot keeps the selection in that slot
#[test]
fn deleting_a_delete_word_selection_inside_a_root_slot() {
    deleting_inside_a_root_slot("deleteWord");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a deleteLine selection inside a root slot keeps the selection in that slot
#[test]
fn deleting_a_delete_line_selection_inside_a_root_slot() {
    deleting_inside_a_root_slot("deleteLine");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a removeText selection inside a root slot keeps the selection in that slot
#[test]
fn deleting_a_remove_text_selection_inside_a_root_slot() {
    deleting_inside_a_root_slot("removeText");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a node selection inside a root slot keeps the selection in that slot
#[test]
fn deleting_a_node_selection_inside_a_root_slot() {
    deleting_inside_a_root_slot("node");
}

/// `deleting all children with a %s selection restores a paragraph before
/// root slots`.
fn deleting_all_children_restores_a_paragraph(kind: &'static str) {
    let editor = create_slot_editor();
    update(&editor, move || {
        let root = get_root();
        let paragraph = para(&[create_text_node("Body")]);
        root.append(std::slice::from_ref(&paragraph))?;
        let slot = slot_container(&["Footnote"]);
        set_slot(&root, "footnotes", &slot)?;
        if kind == "node" {
            let selection = create_node_selection();
            selection.add(paragraph.get_key());
            set_selection(Some(selection.clone()));
            selection.delete_nodes()?;
        } else {
            delete_with(&select_all()?, kind)?;
        }
        assert_eq!(root.get_children_size(), 1);
        let replacement = root.get_first_child().unwrap();
        assert_eq!(replacement.get_type(), "paragraph");
        assert!(replacement.is_empty());
        assert_eq!(get_slot(&root, "footnotes"), Some(slot.clone()));
        assert_eq!(slot.get_text_content(), "Footnote");
        let selection = get_selection().unwrap();
        assert!(selection.is_range());
        assert!(selection.range().unwrap().is_collapsed());
        assert_eq!(
            range_point(&selection, Which::Anchor).get_node()?,
            replacement
        );
        selection.insert_text("Replacement")?;
        assert_eq!(replacement.get_text_content(), "Replacement");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a deleteCharacter selection restores a paragraph before root slots
#[test]
fn deleting_all_children_with_a_delete_character_selection() {
    deleting_all_children_restores_a_paragraph("deleteCharacter");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a deleteWord selection restores a paragraph before root slots
#[test]
fn deleting_all_children_with_a_delete_word_selection() {
    deleting_all_children_restores_a_paragraph("deleteWord");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a deleteLine selection restores a paragraph before root slots
#[test]
fn deleting_all_children_with_a_delete_line_selection() {
    deleting_all_children_restores_a_paragraph("deleteLine");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a removeText selection restores a paragraph before root slots
#[test]
fn deleting_all_children_with_a_remove_text_selection() {
    deleting_all_children_restores_a_paragraph("removeText");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a node selection restores a paragraph before root slots
#[test]
fn deleting_all_children_with_a_node_selection() {
    deleting_all_children_restores_a_paragraph("node");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > $selectAll does not throw when anchor is the slot value root
#[test]
fn select_all_does_not_throw_when_anchor_is_the_slot_value_root() {
    let editor = create_slot_editor();
    let slot_key = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = slot_container(&["Title"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        Ok(slot.get_key().to_owned())
    });
    update(&editor, move || {
        caret_at(&slot_key, 0, PointType::Element);
        Ok(())
    });
    update(&editor, || {
        let selection = get_selection().unwrap();
        assert!(selection.is_range());
        assert_eq!(throws(|| select_all_in(&selection)), None);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > $selectAll does not throw when anchor is the root element
#[test]
fn select_all_does_not_throw_when_anchor_is_the_root_element() {
    let editor = create_slot_editor();
    update(&editor, || {
        let selection = get_root().select(Some(0), Some(0))?;
        assert_eq!(throws(|| select_all_in(&selection)), None);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > setEditorState latches _slotsUsed when the state contains slots
#[test]
fn set_editor_state_latches_slots_used_when_the_state_contains_slots() {
    let editor_a = create_slot_editor();
    let editor_b = create_slot_editor();
    update(&editor_a, || {
        let host = create_paragraph_node();
        set_slot(&host, "title", &slot_container(&["Hello"]))?;
        get_root().append(&[host])
    });
    assert!(editor_a.0.slots_used.get());
    let state_a = editor_a.get_editor_state();
    editor_b.set_editor_state(&state_a, None).unwrap();
    assert!(editor_b.0.slots_used.get());
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > _slotsUsed stays true after the last slot is removed
#[test]
fn slots_used_stays_true_after_the_last_slot_is_removed() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        set_slot(&host, "title", &slot_container(&["Hello"]))?;
        get_root().append(&[host])
    });
    assert!(editor.0.slots_used.get());
    update(&editor, || {
        if let Some(host) = get_root().get_first_child().filter(is_slot_host) {
            remove_slot(&host, "title")?;
        }
        Ok(())
    });
    assert!(editor.0.slots_used.get());
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: audit hardening (insertNodes, cycles, idempotent setSlot) > insertNodes with the caret on an empty slot value inserts into the slot
#[test]
fn insert_nodes_with_the_caret_on_an_empty_slot_value_inserts_into_the_slot() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        let slot_value = create_test_shadow_root_node();
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot_value)?;
        slot_value
            .select(None, None)?
            .insert_nodes(vec![para(&[create_text_node("pasted")])])
    });
    read(&editor, || {
        let host = get_root().get_first_child().unwrap();
        assert_eq!(host.get_type(), "paragraph");
        let slot_value = get_slot(&host, "title").unwrap();
        assert_eq!(slot_value.get_text_content(), "pasted");
        assert_eq!(host.get_children_size(), 0);
        assert_eq!(get_root().get_children_size(), 1);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: audit hardening (insertNodes, cycles, idempotent setSlot) > closing a cycle through the children channel throws instead of hanging
#[test]
fn closing_a_cycle_through_the_children_channel_throws_instead_of_hanging() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let title = slot_container(&["Title"]);
        set_slot(&host, "title", &title)?;
        throws_matching(|| title.append(std::slice::from_ref(&host)), "cycle");
        let inner = title.get_first_child().unwrap();
        assert_eq!(inner.get_type(), "paragraph");
        throws_matching(|| inner.append(std::slice::from_ref(&host)), "cycle");
        let inner_text = inner.get_first_child().unwrap();
        throws_matching(|| inner_text.insert_before(&host, true), "cycle");
        throws_matching(|| inner_text.insert_after(&host, true), "cycle");
        throws_matching(|| inner_text.replace(&host, false), "cycle");
        assert_eq!(get_slot(&host, "title"), Some(title.clone()));
        assert_eq!(get_slot_host(&title), Some(host.clone()));
        assert!(host.get_parent().is_some());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: audit hardening (insertNodes, cycles, idempotent setSlot) > re-setting the same node into the same slot is a no-op
#[test]
fn re_setting_the_same_node_into_the_same_slot_is_a_no_op() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let title = slot_container(&["Title"]);
        set_slot(&host, "title", &title)?;
        assert_eq!(throws(|| set_slot(&host, "title", &title)), None);
        assert_eq!(get_slot(&host, "title"), Some(title.clone()));
        assert_eq!(names(&host), ["title"]);
        assert_eq!(get_slot_name_within_host(&title).as_deref(), Some("title"));
        Ok(())
    });
}

/// A host of class `T` with the slots `(name, text)` set in order.
fn host_with_slots<T: LexicalNode + Default>(
    slots: &'static [(&'static str, &'static str)],
) -> impl FnOnce() -> Result<String, LexicalError> {
    move || {
        let host = create::<T>();
        get_root().append(std::slice::from_ref(&host))?;
        for (name, text) in slots {
            set_slot(&host, name, &slot_container(&[text]))?;
        }
        Ok(host.get_key().to_owned())
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > declared slots set in reverse call order come back in declared order
#[test]
fn declared_slots_set_in_reverse_call_order_come_back_in_declared_order() {
    let editor = create_slot_editor();
    let host_key = update_with(
        &editor,
        host_with_slots::<DeclaredHostNode>(&[("body", "Body"), ("title", "Title")]),
    );
    read(&editor, || {
        let host = node(&host_key);
        assert_eq!(names(&host), ["title", "body"]);
        assert_eq!(host.get_text_content(), "TitleBody");
        assert_eq!(host.get_text_content_size(), 9);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > mixed declared and undeclared names sort declared-first, then code-unit
#[test]
fn mixed_declared_and_undeclared_names_sort_declared_first_then_code_unit() {
    let editor = create_slot_editor();
    let host_key = update_with(
        &editor,
        host_with_slots::<DeclaredHostNode>(&[
            ("zeta", "Z"),
            ("body", "B"),
            ("alpha", "A"),
            ("title", "T"),
        ]),
    );
    read(&editor, || {
        let host = node(&host_key);
        assert_eq!(names(&host), ["title", "body", "alpha", "zeta"]);
        assert_eq!(host.get_text_content(), "TBAZ");
        assert_eq!(host.get_text_content_size(), 4);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > an undeclared host orders slot names in code-unit order
#[test]
fn an_undeclared_host_orders_slot_names_in_code_unit_order() {
    let editor = create_slot_editor();
    let host_key = update_with(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "b", &slot_container(&["1"]))?;
        set_slot(&host, "A", &slot_container(&["2"]))?;
        set_slot(&host, "a", &slot_container(&["3"]))?;
        Ok(host.get_key().to_owned())
    });
    assert_eq!(read(&editor, || names(&node(&host_key))), ["A", "a", "b"]);
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > a subclass redeclaration overrides the inherited order
#[test]
fn a_subclass_redeclaration_overrides_the_inherited_order() {
    let editor = create_slot_editor();
    let host_key = update_with(
        &editor,
        host_with_slots::<ReorderedHostNode>(&[("title", "Title"), ("body", "Body")]),
    );
    assert_eq!(read(&editor, || names(&node(&host_key))), ["body", "title"]);
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > exportJSON emits slots keys in canonical order
#[test]
fn export_json_emits_slots_keys_in_canonical_order() {
    let editor = create_slot_editor();
    update_with(
        &editor,
        host_with_slots::<DeclaredHostNode>(&[
            ("body", "Body"),
            ("alpha", "Alpha"),
            ("title", "Title"),
        ]),
    );
    let json = editor.get_editor_state().to_json(false);
    let host_json = &json
        .get("root")
        .unwrap()
        .get("children")
        .unwrap()
        .as_array()
        .unwrap()[0];
    let slots = host_json
        .get("$slots")
        .expect("$slots")
        .as_object()
        .unwrap();
    let keys: Vec<&str> = slots.iter().map(|(k, _)| k).collect();
    assert_eq!(keys, ["title", "body", "alpha"]);
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > a duplicate name in a declaration throws when the rank is first computed
#[test]
fn a_duplicate_name_in_a_declaration_throws_when_the_rank_is_first_computed() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create::<DupDeclaredHostNode>();
        get_root().append(std::slice::from_ref(&host))?;
        assert_eq!(
            throws(|| set_slot(&host, "a", &slot_container(&["A"]))),
            None
        );
        throws_matching(
            || set_slot(&host, "b", &slot_container(&["B"])),
            "more than once",
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > a reserved name in a declaration throws when the rank is first computed
#[test]
fn a_reserved_name_in_a_declaration_throws_when_the_rank_is_first_computed() {
    let editor = create_slot_editor();
    update(&editor, || {
        let host = create::<ReservedDeclaredHostNode>();
        get_root().append(std::slice::from_ref(&host))?;
        assert_eq!(
            throws(|| set_slot(&host, "a", &slot_container(&["A"]))),
            None
        );
        throws_matching(
            || set_slot(&host, "b", &slot_container(&["B"])),
            "reserved slot name",
        );
        Ok(())
    });
}

/// `host.__slots`, in the state being read.
fn slots_of(key: &str) -> Option<Rc<crate::slot::SlotMap>> {
    node(key).get_latest().base().slots.clone()
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: copy-on-write slot map > a host version cloned without slot changes shares the slot map
#[test]
fn a_host_version_cloned_without_slot_changes_shares_the_slot_map() {
    let editor = create_slot_editor();
    let host_key = update_with(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot_container(&["Title"]))?;
        Ok(host.get_key().to_owned())
    });
    let map_before = editor.get_editor_state().read(|| slots_of(&host_key));
    assert!(map_before.is_some());
    let h = host_key.clone();
    update(&editor, move || {
        node(&h).set_element_style("color: red");
        Ok(())
    });
    let map_after = editor.get_editor_state().read(|| slots_of(&host_key));
    assert!(Rc::ptr_eq(
        map_after.as_ref().unwrap(),
        map_before.as_ref().unwrap()
    ));
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: copy-on-write slot map > a slot mutation clones the map once per version and leaves prior versions intact
#[test]
fn a_slot_mutation_clones_the_map_once_per_version_and_leaves_prior_versions_intact() {
    let editor = create_slot_editor();
    let host_key = update_with(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "a", &slot_container(&["A"]))?;
        Ok(host.get_key().to_owned())
    });
    let state_a = editor.get_editor_state();
    let map_a = state_a.read(|| slots_of(&host_key)).unwrap();
    let h = host_key.clone();
    let (first, second) = update_with(&editor, move || {
        let host = node(&h);
        set_slot(&host, "b", &slot_container(&["B"]))?;
        // The maps' identities: holding an `Rc` would itself make the next
        // write copy the map (`Rc::make_mut`).
        let first = Rc::as_ptr(&slots_of(&h).unwrap()) as usize;
        set_slot(&host, "c", &slot_container(&["C"]))?;
        let second = Rc::as_ptr(&slots_of(&h).unwrap()) as usize;
        Ok((first, second))
    });
    assert_ne!(first, Rc::as_ptr(&map_a) as usize);
    assert_eq!(second, first);
    assert_eq!(state_a.read(|| names(&node(&host_key))), ["a"]);
    assert!(Rc::ptr_eq(
        &state_a.read(|| slots_of(&host_key)).unwrap(),
        &map_a
    ));
    assert_eq!(map_a.len(), 1);
}

/// `$createLineSlotHost()`: a host paragraph `body` with a paragraph
/// `Title` as its `title` slot.
fn create_line_slot_host() -> Result<(NodeRef, NodeRef), LexicalError> {
    let host = para(&[create_text_node("body")]);
    get_root().append(std::slice::from_ref(&host))?;
    let line = para(&[create_text_node("Title")]);
    set_slot(&host, "title", &line)?;
    Ok((host, line))
}

fn first_text_of(line: &NodeRef) -> NodeRef {
    let text = line.get_first_child().unwrap();
    assert!(text.is_text());
    text
}

fn line_break_count(line: &NodeRef) -> usize {
    line.get_children()
        .iter()
        .filter(|c| c.get_type() == "linebreak")
        .count()
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > typing and inline insertNodes land inside the value
#[test]
fn typing_and_inline_insert_nodes_land_inside_the_value() {
    let editor = create_slot_editor();
    let line = update_with(&editor, || {
        let (_, line) = create_line_slot_host()?;
        let selection = first_text_of(&line).select(Some(5), Some(5))?;
        selection.insert_text("!")?;
        selection.insert_nodes(vec![create_text_node("?")])?;
        Ok(line)
    });
    read(&editor, || {
        assert_eq!(line.get_text_content(), "Title!?");
        assert_eq!(get_root().get_children_size(), 1);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > Enter inside the value is a no-op (single-block scope)
#[test]
fn enter_inside_the_value_is_a_no_op() {
    let editor = create_slot_editor();
    let (host, line) = update_with(&editor, || {
        let (host, line) = create_line_slot_host()?;
        assert_eq!(
            first_text_of(&line)
                .select(Some(2), Some(2))?
                .insert_paragraph()?,
            None
        );
        Ok((host, line))
    });
    read(&editor, || {
        assert_eq!(line.get_text_content(), "Title");
        assert_eq!(get_slot_host(&line), Some(host.clone()));
        assert_eq!(get_root().get_children_size(), 1);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > multi-block paste flattens to inline content like an <input>
#[test]
fn multi_block_paste_flattens_to_inline_content() {
    let editor = create_slot_editor();
    let line = update_with(&editor, || {
        let (_, line) = create_line_slot_host()?;
        first_text_of(&line)
            .select(Some(5), Some(5))?
            .insert_nodes(vec![
                para(&[
                    create_text_node("A"),
                    create_line_break_node(),
                    create_text_node("B"),
                ]),
                para(&[create_text_node("C")]),
                create_test_decorator_node(false),
            ])?;
        Ok(line)
    });
    read(&editor, || {
        assert_eq!(line.get_text_content(), "TitleABC");
        assert_eq!(get_root().get_children_size(), 1);
        assert_eq!(get_root().get_text_content(), "TitleABCbody");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > paste into an EMPTY block value inserts without seeding a paragraph
#[test]
fn paste_into_an_empty_block_value_inserts_without_seeding_a_paragraph() {
    let editor = create_slot_editor();
    let line = update_with(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let line = create_paragraph_node();
        set_slot(&host, "title", &line)?;
        line.select(None, None)?
            .insert_nodes(vec![para(&[create_text_node("pasted")])])?;
        Ok(line)
    });
    read(&editor, || {
        assert_eq!(line.get_text_content(), "pasted");
        assert!(line.get_first_child().unwrap().is_text());
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > repeated linebreaks into a bare block value stay in insertion order (#8897)
#[test]
fn repeated_linebreaks_into_a_bare_block_value_stay_in_insertion_order() {
    let editor = create_slot_editor();
    let line = update_with(&editor, || {
        let (_, line) = create_line_slot_host()?;
        first_text_of(&line)
            .select(Some(5), Some(5))?
            .insert_line_break(false)?;
        Ok(line)
    });
    let l = line.clone();
    update(&editor, move || {
        for _ in 0..3 {
            l.get_last_child()
                .unwrap()
                .select_end()?
                .insert_line_break(false)?;
        }
        Ok(())
    });
    read(&editor, || {
        assert_eq!(line.get_children()[0].get_text_content(), "Title");
        assert_eq!(line_break_count(&line), 4);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > insertNodes at the end of a bare block value inserts at the end, not the start
#[test]
fn insert_nodes_at_the_end_of_a_bare_block_value_inserts_at_the_end() {
    let editor = create_slot_editor();
    let line = update_with(&editor, || Ok(create_line_slot_host()?.1));
    let l = line.clone();
    update(&editor, move || {
        let size = l.get_children_size();
        l.select(Some(size), Some(size))?
            .insert_nodes(vec![create_text_node("!")])
    });
    assert_eq!(read(&editor, || line.get_text_content()), "Title!");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > backspace deletes trailing linebreaks in a bare block value one at a time (#8898)
#[test]
fn backspace_deletes_trailing_linebreaks_in_a_bare_block_value_one_at_a_time() {
    let editor = create_slot_editor();
    let line = update_with(&editor, || {
        let (_, line) = create_line_slot_host()?;
        first_text_of(&line)
            .select(Some(5), Some(5))?
            .insert_line_break(false)?;
        line.get_last_child()
            .unwrap()
            .select_end()?
            .insert_line_break(false)?;
        Ok(line)
    });
    assert_eq!(read(&editor, || line_break_count(&line)), 2);
    for expected in [1, 0] {
        let l = line.clone();
        update(&editor, move || {
            l.get_last_child()
                .unwrap()
                .select_end()?
                .delete_character(true)
        });
        assert_eq!(read(&editor, || line_break_count(&line)), expected);
    }
    assert_eq!(read(&editor, || line.get_text_content()), "Title");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > the boundary still scopes selection and select-all
#[test]
fn the_boundary_still_scopes_selection_and_select_all() {
    let editor = create_slot_editor();
    let line = update_with(&editor, || {
        let (_, line) = create_line_slot_host()?;
        let text = first_text_of(&line);
        assert_eq!(text.get_top_level_element()?, Some(line.clone()));
        let scoped = select_all_in(&text.select(Some(2), Some(2))?)?;
        assert_eq!(scoped.get_text_content()?, "Title");
        let anchor = range_point(&scoped, Which::Anchor).get_node()?;
        let focus = range_point(&scoped, Which::Focus).get_node()?;
        assert_eq!(get_slot_frame(&anchor), Some(line.clone()));
        assert_eq!(get_slot_frame(&focus), Some(line.clone()));
        Ok(line)
    });
    read(&editor, || {
        assert_eq!(get_slot_frame(&line), Some(line.clone()))
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > $getNearestRootOrShadowRoot stops at the slot value
#[test]
fn get_nearest_root_or_shadow_root_stops_at_the_slot_value() {
    let editor = create_slot_editor();
    update(&editor, || {
        let (_, line) = create_line_slot_host()?;
        let text = line.get_first_child().unwrap();
        assert_eq!(get_nearest_root_or_shadow_root(&text)?, line);
        assert_eq!(get_nearest_root_or_shadow_root(&line)?, line);
        let host2 = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host2))?;
        let container = slot_container(&["inside"]);
        set_slot(&host2, "media", &container)?;
        let inner_paragraph = container.get_first_child().unwrap();
        assert_eq!(inner_paragraph.get_type(), "paragraph");
        assert_eq!(
            get_nearest_root_or_shadow_root(&inner_paragraph)?,
            container
        );
        assert_eq!(get_nearest_root_or_shadow_root(&container)?, container);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > backspace at the start of the value stays inside the boundary
#[test]
fn backspace_at_the_start_of_the_value_stays_inside_the_boundary() {
    let editor = create_slot_editor();
    let line = update_with(&editor, || Ok(create_line_slot_host()?.1));
    let l = line.clone();
    update(&editor, move || {
        first_text_of(&l)
            .select(Some(0), Some(0))?
            .delete_character(true)
    });
    read(&editor, || {
        assert_eq!(line.get_text_content(), "Title");
        assert_eq!(get_root().get_text_content(), "Titlebody");
        assert_eq!(get_root().get_children_size(), 1);
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: hydrate-time normalize (#8712) > a slot value with a raw TextNode child gets a paragraph wrap on setEditorState
#[test]
fn a_slot_value_with_a_raw_text_node_child_gets_a_paragraph_wrap_on_set_editor_state() {
    let editor = create_slot_editor();
    let invalid = r#"{"root":{"children":[{"$slots":{"title":{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"raw","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"test_shadow_root","version":1}},"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;
    let parsed = editor.parse_editor_state(invalid).unwrap();
    editor.set_editor_state(&parsed, None).unwrap();
    read(&editor, || {
        let host = get_root().get_first_child().unwrap();
        assert_eq!(host.get_type(), "paragraph");
        let slot = get_slot(&host, "title").unwrap();
        assert!(slot.get_latest().is::<TestShadowRootNode>());
        let children = slot.get_children();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].get_type(), "paragraph");
        assert_eq!(children[0].get_text_content(), "raw");
    });
}

/// A host with a `title` slot holding a block decorator; the slot.
fn decorator_slot() -> Result<(NodeRef, NodeRef), LexicalError> {
    let host = create_paragraph_node();
    let slot = create_test_shadow_root_node();
    slot.append(&[create_test_decorator_node(false)])?;
    get_root().append(std::slice::from_ref(&host))?;
    set_slot(&host, "title", &slot)?;
    Ok((host, slot))
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: typing-path paragraph wrap (#8712) > element-mode caret before a decorator wraps the inserted text in a paragraph
#[test]
fn element_mode_caret_before_a_decorator_wraps_the_inserted_text_in_a_paragraph() {
    let editor = create_slot_editor();
    let (_, slot) = update_with(&editor, decorator_slot);
    let s = slot.clone();
    update(&editor, move || {
        assert!(s.is_element());
        s.select(Some(0), Some(0))?.insert_text("typed")
    });
    read(&editor, || {
        let first_child = slot.get_children()[0].clone();
        assert_eq!(first_child.get_type(), "paragraph");
        assert_eq!(first_child.get_text_content(), "typed");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: insertNodes redirect termination (#8712) > seeds a paragraph when the slot value starts with a decorator
#[test]
fn seeds_a_paragraph_when_the_slot_value_starts_with_a_decorator() {
    let editor = create_slot_editor();
    let (host, slot) = update_with(&editor, decorator_slot);
    let s = slot.clone();
    update(&editor, move || {
        s.select(Some(0), Some(0))?
            .insert_nodes(vec![create_text_node("inserted")])
    });
    read(&editor, || {
        assert!(slot.get_children().len() >= 2);
        assert!(slot.get_text_content().contains("inserted"));
        assert_eq!(get_slot(&host, "title"), Some(slot.clone()));
    });
}

/// `$appendHostWithSlot(parent, host)`: the slot's text node.
fn append_host_with_slot(parent: &NodeRef, host: &NodeRef) -> Result<NodeRef, LexicalError> {
    let text = create_text_node("C");
    parent.append(std::slice::from_ref(host))?;
    let value = create_test_shadow_root_node();
    value.append(&[para(std::slice::from_ref(&text))])?;
    set_slot(host, "title", &value)?;
    Ok(text)
}

fn edit_slot(editor: &Editor, text_key: &str, text: &'static str) {
    let key = text_key.to_owned();
    update(editor, move || {
        let node = node(&key);
        assert!(node.is_text());
        node.set_text_content(text);
        Ok(())
    });
}

fn root_text(editor: &Editor) -> String {
    read(editor, || get_root().get_text_content())
}

fn ab_cd_paragraph() -> Result<NodeRef, LexicalError> {
    let paragraph = para(&[
        create_text_node("ab"),
        create_line_break_node(),
        create_text_node("cd "),
    ]);
    get_root().append(std::slice::from_ref(&paragraph))?;
    Ok(paragraph)
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > an inline decorator host that ends a paragraph of four children
#[test]
fn an_inline_decorator_host_that_ends_a_paragraph_of_four_children() {
    let editor = create_slot_editor();
    let text_key = update_with(&editor, || {
        let paragraph = ab_cd_paragraph()?;
        Ok(
            append_host_with_slot(&paragraph, &create_slot_text_host(true))?
                .get_key()
                .to_owned(),
        )
    });
    for text in ["CD", "CDE", "CDEF"] {
        edit_slot(&editor, &text_key, text);
        assert_eq!(root_text(&editor), format!("ab\ncd {text}"));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > a block decorator host that is the last of four blocks
#[test]
fn a_block_decorator_host_that_is_the_last_of_four_blocks() {
    let editor = create_slot_editor();
    let text_key = update_with(&editor, || {
        let root = get_root();
        for text in ["a", "b", "c"] {
            root.append(&[para(&[create_text_node(text)])])?;
        }
        Ok(append_host_with_slot(&root, &create_slot_text_host(false))?
            .get_key()
            .to_owned())
    });
    for text in ["CD", "CDE"] {
        edit_slot(&editor, &text_key, text);
        assert_eq!(root_text(&editor), format!("a\n\nb\n\nc\n\n{text}"));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > a text appended after an edited decorator host
#[test]
fn a_text_appended_after_an_edited_decorator_host() {
    let editor = create_slot_editor();
    let (host_key, text_key) = update_with(&editor, || {
        let paragraph = ab_cd_paragraph()?;
        let host = create_slot_text_host(true);
        let text = append_host_with_slot(&paragraph, &host)?;
        Ok((host.get_key().to_owned(), text.get_key().to_owned()))
    });
    edit_slot(&editor, &text_key, "CD");
    update(&editor, move || {
        node(&host_key)
            .insert_after(&create_text_node(" ef"), true)
            .map(drop)
    });
    assert_eq!(root_text(&editor), "ab\ncd CD ef");
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > a text removed after an edited decorator host
#[test]
fn a_text_removed_after_an_edited_decorator_host() {
    let editor = create_slot_editor();
    let (after_key, text_key) = update_with(&editor, || {
        let paragraph = ab_cd_paragraph()?;
        let text = append_host_with_slot(&paragraph, &create_slot_text_host(true))?;
        let after = create_text_node(" ef");
        paragraph.append(std::slice::from_ref(&after))?;
        Ok((after.get_key().to_owned(), text.get_key().to_owned()))
    });
    edit_slot(&editor, &text_key, "CD");
    assert_eq!(root_text(&editor), "ab\ncd CD ef");
    update(&editor, move || node(&after_key).remove(false));
    assert_eq!(root_text(&editor), "ab\ncd CD");
}

/// The `test.each` of an inline host that ends a paragraph.
fn inline_host_ends_a_paragraph(before: fn() -> Vec<NodeRef>, host: fn() -> NodeRef, prefix: &str) {
    let editor = create_slot_editor();
    let text_key = update_with(&editor, move || {
        let paragraph = para(&before());
        get_root().append(std::slice::from_ref(&paragraph))?;
        Ok(append_host_with_slot(&paragraph, &host())?
            .get_key()
            .to_owned())
    });
    for text in ["CD", "CDE", "CDEF"] {
        edit_slot(&editor, &text_key, text);
        assert_eq!(root_text(&editor), format!("{prefix}{text}"));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > an inline decorator host that ends a paragraph of three children
#[test]
fn an_inline_decorator_host_that_ends_a_paragraph_of_three_children() {
    inline_host_ends_a_paragraph(
        || vec![create_text_node("ab"), create_text_node("cd ")],
        || create_slot_text_host(true),
        "abcd ",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > an inline element host that ends a paragraph of four children
#[test]
fn an_inline_element_host_that_ends_a_paragraph_of_four_children() {
    inline_host_ends_a_paragraph(
        || {
            vec![
                create_text_node("ab"),
                create_line_break_node(),
                create_text_node("cd "),
            ]
        },
        create_test_inline_element_node,
        "ab\ncd ",
    );
}

/// A [`SelectionMovement`] over text nodes laid out in a given order, as a
/// browser renders them: `Selection.modify` by word walks the concatenated
/// text (a new line counts as a space) and can cross a slot boundary, which
/// lives only in the model.
struct LaidOutMovement(Rc<std::cell::RefCell<Vec<(String, bool)>>>);

impl crate::SelectionMovement for LaidOutMovement {
    fn landed_point(
        &self,
        from: &crate::Point,
        is_backward: bool,
        granularity: crate::SelectionGranularity,
    ) -> Result<Option<crate::Point>, LexicalError> {
        assert!(granularity == crate::SelectionGranularity::Word);
        // Each text node's start in the concatenated text.
        let mut text: Vec<char> = Vec::new();
        let mut segments = Vec::new();
        for (key, new_line) in self.0.borrow().iter() {
            if *new_line && !text.is_empty() {
                text.push(' ');
            }
            let content: Vec<char> = node(key).get_text_content().chars().collect();
            segments.push((key.clone(), text.len(), content.len()));
            text.extend(content);
        }
        let (_, start, _) = segments.iter().find(|(k, ..)| *k == from.key).unwrap();
        let mut o = start + from.offset;
        let from_offset = o;
        if is_backward {
            while o > 0 && text[o - 1] == ' ' {
                o -= 1;
            }
            while o > 0 && text[o - 1] != ' ' {
                o -= 1;
            }
        } else {
            while o < text.len() && text[o] == ' ' {
                o += 1;
            }
            while o < text.len() && text[o] != ' ' {
                o += 1;
            }
        }
        Ok((o != from_offset).then(|| {
            // At the edge of two texts, the one it moved into.
            let mut at = segments
                .iter()
                .filter(|(_, s, len)| *s <= o && o <= s + len);
            let (key, start, _) = if is_backward {
                at.next_back()
            } else {
                at.next()
            }
            .unwrap();
            crate::Point::new(key.clone(), o - start, PointType::Text)
        }))
    }
}

/// `SlotWordDeletion.test.ts` (a browser test) `deletion at a slot
/// boundary`: word deletion where the browser's movement lands in another
/// slot frame stops at the boundary in the anchor's frame. Each case gives
/// the layout, the text deleted from, the offset, the direction, and the
/// text of `note` and `sibling` after it.
#[test]
fn word_deletion_stops_at_a_slot_boundary() {
    type Case = (
        &'static str,
        &'static str,
        usize,
        bool,
        &'static str,
        &'static str,
    );
    let cases: &[Case] = &[
        // word Backspace at the start of the sibling deletes nothing
        // across the slot (the root's slots render before its children).
        (
            "root slot",
            "sibling",
            0,
            true,
            "Footnote words",
            "Body words",
        ),
        // word Backspace in a word that starts in an inline slot deletes up
        // to the slot.
        ("inline slot", "sibling", 2, true, "foo", "r"),
        // word Delete in an inline slot word that continues past the slot
        // deletes to the slot end.
        ("inline slot", "note", 1, false, "f", "bar"),
        // word Delete in a body word that runs into an inline root slot
        // deletes to the slot.
        ("inline root slot", "sibling", 1, false, "foo", "b"),
        // word Delete at the end of the paragraph before the host deletes
        // nothing (and moves no text into another block).
        (
            "element host",
            "before",
            12,
            false,
            "Footnote words",
            "After words",
        ),
        // Not in the browser test, from JS's code path: word Backspace in a
        // word that starts in an inline host's slot stops beside the host,
        // after it (selection mode would stop before it and delete the
        // host's text too).
        ("inline host", "sibling", 1, true, "cd", "f"),
    ];
    for &(layout, region, offset, is_backward, want_note, want_sibling) in cases {
        let editor = create_slot_editor();
        let order = Rc::new(std::cell::RefCell::new(Vec::new()));
        editor.set_selection_movement(Some(Rc::new(LaidOutMovement(order.clone()))));
        let keys = Rc::new(std::cell::RefCell::new((
            String::new(),
            String::new(),
            String::new(),
        )));
        let k = keys.clone();
        update(&editor, move || {
            let root = get_root();
            let note_text = create_text_node(match layout {
                "root slot" | "element host" => "Footnote words",
                "inline host" => "cd",
                _ => "foo",
            });
            let sibling_text = create_text_node(match layout {
                "root slot" => "Body words",
                "inline host" => "ef",
                "element host" => "After words",
                _ => "bar",
            });
            let before_text = create_text_node("Before words");
            let before_key = before_text.get_key().to_owned();
            let (note, sibling) = (
                note_text.get_key().to_owned(),
                sibling_text.get_key().to_owned(),
            );
            match layout {
                "root slot" => {
                    root.append(&[para(&[sibling_text])])?;
                    let container = create_test_shadow_root_node();
                    container.append(&[para(&[note_text])])?;
                    set_slot(&root, "footnotes", &container)?;
                    *order.borrow_mut() = vec![(note.clone(), true), (sibling.clone(), true)];
                }
                "inline root slot" => {
                    root.append(&[para(&[sibling_text])])?;
                    set_slot(&root, "inline", &para(&[note_text]))?;
                    // Mounted after the body, on the same line.
                    *order.borrow_mut() = vec![(sibling.clone(), true), (note.clone(), false)];
                }
                "element host" => {
                    let host = para(&[sibling_text]);
                    let before = before_text.get_key().to_owned();
                    root.append(&[para(&[before_text]), host.clone()])?;
                    let container = create_test_shadow_root_node();
                    container.append(&[para(&[note_text])])?;
                    set_slot(&host, "note", &container)?;
                    *order.borrow_mut() = vec![
                        (before, true),
                        (note.clone(), true),
                        (sibling.clone(), true),
                    ];
                }
                "inline host" => {
                    let host = create_test_inline_element_node();
                    host.append(&[create_text_node("x")])?;
                    let before = before_text.get_key().to_owned();
                    before_text.set_text_content("ab ");
                    root.append(&[para(&[before_text, host.clone(), sibling_text])])?;
                    set_slot(&host, "note", &para(&[note_text]))?;
                    // On one line: the host's slot, then its children.
                    *order.borrow_mut() = vec![
                        (before, true),
                        (note.clone(), false),
                        (host.get_first_child_or_throw()?.get_key().to_owned(), false),
                        (sibling.clone(), false),
                    ];
                }
                _ => {
                    let host = para(&[sibling_text]);
                    root.append(std::slice::from_ref(&host))?;
                    set_slot(&host, "inline", &para(&[note_text]))?;
                    // An element's slots render before its children.
                    *order.borrow_mut() = vec![(note.clone(), true), (sibling.clone(), false)];
                }
            }
            *k.borrow_mut() = (note, sibling, before_key);
            Ok(())
        });
        let (note, sibling, before) = keys.borrow().clone();
        let at = match region {
            "note" => note.clone(),
            "before" => before,
            _ => sibling.clone(),
        };
        update(&editor, move || {
            node(&at)
                .select(Some(offset), Some(offset))?
                .delete_word(is_backward)
        });
        let label = format!("{layout}: {region}:{offset} backward={is_backward}");
        assert_eq!(
            read(&editor, || (
                node(&note).get_text_content(),
                node(&sibling).get_text_content()
            )),
            (want_note.to_owned(), want_sibling.to_owned()),
            "{label}"
        );
        if layout == "inline host" {
            assert_eq!(
                read(&editor, || get_root().get_text_content()),
                "ab cdxf",
                "{label}"
            );
        }
        if layout == "element host" {
            assert_eq!(
                read(&editor, || get_root().get_children_size()),
                2,
                "{label}"
            );
        }
    }
}
