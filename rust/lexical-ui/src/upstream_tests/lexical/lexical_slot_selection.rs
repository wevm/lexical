//! Ports of `packages/lexical/src/__tests__/unit/LexicalSlotSelection.test.ts`:
//! the programmatic selections, which a commit clamps to one slot frame, on
//! editors with a no-op root element. The tests that drag a DOM selection
//! are skipped in `rust/parity/skips.json`.

#![cfg(feature = "extension")]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::upstream_tests::support::{
    TestShadowRootNode, create_test_shadow_root_node, set_test_root_element, update,
};
use crate::{
    InitialEditorStateType, LexicalError, LexicalExtension, LexicalNode, NodeBase, NodeClass,
    NodeRef, Point, PointType, SelectionRef, Which, create, create_paragraph_node,
    create_range_selection, create_text_node, define_extension, get_node_by_key, get_root,
    get_selection, get_slot_host, select_all_in, set_selection, set_slot,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Keys = Rc<RefCell<HashMap<&'static str, String>>>;

/// `$slotFrameKeyOf(node)`: the key of the innermost slot root holding
/// `node`, if any.
fn slot_frame_key_of(node: &NodeRef) -> Option<String> {
    let mut current = Some(node.clone());
    while let Some(node) = current {
        if get_slot_host(&node).is_some() {
            return Some(node.get_key().to_owned());
        }
        current = node.get_parent();
    }
    None
}

/// `buildEditorFromExtensions(defineExtension({$initialEditorState, name,
/// nodes}))` on a root element.
fn build(
    name: &str,
    nodes: Vec<NodeClass>,
    initial: impl Fn(&mut HashMap<&'static str, String>) -> Result<(), LexicalError> + 'static,
) -> (LexicalEditorWithDispose, Keys) {
    let keys: Keys = Rc::default();
    let k = keys.clone();
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
            initial(&mut k.borrow_mut())
        }))),
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    (editor, keys)
}

fn paragraph_of(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(children)?;
    Ok(paragraph)
}

fn shadow_root_of(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    let node = create_test_shadow_root_node();
    node.append(children)?;
    Ok(node)
}

/// `buildSlotEditor()`: host -> slots `title` ("Title") and `subtitle`
/// ("Subtitle"), and the child body ("Body").
fn build_slot_editor() -> (LexicalEditorWithDispose, Keys) {
    build(
        "[slot-selection-poc]",
        vec![NodeClass::of::<TestShadowRootNode>()],
        |keys| {
            let host = create_paragraph_node();
            let title_text = create_text_node("Title");
            let title = shadow_root_of(&[paragraph_of(std::slice::from_ref(&title_text))?])?;
            let subtitle_text = create_text_node("Subtitle");
            let subtitle = shadow_root_of(&[paragraph_of(std::slice::from_ref(&subtitle_text))?])?;
            let body_text = create_text_node("Body");
            let body = paragraph_of(std::slice::from_ref(&body_text))?;
            get_root().clear()?;
            get_root().append(std::slice::from_ref(&host))?;
            host.append(std::slice::from_ref(&body))?;
            set_slot(&host, "title", &title)?;
            set_slot(&host, "subtitle", &subtitle)?;
            for (name, node) in [
                ("host", &host),
                ("title", &title),
                ("titleText", &title_text),
                ("subtitle", &subtitle),
                ("subtitleText", &subtitle_text),
                ("body", &body),
                ("bodyText", &body_text),
            ] {
                keys.insert(name, node.get_key().to_owned());
            }
            Ok(())
        },
    )
}

/// `buildRootSlotEditor()`: root -> child paragraph ("Body") and slot
/// `footnotes` ("Footnote").
fn build_root_slot_editor() -> (LexicalEditorWithDispose, Keys) {
    build(
        "[root-slot-selection]",
        vec![NodeClass::of::<TestShadowRootNode>()],
        |keys| {
            let body_text = create_text_node("Body");
            let footnote_text = create_text_node("Footnote");
            let footnotes = shadow_root_of(&[paragraph_of(std::slice::from_ref(&footnote_text))?])?;
            let root = get_root();
            root.clear()?;
            root.append(&[paragraph_of(std::slice::from_ref(&body_text))?])?;
            set_slot(&root, "footnotes", &footnotes)?;
            keys.insert("bodyText", body_text.get_key().to_owned());
            keys.insert("footnotes", footnotes.get_key().to_owned());
            keys.insert("footnoteText", footnote_text.get_key().to_owned());
            Ok(())
        },
    )
}

/// `buildNestedSlotEditor()`: root -> "Body" and a card; the card's slot
/// `title` holds "Intro", a heading whose slot `inner` holds "Deep", and
/// "Outro"; the title hosts slot `caption` ("Caption"); the card's slot
/// `aside` ("Aside") comes first in its slot order.
fn build_nested_slot_editor() -> (LexicalEditorWithDispose, Keys) {
    build(
        "[nested-slot-selection]",
        vec![NodeClass::of::<TestShadowRootNode>()],
        |keys| {
            let body_text = create_text_node("Body");
            let card = create_paragraph_node();
            let title = create_test_shadow_root_node();
            let heading = paragraph_of(&[create_text_node("Heading")])?;
            let deep_text = create_text_node("Deep");
            let deep = shadow_root_of(&[paragraph_of(std::slice::from_ref(&deep_text))?])?;
            let caption_text = create_text_node("Caption");
            let caption = shadow_root_of(&[paragraph_of(std::slice::from_ref(&caption_text))?])?;
            get_root().clear()?;
            get_root().append(&[
                paragraph_of(std::slice::from_ref(&body_text))?,
                card.clone(),
            ])?;
            let intro_text = create_text_node("Intro");
            title.append(&[
                paragraph_of(std::slice::from_ref(&intro_text))?,
                heading.clone(),
                paragraph_of(&[create_text_node("Outro")])?,
            ])?;
            set_slot(&card, "title", &title)?;
            set_slot(&heading, "inner", &deep)?;
            set_slot(&title, "caption", &caption)?;
            let aside_text = create_text_node("Aside");
            let aside = shadow_root_of(&[paragraph_of(std::slice::from_ref(&aside_text))?])?;
            set_slot(&card, "aside", &aside)?;
            for (name, node) in [
                ("aside", &aside),
                ("asideText", &aside_text),
                ("captionText", &caption_text),
                ("bodyText", &body_text),
                ("title", &title),
                ("introText", &intro_text),
                ("deep", &deep),
                ("deepText", &deep_text),
            ] {
                keys.insert(name, node.get_key().to_owned());
            }
            Ok(())
        },
    )
}

fn key(keys: &Keys, name: &str) -> String {
    if name == "root" {
        return get_root().get_key().to_owned();
    }
    keys.borrow()[name].clone()
}

/// `$createRangeSelection()` with its anchor and focus set, then
/// `$setSelection` and `$getSelection()`, which must be a range selection.
fn set_and_get(anchor: (&str, usize, PointType), focus: (&str, usize, PointType)) -> SelectionRef {
    let selection = create_range_selection();
    selection
        .point_set(Which::Anchor, anchor.0, anchor.1, anchor.2)
        .unwrap();
    selection
        .point_set(Which::Focus, focus.0, focus.1, focus.2)
        .unwrap();
    set_selection(Some(selection));
    active()
}

/// `$getSelection()`, asserted to be a range selection.
fn active() -> SelectionRef {
    let active = get_selection().expect("a selection");
    assert!(active.is_range());
    active
}

fn point(selection: &SelectionRef, which: Which) -> Point {
    selection.point(which).unwrap()
}

fn node_of(selection: &SelectionRef, which: Which) -> NodeRef {
    point(selection, which).get_node().unwrap()
}

const TEXT: PointType = PointType::Text;
const ELEMENT: PointType = PointType::Element;

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection with anchor in a slot, focus in the body, clamps focus into the slot
#[test]
fn a_programmatic_selection_with_anchor_in_a_slot_focus_in_the_body_clamps_focus_into_the_slot() {
    let (editor, keys) = build_slot_editor();
    update(&editor, move || {
        let active = set_and_get(
            (&key(&keys, "titleText"), 2, TEXT),
            (&key(&keys, "bodyText"), 2, TEXT),
        );
        let title = Some(key(&keys, "title"));
        assert_eq!(slot_frame_key_of(&node_of(&active, Which::Anchor)), title);
        assert_eq!(slot_frame_key_of(&node_of(&active, Which::Focus)), title);
        assert_ne!(point(&active, Which::Focus).key, key(&keys, "bodyText"));
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection with anchor in the body, focus in a slot of the host containing it, clamps focus to the start of the host
#[test]
fn a_programmatic_selection_with_anchor_in_the_body_focus_in_a_slot_of_the_host_containing_it_clamps_focus_to_the_start_of_the_host()
 {
    let (editor, keys) = build_slot_editor();
    update(&editor, move || {
        let active = set_and_get(
            (&key(&keys, "bodyText"), 1, TEXT),
            (&key(&keys, "titleText"), 1, TEXT),
        );
        assert_eq!(slot_frame_key_of(&node_of(&active, Which::Anchor)), None);
        assert_eq!(point(&active, Which::Focus).key, key(&keys, "host"));
        assert_eq!(point(&active, Which::Focus).offset, 0);
        assert_eq!(active.get_text_content()?, "B");
        Ok(())
    });
    editor.dispose();
}

/// `a programmatic selection $name` (the slot editor's cases): the anchor
/// stays, the focus lands on `expected`.
fn slot_case(
    build: fn() -> (LexicalEditorWithDispose, Keys),
    anchor: (&'static str, usize, PointType),
    focus: (&'static str, usize, PointType),
    expected: (&'static str, usize),
) {
    let (editor, keys) = build();
    update(&editor, move || {
        let active = set_and_get(
            (&key(&keys, anchor.0), anchor.1, anchor.2),
            (&key(&keys, focus.0), focus.1, focus.2),
        );
        assert_eq!(point(&active, Which::Anchor).key, key(&keys, anchor.0));
        let focus = point(&active, Which::Focus);
        assert_eq!(
            (focus.key, focus.offset),
            (key(&keys, expected.0), expected.1)
        );
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection from just before a host into its slot p…
#[test]
fn a_programmatic_selection_from_just_before_a_host_into_its_slot_pushes_focus_past_the_host() {
    slot_case(
        build_slot_editor,
        ("root", 0, ELEMENT),
        ("titleText", 2, TEXT),
        ("root", 1),
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection from a slot to just before its host cla…
#[test]
fn a_programmatic_selection_from_a_slot_to_just_before_its_host_clamps_focus_to_the_slot_start() {
    slot_case(
        build_slot_editor,
        ("titleText", 2, TEXT),
        ("root", 0, ELEMENT),
        ("title", 0),
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection from a slot to a removed node clamps focus to the slot edge
#[test]
fn a_programmatic_selection_from_a_slot_to_a_removed_node_clamps_focus_to_the_slot_edge() {
    let (editor, keys) = build_slot_editor();
    update(&editor, move || {
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, &key(&keys, "titleText"), 1, TEXT)?;
        selection.point_set(Which::Focus, &key(&keys, "bodyText"), 1, TEXT)?;
        let body_text = get_node_by_key(&key(&keys, "bodyText")).unwrap();
        assert!(body_text.is_text());
        body_text.remove(false)?;
        set_selection(Some(selection));
        let active = active();
        assert_eq!(point(&active, Which::Anchor).key, key(&keys, "titleText"));
        assert_eq!(
            slot_frame_key_of(&node_of(&active, Which::Focus)),
            Some(key(&keys, "title"))
        );
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection entirely within the body is left untouched
#[test]
fn a_programmatic_selection_entirely_within_the_body_is_left_untouched() {
    let (editor, keys) = build_slot_editor();
    update(&editor, move || {
        let body_text = get_node_by_key(&key(&keys, "bodyText")).unwrap();
        assert!(body_text.is_text());
        body_text.select(Some(1), Some(3))?;
        let active = active();
        let (anchor, focus) = (point(&active, Which::Anchor), point(&active, Which::Focus));
        assert_eq!(anchor.key, key(&keys, "bodyText"));
        assert_eq!(anchor.offset, 1);
        assert_eq!(focus.key, key(&keys, "bodyText"));
        assert_eq!(focus.offset, 3);
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > an in-place point mutation that straddles a slot is clamped at commit
#[test]
fn an_in_place_point_mutation_that_straddles_a_slot_is_clamped_at_commit() {
    let (editor, keys) = build_slot_editor();
    let k = keys.clone();
    update(&editor, move || {
        let body_text = get_node_by_key(&key(&k, "bodyText")).unwrap();
        assert!(body_text.is_text());
        body_text.select(Some(1), Some(2)).map(drop)
    });
    let k = keys.clone();
    update(&editor, move || {
        active().point_set(Which::Focus, &key(&k, "titleText"), 1, TEXT)
    });
    editor
        .read(|| {
            let active = active();
            assert_eq!(slot_frame_key_of(&node_of(&active, Which::Anchor)), None);
            assert_eq!(slot_frame_key_of(&node_of(&active, Which::Focus)), None);
            assert_ne!(point(&active, Which::Focus).key, key(&keys, "titleText"));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > $selectAll from inside a slot scopes to the slot, not the whole document
#[test]
fn select_all_from_inside_a_slot_scopes_to_the_slot_not_the_whole_document() {
    let (editor, keys) = build_slot_editor();
    update(&editor, move || {
        let title_text = get_node_by_key(&key(&keys, "titleText")).unwrap();
        assert!(title_text.is_text());
        let selection = title_text.select(Some(1), Some(1))?;
        let out = select_all_in(&selection)?;
        let title = Some(key(&keys, "title"));
        assert_eq!(slot_frame_key_of(&node_of(&out, Which::Anchor)), title);
        assert_eq!(slot_frame_key_of(&node_of(&out, Which::Focus)), title);
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > $selectAll from outside any slot still scopes to the whole document
#[test]
fn select_all_from_outside_any_slot_still_scopes_to_the_whole_document() {
    let (editor, keys) = build_slot_editor();
    update(&editor, move || {
        let body_text = get_node_by_key(&key(&keys, "bodyText")).unwrap();
        assert!(body_text.is_text());
        let selection = body_text.select(Some(1), Some(1))?;
        let out = select_all_in(&selection)?;
        assert_eq!(slot_frame_key_of(&node_of(&out, Which::Anchor)), None);
        assert_eq!(slot_frame_key_of(&node_of(&out, Which::Focus)), None);
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection across two slots clamps focus into the anchor slot
#[test]
fn a_programmatic_selection_across_two_slots_clamps_focus_into_the_anchor_slot() {
    let (editor, keys) = build_slot_editor();
    update(&editor, move || {
        let active = set_and_get(
            (&key(&keys, "titleText"), 1, TEXT),
            (&key(&keys, "subtitleText"), 1, TEXT),
        );
        let title = Some(key(&keys, "title"));
        assert_eq!(slot_frame_key_of(&node_of(&active, Which::Anchor)), title);
        assert_eq!(slot_frame_key_of(&node_of(&active, Which::Focus)), title);
        assert_ne!(point(&active, Which::Focus).key, key(&keys, "subtitleText"));
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across a root slot > a programmatic selection with anchor in the body, focus in a root slot, clamps focus to the root edge
#[test]
fn a_programmatic_selection_with_anchor_in_the_body_focus_in_a_root_slot_clamps_focus_to_the_root_edge()
 {
    let (editor, keys) = build_root_slot_editor();
    update(&editor, move || {
        let active = set_and_get(
            (&key(&keys, "bodyText"), 1, TEXT),
            (&key(&keys, "footnoteText"), 3, TEXT),
        );
        assert_eq!(point(&active, Which::Anchor).key, key(&keys, "bodyText"));
        assert_eq!(slot_frame_key_of(&node_of(&active, Which::Focus)), None);
        assert_eq!(point(&active, Which::Focus).key, get_root().get_key());
        assert_eq!(point(&active, Which::Focus).offset, 0);
        assert_eq!(active.get_text_content()?, "B");
        Ok(())
    });
    editor.dispose();
}

/// `a programmatic selection from the body into nested slot %s pushes focus
/// past the outer host`.
fn from_the_body_into_nested_slot(focus_key: &'static str) {
    let (editor, keys) = build_nested_slot_editor();
    update(&editor, move || {
        let active = set_and_get(
            (&key(&keys, "bodyText"), 1, TEXT),
            (&key(&keys, focus_key), 2, TEXT),
        );
        assert_eq!(point(&active, Which::Anchor).key, key(&keys, "bodyText"));
        assert_eq!(slot_frame_key_of(&node_of(&active, Which::Focus)), None);
        assert_eq!(point(&active, Which::Focus).key, get_root().get_key());
        assert_eq!(point(&active, Which::Focus).offset, 2);
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from the body into nested slot deepText pushes focus past the outer host
#[test]
fn a_programmatic_selection_from_the_body_into_nested_slot_deep_text_pushes_focus_past_the_outer_host()
 {
    from_the_body_into_nested_slot("deepText");
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from the body into nested slot captionText pushes focus past the outer host
#[test]
fn a_programmatic_selection_from_the_body_into_nested_slot_caption_text_pushes_focus_past_the_outer_host()
 {
    from_the_body_into_nested_slot("captionText");
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a slot into a slot nested in it pushes focus past the host in the anchor slot
#[test]
fn a_programmatic_selection_from_a_slot_into_a_slot_nested_in_it_pushes_focus_past_the_host_in_the_anchor_slot()
 {
    slot_case(
        build_nested_slot_editor,
        ("introText", 1, TEXT),
        ("deepText", 2, TEXT),
        ("title", 2),
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a slot into its own slot value's slot clamps focus to the slot edge
#[test]
fn a_programmatic_selection_from_a_slot_into_its_own_slot_values_slot_clamps_focus_to_the_slot_edge()
 {
    slot_case(
        build_nested_slot_editor,
        ("introText", 1, TEXT),
        ("captionText", 2, TEXT),
        ("title", 0),
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a nested slot into an earlier sibl…
#[test]
fn a_programmatic_selection_from_a_nested_slot_into_an_earlier_sibling_of_its_outer_slot() {
    slot_case(
        build_nested_slot_editor,
        ("deepText", 1, TEXT),
        ("asideText", 2, TEXT),
        ("deep", 0),
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from the end of the root into a nested …
#[test]
fn a_programmatic_selection_from_the_end_of_the_root_into_a_nested_slot() {
    slot_case(
        build_nested_slot_editor,
        ("root", 2, ELEMENT),
        ("deepText", 2, TEXT),
        ("root", 1),
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a nested slot to the end of the ro…
#[test]
fn a_programmatic_selection_from_a_nested_slot_to_the_end_of_the_root() {
    slot_case(
        build_nested_slot_editor,
        ("deepText", 1, TEXT),
        ("root", 2, ELEMENT),
        ("deep", 1),
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a nested slot into the body clamps focus into the slot
#[test]
fn a_programmatic_selection_from_a_nested_slot_into_the_body_clamps_focus_into_the_slot() {
    slot_case(
        build_nested_slot_editor,
        ("deepText", 1, TEXT),
        ("bodyText", 1, TEXT),
        ("deep", 0),
    );
}

/// `TestDecoratorNode` (`__tests__/utils`) after `setIsInline(false)`.
#[derive(Clone, Debug, Default)]
struct TestDecoratorNode {
    base: NodeBase,
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
        false
    }
    fn get_text_content(&self) -> String {
        "Hello world".to_owned()
    }
}

/// `buildDecoratorSlotEditor()`: host -> slot `media` (a block decorator)
/// and the child body ("Body").
fn build_decorator_slot_editor() -> (LexicalEditorWithDispose, Keys) {
    build(
        "[slot-selection-decorator-key-reject]",
        vec![
            NodeClass::of::<TestShadowRootNode>(),
            NodeClass::of::<TestDecoratorNode>(),
        ],
        |keys| {
            let host = create_paragraph_node();
            let decorator_slot = create::<TestDecoratorNode>();
            let body_text = create_text_node("Body");
            let body = paragraph_of(std::slice::from_ref(&body_text))?;
            get_root().clear()?;
            get_root().append(std::slice::from_ref(&host))?;
            host.append(std::slice::from_ref(&body))?;
            set_slot(&host, "media", &decorator_slot)?;
            for (name, node) in [
                ("host", &host),
                ("decoratorSlot", &decorator_slot),
                ("body", &body),
                ("bodyText", &body_text),
            ] {
                keys.insert(name, node.get_key().to_owned());
            }
            Ok(())
        },
    )
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: Point.set rejects decorator key targets > Point.set rejects decorator key with text type
#[test]
fn point_set_rejects_decorator_key_with_text_type() {
    let (editor, keys) = build_decorator_slot_editor();
    update(&editor, move || {
        let sel = create_range_selection();
        assert!(
            sel.point_set(Which::Anchor, &key(&keys, "decoratorSlot"), 0, TEXT)
                .is_err()
        );
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: Point.set rejects decorator key targets > Point.set rejects decorator key with element type
#[test]
fn point_set_rejects_decorator_key_with_element_type() {
    let (editor, keys) = build_decorator_slot_editor();
    update(&editor, move || {
        let sel = create_range_selection();
        assert!(
            sel.point_set(Which::Anchor, &key(&keys, "decoratorSlot"), 0, ELEMENT)
                .is_err()
        );
        Ok(())
    });
    editor.dispose();
}
