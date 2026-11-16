//! Ports of `packages/lexical/src/__tests__/unit/LexicalSerializationContext.test.ts`.

use crate::json::{Object, Value, parse};
use crate::upstream_tests::support::json_to_equal;
use crate::{
    CreateEditorArgs, Editor, UpdateOptions, create_editor, create_paragraph_node,
    create_text_node, get_root,
};

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// The JSON as `JSON.stringify` writes it: without `undefined` properties.
fn clean(value: &Value) -> Value {
    parse(&value.to_string()).unwrap()
}

/// `expect(actual).toMatchObject(expected)`, for objects of scalars.
fn assert_matches_object(actual: &Value, expected: &str) {
    let expected = parse(expected).unwrap();
    for (key, value) in expected.as_object().unwrap().iter() {
        assert_eq!(actual.get(key), Some(value), "{key} of {actual}");
    }
}

fn children_of(node: &Value) -> Vec<Value> {
    node.get("children").unwrap().as_array().unwrap().clone()
}

fn has(node: &Value, key: &str) -> bool {
    node.get(key).is_some_and(|v| !v.is_undefined())
}

/// `compact export`'s `beforeEach`: a paragraph of a plain and a bold text.
fn compact_export_editor() -> Editor {
    let editor = create_editor(CreateEditorArgs::default());
    editor
        .update(
            || {
                get_root().clear()?;
                let bold = create_text_node("bold");
                bold.set_format(1.0);
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("plain"), bold])?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();
    editor
}

fn root_json(editor: &Editor, compact: bool) -> Value {
    let state = editor
        .read(|| ())
        .map(|_| editor.get_editor_state())
        .unwrap();
    clean(state.to_json(compact).get("root").unwrap())
}

fn text_and_format(state: &crate::EditorState) -> Vec<(String, f64)> {
    state.read(|| {
        get_root()
            .get_all_text_nodes()
            .iter()
            .map(|n| {
                (
                    n.get_text_content(),
                    n.get_latest().text().unwrap().get_format(),
                )
            })
            .collect()
    })
}

fn state_with_root(root: &Value) -> Value {
    let mut json = Object::new();
    json.insert("root", root.clone());
    Value::Object(json)
}

// upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > legacy is the default and is unchanged
#[test]
fn legacy_is_the_default_and_is_unchanged() {
    let editor = compact_export_editor();
    let root = root_json(&editor, false);
    let paragraph = &children_of(&root)[0];
    let texts = children_of(paragraph);
    // Every property written out, including the deprecated `version`.
    assert_eq!(root.get("version"), Some(&1.0.into()));
    assert_matches_object(
        paragraph,
        r#"{"direction":null,"format":"","indent":0,"type":"paragraph","version":1}"#,
    );
    assert_matches_object(
        &texts[0],
        r#"{"detail":0,"format":0,"mode":"normal","style":"","text":"plain","type":"text","version":1}"#,
    );
    assert_matches_object(&texts[1], r#"{"format":1,"text":"bold"}"#);
}

// upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > compact omits version and every default-valued property
#[test]
fn compact_omits_version_and_every_default_valued_property() {
    let editor = compact_export_editor();
    let root = root_json(&editor, true);
    let paragraph = &children_of(&root)[0];
    let texts = children_of(paragraph);
    assert!(!has(&root, "version"));
    // A paragraph with no formatting keeps only its structure.
    let mut keys: Vec<&str> = paragraph.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["children", "type"]);
    // Text keeps what differs from the default, drops the rest.
    assert!(json_to_equal(
        &texts[0],
        &parse(r#"{"text":"plain","type":"text"}"#).unwrap()
    ));
    assert!(json_to_equal(
        &texts[1],
        &parse(r#"{"format":1,"text":"bold","type":"text"}"#).unwrap()
    ));
}

// upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > compact JSON parses back to the same document as legacy JSON
#[test]
fn compact_json_parses_back_to_the_same_document_as_legacy_json() {
    let editor = compact_export_editor();
    let legacy = root_json(&editor, false);
    let compact = root_json(&editor, true);
    assert!(compact.to_string().encode_utf16().count() < legacy.to_string().encode_utf16().count());

    let from_compact = editor
        .parse_editor_state_json(&state_with_root(&compact))
        .unwrap();
    let from_legacy = editor
        .parse_editor_state_json(&state_with_root(&legacy))
        .unwrap();
    for state in [&from_compact, &from_legacy] {
        assert_eq!(
            text_and_format(state),
            [("plain".to_owned(), 0.0), ("bold".to_owned(), 1.0)]
        );
    }
    assert!(json_to_equal(
        &from_compact.to_json(false),
        &from_legacy.to_json(false)
    ));
}

// upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > a formatted first text child survives compaction (#7971)
#[test]
fn a_formatted_first_text_child_survives_compaction() {
    let editor = compact_export_editor();
    editor
        .update(
            || {
                get_root().clear()?;
                let bold = create_text_node("bold");
                bold.set_format(1.0);
                bold.set_style("color: red");
                let paragraph = create_paragraph_node();
                paragraph.append(&[bold])?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();
    let legacy = root_json(&editor, false);
    let compact = root_json(&editor, true);
    assert_matches_object(
        &children_of(&compact)[0],
        r#"{"textFormat":1,"textStyle":"color: red"}"#,
    );
    let from_legacy = editor
        .parse_editor_state_json(&state_with_root(&legacy))
        .unwrap();
    let from_compact = editor
        .parse_editor_state_json(&state_with_root(&compact))
        .unwrap();
    for state in [from_legacy, from_compact] {
        let fields = state.read(|| {
            let paragraph = get_root().get_first_child_or_throw().unwrap();
            assert!(paragraph.is_element());
            let latest = paragraph.get_latest();
            let element = latest.element().unwrap();
            (
                element.get_text_format(),
                element.get_text_style().to_owned(),
            )
        });
        assert_eq!(fields, (1.0, "color: red".to_owned()));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > the form does not leak outside its callback
#[test]
fn the_form_does_not_leak_outside_its_callback() {
    let editor = compact_export_editor();
    root_json(&editor, true);
    assert_eq!(
        children_of(&root_json(&editor, false))[0].get("version"),
        Some(&1.0.into())
    );
}

#[cfg(feature = "extension")]
mod with_extensions {
    use super::*;
    use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
    use crate::node::NodeBase;
    use crate::schema::{NodeSchema, node_schema, raw_value, with_accessors};
    use crate::{
        ElementData, LexicalNode, NodeRef, TextData, is_compact_export, with_compact_export,
    };
    use crate::{
        InitialEditorStateType, LexicalExtension, NodeClass, create, define_extension, set_slot,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    /// `TestDecoratorNode` with `setIsInline(false)`.
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
    }

    /// `TestShadowRootNode`.
    #[derive(Clone, Debug, Default)]
    struct TestShadowRootNode {
        base: NodeBase,
        element: ElementData,
    }

    impl LexicalNode for TestShadowRootNode {
        fn get_type(&self) -> &'static str {
            "test_shadow_root"
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
        fn is_shadow_root(&self) -> bool {
            true
        }
    }

    fn build(
        name: &str,
        nodes: Vec<NodeClass>,
        initial: impl Fn() -> Result<(), crate::LexicalError> + 'static,
    ) -> LexicalEditorWithDispose {
        build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
            nodes,
            ..LexicalExtension::<(), (), ()>::new(name, ())
        })
        .into()])
        .unwrap()
    }

    fn hi_paragraph() -> Result<(), crate::LexicalError> {
        get_root().clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("hi")])?;
        get_root().append(&[paragraph])
    }

    fn root_of(json: &Value) -> Value {
        json.get("root").unwrap().clone()
    }

    // upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export: slot hosts > a decorator host keeps its slots in both forms
    #[test]
    fn a_decorator_host_keeps_its_slots_in_both_forms() {
        let editor = build(
            "[serialization-slots]",
            vec![
                NodeClass::of::<TestDecoratorNode>(),
                NodeClass::of::<TestShadowRootNode>(),
            ],
            || {
                let host = create::<TestDecoratorNode>();
                let slot = create::<TestShadowRootNode>();
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("inside")])?;
                slot.append(&[paragraph])?;
                get_root().clear()?;
                get_root().append(std::slice::from_ref(&host))?;
                set_slot(&host, "body", &slot)?;
                Ok(())
            },
        );
        for compact in [false, true] {
            let root = editor
                .read(|| editor.get_editor_state().to_json(compact))
                .unwrap();
            // A decorator host has no children array, so its slots are the
            // only subtree it carries.
            let host = &children_of(&root_of(&root))[0];
            assert_eq!(host.get("type"), Some(&"test_decorator".into()));
            assert!(host.get("$slots").unwrap().to_string().contains("inside"));
        }
        editor.dispose();
    }

    fn hi_editor(name: &str) -> LexicalEditorWithDispose {
        build(name, Vec::new(), hi_paragraph)
    }

    // upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::editorState.toJSON states its form at the call site > an explicit form wins over the ambient one, and types as itself
    #[test]
    fn an_explicit_form_wins_over_the_ambient_one() {
        let editor = hi_editor("[serialization-tojson]");
        // The initial state is applied on the first read.
        let state = editor.read(|| editor.get_editor_state()).unwrap();
        assert!(state.to_json(false).to_string().contains("hi"));
        assert!(has(&root_of(&state.to_json(false)), "version"));
        assert!(!has(&root_of(&state.to_json(true)), "version"));
        with_compact_export(true, || {
            assert!(has(&root_of(&state.to_json(false)), "version"));
            assert!(!has(&root_of(&state.to_json(true)), "version"));
        });
        editor.dispose();
    }

    // upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::editorState.toJSON states its form at the call site > no argument writes the legacy form whatever encloses it
    #[test]
    fn no_argument_writes_the_legacy_form_whatever_encloses_it() {
        // `to_json` has no optional argument: JS's `toJSON()` is
        // `to_json(false)`, which must not turn compact inside an enclosing
        // `with_compact_export(true, ...)`.
        let editor = hi_editor("[serialization-tojson]");
        let state = editor.read(|| editor.get_editor_state()).unwrap();
        assert!(has(&root_of(&state.to_json(false)), "version"));
        with_compact_export(true, || {
            assert!(has(&root_of(&state.to_json(false)), "version"));
            // The form is still this call's to state.
            assert!(!has(&root_of(&state.to_json(true)), "version"));
        });
        editor.dispose();
    }

    // upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::editorState.toJSON states its form at the call site > a nested editor follows the document containing it
    #[test]
    fn a_nested_editor_follows_the_document_containing_it() {
        let editor = hi_editor("[serialization-tojson]");
        editor
            .read(|| {
                let json = editor.to_json();
                assert!(has(&root_of(json.get("editorState").unwrap()), "version"));
            })
            .unwrap();
        with_compact_export(true, || {
            editor
                .read(|| {
                    let json = editor.to_json();
                    assert!(!has(&root_of(json.get("editorState").unwrap()), "version"));
                })
                .unwrap();
        });
        editor.dispose();
    }

    // upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::$isCompactExport reports the walk, not a single node > the export walk establishes it, in both directions
    #[test]
    fn the_export_walk_establishes_it_in_both_directions() {
        let editor = hi_editor("[serialization-iscompact]");
        editor
            .read(|| {
                assert!(!is_compact_export());
                with_compact_export(true, || assert!(is_compact_export()));
                // …and restored, including from a nested scope that turns it
                // back off.
                with_compact_export(true, || {
                    with_compact_export(false, || assert!(!is_compact_export()));
                    assert!(is_compact_export());
                });
                assert!(!is_compact_export());
            })
            .unwrap();
        editor.dispose();
    }

    thread_local! {
        static SEEN: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
        static PROBE_NODE_SCHEMA: NodeSchema = node_schema(vec![(
            "probe",
            with_accessors(
                raw_value(),
                Some(Rc::new(|_node| {
                    SEEN.with(|s| s.borrow_mut().push(is_compact_export()));
                    Value::Bool(is_compact_export())
                })),
                // Not `None`: a derived property is skipped by the compact
                // walk without its getter being called at all.
                Some(Rc::new(|_node, _value| {})),
            )
            .unwrap(),
        )])
        .unwrap();
    }

    /// `ProbeNode`: a `TextNode` whose `probe` getter records the form.
    #[derive(Clone, Debug, Default)]
    struct ProbeNode {
        base: NodeBase,
        text: TextData,
    }

    impl LexicalNode for ProbeNode {
        fn get_type(&self) -> &'static str {
            "serialization-probe"
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
        fn json_schema_chain(&self) -> Vec<NodeSchema> {
            vec![
                PROBE_NODE_SCHEMA.with(Clone::clone),
                crate::nodes::text::text_node_schema(),
            ]
        }
    }

    // upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::$isCompactExport reports the walk, not a single node > a schema getter observes it, which is what it is for
    #[test]
    fn a_schema_getter_observes_it_which_is_what_it_is_for() {
        SEEN.with(|s| s.borrow_mut().clear());
        let editor = build(
            "[serialization-probe]",
            vec![NodeClass::of::<ProbeNode>()],
            || {
                get_root().clear()?;
                // A text node with no text is removed by normalization.
                let probe: NodeRef = create::<ProbeNode>();
                probe.set_text_content("x");
                let paragraph = create_paragraph_node();
                paragraph.append(&[probe])?;
                get_root().append(&[paragraph])
            },
        );
        let state = editor.read(|| editor.get_editor_state()).unwrap();
        // Each document walk hands the getter the form that walk writes.
        assert!(state.to_json(false).get("root").is_some());
        assert!(state.to_json(true).get("root").is_some());
        assert_eq!(SEEN.with(|s| s.borrow().clone()), [false, true]);
        editor.dispose();
    }

    // upstream: lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::$isCompactExport reports the walk, not a single node > a bare node.exportJSON(true) does not establish it
    #[test]
    fn a_bare_node_export_json_true_does_not_establish_it() {
        let editor = hi_editor("[serialization-iscompact]");
        editor
            .read(|| {
                let paragraph = get_root().get_first_child_or_throw().unwrap();
                paragraph.export_json(true);
                assert!(!is_compact_export());
            })
            .unwrap();
        editor.dispose();
    }
}
