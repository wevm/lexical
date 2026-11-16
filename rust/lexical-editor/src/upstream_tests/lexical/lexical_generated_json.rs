//! Ports of `packages/lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts`.
//! The generated code of a class is its `LexicalNode::generated_export_json`
//! and `generated_update_from_json` (`LexicalGeneratedJSON.ts`, ported by
//! hand), which the walk of its schema (`schema::walk_export_json`) checks.

use crate::json::{Object, Value, parse};
use crate::lexical_utils::with_writable;
use crate::node::NodeBase;
use crate::nodes::line_break::{LineBreakNode, create_line_break_node};
use crate::nodes::paragraph::ParagraphNode;
use crate::nodes::tab::{TabNode, create_tab_node};
use crate::nodes::text::TextNode;
use crate::schema::walk_export_json;
use crate::upstream_tests::support::json_to_equal;
use crate::{
    CreateEditorArgs, Direction, Editor, LexicalNode, LexicalNodeReplacement, NodeClass, NodeRef,
    RequiredNodeStateConfig, StateConfig, TextData, UpdateOptions, create, create_editor,
    create_paragraph_node, create_state, create_text_node, get_root, get_state, set_state,
};
use std::rc::Rc;

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn update(editor: &Editor, f: impl FnOnce() -> Result<(), crate::LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

/// `initializeUnitTest`'s editor.
fn test_editor() -> Editor {
    create_editor(CreateEditorArgs {
        namespace: "test".to_owned(),
        ..CreateEditorArgs::default()
    })
}

fn object(json: &str) -> Object {
    parse(json).unwrap().as_object().unwrap().clone()
}

/// `$expectSameJSON(node)`: the generated exporter agrees with the walk in
/// both forms, key order too.
fn expect_same_json(node: &NodeRef) {
    let latest = node.get_latest();
    assert!(latest.generated_export_json(false).is_some());
    for compact in [false, true] {
        let Some(from_generated) = latest.generated_export_json(compact) else {
            continue;
        };
        let from_walk = walk_export_json(&*latest, compact);
        assert!(
            json_to_equal(
                &Value::Object(from_generated.clone()),
                &Value::Object(from_walk.clone())
            ),
            "{compact}: {} vs {}",
            Value::Object(from_generated.clone()),
            Value::Object(from_walk.clone())
        );
        assert_eq!(
            from_generated.keys().collect::<Vec<_>>(),
            from_walk.keys().collect::<Vec<_>>()
        );
    }
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated exportJSON > every generated exporter agrees with the schema-driven walk
#[test]
fn every_generated_exporter_agrees_with_the_schema_driven_walk() {
    let editor = test_editor();
    update(&editor, || {
        let text = create_text_node("hello");
        text.set_format_type("bold");
        text.set_style("color: red");
        expect_same_json(&text);
        expect_same_json(&create_text_node(""));
        let token = create_text_node("tok");
        token.set_mode("token")?;
        token.set_detail_type("directionless")?;
        expect_same_json(&token);
        expect_same_json(&create_line_break_node());
        expect_same_json(&create_tab_node());
        let paragraph = create_paragraph_node();
        paragraph.set_direction(Some(Direction::Rtl));
        paragraph.set_indent(3.0)?;
        expect_same_json(&paragraph);
        expect_same_json(&create_paragraph_node());
        // Both arms of the `when` gate.
        let styled = create_paragraph_node();
        styled.set_text_format(f64::from(crate::constants::IS_BOLD));
        styled.set_text_style("color: red");
        expect_same_json(&styled);
        // The predicate false with a value that is not the default.
        let with_text = create_paragraph_node();
        with_text.set_text_format(f64::from(crate::constants::IS_BOLD));
        with_text.set_text_style("color: red");
        with_text.append(&[create_text_node("x")])?;
        expect_same_json(&with_text);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated exportJSON > an exportJSON override composes with its generated exporter
#[test]
fn an_export_json_override_composes_with_its_generated_exporter() {
    let editor = test_editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        let text = create_text_node("x");
        text.set_format_type("bold");
        paragraph.append(&[text])?;
        let json = paragraph.export_json(false);
        // The first text child's format, not the paragraph's own.
        assert_eq!(json.get("textFormat"), Some(&1.0.into()));
        assert_eq!(json.get("textStyle"), Some(&"".into()));
        assert_eq!(json.get("type"), Some(&"paragraph".into()));
        // The back-fill assigns into keys the generated literal already has,
        // at their schema position.
        assert_eq!(
            json.keys().collect::<Vec<_>>(),
            [
                "children",
                "direction",
                "format",
                "indent",
                "textFormat",
                "textStyle",
                "type",
                "version"
            ]
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated exportJSON > NodeState still reaches the JSON through a generated exporter
#[test]
fn node_state_still_reaches_the_json_through_a_generated_exporter() {
    let editor = test_editor();
    update(&editor, || {
        let node = create_text_node("hi");
        assert!(node.export_json(false).get("$").is_none());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the compact form is generated too > every class that has a legacy form has one
#[test]
fn every_class_that_has_a_legacy_form_has_one() {
    let nodes: [Box<dyn LexicalNode>; 4] = [
        Box::new(TextNode::default()),
        Box::new(ParagraphNode::default()),
        Box::new(LineBreakNode::default()),
        Box::new(TabNode::default()),
    ];
    let editor = test_editor();
    editor
        .read(|| {
            for node in &nodes {
                let legacy = node.generated_export_json(false).expect("generated code");
                let compact = node.generated_export_json(true).expect("a compact form");
                assert_ne!(Value::Object(legacy), Value::Object(compact));
            }
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the compact form is generated too > it omits what parsing restores, and nothing else
#[test]
fn it_omits_what_parsing_restores_and_nothing_else() {
    let editor = test_editor();
    update(&editor, || {
        // Every property at its default: the compact form is the type alone.
        assert!(json_to_equal(
            &Value::Object(create_text_node("").export_json(true)),
            &parse(r#"{"type":"text"}"#).unwrap()
        ));
        let token = create_text_node("hi");
        token.set_mode("token")?;
        assert!(json_to_equal(
            &Value::Object(token.export_json(true)),
            &parse(r#"{"mode":"token","text":"hi","type":"text"}"#).unwrap()
        ));
        Ok(())
    });
}

thread_local! {
    static PROBE_STATE: StateConfig<String> =
        create_state("probeFlag", |v| v.as_str().unwrap_or("").to_owned());
    static FLAT_STATE: StateConfig<String> =
        create_state("flatFlag", |v| v.as_str().unwrap_or("").to_owned());
}

/// A text node class of its own, without generated code unless a test
/// adds it.
macro_rules! text_class {
    ($name:ident, $type:literal $(, $($body:tt)*)?) => {
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
                vec!["text"]
            }
            $($($body)*)?
        }
    };
}

text_class! {
    StatefulText,
    "stateful-text",
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![PROBE_STATE.with(|s| RequiredNodeStateConfig::new(s, false))]
    }
}

text_class!(ReplacedText, "replaced-text");

text_class! {
    FlatStateText,
    "flat-state-text",
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![FLAT_STATE.with(|s| RequiredNodeStateConfig::new(s, true))]
    }
    /// A parser that applies only the schema's fields, as every generated
    /// parser does.
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        self.text.text = json
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        true
    }
}

fn editor_with(nodes: Vec<NodeClass>) -> Editor {
    create_editor(CreateEditorArgs {
        nodes,
        ..CreateEditorArgs::default()
    })
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the synthesized importJSON > NodeState in the JSON is applied
#[test]
fn node_state_in_the_json_is_applied() {
    let editor = editor_with(vec![NodeClass::of::<StatefulText>()]);
    update(&editor, || {
        let node = create::<StatefulText>();
        node.set_text_content("hi");
        set_state(&node, &PROBE_STATE.with(Clone::clone), "kept".to_owned())?;
        get_root().clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[node])?;
        get_root().append(&[paragraph])
    });
    let json = editor.get_editor_state().to_json(false).to_string();
    assert!(json.contains(&format!("\"{}\"", crate::constants::NODE_STATE_KEY)));
    editor.parse_editor_state(&json).unwrap().read(|| {
        let paragraph = get_root().get_first_child_or_throw().unwrap();
        assert_eq!(paragraph.get_type(), "paragraph");
        let text = paragraph.get_first_child_or_throw().unwrap();
        assert_eq!(
            get_state(&text, &PROBE_STATE.with(Clone::clone)).unwrap(),
            "kept"
        );
    });
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the synthesized importJSON > a replacement of the class is imported as itself
#[test]
fn a_replacement_of_the_class_is_imported_as_itself() {
    let editor = editor_with(vec![
        NodeClass::of::<ReplacedText>(),
        LexicalNodeReplacement {
            replace: NodeClass::of::<TextNode>(),
            with: Rc::new(|node: &NodeRef| {
                let replacement = create::<ReplacedText>();
                replacement.set_text_content(&node.get_text_content());
                Ok(replacement)
            }),
            with_klass: Some(NodeClass::of::<ReplacedText>()),
        }
        .into(),
    ]);
    update(&editor, || {
        let node = create_text_node("hello");
        node.set_style("color: red");
        assert!(node.get_latest().is::<ReplacedText>());
        get_root().clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[node])?;
        get_root().append(&[paragraph])
    });
    let json = editor.get_editor_state().to_json(false).to_string();
    editor.parse_editor_state(&json).unwrap().read(|| {
        let paragraph = get_root().get_first_child_or_throw().unwrap();
        assert_eq!(paragraph.get_type(), "paragraph");
        let node = paragraph.get_first_child_or_throw().unwrap();
        assert!(node.get_latest().is::<ReplacedText>());
        assert_eq!(node.get_text_content(), "hello");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the synthesized importJSON > flat NodeState is applied before the generated parser runs
#[test]
fn flat_node_state_is_applied_before_the_generated_parser_runs() {
    let editor = editor_with(vec![NodeClass::of::<FlatStateText>()]);
    update(&editor, || {
        // `FlatStateText.importJSON(json)`.
        let import_json = crate::get_editor()
            .get_registered_node("flat-state-text")
            .unwrap()
            .import_json;
        let node = import_json(&object(
            r#"{"flatFlag":"set","text":"hello","type":"flat-state-text","version":1}"#,
        ))?;
        assert_eq!(get_state(&node, &FLAT_STATE.with(Clone::clone))?, "set");
        assert_eq!(node.get_text_content(), "hello");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated updateFromJSON > only a class the generator could compile has one
#[test]
fn only_a_class_the_generator_could_compile_has_one() {
    let empty = Object::new();
    assert!(TextNode::default().generated_update_from_json(&empty));
    assert!(ParagraphNode::default().generated_update_from_json(&empty));
    // TabNode applies only what it does not derive, with a parser of its own.
    assert!(TabNode::default().generated_update_from_json(&empty));
    // LineBreakNode declares no properties: generated export, no parser.
    let editor = test_editor();
    editor
        .read(|| {
            assert!(
                LineBreakNode::default()
                    .generated_export_json(false)
                    .is_some()
            )
        })
        .unwrap();
    assert!(!LineBreakNode::default().generated_update_from_json(&empty));
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated updateFromJSON > a generated parser returns the node the walk would have
#[test]
fn a_generated_parser_returns_the_node_the_walk_would_have() {
    // A Rust parser writes the writable node it is given, so the node it
    // ends on is the one in the editor state.
    let editor = create_editor(CreateEditorArgs::default());
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        let update_paragraph = |json: &str| {
            let json = object(json);
            with_writable(paragraph.get_key(), |node| {
                assert!(node.generated_update_from_json(&json));
            });
        };
        update_paragraph(r#"{"format":"center","indent":3}"#);
        let element =
            |f: fn(&crate::ElementData) -> Value| f(paragraph.get_latest().element().unwrap());
        assert_eq!(
            element(|e| e.get_format_type().into()),
            Value::from("center")
        );
        assert_eq!(element(|e| e.get_indent().into()), Value::from(3.0));
        // The domain is the emitted bounds': indent is `integer, min 0`.
        update_paragraph(r#"{"indent":-2}"#);
        assert_eq!(element(|e| e.get_indent().into()), Value::from(0.0));
        update_paragraph(r#"{"indent":1.5}"#);
        assert_eq!(element(|e| e.get_indent().into()), Value::from(0.0));
        Ok(())
    });
}

fn text_fields(node: &NodeRef) -> (f64, f64, &'static str) {
    let latest = node.get_latest();
    let text = latest.text().unwrap();
    (text.get_format(), text.get_detail(), text.get_mode())
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated updateFromJSON > a hostile key cannot reach Object.prototype through a table
#[test]
fn a_hostile_key_cannot_reach_object_prototype_through_a_table() {
    let editor = test_editor();
    update(&editor, || {
        for hostile in [
            "toString",
            "constructor",
            "hasOwnProperty",
            "__proto__",
            "valueOf",
        ] {
            let node = create_text_node("");
            node.update_from_json(&object(&format!(
                r#"{{"detail":"{hostile}","format":"{hostile}","mode":"{hostile}","style":"","text":""}}"#
            )))?;
            assert_eq!(text_fields(&node), (0.0, 0.0, "normal"));
        }
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated updateFromJSON > the legacy string spellings still parse
#[test]
fn the_legacy_string_spellings_still_parse() {
    let editor = test_editor();
    update(&editor, || {
        let node = create_text_node("");
        node.update_from_json(&object(
            r#"{"detail":"unmergeable","format":"bold","mode":"token","style":"","text":"x"}"#,
        ))?;
        assert_eq!(text_fields(&node), (1.0, 2.0, "token"));
        Ok(())
    });
}
