//! Ports of `packages/lexical/src/__tests__/unit/LexicalUtils.test.ts`.
//! JS's `new Klass(...)` is `set_node_key(Box::new(...))`, and `toBe` on a
//! node is the same key (or, where the test asks that a node was not made
//! writable, the same `Rc` in the node map).

use crate::events::KeyboardEvent;
use crate::lexical_utils::{generate_random_key, get_node_by_key_impl, set_node_key};
use crate::node::NodeBase;
use crate::nodes::line_break::LineBreakNode;
use crate::nodes::paragraph::ParagraphNode;
use crate::nodes::tab::TabNode;
use crate::nodes::text::{TextData, TextNode};
use crate::updates::get_active_editor_state;
use crate::upstream_tests::support::{initialize_unit_test, panic_message, panics};
use crate::{
    CreateEditorArgs, Direction, Editor, ElementData, KeyboardEventModifierMask, LexicalError,
    LexicalNode, LexicalNodeReplacement, ModifierExpectation, NodeClass, NodeRef, StateConfig,
    UpdateOptions, apply_node_replacement, copy_node, create_editor, create_paragraph_node,
    create_state, create_text_node, get_cached_type_to_node_map, get_node_by_key,
    get_registered_subtype_map, get_root, get_state, get_text_direction, is_exact_shortcut_match,
    is_token_or_segmented, nodes_of_type, on_update, reset_random_key, set_state,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > resetRandomKey()
#[test]
fn reset_random_key_restarts_keys() {
    reset_random_key();
    let key1 = generate_random_key();
    reset_random_key();
    let key2 = generate_random_key();
    assert_ne!(key1, "");
    assert_ne!(key2, "");
    assert_eq!(key1, key2);
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > generateRandomKey()
#[test]
fn generate_random_key_is_unique() {
    let key1 = generate_random_key();
    let key2 = generate_random_key();
    assert_ne!(key1, "");
    assert_ne!(key2, "");
    assert_ne!(key1, key2);
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > getTextDirection()
#[test]
fn text_direction() {
    for text in ["", " ", "0"] {
        assert_eq!(get_text_direction(text), None, "{text:?}");
    }
    for text in [
        "A", "Z", "a", "z", "\u{00C0}", "\u{00D6}", "\u{00D8}", "\u{00F6}", "\u{00F8}", "\u{02B8}",
        "\u{0300}", "\u{0590}", "\u{0800}", "\u{1FFF}", "\u{200E}", "\u{2C00}", "\u{FB1C}",
        "\u{FE00}", "\u{FE6F}", "\u{FEFD}", "\u{FFFF}",
    ] {
        assert_eq!(get_text_direction(text), Some(Direction::Ltr), "{text:?}");
    }
    for text in [
        "\u{0591}", "\u{07FF}", "\u{FB1D}", "\u{FDFD}", "\u{FE70}", "\u{FEFC}",
    ] {
        assert_eq!(get_text_direction(text), Some(Direction::Rtl), "{text:?}");
    }
}

/// `new KeyboardEvent('keydown', {code, key, ctrlKey, shiftKey, metaKey})`.
fn key_event(code: &str, key: &str, ctrl: bool, shift: bool, meta: bool) -> KeyboardEvent {
    KeyboardEvent {
        code: code.to_owned(),
        key: key.to_owned(),
        ctrl_key: ctrl,
        shift_key: shift,
        meta_key: meta,
        ..KeyboardEvent::default()
    }
}

const CTRL: KeyboardEventModifierMask = KeyboardEventModifierMask {
    alt_key: ModifierExpectation::Released,
    ctrl_key: ModifierExpectation::Pressed,
    meta_key: ModifierExpectation::Released,
    shift_key: ModifierExpectation::Released,
    control_or_other: None,
};

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() matches by event.key for single-letter
#[test]
fn shortcut_matches_by_key_for_single_letter() {
    assert!(is_exact_shortcut_match(
        &key_event("KeyZ", "z", true, false, false),
        "z",
        &CTRL
    ));
    assert!(is_exact_shortcut_match(
        &key_event("KeyZ", "Z", true, false, false),
        "z",
        &CTRL
    ));
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() matches to event.key for ASCII remapped layout (English (US) Dvorak)
#[test]
fn shortcut_matches_by_key_for_remapped_ascii_layout() {
    for key in ["x", "X"] {
        let event = key_event("KeyB", key, true, false, false);
        assert!(is_exact_shortcut_match(&event, "x", &CTRL));
        assert!(!is_exact_shortcut_match(&event, "b", &CTRL));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() fallback to event.code for single-letter in event.key via non-English layout
#[test]
fn shortcut_falls_back_to_code_for_non_english_layout() {
    assert!(is_exact_shortcut_match(
        &key_event("KeyZ", "я", true, false, false),
        "z",
        &CTRL
    ));
    assert!(is_exact_shortcut_match(
        &key_event("KeyZ", "Я", true, false, false),
        "z",
        &CTRL
    ));
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() matches special keys
#[test]
fn shortcut_matches_special_keys() {
    for key in ["Enter", "Tab", "Delete"] {
        assert!(
            is_exact_shortcut_match(&key_event("", key, true, false, false), key, &CTRL),
            "{key}"
        );
    }
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() matches optional keys
#[test]
fn shortcut_matches_optional_keys() {
    let mask = |ctrl, shift, meta| {
        let pressed = |p| {
            if p {
                ModifierExpectation::Pressed
            } else {
                ModifierExpectation::Released
            }
        };
        KeyboardEventModifierMask {
            ctrl_key: pressed(ctrl),
            shift_key: pressed(shift),
            meta_key: pressed(meta),
            ..KeyboardEventModifierMask::default()
        }
    };
    assert!(is_exact_shortcut_match(
        &key_event("", "a", true, false, false),
        "a",
        &mask(true, false, false)
    ));
    assert!(is_exact_shortcut_match(
        &key_event("", "a", false, true, false),
        "a",
        &mask(false, true, false)
    ));
    assert!(is_exact_shortcut_match(
        &key_event("", "a", false, false, true),
        "a",
        &mask(false, false, true)
    ));
    assert!(!is_exact_shortcut_match(
        &key_event("", "a", false, false, false),
        "a",
        &mask(true, false, false)
    ));
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isTokenOrSegmented()
#[test]
fn token_or_segmented() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    editor
        .update(
            || {
                let node = create_text_node("foo");
                assert!(!is_token_or_segmented(&node));
                let token_node = create_text_node("");
                token_node.set_mode("token")?;
                assert!(is_token_or_segmented(&token_node));
                let segmented_node = create_text_node("foo");
                segmented_node.set_mode("segmented")?;
                assert!(is_token_or_segmented(&segmented_node));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $getNodeByKey
#[test]
fn node_by_key() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let k = keys.clone();
    editor
        .update(
            move || {
                let paragraph_node = set_node_key(Box::new(ParagraphNode::default()));
                let text_node = set_node_key(Box::new(TextNode::new("foo")));
                paragraph_node.append(std::slice::from_ref(&text_node))?;
                get_root().append(std::slice::from_ref(&paragraph_node))?;
                k.borrow_mut().extend([
                    paragraph_node.get_key().to_owned(),
                    text_node.get_key().to_owned(),
                ]);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let keys = keys.take();
    editor
        .read_with_mode(crate::EditorReadMode::Latest, || {
            let key_of = |key: &str| get_node_by_key(key).map(|n| n.get_key().to_owned());
            assert_eq!(key_of("1").as_ref(), Some(&keys[0]));
            assert_eq!(key_of("2").as_ref(), Some(&keys[1]));
            assert_eq!(key_of("3"), None);
        })
        .unwrap();
    // `$getNodeByKey()` without an active editor state throws.
    assert!(panics(|| get_node_by_key("1")));
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $nodesOfType
#[test]
fn nodes_of_paragraph_type() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let paragraph_keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let paragraph_keys_now = || {
        let mut keys: Vec<String> = nodes_of_type("paragraph")
            .iter()
            .map(|node| node.get_key().to_owned())
            .collect();
        keys.sort();
        keys
    };
    let keys = paragraph_keys.clone();
    editor
        .update(
            move || {
                let root = get_root();
                let paragraph1 = create_paragraph_node();
                let paragraph2 = create_paragraph_node();
                create_paragraph_node();
                root.append(&[paragraph1.clone(), paragraph2.clone()])?;
                let mut expected = vec![
                    paragraph1.get_key().to_owned(),
                    paragraph2.get_key().to_owned(),
                ];
                expected.sort();
                assert_eq!(paragraph_keys_now(), expected);
                *keys.borrow_mut() = expected;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let expected = paragraph_keys.take();
    editor
        .read_with_mode(crate::EditorReadMode::Latest, || {
            assert_eq!(paragraph_keys_now(), expected);
        })
        .unwrap();
}

/// A list the update callbacks push to.
fn runs() -> (
    Rc<RefCell<Vec<&'static str>>>,
    impl Fn(&'static str) + Clone,
) {
    let runs = Rc::new(RefCell::new(Vec::new()));
    let r = runs.clone();
    (runs, move |run| r.borrow_mut().push(run))
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $onUpdate > deferred even when there are no dirty nodes
#[test]
fn on_update_is_deferred_without_dirty_nodes() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let (runs, push) = runs();
    let (p1, p2, p3) = (push.clone(), push.clone(), push);
    editor
        .update(
            move || {
                on_update(move || p2("second"));
                Ok(())
            },
            UpdateOptions {
                on_update: Some(Rc::new(move || p1("first"))),
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    assert!(runs.borrow().is_empty());
    editor
        .update(
            move || {
                on_update(move || p3("third"));
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    assert!(runs.borrow().is_empty());
    // Flush pending updates
    editor.read(|| ()).unwrap();
    assert_eq!(*runs.borrow(), ["first", "second", "third"]);
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $onUpdate > added fn runs after update, original onUpdate, and prior calls to $onUpdate
#[test]
fn on_update_runs_after_the_update_and_its_on_update() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let (runs, push) = runs();
    let (p1, p2, p3) = (push.clone(), push.clone(), push);
    editor
        .update(
            move || {
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("foo")])?;
                get_root().append(&[paragraph])?;
                on_update(move || p2("second"));
                on_update(move || p3("third"));
                Ok(())
            },
            UpdateOptions {
                on_update: Some(Rc::new(move || p1("first"))),
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    // Flush pending updates
    editor.read(|| ()).unwrap();
    assert_eq!(*runs.borrow(), ["first", "second", "third"]);
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $onUpdate > adding fn throws outside update
#[test]
fn on_update_panics_outside_an_update() {
    assert!(panics(|| on_update(|| {})));
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > getCachedTypeToNodeMap
#[test]
fn cached_type_to_node_map() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let initial_type_to_node_map = get_cached_type_to_node_map(&editor.get_editor_state());
    assert!(Rc::ptr_eq(
        &get_cached_type_to_node_map(&editor.get_editor_state()),
        &initial_type_to_node_map
    ));
    assert_eq!(
        initial_type_to_node_map.keys().collect::<Vec<_>>(),
        [&"root"]
    );
    assert_eq!(initial_type_to_node_map["root"].len(), 1);

    let paragraph_keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let keys = paragraph_keys.clone();
    editor
        .update(
            move || {
                let root = get_root();
                let paragraph1 = create_paragraph_node();
                paragraph1.append(&[create_text_node("a")])?;
                let paragraph2 = create_paragraph_node();
                paragraph2.append(&[create_text_node("b")])?;
                // these will be garbage collected and not in the readonly map
                create_paragraph_node().append(&[create_text_node("c")])?;
                root.append(&[paragraph1.clone(), paragraph2.clone()])?;
                keys.borrow_mut().extend([
                    paragraph1.get_key().to_owned(),
                    paragraph2.get_key().to_owned(),
                ]);
                Ok(())
            },
            discrete(),
        )
        .unwrap();

    let type_to_node_map = get_cached_type_to_node_map(&editor.get_editor_state());
    // verify that the initial cache was not used
    assert!(!Rc::ptr_eq(&type_to_node_map, &initial_type_to_node_map));
    // verify that the cache is used for subsequent calls
    assert!(Rc::ptr_eq(
        &get_cached_type_to_node_map(&editor.get_editor_state()),
        &type_to_node_map
    ));
    assert_eq!(type_to_node_map.len(), 3);
    let mut types: Vec<_> = type_to_node_map.keys().copied().collect();
    types.sort_unstable();
    assert_eq!(types, ["paragraph", "root", "text"]);
    let paragraph_map = &type_to_node_map["paragraph"];
    let mut keys: Vec<_> = paragraph_map.keys().cloned().collect();
    keys.sort();
    let mut expected = paragraph_keys.take();
    expected.sort();
    assert_eq!(keys, expected);
    let text_map = &type_to_node_map["text"];
    assert_eq!(text_map.len(), 2);
    let mut texts: Vec<_> = text_map
        .values()
        .map(|node| node.text().unwrap().text.clone())
        .collect();
    texts.sort();
    assert_eq!(texts, ["a", "b"]);
}

// $applyNodeReplacement

/// A text node class of the test's own: `ExtendedTextNode` and
/// `ExtendedExtendedTextNode`.
macro_rules! text_class {
    ($name:ident, $type:literal, [$($super:literal),*]) => {
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            text: TextData,
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
            fn text(&self) -> Option<&TextData> {
                Some(&self.text)
            }
            fn text_mut(&mut self) -> Option<&mut TextData> {
                Some(&mut self.text)
            }
            fn super_types(&self) -> Vec<&'static str> {
                vec![$($super),*]
            }
        }
        impl $name {
            /// `new Klass(text)` (`TextNodeA` is never constructed).
            #[allow(dead_code)]
            fn new_node(text: &str) -> NodeRef {
                set_node_key(Box::new(Self {
                    text: TextData {
                        text: text.to_owned(),
                        ..TextData::default()
                    },
                    ..Self::default()
                }))
            }
        }
    };
}

text_class!(ExtendedTextNode, "extended-text", ["text"]);
text_class!(
    ExtendedExtendedTextNode,
    "extended-extended-text",
    ["extended-text", "text"]
);

/// `initWithTextNode(node)` (and `initWithExtendedTextNode`):
/// `TextNode.prototype.updateFromJSON.call(self, node.exportJSON())`.
fn init_with_text_node(this: NodeRef, node: &NodeRef) -> Result<NodeRef, LexicalError> {
    this.update_from_json(&node.export_json(false))?;
    Ok(this)
}

/// `$createExtendedTextNode(text)`.
fn create_extended_text_node(text: &str) -> Result<NodeRef, LexicalError> {
    apply_node_replacement(&ExtendedTextNode::new_node(text))
}

/// `$createExtendedExtendedTextNode(text)`.
fn create_extended_extended_text_node(text: &str) -> Result<NodeRef, LexicalError> {
    apply_node_replacement(&ExtendedExtendedTextNode::new_node(text))
}

/// `{replace: TextNode, with: node => $createExtendedTextNode().initWithTextNode(node), withKlass}`.
fn text_to_extended(with_klass: NodeClass) -> NodeClass {
    LexicalNodeReplacement {
        replace: NodeClass::of::<TextNode>(),
        with: Rc::new(|node| init_with_text_node(create_extended_text_node("")?, node)),
        with_klass: Some(with_klass),
    }
    .into()
}

/// `{replace: ExtendedTextNode, with: node => $createExtendedExtendedTextNode().initWithExtendedTextNode(node), withKlass: ExtendedExtendedTextNode}`.
fn extended_to_extended_extended() -> NodeClass {
    LexicalNodeReplacement {
        replace: NodeClass::of::<ExtendedTextNode>(),
        with: Rc::new(|node| init_with_text_node(create_extended_extended_text_node("")?, node)),
        with_klass: Some(NodeClass::of::<ExtendedExtendedTextNode>()),
    }
    .into()
}

/// `createEditor({nodes, onError(err) { throw err; }})`.
fn throwing_editor(nodes: Vec<NodeClass>) -> Editor {
    create_editor(CreateEditorArgs {
        nodes,
        ..CreateEditorArgs::default()
    })
}

/// What the discrete update that appends a paragraph with `text` throws.
fn append_fails(editor: &Editor, text: impl FnOnce() -> NodeRef + 'static) -> String {
    let update = || {
        editor.update(
            move || {
                let paragraph = create_paragraph_node();
                paragraph.append(&[text()])?;
                get_root().clear()?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
    };
    match catch_unwind(AssertUnwindSafe(update)) {
        Ok(result) => result.expect_err("a failed update").message().to_owned(),
        Err(payload) => panic_message(&*payload),
    }
}

fn assert_contains(message: &str, expected: &str) {
    assert!(message.contains(expected), "{message}");
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates replace node configuration
#[test]
fn validates_replace_node_configuration() {
    let editor = throwing_editor(vec![text_to_extended(NodeClass::of::<
        ExtendedExtendedTextNode,
    >())]);
    assert_contains(
        &append_fails(&editor, || create_text_node("text")),
        "Attempted to create node ExtendedTextNode that was not configured to be used on the editor",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates replace node type withKlass
#[test]
fn validates_replace_node_type_with_klass() {
    let editor = throwing_editor(vec![
        LexicalNodeReplacement {
            replace: NodeClass::of::<TextNode>(),
            with: Rc::new(|node| Ok(node.clone())),
            with_klass: Some(NodeClass::of::<ExtendedTextNode>()),
        }
        .into(),
    ]);
    assert_contains(
        &append_fails(&editor, || create_text_node("text")),
        "$applyNodeReplacement failed. Expected replacement node to be an instance of ExtendedTextNode with type extended-text but returned TextNode with type text from original node TextNode with type text",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates replace node type change
#[test]
fn validates_replace_node_type_change() {
    let warnings = Rc::new(RefCell::new(Vec::<String>::new()));
    let w = warnings.clone();
    let editor = crate::with_warning_handler(
        move |message| w.borrow_mut().push(message.to_owned()),
        || {
            throwing_editor(vec![
                LexicalNodeReplacement {
                    replace: NodeClass::of::<TextNode>(),
                    with: Rc::new(|node| {
                        Ok(set_node_key(Box::new(TextNode::new(
                            node.get_text_content(),
                        ))))
                    }),
                    with_klass: None,
                }
                .into(),
            ])
        },
    );
    assert_eq!(
        *warnings.borrow(),
        [
            "Override for TextNode specifies 'replace' without 'withKlass'. 'withKlass' will be required in a future version."
        ]
    );
    assert_contains(
        &append_fails(&editor, || create_text_node("text")),
        "$applyNodeReplacement failed. Ensure replacement node TextNode with type text is a subclass of the original node TextNode with type text",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates replace node configuration withKlass
#[test]
fn validates_replace_node_configuration_with_klass() {
    let editor = throwing_editor(vec![text_to_extended(NodeClass::of::<ExtendedTextNode>())]);
    assert_contains(
        &append_fails(&editor, || create_text_node("text")),
        "Attempted to create node ExtendedTextNode that was not configured to be used on the editor",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates nested replace node configuration
#[test]
fn validates_nested_replace_node_configuration() {
    let editor = throwing_editor(vec![
        NodeClass::of::<ExtendedTextNode>(),
        extended_to_extended_extended(),
    ]);
    assert_contains(
        &append_fails(&editor, || create_extended_text_node("text").unwrap()),
        "Attempted to create node ExtendedExtendedTextNode that was not configured to be used on the editor",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates nested replace node configuration withKlass
#[test]
fn validates_nested_replace_node_configuration_with_klass() {
    let editor = throwing_editor(vec![
        NodeClass::of::<ExtendedTextNode>(),
        text_to_extended(NodeClass::of::<ExtendedTextNode>()),
        extended_to_extended_extended(),
    ]);
    assert_contains(
        &append_fails(&editor, || create_text_node("text")),
        "Attempted to create node ExtendedExtendedTextNode that was not configured to be used on the editor",
    );
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > nested replace node configuration works
#[test]
fn nested_replace_node_configuration_works() {
    let editor = throwing_editor(vec![
        NodeClass::of::<ExtendedTextNode>(),
        NodeClass::of::<ExtendedExtendedTextNode>(),
        text_to_extended(NodeClass::of::<ExtendedTextNode>()),
        extended_to_extended_extended(),
    ]);
    editor
        .update(
            || {
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("text")])?;
                get_root().clear()?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| {
            let text_nodes = get_root().get_all_text_nodes();
            assert_eq!(text_nodes.len(), 1);
            assert!(text_nodes[0].get_latest().is::<ExtendedExtendedTextNode>());
            assert_eq!(text_nodes[0].get_text_content(), "text");
        })
        .unwrap();
}

// $copyNode

thread_local! {
    static STRING_STATE: StateConfig<String> =
        create_state("string-state", |v| v.as_str().unwrap_or("").to_owned());
}

fn string_state() -> StateConfig<String> {
    STRING_STATE.with(Clone::clone)
}

/// `ExtendedParagraphNode`: a `ParagraphNode` with a `__string` field
/// (`'default'`) its `afterCloneFrom` copies.
#[derive(Clone, Debug)]
struct ExtendedParagraphNode {
    base: NodeBase,
    element: ElementData,
    string: String,
}

impl Default for ExtendedParagraphNode {
    fn default() -> Self {
        Self {
            base: NodeBase::default(),
            element: ElementData::default(),
            string: "default".to_owned(),
        }
    }
}

impl LexicalNode for ExtendedParagraphNode {
    fn get_type(&self) -> &'static str {
        "extended-paragraph"
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
    fn super_types(&self) -> Vec<&'static str> {
        vec!["paragraph"]
    }
}

/// `node.setString(value)`.
fn set_string(node: &NodeRef, value: &str) {
    let value = value.to_owned();
    crate::lexical_utils::with_writable(node.get_key(), |n| {
        n.downcast_mut::<ExtendedParagraphNode>().unwrap().string = value;
    });
}

/// `node.__string` of the latest version.
fn string_of(node: &NodeRef) -> String {
    node.get_latest()
        .downcast_ref::<ExtendedParagraphNode>()
        .unwrap()
        .string
        .clone()
}

fn copy_node_editor() -> Editor {
    throwing_editor(vec![
        NodeClass::of::<ExtendedParagraphNode>(),
        NodeClass::of::<TextNode>(),
        NodeClass::of::<ParagraphNode>(),
    ])
}

/// `$applyNodeReplacement(new ExtendedParagraphNode())`.
fn create_extended_paragraph_node() -> Result<NodeRef, LexicalError> {
    apply_node_replacement(&set_node_key(Box::new(ExtendedParagraphNode::default())))
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::$copyNode > does not mark the original as dirty
#[test]
fn copy_node_does_not_mark_the_original_as_dirty() {
    let editor = copy_node_editor();
    let key = Rc::new(RefCell::new(String::new()));
    let k = key.clone();
    editor
        .update(
            move || {
                let initial_paragraph = create_extended_paragraph_node()?;
                initial_paragraph.append(&[create_text_node("text")])?;
                get_root().clear()?;
                get_root().append(std::slice::from_ref(&initial_paragraph))?;
                *k.borrow_mut() = initial_paragraph.get_key().to_owned();
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let key = key.take();
    // The node object the committed state holds (`initialParagraph`).
    let committed = get_node_by_key_impl(&editor.get_editor_state(), &key).unwrap();
    editor
        .update(
            move || {
                let first_child = || {
                    let child = get_root().get_first_child().unwrap();
                    get_node_by_key_impl(&get_active_editor_state(), child.get_key()).unwrap()
                };
                assert!(Rc::ptr_eq(&first_child(), &committed));
                let copied_paragraph = copy_node(&NodeRef::new(&key));
                assert!(Rc::ptr_eq(&first_child(), &committed));
                assert_ne!(copied_paragraph.get_key(), key);
                assert!(copied_paragraph.get_latest().is::<ExtendedParagraphNode>());
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::$copyNode > returns a shallow copy
#[test]
fn copy_node_returns_a_shallow_copy() {
    let editor = copy_node_editor();
    let keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let k = keys.clone();
    editor
        .update(
            move || {
                let initial_paragraph = create_extended_paragraph_node()?;
                set_string(&initial_paragraph, "non-default");
                set_state(
                    &initial_paragraph,
                    &string_state(),
                    "non-default".to_owned(),
                )?;
                let root = get_root();
                root.clear()?;
                initial_paragraph.append(&[create_text_node("text")])?;
                root.append(std::slice::from_ref(&initial_paragraph))?;
                let copied_paragraph = copy_node(&initial_paragraph);
                root.append(std::slice::from_ref(&copied_paragraph))?;
                set_string(&initial_paragraph, "not-aliased");
                set_state(
                    &initial_paragraph,
                    &string_state(),
                    "not-aliased".to_owned(),
                )?;
                k.borrow_mut().extend([
                    initial_paragraph.get_key().to_owned(),
                    copied_paragraph.get_key().to_owned(),
                ]);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    let keys = keys.take();
    editor
        .read(|| {
            let initial = NodeRef::new(&keys[0]);
            let copied = NodeRef::new(&keys[1]);
            assert_eq!(
                get_root()
                    .get_children()
                    .iter()
                    .map(|n| n.get_key().to_owned())
                    .collect::<Vec<_>>(),
                keys
            );
            assert_eq!(initial.get_text_content(), "text");
            assert_eq!(copied.get_text_content(), "");
            assert_eq!(get_state(&initial, &string_state()).unwrap(), "not-aliased");
            assert_eq!(get_state(&copied, &string_state()).unwrap(), "non-default");
            assert_eq!(string_of(&initial), "not-aliased");
            assert_eq!(string_of(&copied), "non-default");
        })
        .unwrap();
}

// getRegisteredSubtypeMap

/// `[...map].map(([type, subtypes]) => [type, [...subtypes].sort()])`.
fn subtype_object(nodes: &[NodeClass]) -> BTreeMap<&'static str, Vec<&'static str>> {
    get_registered_subtype_map(nodes)
        .into_iter()
        .map(|(node_type, subtypes)| {
            let mut subtypes: Vec<_> = subtypes.into_iter().collect();
            subtypes.sort_unstable();
            (node_type, subtypes)
        })
        .collect()
}

text_class!(TextNodeA, "text-a", ["text"]);

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::getRegisteredSubtypeMap > maps each type to itself and its registered subclass types
#[test]
fn subtype_map_maps_each_type_to_itself_and_its_subclasses() {
    assert_eq!(
        subtype_object(&[
            NodeClass::of::<TextNode>(),
            NodeClass::of::<TabNode>(),
            NodeClass::of::<ParagraphNode>(),
            NodeClass::of::<LineBreakNode>(),
        ]),
        BTreeMap::from([
            ("linebreak", vec!["linebreak"]),
            ("paragraph", vec!["paragraph"]),
            ("tab", vec!["tab"]),
            ("text", vec!["tab", "text"]),
        ])
    );
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::getRegisteredSubtypeMap > expands a $config subclass under its base type
#[test]
fn subtype_map_expands_a_subclass_under_its_base_type() {
    assert_eq!(
        subtype_object(&[NodeClass::of::<TextNode>(), NodeClass::of::<TextNodeA>()]),
        BTreeMap::from([("text", vec!["text", "text-a"]), ("text-a", vec!["text-a"])])
    );
}

// upstream: lexical/src/__tests__/unit/LexicalUtils.test.ts::getRegisteredSubtypeMap > omits an unregistered base type even when a subclass is registered
#[test]
fn subtype_map_omits_an_unregistered_base_type() {
    assert_eq!(
        subtype_object(&[NodeClass::of::<TextNodeA>()]),
        BTreeMap::from([("text-a", vec!["text-a"])])
    );
}
