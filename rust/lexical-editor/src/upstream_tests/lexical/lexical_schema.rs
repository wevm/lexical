//! Ports of `packages/lexical/src/__tests__/unit/LexicalSchema.test.ts`,
//! after Swift's (`swift/Tests/LexicalTests/LexicalSchema*Tests.swift`). A
//! schema's domain is the crate's JSON [`Value`], whose `Undefined` is JS's
//! `undefined`; a JS `toBe` on an array or object compares content here, as
//! the schema module documents.

use crate::json::{Object, Value, parse};
use crate::lexical_utils::with_writable;
use crate::node::NodeBase;
use crate::schema::{
    NodeSchema, NumberValueOptions, SerializationSchema, SerializationSchemaMeta, aliased_value,
    array_value, boolean_value, enum_value, is_schema_default, is_schema_equal, node_schema,
    nullable, number_value, object_value, optional, raw_value, string_value, transform_value,
    union_value, with_accessors, with_field,
};
use crate::upstream_tests::support::json_to_equal;
use crate::{
    CreateEditorArgs, Editor, ElementData, LexicalError, LexicalNode, NodeClass, NodeRef,
    UpdateOptions, create, create_editor, create_paragraph_node, create_text_node, get_root,
};
use std::rc::Rc;

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

/// `initializeUnitTest`'s editor.
fn test_editor() -> Editor {
    create_editor(CreateEditorArgs {
        namespace: "test".to_owned(),
        ..CreateEditorArgs::default()
    })
}

fn json(text: &str) -> Value {
    parse(text).unwrap()
}

fn object(text: &str) -> Object {
    json(text).as_object().unwrap().clone()
}

/// `schema(JSON.parse(text))`.
fn p(schema: &SerializationSchema, text: &str) -> Value {
    schema.parse(&json(text))
}

/// `schema(undefined)`.
fn undef(schema: &SerializationSchema) -> Value {
    schema.parse(&Value::Undefined)
}

fn num(default: f64) -> SerializationSchema {
    number_value(default, NumberValueOptions::default()).unwrap()
}

fn num_with(default: f64, options: NumberValueOptions) -> SerializationSchema {
    number_value(default, options).unwrap()
}

fn strs(values: &[&str]) -> Vec<Value> {
    values.iter().map(|v| Value::from(*v)).collect()
}

fn enumeration(values: &[&str]) -> SerializationSchema {
    enum_value(strs(values), None).unwrap()
}

fn accepts(schema: &SerializationSchema, value: &Value) -> bool {
    schema.accepts().expect("an accepts")(value)
}

fn text_of(node: &NodeRef) -> crate::TextData {
    node.text_data()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > stringValue
#[test]
fn string_value_parses() {
    let parse = string_value("");
    assert_eq!(p(&parse, r#""hello""#), Value::from("hello"));
    assert_eq!(p(&parse, r#""""#), Value::from(""));
    assert_eq!(undef(&parse), Value::from(""));
    assert_eq!(p(&parse, "null"), Value::from(""));
    assert_eq!(p(&parse, "42"), Value::from(""));
    assert_eq!(p(&parse, "{}"), Value::from(""));
    assert_eq!(undef(&string_value("fallback")), Value::from("fallback"));
    assert_eq!(p(&string_value("fallback"), "42"), Value::from("fallback"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > numberValue
#[test]
fn number_value_parses() {
    let parse = num(0.0);
    assert_eq!(p(&parse, "42"), json("42"));
    assert_eq!(p(&parse, "0"), json("0"));
    assert_eq!(p(&parse, "-1.5"), json("-1.5"));
    assert_eq!(p(&parse, r#""42""#), json("42"));
    assert_eq!(undef(&parse), json("0"));
    assert_eq!(p(&parse, r#""forty-two""#), json("0"));
    for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(parse.parse(&Value::Number(n)), json("0"));
    }
    assert_eq!(undef(&num(1.0)), json("1"));
    assert_eq!(num(1.0).parse(&Value::Number(f64::NAN)), json("1"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > booleanValue
#[test]
fn boolean_value_parses() {
    let parse = boolean_value(false);
    assert_eq!(p(&parse, "true"), json("true"));
    assert_eq!(p(&parse, "false"), json("false"));
    assert_eq!(undef(&parse), json("false"));
    assert_eq!(p(&parse, "0"), json("false"));
    assert_eq!(p(&parse, r#""true""#), json("false"));
    assert_eq!(undef(&boolean_value(true)), json("true"));
    assert_eq!(p(&boolean_value(true), "1"), json("true"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > enumValue
#[test]
fn enum_value_parses() {
    let parse = enumeration(&["normal", "token", "segmented"]);
    assert_eq!(p(&parse, r#""token""#), Value::from("token"));
    assert_eq!(p(&parse, r#""bogus""#), Value::from("normal"));
    assert_eq!(undef(&parse), Value::from("normal"));
    assert_eq!(p(&parse, "42"), Value::from("normal"));
    let with_default = enum_value(strs(&["normal", "token"]), Some("token".into())).unwrap();
    assert_eq!(p(&with_default, r#""bogus""#), Value::from("token"));
    let direction = enum_value(vec![Value::Null, "ltr".into(), "rtl".into()], None).unwrap();
    assert_eq!(p(&direction, r#""rtl""#), Value::from("rtl"));
    assert_eq!(p(&direction, r#""bogus""#), Value::Null);
    let error = enum_value(Vec::new(), None).unwrap_err();
    assert!(error.message().contains("must not be empty"), "{error}");
    assert_eq!(p(&direction, "null"), Value::Null);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > nullable
#[test]
fn nullable_parses() {
    let parse = nullable(string_value(""), false).unwrap();
    assert_eq!(p(&parse, r#""noopener""#), Value::from("noopener"));
    assert_eq!(p(&parse, r#""""#), Value::from(""));
    assert_eq!(p(&parse, "null"), Value::Null);
    assert_eq!(undef(&parse), Value::Null);
    assert_eq!(p(&parse, "42"), Value::from(""));
    let direction = nullable(enumeration(&["ltr", "rtl"]), false).unwrap();
    assert_eq!(p(&direction, r#""rtl""#), Value::from("rtl"));
    assert_eq!(p(&direction, "null"), Value::Null);
    assert_eq!(undef(&direction), Value::Null);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > optional
#[test]
fn optional_parses() {
    let parse = optional(num(0.0), false).unwrap();
    assert_eq!(p(&parse, "120"), json("120"));
    assert_eq!(undef(&parse), Value::Undefined);
    assert_eq!(p(&parse, "null"), json("0"));
    assert_eq!(p(&parse, r#""nope""#), json("0"));
    let language = optional(nullable(string_value(""), false).unwrap(), false).unwrap();
    assert_eq!(undef(&language), Value::Undefined);
    assert_eq!(p(&language, "null"), Value::Null);
    assert_eq!(p(&language, r#""js""#), Value::from("js"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > arrayValue
#[test]
fn array_value_parses() {
    let parse = array_value(string_value("")).unwrap();
    assert_eq!(p(&parse, r#"["a","b"]"#), json(r#"["a","b"]"#));
    assert_eq!(p(&parse, "[]"), json("[]"));
    assert_eq!(p(&parse, r#"["a",42,null]"#), json(r#"["a","",""]"#));
    assert_eq!(undef(&parse), json("[]"));
    assert_eq!(p(&parse, r#""nope""#), json("[]"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > objectValue validates and fully populates
#[test]
fn object_value_validates_and_fully_populates() {
    let parse = object_value(vec![
        ("count", num(0.0)),
        ("tags", array_value(string_value("")).unwrap()),
        ("title", string_value("")),
    ])
    .unwrap();
    assert_eq!(
        p(&parse, r#"{"count":2,"tags":["x"],"title":"hi"}"#),
        json(r#"{"count":2,"tags":["x"],"title":"hi"}"#)
    );
    assert_eq!(
        p(&parse, r#"{"count":"bad","extra":true}"#),
        json(r#"{"count":0,"tags":[],"title":""}"#)
    );
    assert_eq!(undef(&parse), json(r#"{"count":0,"tags":[],"title":""}"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > the default is recoverable via parse(undefined)
#[test]
fn the_default_is_recoverable_via_parse_undefined() {
    assert_eq!(undef(&string_value("")), Value::from(""));
    assert_eq!(undef(&num(7.0)), json("7"));
    assert_eq!(undef(&boolean_value(true)), json("true"));
    let b = enum_value(strs(&["a", "b"]), Some("b".into())).unwrap();
    assert_eq!(undef(&b), Value::from("b"));
    assert_eq!(
        undef(&nullable(string_value(""), false).unwrap()),
        Value::Null
    );
}

/// A document with one paragraph holding `children`.
fn paragraph_document(children: &str) -> String {
    format!(
        r#"{{"root":{{"children":[{{"children":[{children}],"type":"paragraph","version":1}}],"type":"root","version":1}}}}"#
    )
}

fn expect_text(node: &NodeRef, text: &str, format: f64, detail: f64, mode: &str, style: &str) {
    let data = text_of(node);
    assert_eq!(data.get_text(), text);
    assert_eq!(data.get_format(), format);
    assert_eq!(data.get_detail(), detail);
    assert_eq!(data.get_mode(), mode);
    assert_eq!(data.get_style(), style);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TextNode applies defaults when properties are missing
#[test]
fn text_node_applies_defaults_when_properties_are_missing() {
    update(&test_editor(), || {
        let node = create_text_node("initial");
        node.update_from_json(&Object::new())?;
        expect_text(&node, "", 0.0, 0.0, "normal", "");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TextNode rejects out-of-domain values
#[test]
fn text_node_rejects_out_of_domain_values() {
    update(&test_editor(), || {
        let node = create_text_node("initial");
        node.update_from_json(&object(
            r#"{"detail":"nope","format":null,"mode":"bogus","style":42,"text":99}"#,
        ))?;
        expect_text(&node, "", 0.0, 0.0, "normal", "");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TextNode preserves valid values
#[test]
fn text_node_preserves_valid_values() {
    update(&test_editor(), || {
        let node = create_text_node("initial");
        node.update_from_json(&object(
            r#"{"detail":0,"format":1,"mode":"token","style":"color: red","text":"hello"}"#,
        ))?;
        expect_text(&node, "hello", 1.0, 0.0, "token", "color: red");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > parseEditorState round-trips compact node JSON
#[test]
fn parse_editor_state_round_trips_compact_node_json() {
    let editor = test_editor();
    let state = editor
        .parse_editor_state(&paragraph_document(
            r#"{"text":"hello","type":"text","version":1}"#,
        ))
        .unwrap();
    state.read(|| {
        let texts = get_root().get_all_text_nodes();
        assert_eq!(texts.len(), 1);
        expect_text(&texts[0], "hello", 0.0, 0.0, "normal", "");
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TextNode accepts the legacy string names for format and detail
#[test]
fn text_node_accepts_the_legacy_string_names_for_format_and_detail() {
    update(&test_editor(), || {
        let node = create_text_node("x");
        node.update_from_json(&object(r#"{"detail":"directionless","format":"bold"}"#))?;
        assert!(node.has_format("bold"));
        assert!(node.is_directionless());
        Ok(())
    });
}

fn expect_tab(node: &NodeRef) {
    assert!(node.get_latest().is::<crate::nodes::tab::TabNode>());
    assert_eq!(node.get_text_content(), "\t");
    assert!(node.is_unmergeable());
    assert_eq!(text_of(node).get_mode(), "normal");
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TabNode ignores JSON for its fixed properties
#[test]
fn tab_node_ignores_json_for_its_fixed_properties() {
    let editor = test_editor();
    for bad in [
        r#","mode":"token""#,
        r#","detail":0"#,
        r#","text":"xyz""#,
        r#","detail":"directionless","mode":"segmented","text":"""#,
    ] {
        let state = editor
            .parse_editor_state(&paragraph_document(&format!(
                r#"{{"type":"tab","version":1{bad}}}"#
            )))
            .unwrap();
        state.read(|| expect_tab(&get_root().get_all_text_nodes()[0]));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TabNode imports fully compact JSON
#[test]
fn tab_node_imports_fully_compact_json() {
    let editor = test_editor();
    let state = editor
        .parse_editor_state(&paragraph_document(
            r#"{"type":"tab","version":1},{"detail":"bogus","text":"xyz","type":"tab","version":1}"#,
        ))
        .unwrap();
    state.read(|| {
        let texts = get_root().get_all_text_nodes();
        assert_eq!(texts.len(), 2);
        texts.iter().for_each(expect_tab);
    });
}

fn number_meta(schema: &SerializationSchema) -> (Option<f64>, Option<f64>, bool, bool) {
    match schema.meta() {
        SerializationSchemaMeta::Number {
            min,
            max,
            integer,
            clamp,
        } => (*min, *max, *integer, *clamp),
        _ => panic!("not a number schema"),
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > min rejects values below the bound
#[test]
fn min_rejects_values_below_the_bound() {
    let span = num_with(
        1.0,
        NumberValueOptions {
            min: Some(1.0),
            ..Default::default()
        },
    );
    assert_eq!(p(&span, "3"), json("3"));
    assert_eq!(p(&span, "0"), json("1"));
    assert_eq!(p(&span, "-4"), json("1"));
    assert_eq!(undef(&span), json("1"));
    assert_eq!(span.default_value(), &json("1"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > integer rejects fractional values
#[test]
fn integer_rejects_fractional_values() {
    let span = num_with(
        1.0,
        NumberValueOptions {
            integer: true,
            min: Some(1.0),
            ..Default::default()
        },
    );
    assert_eq!(p(&span, "2"), json("2"));
    assert_eq!(p(&span, "2.5"), json("1"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > max rejects values above the bound
#[test]
fn max_rejects_values_above_the_bound() {
    let pct = num_with(
        0.0,
        NumberValueOptions {
            min: Some(0.0),
            max: Some(100.0),
            ..Default::default()
        },
    );
    assert_eq!(p(&pct, "100"), json("100"));
    assert_eq!(p(&pct, "101"), json("0"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > clamp brings an out-of-range value to the nearest bound
#[test]
fn clamp_brings_an_out_of_range_value_to_the_nearest_bound() {
    let indent = num_with(
        0.0,
        NumberValueOptions {
            integer: true,
            min: Some(0.0),
            max: Some(128.0),
            clamp: true,
        },
    );
    assert_eq!(p(&indent, "5"), json("5"));
    assert_eq!(p(&indent, "1e6"), json("128"));
    assert_eq!(p(&indent, "-4"), json("0"));
    assert_eq!(p(&indent, r#""1e6""#), json("128"));
    assert_eq!(p(&indent, "1.5"), json("0"));
    assert_eq!(indent.parse(&Value::Number(f64::INFINITY)), json("0"));
    assert_eq!(p(&indent, r#""banana""#), json("0"));
    assert_eq!(undef(&indent), json("0"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > an integer domain rounds its bounds inward
#[test]
fn an_integer_domain_rounds_its_bounds_inward() {
    let parse = num_with(
        1.0,
        NumberValueOptions {
            integer: true,
            min: Some(0.5),
            max: Some(2.5),
            clamp: true,
        },
    );
    assert_eq!(p(&parse, "0"), json("1"));
    assert_eq!(p(&parse, "3"), json("2"));
    assert_eq!(parse.parse(&p(&parse, "0")), p(&parse, "0"));
    assert_eq!(parse.parse(&p(&parse, "3")), p(&parse, "3"));
    assert_eq!(number_meta(&parse), (Some(1.0), Some(2.0), true, true));
    let strict = num_with(
        1.0,
        NumberValueOptions {
            integer: true,
            min: Some(0.5),
            max: Some(2.5),
            clamp: false,
        },
    );
    assert_eq!(p(&strict, "0"), json("1"));
    assert_eq!(p(&strict, "1"), json("1"));
    assert_eq!(p(&strict, "2"), json("2"));
    assert_eq!(p(&strict, "3"), json("1"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > a fractional domain keeps its bounds
#[test]
fn a_fractional_domain_keeps_its_bounds() {
    let options = NumberValueOptions {
        min: Some(0.5),
        max: Some(2.5),
        ..Default::default()
    };
    assert_eq!(
        number_meta(&num_with(1.0, options)),
        (Some(0.5), Some(2.5), false, false)
    );
    let clamped = num_with(
        1.0,
        NumberValueOptions {
            clamp: true,
            ..options
        },
    );
    assert_eq!(p(&clamped, "3"), json("2.5"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > an empty domain is refused where it is declared
#[test]
fn an_empty_domain_is_refused_where_it_is_declared() {
    let error = number_value(
        0.0,
        NumberValueOptions {
            min: Some(2.0),
            max: Some(1.0),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        error
            .message()
            .contains("the domain is empty; min 2 is above max 1"),
        "{error}"
    );
    let error = number_value(
        0.0,
        NumberValueOptions {
            integer: true,
            min: Some(0.5),
            max: Some(0.7),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        error.message().contains("rounded inward from 0.5..0.7"),
        "{error}"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > clamp widens what the domain accepts
#[test]
fn clamp_widens_what_the_domain_accepts() {
    let max = |clamp| {
        num_with(
            0.0,
            NumberValueOptions {
                max: Some(10.0),
                clamp,
                ..Default::default()
            },
        )
    };
    assert!(accepts(&max(true), &json("99")));
    assert!(!accepts(&max(false), &json("99")));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > the domain is recorded on meta
#[test]
fn the_domain_is_recorded_on_meta() {
    let integer = num_with(
        1.0,
        NumberValueOptions {
            integer: true,
            min: Some(1.0),
            ..Default::default()
        },
    );
    assert_eq!(number_meta(&integer), (Some(1.0), None, true, false));
    let clamped = num_with(
        0.0,
        NumberValueOptions {
            max: Some(8.0),
            clamp: true,
            ..Default::default()
        },
    );
    assert_eq!(number_meta(&clamped), (None, Some(8.0), false, true));
}

fn omit_default_width() -> SerializationSchema {
    optional(num(0.0), true).unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > optional({omitDefault}) > treats a default-valued input as absent
#[test]
fn treats_a_default_valued_input_as_absent() {
    let width = omit_default_width();
    assert_eq!(p(&width, "120"), json("120"));
    assert_eq!(p(&width, "0"), Value::Undefined);
    assert_eq!(p(&width, r#""nonsense""#), Value::Undefined);
    assert_eq!(undef(&width), Value::Undefined);
    assert_eq!(width.default_value(), &Value::Undefined);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > optional({omitDefault}) > matches the `value || undefined` idiom it replaces
#[test]
fn matches_the_value_or_undefined_idiom_it_replaces() {
    let width = omit_default_width();
    for value in [json("0"), json("120"), json("-3"), Value::Undefined] {
        let idiom = if value == json("0") {
            Value::Undefined
        } else {
            value.clone()
        };
        assert_eq!(width.parse(&value), idiom);
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > optional({omitDefault}) > without the option a default-valued input is kept
#[test]
fn without_the_option_a_default_valued_input_is_kept() {
    assert_eq!(p(&optional(num(0.0), false).unwrap(), "0"), json("0"));
}

fn dimension() -> SerializationSchema {
    union_value(
        vec![num(0.0), enumeration(&["inherit"])],
        Some("inherit".into()),
    )
    .unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > accepts a value from either member
#[test]
fn union_accepts_a_value_from_either_member() {
    let dimension = dimension();
    assert_eq!(p(&dimension, "640"), json("640"));
    assert_eq!(p(&dimension, r#""inherit""#), Value::from("inherit"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > falls back when no member accepts the value
#[test]
fn union_falls_back_when_no_member_accepts_the_value() {
    let dimension = dimension();
    assert_eq!(p(&dimension, r#""banana""#), Value::from("inherit"));
    assert_eq!(undef(&dimension), Value::from("inherit"));
    assert_eq!(dimension.default_value(), &Value::from("inherit"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > defaults to the first member default when none is given
#[test]
fn union_defaults_to_the_first_member_default_when_none_is_given() {
    let union = union_value(vec![num(7.0), enumeration(&["x"])], None).unwrap();
    assert_eq!(p(&union, "null"), json("7"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > records its members on meta
#[test]
fn union_records_its_members_on_meta() {
    let dimension = dimension();
    let SerializationSchemaMeta::Union { members } = dimension.meta() else {
        panic!("not a union");
    };
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].meta().kind(), "number");
    assert_eq!(members[1].meta().kind(), "enum");
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > defers equality to whichever member has one
#[test]
fn union_defers_equality_to_whichever_member_has_one() {
    let tags = union_value(
        vec![
            array_value(string_value("")).unwrap(),
            enumeration(&["all"]),
        ],
        None,
    )
    .unwrap();
    assert!(is_schema_equal(&tags, &json(r#"["a"]"#), &json(r#"["a"]"#)));
    assert!(!is_schema_equal(
        &tags,
        &json(r#"["a"]"#),
        &json(r#"["b"]"#)
    ));
    assert!(is_schema_equal(&tags, &"all".into(), &"all".into()));
    assert!(!is_schema_equal(&tags, &"all".into(), &json(r#"["a"]"#)));
    let dimension = dimension();
    assert!(is_schema_equal(&dimension, &json("640"), &json("640")));
    assert!(!is_schema_equal(
        &dimension,
        &json("640"),
        &"inherit".into()
    ));
}

/// `transformValue(unionValue([numberValue(), enumValue(['bold',
/// 'italic'])], 0), v => typeof v === 'string' ? NAME_TO_BIT[v] : v)`.
fn format_schema() -> SerializationSchema {
    transform_value(
        union_value(
            vec![num(0.0), enumeration(&["bold", "italic"])],
            Some(json("0")),
        )
        .unwrap(),
        |value| match value.as_str() {
            Some("bold") => json("1"),
            Some("italic") => json("2"),
            Some(_) => Value::Undefined,
            None => value,
        },
        None,
    )
    .unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > transformValue > normalizes accepted values into the target domain
#[test]
fn transform_normalizes_accepted_values_into_the_target_domain() {
    let format = format_schema();
    assert_eq!(p(&format, "4"), json("4"));
    assert_eq!(p(&format, r#""bold""#), json("1"));
    assert_eq!(p(&format, r#""italic""#), json("2"));
    assert_eq!(p(&format, r#""junk""#), json("0"));
    assert_eq!(undef(&format), json("0"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > transformValue > defaultValue is the transformed inner default
#[test]
fn transform_default_value_is_the_transformed_inner_default() {
    assert_eq!(format_schema().default_value(), &json("0"));
    let upper = transform_value(
        string_value("a"),
        |v| Value::String(v.as_str().unwrap().to_uppercase()),
        None,
    )
    .unwrap();
    assert_eq!(upper.default_value(), &Value::from("A"));
    assert_eq!(p(&upper, r#""bc""#), Value::from("BC"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > transformValue > meta names the transform and holds the inner schema
#[test]
fn transform_meta_names_the_transform_and_holds_the_inner_schema() {
    let inner = string_value("");
    let t = transform_value(
        inner.clone(),
        |v| Value::Number(v.as_str().unwrap().encode_utf16().count() as f64),
        None,
    )
    .unwrap();
    assert_eq!(t.meta().kind(), "transform");
    let SerializationSchemaMeta::Transform { inner: held } = t.meta() else {
        panic!("not a transform");
    };
    assert!(held.is(&inner));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > rawValue > passes an unvalidated value through
#[test]
fn raw_value_passes_an_unvalidated_value_through() {
    let raw = raw_value();
    let value = json(r#"{"editorState":{"root":{}}}"#);
    assert_eq!(raw.parse(&value), value);
    assert_eq!(undef(&raw), Value::Undefined);
    assert_eq!(raw.default_value(), &Value::Undefined);
    assert_eq!(raw.meta().kind(), "raw");
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > exportJSON is written from the schema > a getter returning undefined omits its property
#[test]
fn a_getter_returning_undefined_omits_its_property() {
    // JS's `getSerializedTextFormat` returning `undefined` is the `when`
    // of `textFormat` and `textStyle` here: what the schema writes for an
    // element whose text gives them. (`ParagraphNode.exportJSON` then adds
    // its backwards-compatible values, #7971, in both.)
    update(&test_editor(), || {
        let paragraph = create_paragraph_node();
        let text = create_text_node("x");
        text.set_format(1.0);
        paragraph.append(&[text])?;
        let exported = crate::schema::export_node_json_once(&*paragraph.get_latest(), false);
        // `undefined`, which `JSON.stringify` leaves out.
        assert!(
            exported.get("textFormat").is_none_or(Value::is_undefined),
            "{exported:?}"
        );
        assert!(exported.get("textStyle").is_none_or(Value::is_undefined));
        let styled = create_paragraph_node();
        styled.set_text_style("color: red");
        let schema_json = crate::schema::export_node_json_once(&*styled.get_latest(), false);
        assert_eq!(
            schema_json.get("textStyle"),
            Some(&Value::from("color: red"))
        );
        assert_eq!(
            styled.export_json(false).get("textStyle"),
            Some(&Value::from("color: red"))
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > exportJSON is written from the schema > a property is read through the getter its schema names
#[test]
fn a_property_is_read_through_the_getter_its_schema_names() {
    update(&test_editor(), || {
        let text = create_text_node("hi").export_json(false);
        assert_eq!(text.get("text"), Some(&Value::from("hi")));
        assert_eq!(text.get("type"), Some(&Value::from("text")));
        let paragraph = create_paragraph_node().export_json(false);
        assert_eq!(paragraph.get("format"), Some(&Value::from("")));
        assert_eq!(paragraph.get("type"), Some(&Value::from("paragraph")));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > exportJSON is written from the schema > export round-trips through import for every schema property
#[test]
fn export_round_trips_through_import_for_every_schema_property() {
    update(&test_editor(), || {
        let node = create_text_node("round trip");
        node.set_format(1.0);
        node.set_style("color: red");
        node.set_mode("token")?;
        let exported = node.export_json(false);
        let restored = create_text_node("");
        restored.update_from_json(&exported)?;
        assert_eq!(restored.export_json(false), exported);
        Ok(())
    });
}

fn editor_with(nodes: Vec<NodeClass>) -> Editor {
    create_editor(CreateEditorArgs {
        nodes,
        ..CreateEditorArgs::default()
    })
}

/// The registered class's `importJSON`.
fn import_json(node_type: &str, json: &Object) -> NodeRef {
    let import = crate::get_editor()
        .get_registered_node(node_type)
        .unwrap()
        .import_json;
    import(json).unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a declared fallback survives a member that accepts undefined
#[test]
fn a_declared_fallback_survives_a_member_that_accepts_undefined() {
    let union = union_value(vec![raw_value(), num(0.0)], Some(json("5"))).unwrap();
    assert_eq!(union.default_value(), &json("5"));
    let union = union_value(
        vec![
            optional(num(0.0), false).unwrap(),
            enumeration(&["inherit"]),
        ],
        Some("inherit".into()),
    )
    .unwrap();
    assert_eq!(union.default_value(), &Value::from("inherit"));
    let align = enum_value(
        vec![Value::Undefined, "middle".into(), "bottom".into()],
        Some("middle".into()),
    )
    .unwrap();
    assert_eq!(align.default_value(), &Value::from("middle"));
    let union = union_value(
        vec![optional(num(0.0), false).unwrap(), string_value("")],
        Some(Value::Undefined),
    )
    .unwrap();
    assert_eq!(union.default_value(), &Value::Undefined);
    union_value(vec![num(0.0), string_value("")], Some(Value::Undefined)).unwrap();
    let middle = vec!["middle".into(), Value::Undefined];
    let e = enum_value(middle.clone(), Some(Value::Undefined)).unwrap();
    assert_eq!(e.default_value(), &Value::Undefined);
    let e = enum_value(middle, None).unwrap();
    assert_eq!(e.default_value(), &Value::from("middle"));
    for (values, default) in [
        (strs(&["middle", "bottom"]), Value::Undefined),
        (strs(&["a", "b"]), Value::from("c")),
    ] {
        let error = enum_value(values, Some(default)).unwrap_err();
        assert!(
            error
                .message()
                .contains("enumValue: the default value is not one of the values"),
            "{error}"
        );
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a schema shape describes what a property parses to, not what it reads
#[test]
fn a_schema_shape_describes_what_a_property_parses_to_not_what_it_reads() {
    let schema = object_value(vec![("count", num(0.0)), ("label", string_value(""))]).unwrap();
    assert_eq!(schema.default_value(), &json(r#"{"count":0,"label":""}"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a union accepts exactly what its members accept
#[test]
fn a_union_accepts_exactly_what_its_members_accept() {
    let schema = union_value(
        vec![
            enum_value(vec![Value::Number(f64::NAN), json("1")], None).unwrap(),
            num(0.0),
        ],
        Some(json("1")),
    )
    .unwrap();
    let parsed = schema.parse(&Value::Number(f64::NAN));
    assert!(parsed.as_f64().is_some_and(f64::is_nan), "{parsed}");
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a reference-typed default cannot be mutated into every node
#[test]
fn a_reference_typed_default_cannot_be_mutated_into_every_node() {
    // A Rust value is owned by whoever holds it: changing a copy of the
    // default changes no other value.
    let schema = array_value(string_value("")).unwrap();
    let mut leaked = schema.default_value().clone();
    if let Value::Array(items) = &mut leaked {
        items.push("leak".into());
    }
    assert_eq!(schema.default_value(), &json("[]"));
    assert_eq!(undef(&schema), json("[]"));
    let mut parsed = p(&schema, r#"["a"]"#);
    if let Value::Array(items) = &mut parsed {
        items.push("b".into());
    }
    assert_eq!(parsed, json(r#"["a","b"]"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a field named for an Object.prototype member is refused
#[test]
fn a_field_named_for_an_object_prototype_member_is_refused() {
    let error = object_value(vec![("toString", raw_value())]).unwrap_err();
    assert!(
        error
            .message()
            .contains(r#"objectValue: "toString" is a member of Object.prototype"#),
        "{error}"
    );
    let error = object_value(vec![("__proto__", raw_value())]).unwrap_err();
    assert!(
        error
            .message()
            .contains(r#""__proto__" is not a valid field name"#),
        "{error}"
    );
    let schema = object_value(vec![("label", raw_value())]).unwrap();
    // `toEqual`, where a property holding `undefined` is absent.
    assert!(json_to_equal(&p(&schema, "{}"), &json("{}")));
    assert_eq!(
        p(&schema, r#"{"label":"mine"}"#),
        json(r#"{"label":"mine"}"#)
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::the export and parse shapes differ only where parsing is looser > exportJSON(true) type-checks as the value it actually returns
#[test]
fn export_json_true_type_checks_as_the_value_it_actually_returns() {
    // The run-time half: the compact form has no `version`.
    update(&test_editor(), || {
        let node = create_paragraph_node();
        assert!(node.export_json(true).get("version").is_none());
        assert_eq!(node.export_json(false).get("version"), Some(&json("1")));
        Ok(())
    });
}

/// `FieldNode`: an element whose `label` is its `label` field; `set_label`
/// and `get_label` are its methods, which the walk never calls.
#[derive(Clone, Debug)]
struct FieldNode {
    base: NodeBase,
    element: ElementData,
    label: String,
    calls: u32,
}

impl Default for FieldNode {
    fn default() -> Self {
        Self {
            base: NodeBase::default(),
            element: ElementData::default(),
            label: "default".to_owned(),
            calls: 0,
        }
    }
}

thread_local! {
    static FIELD_NODE_SCHEMA: NodeSchema = node_schema(vec![(
        "label",
        with_field(
            string_value("default"),
            |n: &FieldNode| &n.label,
            |n: &mut FieldNode| &mut n.label,
        )
        .unwrap(),
    )])
    .unwrap();
}

impl LexicalNode for FieldNode {
    fn get_type(&self) -> &'static str {
        "field-node"
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
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            FIELD_NODE_SCHEMA.with(Clone::clone),
            crate::nodes::element::element_node_schema(),
        ]
    }
}

fn field_node(node: &NodeRef) -> FieldNode {
    node.get_latest()
        .downcast_ref::<FieldNode>()
        .unwrap()
        .clone()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::withField compiles to direct field access > the field is read and written directly, not through the methods
#[test]
fn the_field_is_read_and_written_directly_not_through_the_methods() {
    let editor = editor_with(vec![NodeClass::of::<FieldNode>()]);
    update(&editor, || {
        let node = import_json(
            "field-node",
            &object(r#"{"label":"hello","type":"field-node"}"#),
        );
        assert_eq!(field_node(&node).label, "hello");
        assert_eq!(field_node(&node).calls, 0);
        assert_eq!(
            node.export_json(false).get("label"),
            Some(&Value::from("hello"))
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::withField compiles to direct field access > an absent property still parses to the schema default
#[test]
fn an_absent_property_still_parses_to_the_schema_default() {
    let editor = editor_with(vec![NodeClass::of::<FieldNode>()]);
    update(&editor, || {
        let bare = import_json("field-node", &object(r#"{"type":"field-node"}"#));
        assert_eq!(field_node(&bare).label, "default");
        Ok(())
    });
}

/// A `TextNode` subclass.
macro_rules! text_subclass {
    ($name:ident, $type:literal $(, $schema:expr)?) => {
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            text: crate::TextData,
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
            fn text(&self) -> Option<&crate::TextData> {
                Some(&self.text)
            }
            fn text_mut(&mut self) -> Option<&mut crate::TextData> {
                Some(&mut self.text)
            }
            fn super_types(&self) -> Vec<&'static str> {
                vec!["text"]
            }
            $(
                fn json_schema_chain(&self) -> Vec<NodeSchema> {
                    $schema
                }
            )?
        }
    };
}

/// `LoudTextNode.getStyle()`: TextNode's, then `;loud`.
fn loud_get_style(node: &dyn LexicalNode) -> String {
    format!("{};loud", node.text().unwrap().get_style())
}

/// `LoudTextNode.setStyle(style)`: TextNode's, of `style;set`.
fn loud_set_style(node: &NodeRef, style: &str) {
    node.set_style(&format!("{style};set"));
}

thread_local! {
    /// What `LoudTextNode`'s overrides of `getStyle` and `setStyle` make
    /// of TextNode's `style`, which a Rust subclass declares itself.
    static LOUD_TEXT_SCHEMA: NodeSchema = node_schema(vec![(
        "style",
        with_accessors(
            string_value(""),
            Some(Rc::new(|node| loud_get_style(node).into())),
            Some(Rc::new(|node, value| {
                node.text_mut().unwrap().style = format!("{};set", value.as_str().unwrap_or(""));
            })),
        )
        .unwrap(),
    )])
    .unwrap();
}

text_subclass!(
    LoudTextNode,
    "loud-text",
    vec![
        LOUD_TEXT_SCHEMA.with(Clone::clone),
        crate::nodes::text::text_node_schema()
    ]
);
text_subclass!(QuietTextNode, "quiet-text");

fn loud_editor() -> Editor {
    editor_with(vec![
        NodeClass::of::<LoudTextNode>(),
        NodeClass::of::<QuietTextNode>(),
    ])
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a field stands in for its accessor only while nobody overrides it > an overridden accessor is consulted in both directions
#[test]
fn an_overridden_accessor_is_consulted_in_both_directions() {
    update(&loud_editor(), || {
        let node = create::<LoudTextNode>();
        loud_set_style(&node, "color: red");
        assert_eq!(loud_get_style(&*node.get_latest()), "color: red;set;loud");
        assert_eq!(
            node.export_json(false).get("style"),
            Some(&Value::from("color: red;set;loud"))
        );
        let parsed = create::<LoudTextNode>();
        parsed.update_from_json(&object(r#"{"style":"color: blue","text":"x"}"#))?;
        assert_eq!(
            loud_get_style(&*parsed.get_latest()),
            "color: blue;set;loud"
        );
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a field stands in for its accessor only while nobody overrides it > a property whose accessor is not overridden keeps the field path
#[test]
fn a_property_whose_accessor_is_not_overridden_keeps_the_field_path() {
    update(&loud_editor(), || {
        let node = create::<LoudTextNode>();
        node.set_text_content("hello");
        assert_eq!(
            node.export_json(false).get("text"),
            Some(&Value::from("hello"))
        );
        let quiet = create::<QuietTextNode>();
        quiet.set_style("color: red");
        assert_eq!(
            quiet.export_json(false).get("style"),
            Some(&Value::from("color: red"))
        );
        Ok(())
    });
}

thread_local! {
    /// `WrappedElementNode`'s override of `getTextFormat`, which the
    /// `textFormat` property reads through.
    static WRAPPED_ELEMENT_SCHEMA: NodeSchema = node_schema(vec![(
        "textFormat",
        with_accessors(
            num(0.0),
            Some(Rc::new(|node| {
                Value::Number(((node.element().unwrap().get_text_format() as i32) | 1).into())
            })),
            Some(Rc::new(|node, value| {
                node.element_mut().unwrap().text_format = value.as_f64().unwrap_or(0.0);
            })),
        )
        .unwrap()
        .when(|node| node.should_serialize_text_styles()),
    )])
    .unwrap();
}

#[derive(Clone, Debug, Default)]
struct WrappedElementNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for WrappedElementNode {
    fn get_type(&self) -> &'static str {
        "wrapped-element"
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
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            WRAPPED_ELEMENT_SCHEMA.with(Clone::clone),
            crate::nodes::element::element_node_schema(),
        ]
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a field stands in for its accessor only while nobody overrides it > a spelled accessor is guarded alongside the one it wraps
#[test]
fn a_spelled_accessor_is_guarded_alongside_the_one_it_wraps() {
    let editor = editor_with(vec![NodeClass::of::<WrappedElementNode>()]);
    update(&editor, || {
        let node = create::<WrappedElementNode>();
        node.set_text_format(2.0);
        assert_eq!(node.export_json(false).get("textFormat"), Some(&json("3")));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a field stands in for its accessor only while nobody overrides it > setterTable and getterTable carry a property whose two forms differ
#[test]
fn setter_table_and_getter_table_carry_a_property_whose_two_forms_differ() {
    update(&loud_editor(), || {
        let node = create::<QuietTextNode>();
        node.set_mode("segmented")?;
        assert_eq!(
            node.export_json(false).get("mode"),
            Some(&Value::from("segmented"))
        );
        let parsed = create::<QuietTextNode>();
        parsed.update_from_json(&object(r#"{"mode":"token","text":"x"}"#))?;
        assert_eq!(text_of(&parsed).get_mode(), "token");
        assert_eq!(
            parsed.export_json(false).get("mode"),
            Some(&Value::from("token"))
        );
        let bogus = create::<QuietTextNode>();
        bogus.update_from_json(&object(r#"{"mode":"nonsense","text":"x"}"#))?;
        assert_eq!(text_of(&bogus).get_mode(), "normal");
        Ok(())
    });
}

fn is_equal(schema: &SerializationSchema, a: &str, b: &str) -> bool {
    schema.is_equal().expect("an isEqual")(&json(a), &json(b))
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::reference-typed defaults compact by content > an array equal to its default is dropped, a differing one is not
#[test]
fn an_array_equal_to_its_default_is_dropped_a_differing_one_is_not() {
    let ids = array_value(string_value("")).unwrap();
    assert!(is_equal(&ids, "[]", "[]"));
    assert!(is_equal(&ids, r#"["a"]"#, r#"["a"]"#));
    assert!(!is_equal(&ids, r#"["a"]"#, r#"["b"]"#));
    assert!(!is_equal(&ids, r#"["a"]"#, "[]"));
    assert!(is_schema_default(&ids, &json("[]")));
    assert!(!is_schema_default(&ids, &json(r#"["a"]"#)));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::reference-typed defaults compact by content > an object compares by its declared fields
#[test]
fn an_object_compares_by_its_declared_fields() {
    let point = object_value(vec![("x", num(0.0)), ("y", num(0.0))]).unwrap();
    assert!(is_equal(&point, r#"{"x":0,"y":0}"#, r#"{"x":0,"y":0}"#));
    assert!(!is_equal(&point, r#"{"x":1,"y":0}"#, r#"{"x":0,"y":0}"#));
}

#[derive(Clone, Debug, Default)]
struct TagsNode {
    base: NodeBase,
    element: ElementData,
    tags: Vec<String>,
}

thread_local! {
    static TAGS_NODE_SCHEMA: NodeSchema = node_schema(vec![(
        "tags",
        with_field(
            array_value(string_value("")).unwrap(),
            |n: &TagsNode| &n.tags,
            |n: &mut TagsNode| &mut n.tags,
        )
        .unwrap(),
    )])
    .unwrap();
}

impl LexicalNode for TagsNode {
    fn get_type(&self) -> &'static str {
        "tags-node"
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
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            TAGS_NODE_SCHEMA.with(Clone::clone),
            crate::nodes::element::element_node_schema(),
        ]
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::reference-typed defaults compact by content > a node with an array property compacts it away when empty
#[test]
fn a_node_with_an_array_property_compacts_it_away_when_empty() {
    let editor = editor_with(vec![NodeClass::of::<TagsNode>()]);
    update(&editor, || {
        let empty = create::<TagsNode>();
        let full = create::<TagsNode>();
        with_writable(full.get_key(), |n| {
            n.downcast_mut::<TagsNode>().unwrap().tags = vec!["a".to_owned()];
        });
        get_root().append(&[empty, full])
    });
    let compact = editor.get_editor_state().to_json(true);
    let children = compact
        .get("root")
        .unwrap()
        .get("children")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(children[0].get("tags").is_none());
    assert_eq!(children[1].get("tags"), Some(&json(r#"["a"]"#)));
    let state = editor.parse_editor_state(&compact.to_string()).unwrap();
    let tags = state.read(|| {
        get_root()
            .get_children()
            .iter()
            .map(|n| {
                n.get_latest()
                    .downcast_ref::<TagsNode>()
                    .unwrap()
                    .tags
                    .clone()
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(tags, vec![Vec::<String>::new(), vec!["a".to_owned()]]);
}

/// `writeThenRewrite(editor, klass, write)`: creates a node of `T`, writes
/// it, then makes it writable in a second update, so what comes back is a
/// clone, with its key.
fn write_then_rewrite<T: LexicalNode + Default + Clone>(
    editor: &Editor,
    write: impl FnOnce(&mut T) + 'static,
) -> (T, String) {
    let key = Rc::new(std::cell::RefCell::new(String::new()));
    let k = key.clone();
    update(editor, move || {
        let node = create::<T>();
        with_writable(node.get_key(), |n| write(n.downcast_mut::<T>().unwrap()));
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&node))?;
        *k.borrow_mut() = node.get_key().to_owned();
        Ok(())
    });
    let cloned = Rc::new(std::cell::RefCell::new(None));
    let (c, k) = (cloned.clone(), key.clone());
    update(editor, move || {
        let first = get_root().get_first_child().unwrap();
        assert_eq!(first.get_key(), *k.borrow());
        let before = first.get_latest();
        let writable = with_writable(first.get_key(), |n| n.downcast_ref::<T>().unwrap().clone());
        assert!(!Rc::ptr_eq(&before, &first.get_latest()), "a clone");
        *c.borrow_mut() = Some(writable);
        Ok(())
    });
    let cloned = cloned.borrow_mut().take().unwrap();
    let key = key.borrow().clone();
    (cloned, key)
}

/// An element whose properties are fields of its own (`CalloutNode`).
macro_rules! field_element {
    ($name:ident, $type:literal, [$($field:ident),*], $schema:ident) => {
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            element: ElementData,
            $($field: String,)*
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
            fn json_schema_chain(&self) -> Vec<NodeSchema> {
                let mut chain = $schema.with(Clone::clone);
                chain.push(crate::nodes::element::element_node_schema());
                chain
            }
        }
    };
}

thread_local! {
    static CALLOUT_SCHEMA: Vec<NodeSchema> = vec![node_schema(vec![
        ("label", with_field(string_value(""), |n: &CalloutNode| &n.label, |n: &mut CalloutNode| &mut n.label).unwrap()),
        (
            "level",
            with_field(
                num_with(1.0, NumberValueOptions { integer: true, min: Some(1.0), max: Some(3.0), clamp: false }),
                |n: &CalloutNode| &n.level,
                |n: &mut CalloutNode| &mut n.level,
            )
            .unwrap(),
        ),
        (
            "tone",
            with_field(enumeration(&["info", "warning", "danger"]), |n: &CalloutNode| &n.tone, |n: &mut CalloutNode| &mut n.tone).unwrap(),
        ),
    ])
    .unwrap()];
    static CARRY_BASE_SCHEMA: NodeSchema = node_schema(vec![(
        "base",
        with_field(string_value(""), |n: &CarryDerivedNode| &n.base_field, |n: &mut CarryDerivedNode| &mut n.base_field).unwrap(),
    )])
    .unwrap();
    /// `CarryDerivedNode`'s chain: its own `derived`, then `CarryBaseNode`'s
    /// `base`, then ElementNode's.
    static CARRY_DERIVED_SCHEMA: Vec<NodeSchema> = vec![
        node_schema(vec![(
            "derived",
            with_field(string_value(""), |n: &CarryDerivedNode| &n.derived, |n: &mut CarryDerivedNode| &mut n.derived).unwrap(),
        )])
        .unwrap(),
        CARRY_BASE_SCHEMA.with(Clone::clone),
    ];
}

#[derive(Clone, Debug)]
struct CalloutNode {
    base: NodeBase,
    element: ElementData,
    label: String,
    level: f64,
    tone: String,
}

impl Default for CalloutNode {
    fn default() -> Self {
        Self {
            base: NodeBase::default(),
            element: ElementData::default(),
            label: String::new(),
            level: 1.0,
            tone: "info".to_owned(),
        }
    }
}

impl LexicalNode for CalloutNode {
    fn get_type(&self) -> &'static str {
        "callout"
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
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        let mut chain = CALLOUT_SCHEMA.with(Clone::clone);
        chain.push(crate::nodes::element::element_node_schema());
        chain
    }
}

field_element!(
    CarryDerivedNode,
    "carry-derived",
    [base_field, derived],
    CARRY_DERIVED_SCHEMA
);

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > a node that declares only fields needs no afterCloneFrom
#[test]
fn a_node_that_declares_only_fields_needs_no_after_clone_from() {
    let editor = editor_with(vec![NodeClass::of::<CalloutNode>()]);
    let (_, key) = write_then_rewrite(&editor, |node: &mut CalloutNode| {
        node.label = "Heads up".to_owned();
        node.level = 3.0;
        node.tone = "danger".to_owned();
    });
    let exported = editor
        .read(move || NodeRef::new(key).export_json(false))
        .unwrap();
    assert_eq!(exported.get("label"), Some(&Value::from("Heads up")));
    assert_eq!(exported.get("level"), Some(&json("3")));
    assert_eq!(exported.get("tone"), Some(&Value::from("danger")));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > each class carries its own fields and delegates the rest
#[test]
fn each_class_carries_its_own_fields_and_delegates_the_rest() {
    let editor = editor_with(vec![NodeClass::of::<CarryDerivedNode>()]);
    let (cloned, _) = write_then_rewrite(&editor, |node: &mut CarryDerivedNode| {
        node.base_field = "from the base".to_owned();
        node.derived = "from the subclass".to_owned();
    });
    assert_eq!(cloned.base_field, "from the base");
    assert_eq!(cloned.derived, "from the subclass");
}

thread_local! {
    static ACCESSOR_CLONES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// `AccessorNode`: `ids` through accessor methods, with a clone of its own
/// (Rust's hand-written `afterCloneFrom`) that counts its calls.
#[derive(Debug, Default)]
struct AccessorNode {
    base: NodeBase,
    element: ElementData,
    ids: Vec<String>,
}

impl Clone for AccessorNode {
    fn clone(&self) -> Self {
        ACCESSOR_CLONES.with(|c| c.set(c.get() + 1));
        Self {
            base: self.base.clone(),
            element: self.element.clone(),
            ids: self.ids.clone(),
        }
    }
}

thread_local! {
    static ACCESSOR_SCHEMA: NodeSchema = node_schema(vec![(
        "ids",
        with_accessors(
            array_value(string_value("")).unwrap(),
            Some(crate::schema::schema_getter::<AccessorNode>(|n| {
                Value::Array(n.ids.iter().map(|s| s.as_str().into()).collect())
            })),
            Some(crate::schema::schema_setter::<AccessorNode>(|n, value| {
                n.ids = value
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default();
            })),
        )
        .unwrap(),
    )])
    .unwrap();
}

impl LexicalNode for AccessorNode {
    fn get_type(&self) -> &'static str {
        "carry-accessor"
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
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            ACCESSOR_SCHEMA.with(Clone::clone),
            crate::nodes::element::element_node_schema(),
        ]
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > a hand-written afterCloneFrom is left alone
#[test]
fn a_hand_written_after_clone_from_is_left_alone() {
    // A Rust node's clone is its `Clone`; a hand-written one is what runs.
    let editor = editor_with(vec![NodeClass::of::<AccessorNode>()]);
    ACCESSOR_CLONES.with(|c| c.set(0));
    let (cloned, _) = write_then_rewrite(&editor, |node: &mut AccessorNode| {
        node.ids = vec!["a".to_owned(), "b".to_owned()];
    });
    assert_eq!(cloned.ids, ["a", "b"]);
    assert!(ACCESSOR_CLONES.with(std::cell::Cell::get) > 0);
}

thread_local! {
    static RE_BASE_SCHEMA: Vec<NodeSchema> = vec![node_schema(vec![(
        "shared",
        with_field(string_value(""), |n: &ReBaseNode| &n.shared, |n: &mut ReBaseNode| &mut n.shared).unwrap(),
    )])
    .unwrap()];
    /// `ReDerivedNode` re-declares `shared` over `ReBaseNode`'s.
    static RE_DERIVED_SCHEMA: Vec<NodeSchema> = vec![
        node_schema(vec![(
            "shared",
            with_field(string_value("fallback"), |n: &ReDerivedNode| &n.shared, |n: &mut ReDerivedNode| &mut n.shared).unwrap(),
        )])
        .unwrap(),
        node_schema(vec![(
            "shared",
            with_field(string_value(""), |n: &ReDerivedNode| &n.shared, |n: &mut ReDerivedNode| &mut n.shared).unwrap(),
        )])
        .unwrap(),
    ];
}

field_element!(ReBaseNode, "order-base", [shared], RE_BASE_SCHEMA);
field_element!(ReDerivedNode, "order-derived", [shared], RE_DERIVED_SCHEMA);

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > what a base class carries does not depend on registration order
#[test]
fn what_a_base_class_carries_does_not_depend_on_registration_order() {
    let editor = editor_with(vec![
        NodeClass::of::<ReDerivedNode>(),
        NodeClass::of::<ReBaseNode>(),
    ]);
    let (base, _) = write_then_rewrite(&editor, |n: &mut ReBaseNode| {
        n.shared = "base value".to_owned()
    });
    assert_eq!(base.shared, "base value");
    let (derived, _) = write_then_rewrite(&editor, |n: &mut ReDerivedNode| {
        n.shared = "derived value".to_owned()
    });
    assert_eq!(derived.shared, "derived value");
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::review fixes > equality is total: an unvalidated getter result cannot throw
#[test]
fn equality_is_total_an_unvalidated_getter_result_cannot_throw() {
    let ids = array_value(string_value("")).unwrap();
    let point = object_value(vec![("x", num(0.0))]).unwrap();
    for value in [Value::Null, Value::Undefined, json("7"), "x".into()] {
        assert!(!is_schema_default(&ids, &value));
        assert!(!is_schema_default(&point, &value));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::review fixes > optional and nullable keep the inner content comparison
#[test]
fn optional_and_nullable_keep_the_inner_content_comparison() {
    let ids = array_value(string_value("")).unwrap();
    assert_eq!(
        p(&optional(ids.clone(), true).unwrap(), "[]"),
        Value::Undefined
    );
    assert_eq!(p(&nullable(ids.clone(), true).unwrap(), "[]"), Value::Null);
    let opt = optional(ids, false).unwrap();
    let eq = opt.is_equal().expect("an isEqual");
    assert!(eq(&Value::Undefined, &Value::Undefined));
    assert!(!eq(&Value::Undefined, &json("[]")));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::review fixes > a union parses undefined to the fallback it declared
#[test]
fn a_union_parses_undefined_to_the_fallback_it_declared() {
    let schema = union_value(
        vec![
            optional(num(0.0), false).unwrap(),
            enumeration(&["inherit"]),
        ],
        Some("inherit".into()),
    )
    .unwrap();
    assert_eq!(schema.default_value(), &Value::from("inherit"));
    assert_eq!(undef(&schema), Value::from("inherit"));
    assert_eq!(p(&schema, "640"), json("640"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::review fixes > a state built from a schema adopts its equality and default
#[test]
fn a_state_built_from_a_schema_adopts_its_equality_and_default() {
    let ids_state = crate::create_state_with(
        "ids",
        crate::StateValueConfig::<Value>::from_schema(array_value(string_value("")).unwrap()),
    );
    assert!(ids_state.is_equal(&json("[]"), &ids_state.default_value()));
    assert!(!ids_state.is_equal(&json(r#"["a"]"#), &json("[]")));
    let mut leaked = ids_state.default_value();
    if let Value::Array(items) = &mut leaked {
        items.push("leak".into());
    }
    assert_eq!(ids_state.default_value(), json("[]"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > a string spelled as a JSON number is converted
#[test]
fn a_string_spelled_as_a_json_number_is_converted() {
    let parse = num(0.0);
    for (input, expected) in [
        ("2", 2.0),
        ("0", 0.0),
        ("1e3", 1000.0),
        ("1E3", 1000.0),
        ("1e-3", 0.001),
        ("2.5", 2.5),
        ("-4", -4.0),
        ("-0.5", -0.5),
    ] {
        assert_eq!(
            parse.parse(&input.into()),
            Value::Number(expected),
            "{input}"
        );
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > only the JSON grammar is read
#[test]
fn only_the_json_grammar_is_read() {
    let parse = num(7.0);
    for value in [
        "0x10", "0b11", "0o17", "1_000", "+1", "05", ".5", "5.", " 2 ", "\n2", "2n",
    ] {
        assert_eq!(parse.parse(&value.into()), json("7"), "{value:?}");
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > anything that does not read as a finite number is out of domain
#[test]
fn anything_that_does_not_read_as_a_finite_number_is_out_of_domain() {
    let parse = num(7.0);
    for value in [
        "".into(),
        "   ".into(),
        "abc".into(),
        "Infinity".into(),
        "NaN".into(),
        json("true"),
        Value::Null,
        json("[]"),
    ] {
        assert_eq!(parse.parse(&value), json("7"), "{value}");
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > the bounds apply to the converted value
#[test]
fn the_bounds_apply_to_the_converted_value() {
    let span = num_with(
        1.0,
        NumberValueOptions {
            integer: true,
            min: Some(1.0),
            ..Default::default()
        },
    );
    assert_eq!(p(&span, r#""2""#), json("2"));
    assert_eq!(p(&span, r#""0""#), json("1"));
    assert_eq!(p(&span, r#""2.5""#), json("1"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > the reported domain is still numbers
#[test]
fn the_reported_domain_is_still_numbers() {
    assert_eq!(num(0.0).meta().kind(), "number");
    assert_eq!(num(3.0).default_value(), &json("3"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > a union accepts through a converting member
#[test]
fn a_union_accepts_through_a_converting_member() {
    let schema = union_value(vec![num(0.0), enumeration(&["auto"])], Some("auto".into())).unwrap();
    assert_eq!(p(&schema, r#""2""#), json("2"));
    assert_eq!(p(&schema, "2"), json("2"));
    assert_eq!(p(&schema, r#""auto""#), Value::from("auto"));
    assert_eq!(p(&schema, r#""banana""#), Value::from("auto"));
    let zeroed = transform_value(
        schema,
        |v| {
            if v == Value::from("auto") {
                json("0")
            } else {
                v
            }
        },
        None,
    )
    .unwrap();
    assert_eq!(p(&zeroed, r#""auto""#), json("0"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > the union reads a stringified number for a real node property
#[test]
fn the_union_reads_a_stringified_number_for_a_real_node_property() {
    let format = format_schema();
    assert_eq!(p(&format, r#""1""#), json("1"));
    assert_eq!(p(&format, "1"), json("1"));
    assert_eq!(p(&format, r#""bold""#), json("1"));
    assert_eq!(p(&format, r#""junk""#), json("0"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a default is metadata, so nothing hands out a mutable one > a union yields a fresh parse rather than a member default
#[test]
fn a_union_yields_a_fresh_parse_rather_than_a_member_default() {
    let schema = union_value(vec![array_value(string_value("")).unwrap()], None).unwrap();
    assert_eq!(p(&schema, r#"["a"]"#), json(r#"["a"]"#));
    assert_eq!(p(&schema, "[]"), json("[]"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::schema(undefined) is always the schema default > enumValue reads an absent value as its default, not as a member
#[test]
fn enum_value_reads_an_absent_value_as_its_default_not_as_a_member() {
    let schema = enum_value(
        vec![Value::Undefined, "middle".into(), "bottom".into()],
        Some("middle".into()),
    )
    .unwrap();
    assert_eq!(schema.default_value(), &Value::from("middle"));
    assert_eq!(undef(&schema), Value::from("middle"));
    assert_eq!(p(&schema, r#""bottom""#), Value::from("bottom"));
    let leading = enum_value(vec![Value::Undefined, "middle".into()], None).unwrap();
    assert_eq!(leading.default_value(), &Value::Undefined);
    assert_eq!(undef(&leading), Value::Undefined);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::isEqual is total over the values a getter can return > an array with holes is not equal to a dense one of the same length
#[test]
fn an_array_with_holes_is_not_equal_to_a_dense_one_of_the_same_length() {
    // A hole reads as `undefined`.
    let schema = array_value(string_value("")).unwrap();
    let eq = schema.is_equal().unwrap();
    let holes = Value::Array(vec![Value::Undefined; 3]);
    assert!(!eq(&holes, &json(r#"["a","b","c"]"#)));
    assert!(eq(&json(r#"["a","b"]"#), &json(r#"["a","b"]"#)));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::isEqual is total over the values a getter can return > an object carrying undeclared keys is not equal to the default
#[test]
fn an_object_carrying_undeclared_keys_is_not_equal_to_the_default() {
    let schema = object_value(vec![("x", num(0.0))]).unwrap();
    let eq = schema.is_equal().unwrap();
    let default = schema.default_value();
    assert!(eq(&json(r#"{"x":0}"#), default));
    assert!(!eq(&json(r#"{"extra":"keep","x":0}"#), default));
    assert!(!eq(&json("[]"), default));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::isEqual is total over the values a getter can return > transformValue takes an isEqual for a reference-typed output
#[test]
fn transform_value_takes_an_is_equal_for_a_reference_typed_output() {
    let plain = transform_value(array_value(string_value("")).unwrap(), |v| v, None).unwrap();
    assert!(plain.is_equal().is_none());
    let compared = transform_value(
        array_value(string_value("")).unwrap(),
        |v| v,
        Some(Rc::new(|a: &Value, b: &Value| {
            a.as_array().map(Vec::len) == b.as_array().map(Vec::len)
        })),
    )
    .unwrap();
    assert!(compared.is_equal().unwrap()(
        &json("[]"),
        compared.default_value()
    ));
    // and declined for a primitive output
    let error = transform_value(string_value(""), |v| v, Some(Rc::new(|a, b| a == b))).unwrap_err();
    assert!(
        error
            .message()
            .contains("isEqual compares reference-typed values"),
        "{error}"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a primitive accepts what it parses, except where it reads more
#[test]
fn a_primitive_accepts_what_it_parses_except_where_it_reads_more() {
    // The run-time half; the input types are TypeScript's.
    assert_eq!(p(&string_value(""), r#""x""#), Value::from("x"));
    assert_eq!(p(&boolean_value(false), "true"), json("true"));
    assert_eq!(p(&num(0.0), r#""2""#), json("2"));
}

fn bold_italic() -> Vec<(String, Value)> {
    vec![
        ("bold".to_owned(), json("1")),
        ("italic".to_owned(), json("2")),
    ]
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > an alias table widens the accepted input, not the output
#[test]
fn an_alias_table_widens_the_accepted_input_not_the_output() {
    let format = aliased_value(num(0.0), bold_italic()).unwrap();
    assert_eq!(p(&format, r#""bold""#), json("1"));
    assert_eq!(p(&format, r#""2""#), json("2"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > the wrappers widen their inner the way they widen its output
#[test]
fn the_wrappers_widen_their_inner_the_way_they_widen_its_output() {
    let s = string_value("");
    assert_eq!(p(&s, r#""x""#), Value::from("x"));
    assert_eq!(
        undef(&optional(s.clone(), false).unwrap()),
        Value::Undefined
    );
    assert_eq!(p(&nullable(s, false).unwrap(), "null"), Value::Null);
    assert_eq!(
        p(&array_value(num(0.0)).unwrap(), r#"["1",2]"#),
        json("[1,2]")
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a union accepts what any member accepts
#[test]
fn a_union_accepts_what_any_member_accepts() {
    assert_eq!(p(&dimension(), r#""640""#), json("640"));
}

fn composed_keys(node: &dyn LexicalNode) -> Vec<String> {
    let mut keys: Vec<String> = crate::schema::get_composed_schema_fields(node)
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    keys.sort();
    keys
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a node composes the input it accepts across its config chain
#[test]
fn a_node_composes_the_input_it_accepts_across_its_config_chain() {
    // The keys of `LexicalSchemaInput<ParagraphNode>` are the fields its
    // chain composes, ElementNode's; TextNode's `format` reads its legacy
    // names as well as the number.
    assert_eq!(
        composed_keys(&crate::nodes::paragraph::ParagraphNode::default()),
        ["direction", "format", "indent", "textFormat", "textStyle"]
    );
    let text_fields =
        crate::schema::get_composed_schema_fields(&crate::nodes::text::TextNode::default());
    let format = &text_fields.iter().find(|(k, _)| k == "format").unwrap().1;
    assert_eq!(format.schema().parse(&"bold".into()), json("1"));
    assert_eq!(format.schema().parse(&json("2")), json("2"));
}

thread_local! {
    /// `Deep45`'s chain: its own `p45`, the 43 links between that declare
    /// nothing, then `Deep1`'s `p1` and ElementNode's.
    static DEEP_SCHEMA: Vec<NodeSchema> = {
        let mut chain = vec![node_schema(vec![(
            "p45",
            with_field(string_value(""), |n: &Deep45| &n.p45, |n: &mut Deep45| &mut n.p45).unwrap(),
        )])
        .unwrap()];
        chain.extend((2..45).map(|_| node_schema(Vec::new()).unwrap()));
        chain.push(
            node_schema(vec![(
                "p1",
                with_field(string_value(""), |n: &Deep45| &n.p1, |n: &mut Deep45| &mut n.p1).unwrap(),
            )])
            .unwrap(),
        );
        chain
    };
}

field_element!(Deep45, "deep-20", [p1, p45], DEEP_SCHEMA);

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a chain deeper than any bound resolves, base included
#[test]
fn a_chain_deeper_than_any_bound_resolves_base_included() {
    assert_eq!(
        composed_keys(&Deep45::default()),
        [
            "direction",
            "format",
            "indent",
            "p1",
            "p45",
            "textFormat",
            "textStyle"
        ]
    );
}

/// `CaptionNode`: a decorator whose config names no `extends`.
#[derive(Clone, Debug, Default)]
struct CaptionNode {
    base: NodeBase,
    caption: String,
}

thread_local! {
    static CAPTION_SCHEMA: NodeSchema = node_schema(vec![(
        "caption",
        with_field(string_value(""), |n: &CaptionNode| &n.caption, |n: &mut CaptionNode| &mut n.caption).unwrap(),
    )])
    .unwrap();
}

impl LexicalNode for CaptionNode {
    fn get_type(&self) -> &'static str {
        "caption-no-extends"
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
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![CAPTION_SCHEMA.with(Clone::clone)]
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a node that omits extends keeps its own schema
#[test]
fn a_node_that_omits_extends_keeps_its_own_schema() {
    assert_eq!(composed_keys(&CaptionNode::default()), ["caption"]);
}

fn abc() -> SerializationSchema {
    enumeration(&["a", "b", "c"])
}

thread_local! {
    static TAG_BASE_SCHEMA: Vec<NodeSchema> = vec![node_schema(vec![(
        "tag",
        with_field(abc(), |n: &TagBase| &n.tag, |n: &mut TagBase| &mut n.tag).unwrap(),
    )])
    .unwrap()];
    /// `TagSub` re-declares `tag` with an alias the base does not know.
    static TAG_SUB_SCHEMA: Vec<NodeSchema> = vec![
        node_schema(vec![(
            "tag",
            with_field(
                aliased_value(abc(), vec![("third".to_owned(), "c".into())]).unwrap(),
                |n: &TagSub| &n.tag,
                |n: &mut TagSub| &mut n.tag,
            )
            .unwrap(),
        )])
        .unwrap(),
        node_schema(vec![(
            "tag",
            with_field(abc(), |n: &TagSub| &n.tag, |n: &mut TagSub| &mut n.tag).unwrap(),
        )])
        .unwrap(),
    ];
}

field_element!(TagBase, "tag-base", [tag], TAG_BASE_SCHEMA);
field_element!(TagSub, "tag-sub", [tag], TAG_SUB_SCHEMA);

fn composed_field(node: &dyn LexicalNode, key: &str) -> SerializationSchema {
    crate::schema::get_composed_schema_fields(node)
        .into_iter()
        .find(|(k, _)| k == key)
        .unwrap()
        .1
        .schema()
        .clone()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a re-declared property is the subclass’s, not the intersection
#[test]
fn a_re_declared_property_is_the_subclasss_not_the_intersection() {
    let third = Value::from("third");
    assert_eq!(
        composed_field(&TagSub::default(), "tag").parse(&third),
        Value::from("c")
    );
    assert_eq!(
        composed_field(&TagBase::default(), "tag").parse(&third),
        Value::from("a")
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > an object accepts each property’s input, any of them absent
#[test]
fn an_object_accepts_each_propertys_input_any_of_them_absent() {
    let point = object_value(vec![("label", string_value("")), ("x", num(0.0))]).unwrap();
    assert_eq!(p(&point, r#"{"x":"3"}"#), json(r#"{"label":"","x":3}"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::the parse shape accepts what a parser actually accepts > a legacy alias and a stringified number type-check as input
#[test]
fn a_legacy_alias_and_a_stringified_number_type_check_as_input() {
    update(&test_editor(), || {
        let node = create_text_node("hi");
        node.update_from_json(&object(r#"{"detail":"directionless","format":"bold"}"#))?;
        assert_eq!(text_of(&node).get_format(), 1.0);
        assert!(node.is_directionless());
        let numbered = create_text_node("hi");
        numbered.update_from_json(&object(r#"{"format":"2"}"#))?;
        assert_eq!(text_of(&numbered).get_format(), 2.0);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::the parse shape accepts what a parser actually accepts > a property the node does not have is still refused
#[test]
fn a_property_the_node_does_not_have_is_still_refused() {
    // Refused by TypeScript; at run time an unknown key is ignored.
    update(&test_editor(), || {
        let node = create_text_node("hi");
        node.update_from_json(&object(r#"{"frmat":"bold","text":"hi"}"#))?;
        assert_eq!(text_of(&node).get_format(), 0.0);
        assert!(node.export_json(false).get("frmat").is_none());
        Ok(())
    });
}

/// `OptionalFieldNode`: an `optional` property stored in a field that
/// starts without a value.
#[derive(Clone, Debug, Default)]
struct OptionalFieldNode {
    base: NodeBase,
    element: ElementData,
    caption: Option<String>,
}

thread_local! {
    static OPTIONAL_FIELD_SCHEMA: NodeSchema = node_schema(vec![(
        "caption",
        with_field(
            optional(string_value(""), false).unwrap(),
            |n: &OptionalFieldNode| &n.caption,
            |n: &mut OptionalFieldNode| &mut n.caption,
        )
        .unwrap(),
    )])
    .unwrap();
}

impl LexicalNode for OptionalFieldNode {
    fn get_type(&self) -> &'static str {
        "optional-field-node"
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
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            OPTIONAL_FIELD_SCHEMA.with(Clone::clone),
            crate::nodes::element::element_node_schema(),
        ]
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a misspelled field name is caught in both directions > an initialized optional field is an own property
#[test]
fn an_initialized_optional_field_is_an_own_property() {
    let editor = editor_with(vec![NodeClass::of::<OptionalFieldNode>()]);
    update(&editor, || {
        let node = create::<OptionalFieldNode>();
        let exported = node.export_json(false);
        assert!(exported.get("caption").is_none_or(Value::is_undefined));
        assert_eq!(
            exported.get("type"),
            Some(&Value::from("optional-field-node"))
        );
        node.update_from_json(&object(r#"{"caption":"hi"}"#))?;
        assert_eq!(
            node.export_json(false).get("caption"),
            Some(&Value::from("hi"))
        );
        Ok(())
    });
}

/// `toEqual`, where a key holding `undefined` is absent.
#[track_caller]
fn same(a: &Value, b: &Value) {
    assert!(json_to_equal(a, b), "{a:?} vs {b:?}");
}

/// An object whose entries may hold `Value::Undefined`.
fn obj(entries: &[(&str, Value)]) -> Value {
    let mut object = Object::new();
    for (key, value) in entries {
        object.insert(*key, value.clone());
    }
    Value::Object(object)
}

/// `` value => `n${value}` ``.
fn prefix_n(value: Value) -> Value {
    Value::String(format!("n{value}"))
}

/// `value => value.split(',')`.
fn split_commas(value: Value) -> Value {
    Value::Array(
        value
            .as_str()
            .unwrap_or("")
            .split(',')
            .map(Value::from)
            .collect(),
    )
}

/// A copy of `base` whose `accepts` is declared, as `Object.assign(fn,
/// base, {accepts})` makes in JS.
fn declaring(
    base: &SerializationSchema,
    accepts: impl Fn(&Value) -> bool + 'static,
    parse: Option<crate::schema::SchemaParse>,
) -> SerializationSchema {
    let b = base.clone();
    let parse: crate::schema::SchemaParse =
        parse.unwrap_or_else(|| Rc::new(move |v: &Value| b.parse(v)));
    SerializationSchema::new(
        base.meta().clone(),
        base.default_value().clone(),
        base.is_equal().cloned(),
        Some(Rc::new(accepts)),
        move |v| parse(v),
    )
}

fn strings() -> SerializationSchema {
    array_value(string_value("")).unwrap()
}

fn numbers() -> SerializationSchema {
    array_value(num(0.0)).unwrap()
}

fn empty_array() -> Option<Value> {
    Some(json("[]"))
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a value a member normalizes into its default is still that member
#[test]
fn a_value_a_member_normalizes_into_its_default_is_still_that_member() {
    let dimension = dimension();
    assert_eq!(p(&dimension, r#""0""#), json("0"));
    let negative_zero = p(&dimension, r#""-0""#);
    assert!(
        negative_zero
            .as_f64()
            .is_some_and(|n| n == 0.0 && n.is_sign_negative())
    );
    assert_eq!(p(&dimension, "0"), json("0"));
    assert_eq!(p(&dimension, r#""banana""#), Value::from("inherit"));
    assert_eq!(p(&dimension, r#""inherit""#), Value::from("inherit"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > an alias whose target is the inner default is still that member
#[test]
fn an_alias_whose_target_is_the_inner_default_is_still_that_member() {
    let legacy = aliased_value(string_value(""), vec![("none".to_owned(), "".into())]).unwrap();
    assert_eq!(p(&legacy, r#""none""#), Value::from(""));
    let union = union_value(vec![legacy], Some("fallback".into())).unwrap();
    assert_eq!(p(&union, r#""none""#), Value::from(""));
    assert_eq!(p(&union, "42"), Value::from("fallback"));
}

/// `transformValue(objectValue({id, tag: stringValue()}), v => v, {isEqual:
/// (a, b) => a.id === b.id})`.
fn by_id() -> SerializationSchema {
    transform_value(
        object_value(vec![("id", num(0.0)), ("tag", string_value(""))]).unwrap(),
        |v| v,
        Some(Rc::new(|a: &Value, b: &Value| a.get("id") == b.get("id"))),
    )
    .unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a comparator never sees values its own member did not produce
#[test]
fn a_comparator_never_sees_values_its_own_member_did_not_produce() {
    let by_id = by_id();
    let plain = object_value(vec![("v", num(0.0))]).unwrap();
    let union = union_value(vec![by_id.clone(), plain], None).unwrap();
    let eq = union.is_equal().unwrap();
    assert!(!eq(&json(r#"{"v":1}"#), &json(r#"{"v":2}"#)));
    let (a, b) = (json(r#"{"id":1,"tag":"a"}"#), json(r#"{"id":1,"tag":"b"}"#));
    assert!(!eq(&a, &b));
    assert!(is_schema_equal(&by_id, &a, &b));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a wrapper carries its inner membership into the union
#[test]
fn a_wrapper_carries_its_inner_membership_into_the_union() {
    let inherit = || Some(Value::from("inherit"));
    let width = union_value(vec![num(0.0), enumeration(&["inherit"])], inherit()).unwrap();
    assert_eq!(p(&width, r#""0""#), json("0"));
    assert_eq!(p(&width, "640"), json("640"));
    assert_eq!(p(&width, r#""banana""#), Value::from("inherit"));
    for member in [
        transform_value(num(0.0), |v| v, None).unwrap(),
        nullable(num(0.0), false).unwrap(),
        optional(num(0.0), false).unwrap(),
    ] {
        let union = union_value(vec![member, enumeration(&["inherit"])], inherit()).unwrap();
        assert_eq!(p(&union, r#""0""#), json("0"));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a union is itself a member, so nesting one keeps its domain
#[test]
fn a_union_is_itself_a_member_so_nesting_one_keeps_its_domain() {
    let inner = union_value(vec![num(0.0), enumeration(&["inherit"])], Some(json("0"))).unwrap();
    assert!(inner.accepts().is_some());
    let outer = union_value(vec![inner.clone(), string_value("zzz")], Some("zzz".into())).unwrap();
    assert_eq!(p(&outer, r#""0""#), json("0"));
    assert_eq!(p(&outer, "640"), json("640"));
    assert_eq!(p(&outer, r#""inherit""#), Value::from("inherit"));
    assert_eq!(p(&outer, r#""banana""#), Value::from("banana"));
    let wrapped = union_value(
        vec![nullable(inner, false).unwrap(), string_value("zzz")],
        Some("zzz".into()),
    )
    .unwrap();
    assert_eq!(p(&wrapped, r#""0""#), json("0"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > optional lifts membership for undefined only, nullable for both
#[test]
fn optional_lifts_membership_for_undefined_only_nullable_for_both() {
    let inherit = || Some(Value::from("inherit"));
    let with = |member: SerializationSchema| {
        union_value(vec![member, enumeration(&["inherit"])], inherit()).unwrap()
    };
    assert_eq!(p(&dimension(), "null"), Value::from("inherit"));
    assert_eq!(
        p(&with(optional(num(0.0), false).unwrap()), "null"),
        Value::from("inherit")
    );
    assert_eq!(
        p(&with(nullable(num(0.0), false).unwrap()), "null"),
        Value::Null
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a union defers equality to the member that recognizes the pair
#[test]
fn a_union_defers_equality_to_the_member_that_recognizes_the_pair() {
    let ids = union_value(vec![strings(), enumeration(&["all"])], None).unwrap();
    assert!(is_schema_default(&ids, &json("[]")));
    assert!(!is_schema_default(&ids, &json(r#"["a"]"#)));
    assert!(is_schema_equal(
        &ids,
        &json(r#"["a","b"]"#),
        &json(r#"["a","b"]"#)
    ));
    assert!(!is_schema_equal(&ids, &json(r#"["a"]"#), &json(r#"["b"]"#)));
    assert!(is_schema_equal(&ids, &"all".into(), &"all".into()));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a bounded member rejects what falls outside its bounds
#[test]
fn a_bounded_member_rejects_what_falls_outside_its_bounds() {
    let bounded = num_with(
        1.0,
        NumberValueOptions {
            integer: true,
            min: Some(1.0),
            ..Default::default()
        },
    );
    let span = union_value(vec![bounded], Some(json("1"))).unwrap();
    assert_eq!(p(&span, r#""4""#), json("4"));
    assert_eq!(p(&span, r#""0""#), json("1"));
    assert_eq!(p(&span, r#""1.5""#), json("1"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a sparse array is parsed, not passed through > holes go through the item schema like any other element
#[test]
fn holes_go_through_the_item_schema_like_any_other_element() {
    // A hole is an element holding `undefined`.
    let schema = strings();
    let holes = Value::Array(vec![Value::Undefined; 3]);
    assert_eq!(schema.parse(&holes), json(r#"["","",""]"#));
    assert_eq!(p(&schema, r#"["a",42,null]"#), json(r#"["a","",""]"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::objectValue refuses a __proto__ field in every build > because the parse would reparent its result instead of writing it
#[test]
fn because_the_parse_would_reparent_its_result_instead_of_writing_it() {
    let error = object_value(vec![("__proto__", string_value(""))]).unwrap_err();
    assert!(error.message().contains("__proto__"), "{error}");
}

/// `PlainBlock` and `StyledBlock`, which overrides `getTextFormat` and
/// `getTextStyle`; the property's getter reads through those, so the Rust
/// subclass declares them in its schema.
#[derive(Clone, Debug, Default)]
struct PlainBlock {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for PlainBlock {
    fn get_type(&self) -> &'static str {
        "schema-plain-block"
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

thread_local! {
    static STYLED_BLOCK_SCHEMA: NodeSchema = {
        let when = |node: &dyn LexicalNode| node.should_serialize_text_styles();
        node_schema(vec![
            (
                "textFormat",
                with_accessors(num(0.0), Some(Rc::new(|_| json("1"))), Some(Rc::new(|_, _| {})))
                    .unwrap()
                    .when(when),
            ),
            (
                "textStyle",
                with_accessors(
                    string_value(""),
                    Some(Rc::new(|_| "color: red;".into())),
                    Some(Rc::new(|_, _| {})),
                )
                .unwrap()
                .when(when),
            ),
        ])
        .unwrap()
    };
}

#[derive(Clone, Debug, Default)]
struct StyledBlock {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for StyledBlock {
    fn get_type(&self) -> &'static str {
        "schema-styled-block"
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
        vec!["schema-plain-block"]
    }
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            STYLED_BLOCK_SCHEMA.with(Clone::clone),
            crate::nodes::element::element_node_schema(),
        ]
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema does not take back an override the node already had > an overridden getTextFormat/getTextStyle still decides the export
#[test]
fn an_overridden_get_text_format_get_text_style_still_decides_the_export() {
    let editor = editor_with(vec![
        NodeClass::of::<PlainBlock>(),
        NodeClass::of::<StyledBlock>(),
    ]);
    update(&editor, || {
        let styled = create::<StyledBlock>().export_json(false);
        assert_eq!(styled.get("textFormat"), Some(&json("1")));
        assert_eq!(styled.get("textStyle"), Some(&Value::from("color: red;")));
        let plain = create::<PlainBlock>().export_json(false);
        assert!(plain.get("textFormat").is_none_or(Value::is_undefined));
        assert!(plain.get("textStyle").is_none_or(Value::is_undefined));
        Ok(())
    });
}

/// `MigratingText`: a TextNode whose `updateFromJSON` adds a style.
#[derive(Clone, Debug, Default)]
struct MigratingText {
    base: NodeBase,
    text: crate::TextData,
}

impl LexicalNode for MigratingText {
    fn get_type(&self) -> &'static str {
        "schema-migrating-text"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&crate::TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut crate::TextData> {
        Some(&mut self.text)
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["text"]
    }
    fn update_from_json(&mut self, json: &Object) {
        crate::schema::apply_json_setters(self, json);
        self.text.style = "color: red;".to_owned();
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema does not take back an override the node already had > a synthesized importJSON calls an overridden updateFromJSON
#[test]
fn a_synthesized_import_json_calls_an_overridden_update_from_json() {
    let editor = editor_with(vec![NodeClass::of::<MigratingText>()]);
    update(&editor, || {
        let imported = import_json(
            "schema-migrating-text",
            &object(
                r#"{"detail":0,"format":0,"mode":"normal","style":"","text":"hi","type":"schema-migrating-text","version":1}"#,
            ),
        );
        assert_eq!(text_of(&imported).get_style(), "color: red;");
        assert_eq!(imported.get_text_content(), "hi");
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a wrapper answers for its inner schema’s domain > a union picks the wrapper whose inner schema recognizes the value
#[test]
fn a_union_picks_the_wrapper_whose_inner_schema_recognizes_the_value() {
    let schema = union_value(
        vec![
            nullable(string_value(""), true).unwrap(),
            enumeration(&["auto"]),
        ],
        Some("auto".into()),
    )
    .unwrap();
    assert_eq!(p(&schema, r#""""#), Value::Null);
    assert_eq!(p(&schema, r#""left""#), Value::from("left"));
    assert_eq!(p(&schema, r#""auto""#), Value::from("auto"));
    assert_eq!(p(&schema, "42"), Value::from("auto"));
}

fn to_array() -> SerializationSchema {
    transform_value(
        string_value(""),
        split_commas,
        Some(Rc::new(|a: &Value, b: &Value| a == b)),
    )
    .unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a wrapper answers for its inner schema’s domain > the same holds for optional and for a transformed domain
#[test]
fn the_same_holds_for_optional_and_for_a_transformed_domain() {
    let auto = || Some(Value::from("auto"));
    let opt = union_value(
        vec![
            optional(string_value(""), true).unwrap(),
            enumeration(&["auto"]),
        ],
        auto(),
    )
    .unwrap();
    assert_eq!(p(&opt, r#""""#), Value::Undefined);
    let union = union_value(vec![to_array(), enumeration(&["auto"])], auto()).unwrap();
    assert_eq!(p(&union, r#""""#), json(r#"[""]"#));
    assert_eq!(p(&union, r#""a,b""#), json(r#"["a","b"]"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a wrapper answers for its inner schema’s domain > a wrapper still declines what its inner schema declines
#[test]
fn a_wrapper_still_declines_what_its_inner_schema_declines() {
    let schema = union_value(
        vec![
            optional(num(0.0), false).unwrap(),
            enum_value(vec![Value::Null], None).unwrap(),
        ],
        Some(Value::Null),
    )
    .unwrap();
    assert_eq!(p(&schema, "null"), Value::Null);
    assert_eq!(p(&schema, "7"), json("7"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema declares the domain it reads, and only that > a raw value validates nothing, so it declines nothing
#[test]
fn a_raw_value_validates_nothing_so_it_declines_nothing() {
    let raw = raw_value();
    assert!(accepts(&raw, &Value::Undefined));
    assert!(accepts(&raw, &"hi".into()));
    let union = union_value(
        vec![
            object_value(vec![("note", raw), ("title", string_value(""))]).unwrap(),
            enumeration(&["auto"]),
        ],
        Some("auto".into()),
    )
    .unwrap();
    same(
        &union.parse(&obj(&[
            ("note", Value::Undefined),
            ("title", "keep me".into()),
        ])),
        &json(r#"{"title":"keep me"}"#),
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema declares the domain it reads, and only that > an enum listing undefined recognizes it, when it is the default
#[test]
fn an_enum_listing_undefined_recognizes_it_when_it_is_the_default() {
    let top_first = enum_value(vec![Value::Undefined, "top".into()], None).unwrap();
    assert!(accepts(&top_first, &Value::Undefined));
    let top = enum_value(vec!["top".into(), Value::Undefined], None).unwrap();
    assert!(!accepts(&top, &Value::Undefined));
    let error = enum_value(strs(&["top", "middle"]), Some("unset".into())).unwrap_err();
    assert!(
        error
            .message()
            .contains("enumValue: the default value is not one of the values")
    );
    let union = union_value(
        vec![
            object_value(vec![("align", top_first), ("id", string_value(""))]).unwrap(),
            enumeration(&["auto"]),
        ],
        Some("auto".into()),
    )
    .unwrap();
    let expected = json(r#"{"id":"x"}"#);
    same(
        &union.parse(&obj(&[("align", Value::Undefined), ("id", "x".into())])),
        &expected,
    );
    same(
        &union.parse(&obj(&[("align", "sideways".into()), ("id", "x".into())])),
        &expected,
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema declares the domain it reads, and only that > a constrained member still declines its own out-of-domain default
#[test]
fn a_constrained_member_still_declines_its_own_out_of_domain_default() {
    let min = || {
        num_with(
            0.0,
            NumberValueOptions {
                min: Some(1.0),
                ..Default::default()
            },
        )
    };
    assert!(!accepts(&min(), &json("0")));
    let union = union_value(vec![min(), enumeration(&["auto"])], Some("auto".into())).unwrap();
    assert_eq!(p(&union, "0"), Value::from("auto"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema declares the domain it reads, and only that > a transform describes what it reads, not what it writes
#[test]
fn a_transform_describes_what_it_reads_not_what_it_writes() {
    let prefixed = transform_value(num(0.0), prefix_n, None).unwrap();
    assert!(!accepts(&prefixed, &"n0".into()));
    assert!(accepts(&prefixed, &json("0")));
    let aliased = aliased_value(num(0.0), vec![("n0".to_owned(), json("42"))]).unwrap();
    let union = union_value(vec![prefixed, aliased], Some(json("0"))).unwrap();
    assert_eq!(p(&union, r#""n0""#), json("42"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a declaration means what it says > an explicit undefined default is a default
#[test]
fn an_explicit_undefined_default_is_a_default() {
    let members = || vec![num(0.0), optional(string_value(""), false).unwrap()];
    let explicit = union_value(members(), Some(Value::Undefined)).unwrap();
    assert_eq!(explicit.default_value(), &Value::Undefined);
    assert_eq!(
        union_value(members(), None).unwrap().default_value(),
        &json("0")
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > a raw field does not veto its enclosing container
#[test]
fn a_raw_field_does_not_veto_its_enclosing_container() {
    let value = json(r#"[{"label":"keep label","note":"keep note"}]"#);
    let labelled = object_value(vec![("label", string_value("")), ("note", raw_value())]).unwrap();
    let union = union_value(
        vec![numbers(), array_value(labelled).unwrap()],
        empty_array(),
    )
    .unwrap();
    assert_eq!(union.parse(&value), value);
    let tags = json(r#"{"note":"hi","tags":["red","blue"]}"#);
    let union = union_value(
        vec![
            object_value(vec![("note", raw_value()), ("tags", numbers())]).unwrap(),
            object_value(vec![("note", raw_value()), ("tags", strings())]).unwrap(),
        ],
        Some("x".into()),
    )
    .unwrap();
    assert_eq!(union.parse(&tags), tags);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > but a union that merely contains one is not itself a catch-all
#[test]
fn but_a_union_that_merely_contains_one_is_not_itself_a_catch_all() {
    let value = json(r#"["red","42"]"#);
    let with_raw =
        |s: SerializationSchema| union_value(vec![s, raw_value()], empty_array()).unwrap();
    let union = union_value(vec![numbers(), with_raw(strings())], empty_array()).unwrap();
    assert_eq!(union.parse(&value), value);
    let union = union_value(vec![with_raw(numbers()), strings()], empty_array()).unwrap();
    assert_eq!(union.parse(&value), value);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a fit earned through a catch-all ranks below a real one
#[test]
fn and_a_fit_earned_through_a_catch_all_ranks_below_a_real_one() {
    let inner = union_value(vec![num(0.0), raw_value()], Some(json("0"))).unwrap();
    let union = union_value(vec![inner, numbers()], empty_array()).unwrap();
    assert_eq!(p(&union, r#"["42"]"#), json("[42]"));
    let inner = union_value(
        vec![enumeration(&["auto"]), raw_value()],
        Some("auto".into()),
    )
    .unwrap();
    let union = union_value(vec![inner, num(0.0)], Some(json("0"))).unwrap();
    assert_eq!(p(&union, r#""640""#), json("640"));
}

/// Wraps `numberValue()` in `depth` levels built by `wrap`, and counts the
/// leaf's `accepts` calls for one parse.
fn accepts_checks(
    depth: usize,
    wrap: impl Fn(SerializationSchema) -> SerializationSchema,
    value: impl Fn(Value) -> Value,
) -> u32 {
    let base = num(0.0);
    let counter = Rc::new(std::cell::Cell::new(0));
    let (c, b) = (counter.clone(), base.clone());
    let mut schema = declaring(
        &base,
        move |v| {
            c.set(c.get() + 1);
            accepts(&b, v)
        },
        None,
    );
    let mut input = json("1");
    for _ in 0..depth {
        schema = wrap(schema);
        input = value(input);
    }
    counter.set(0);
    schema.parse(&input);
    counter.get()
}

fn wrap_value(v: Value) -> Value {
    obj(&[("value", v)])
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and measuring a union costs one traversal, not two
#[test]
fn and_measuring_a_union_costs_one_traversal_not_two() {
    let counts: Vec<u32> = [4, 8, 12, 16]
        .into_iter()
        .map(|depth| {
            let count = accepts_checks(
                depth,
                |s| {
                    union_value(
                        vec![object_value(vec![("value", s)]).unwrap(), string_value("")],
                        Some("".into()),
                    )
                    .unwrap()
                },
                wrap_value,
            );
            assert!(count as usize <= depth * 4, "{depth}: {count}");
            count
        })
        .collect();
    assert!(counts[3] < counts[0] * 8, "{counts:?}");
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a union field holding undefined is still the fit its members give it
#[test]
fn and_a_union_field_holding_undefined_is_still_the_fit_its_members_give_it() {
    let width = union_value(
        vec![
            optional(num(0.0), false).unwrap(),
            enumeration(&["inherit"]),
        ],
        Some("inherit".into()),
    )
    .unwrap();
    let specific = object_value(vec![("label", string_value("")), ("w", width)]).unwrap();
    let catch_all = object_value(vec![("label", string_value("")), ("w", raw_value())]).unwrap();
    let value = obj(&[("label", "keep me".into()), ("w", Value::Undefined)]);
    let expected = json(r#"{"label":"keep me","w":"inherit"}"#);
    for members in [
        vec![specific.clone(), catch_all.clone()],
        vec![catch_all, specific],
    ] {
        let union = union_value(members, Some("x".into())).unwrap();
        same(&union.parse(&value), &expected);
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and nesting unions directly costs no more than that
#[test]
fn and_nesting_unions_directly_costs_no_more_than_that() {
    let counts: Vec<u32> = [4, 8, 12, 16]
        .into_iter()
        .map(|depth| {
            let count = accepts_checks(
                depth,
                |s| union_value(vec![s, string_value("")], Some("".into())).unwrap(),
                |v| v,
            );
            assert!(count as usize <= depth * 4, "{depth}: {count}");
            count
        })
        .collect();
    assert!(counts[3] < counts[0] * 8, "{counts:?}");
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > but only if that member would really reach it
#[test]
fn but_only_if_that_member_would_really_reach_it() {
    let via_fallback = object_value(vec![(
        "tags",
        union_value(vec![numbers(), raw_value()], empty_array()).unwrap(),
    )])
    .unwrap();
    let value = json(r#"{"tags":["red","42"]}"#);
    assert_eq!(via_fallback.parse(&value), json(r#"{"tags":[0,42]}"#));
    let union = union_value(
        vec![
            via_fallback,
            object_value(vec![("tags", strings())]).unwrap(),
        ],
        Some("x".into()),
    )
    .unwrap();
    assert_eq!(union.parse(&value), value);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a catch-all inside a member is part of what that member fits
#[test]
fn and_a_catch_all_inside_a_member_is_part_of_what_that_member_fits() {
    let value = json(r#"{"a":"","b":[]}"#);
    let union = union_value(
        vec![
            object_value(vec![("a", string_value("")), ("b", string_value(""))]).unwrap(),
            object_value(vec![
                ("a", string_value("")),
                (
                    "b",
                    union_value(vec![string_value(""), raw_value()], Some("".into())).unwrap(),
                ),
            ])
            .unwrap(),
        ],
        Some("x".into()),
    )
    .unwrap();
    assert_eq!(union.parse(&value), value);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a member that normalizes still wins by declaration order
#[test]
fn and_a_member_that_normalizes_still_wins_by_declaration_order() {
    assert_eq!(p(&dimension(), r#""640""#), json("640"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a catch-all is one wherever it is reached from
#[test]
fn and_a_catch_all_is_one_wherever_it_is_reached_from() {
    let value = json(r#"["red","42"]"#);
    let union = union_value(
        vec![
            union_value(vec![numbers(), raw_value()], empty_array()).unwrap(),
            strings(),
        ],
        empty_array(),
    )
    .unwrap();
    assert_eq!(union.parse(&value), value);
    for wrapped in [
        optional(raw_value(), false).unwrap(),
        nullable(raw_value(), false).unwrap(),
        union_value(vec![raw_value()], Some(Value::Undefined)).unwrap(),
    ] {
        let ids = union_value(
            vec![
                object_value(vec![("id", string_value(""))]).unwrap(),
                wrapped.clone(),
            ],
            Some("x".into()),
        )
        .unwrap();
        assert_eq!(p(&ids, r#"{"id":42}"#), json(r#"{"id":""}"#));
        let n = union_value(vec![num(0.0), wrapped], Some(json("0"))).unwrap();
        assert_eq!(p(&n, r#"{"odd":1}"#), json(r#"{"odd":1}"#));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > but a bare raw member still comes last, and still catches
#[test]
fn but_a_bare_raw_member_still_comes_last_and_still_catches() {
    let union = union_value(vec![numbers(), raw_value()], empty_array()).unwrap();
    assert_eq!(p(&union, r#"[1,"x"]"#), json("[1,0]"));
    let union = union_value(vec![num(0.0), raw_value()], Some(json("0"))).unwrap();
    assert_eq!(p(&union, r#"{"odd":1}"#), json(r#"{"odd":1}"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and what a union reports is the member it will actually use
#[test]
fn and_what_a_union_reports_is_the_member_it_will_actually_use() {
    let via_fallback = object_value(vec![
        ("note", raw_value()),
        (
            "tags",
            union_value(vec![numbers(), raw_value()], empty_array()).unwrap(),
        ),
    ])
    .unwrap();
    let specific = object_value(vec![("note", raw_value()), ("tags", strings())]).unwrap();
    let value = json(r#"{"note":"keep me","tags":["red","42"]}"#);
    for members in [
        vec![via_fallback.clone(), specific.clone()],
        vec![specific, via_fallback],
    ] {
        assert_eq!(
            union_value(members, Some("x".into()))
                .unwrap()
                .parse(&value),
            value
        );
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own
#[test]
fn but_a_declared_predicate_still_says_which_values_are_its_own() {
    let general =
        object_value(vec![("kind", string_value("")), ("text", string_value(""))]).unwrap();
    let special_value = json(r#"{"kind":"special","text":""}"#);
    let sv = special_value.clone();
    let special = SerializationSchema::new(
        general.meta().clone(),
        special_value.clone(),
        None,
        Some(Rc::new(|v: &Value| {
            v.get("kind") == Some(&Value::from("special"))
        })),
        move |_| sv.clone(),
    );
    let parse = union_value(vec![special, general], Some("x".into())).unwrap();
    let ordinary = json(r#"{"kind":"ordinary","text":"keep me"}"#);
    assert_eq!(parse.parse(&ordinary), ordinary);
    assert_eq!(
        p(&parse, r#"{"kind":"special","text":"anything"}"#),
        special_value
    );
}

/// `[tagged, stringValue()]`, where `tagged` is `base` declaring that it
/// owns the strings starting with `#`, which it upper-cases.
fn tagged_union(base: SerializationSchema) -> SerializationSchema {
    fn is_tag(v: &Value) -> bool {
        v.as_str().is_some_and(|s| s.starts_with('#'))
    }
    let default = base.default_value().clone();
    let tagged = declaring(
        &base,
        is_tag,
        Some(Rc::new(move |v: &Value| {
            if is_tag(v) {
                Value::String(v.as_str().unwrap().to_uppercase())
            } else {
                default.clone()
            }
        })),
    );
    union_value(vec![tagged, string_value("")], Some("".into())).unwrap()
}

fn check_declared_predicate(base: SerializationSchema) {
    let parse = tagged_union(base);
    assert_eq!(p(&parse, r#""ordinary""#), Value::from("ordinary"));
    assert_eq!(p(&parse, r##""#tag""##), Value::from("#TAG"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: raw
#[test]
fn but_a_declared_predicate_still_says_which_values_are_its_own_raw() {
    check_declared_predicate(raw_value());
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: nullable
#[test]
fn but_a_declared_predicate_still_says_which_values_are_its_own_nullable() {
    check_declared_predicate(nullable(string_value(""), false).unwrap());
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: optional
#[test]
fn but_a_declared_predicate_still_says_which_values_are_its_own_optional() {
    check_declared_predicate(optional(string_value(""), false).unwrap());
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: transform
#[test]
fn but_a_declared_predicate_still_says_which_values_are_its_own_transform() {
    check_declared_predicate(transform_value(string_value(""), |v| v, None).unwrap());
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: aliased
#[test]
fn but_a_declared_predicate_still_says_which_values_are_its_own_aliased() {
    let aliases = vec![("#alias".to_owned(), "#aliased".into())];
    check_declared_predicate(aliased_value(string_value(""), aliases).unwrap());
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: union
#[test]
fn but_a_declared_predicate_still_says_which_values_are_its_own_union() {
    check_declared_predicate(
        union_value(vec![string_value(""), num(0.0)], Some("".into())).unwrap(),
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and naming an accessor on one neither silences it nor its original
#[test]
fn and_naming_an_accessor_on_one_neither_silences_it_nor_its_original() {
    // Accessors are named on a `SchemaField` here, not on the schema, so
    // naming one copies nothing: what holds is that the declared `accepts`
    // stays the author's, on the union's member and through a field.
    let parse = tagged_union(nullable(string_value(""), false).unwrap());
    assert_eq!(p(&parse, r#""ordinary""#), Value::from("ordinary"));
    let SerializationSchemaMeta::Union { members } = parse.meta() else {
        panic!("not a union");
    };
    let tagged = members[0].clone();
    assert!(tagged.declared_accepts().is_some());
    let named = with_accessors(tagged.clone(), None, Some(Rc::new(|_, _| {}))).unwrap();
    assert!(named.schema().is(&tagged));
    assert!(named.schema().declared_accepts().is_some());
    assert_eq!(p(&parse, r#""ordinary""#), Value::from("ordinary"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and a predicate wider than its metadata is not overruled by it
#[test]
fn and_a_predicate_wider_than_its_metadata_is_not_overruled_by_it() {
    let items = strings();
    let i = items.clone();
    let legacy = declaring(
        &items,
        |v| matches!(v, Value::Array(_) | Value::String(_)),
        Some(Rc::new(move |v: &Value| match v {
            Value::String(_) => split_commas(v.clone()),
            _ => i.parse(v),
        })),
    );
    let union = union_value(vec![legacy, num(0.0)], Some(json("0"))).unwrap();
    assert_eq!(p(&union, r#""a,b""#), json(r#"["a","b"]"#));
    assert_eq!(p(&union, "7"), json("7"));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and that holds for a wrapper or a union, not just a container
#[test]
fn and_that_holds_for_a_wrapper_or_a_union_not_just_a_container() {
    fn owns(v: &Value) -> bool {
        matches!(v, Value::Undefined | Value::Number(_)) || v.as_str() == Some("inherit")
    }
    for base in [
        optional(num(0.0), false).unwrap(),
        union_value(vec![num(0.0)], Some(json("0"))).unwrap(),
    ] {
        let b = base.clone();
        let width = declaring(
            &base,
            owns,
            Some(Rc::new(move |v: &Value| {
                if v.as_str() == Some("inherit") {
                    v.clone()
                } else {
                    b.parse(v)
                }
            })),
        );
        let parse = union_value(vec![width, enumeration(&["auto"])], Some("auto".into())).unwrap();
        assert_eq!(p(&parse, r#""inherit""#), Value::from("inherit"));
        assert_eq!(p(&parse, r#""auto""#), Value::from("auto"));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > an object with a transform field round-trips
#[test]
fn an_object_with_a_transform_field_round_trips() {
    let shape = object_value(vec![
        ("title", string_value("")),
        ("v", transform_value(num(0.0), prefix_n, None).unwrap()),
    ])
    .unwrap();
    let union = union_value(vec![shape, enumeration(&["LOST"])], Some("LOST".into())).unwrap();
    assert_eq!(
        p(&union, r#"{"title":"keep me","v":"n5"}"#),
        json(r#"{"title":"keep me","v":"n0"}"#)
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and one whose field declines its own default
#[test]
fn and_one_whose_field_declines_its_own_default() {
    let value = json(r#"{"label":"keep me","n":0}"#);
    let n = num_with(
        0.0,
        NumberValueOptions {
            min: Some(1.0),
            ..Default::default()
        },
    );
    let shape = object_value(vec![("label", string_value("")), ("n", n)]).unwrap();
    let union = union_value(vec![shape, enumeration(&["auto"])], Some("auto".into())).unwrap();
    assert_eq!(union.parse(&value), value);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and a nested object that gained a key keeps its siblings
#[test]
fn and_a_nested_object_that_gained_a_key_keeps_its_siblings() {
    let shape = object_value(vec![
        (
            "item",
            object_value(vec![("id", string_value(""))]).unwrap(),
        ),
        ("title", string_value("")),
    ])
    .unwrap();
    let union = union_value(vec![shape, enumeration(&["auto"])], Some("auto".into())).unwrap();
    assert_eq!(
        p(&union, r#"{"item":{"extra":1,"id":"a"},"title":"T"}"#),
        json(r#"{"item":{"id":"a"},"title":"T"}"#)
    );
}

fn x_or_y() -> SerializationSchema {
    union_value(
        vec![
            object_value(vec![("x", num(0.0))]).unwrap(),
            object_value(vec![("y", num(0.0))]).unwrap(),
        ],
        Some("none".into()),
    )
    .unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > while the strict pass still tells the variants apart
#[test]
fn while_the_strict_pass_still_tells_the_variants_apart() {
    assert_eq!(p(&x_or_y(), r#"{"y":1}"#), json(r#"{"y":1}"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a strict match never claims what the schema itself declines > so a union and its own accepts cannot disagree
#[test]
fn so_a_union_and_its_own_accepts_cannot_disagree() {
    let inner = union_value(
        vec![optional(num(0.0), false).unwrap(), enumeration(&["auto"])],
        Some("auto".into()),
    )
    .unwrap();
    let shape = object_value(vec![("title", string_value("")), ("x", inner)]).unwrap();
    let value = obj(&[("title", "t".into()), ("x", Value::Undefined)]);
    let outer = union_value(
        vec![shape.clone(), enumeration(&["none"])],
        Some("none".into()),
    )
    .unwrap();
    let expected = if accepts(&shape, &value) {
        shape.parse(&value)
    } else {
        "none".into()
    };
    same(&outer.parse(&value), &expected);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a strict match never claims what the schema itself declines > and wrapping a member does not change the union’s answer
#[test]
fn and_wrapping_a_member_does_not_change_the_unions_answer() {
    let member = object_value(vec![
        (
            "note",
            union_value(
                vec![optional(num(0.0), false).unwrap(), string_value("")],
                Some(json("0")),
            )
            .unwrap(),
        ),
        ("title", string_value("")),
    ])
    .unwrap();
    let value = obj(&[("note", Value::Undefined), ("title", "keep me".into())]);
    let mut answers: Vec<String> = [
        member.clone(),
        optional(member.clone(), false).unwrap(),
        nullable(member.clone(), false).unwrap(),
        transform_value(member, |v| v, None).unwrap(),
    ]
    .into_iter()
    .map(|m| {
        union_value(vec![m, enumeration(&["LOST"])], Some("LOST".into()))
            .unwrap()
            .parse(&value)
            .to_string()
    })
    .collect();
    answers.dedup();
    assert_eq!(answers.len(), 1, "{answers:?}");
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a strict match never claims what the schema itself declines > and a union recognizes the value it defaults to
#[test]
fn and_a_union_recognizes_the_value_it_defaults_to() {
    let u = union_value(
        vec![optional(string_value(""), false).unwrap(), num(0.0)],
        Some(Value::Undefined),
    )
    .unwrap();
    assert!(accepts(&u, &Value::Undefined));
    let shape = object_value(vec![("note", u), ("title", string_value(""))]).unwrap();
    assert!(accepts(&shape, &p(&shape, r#"{"title":"x"}"#)));
    let union = union_value(vec![shape, enumeration(&["LOST"])], Some("LOST".into())).unwrap();
    same(&p(&union, r#"{"title":"x"}"#), &json(r#"{"title":"x"}"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > one malformed element does not discard the good ones
#[test]
fn one_malformed_element_does_not_discard_the_good_ones() {
    let union = union_value(vec![strings(), enumeration(&["none"])], Some("none".into())).unwrap();
    assert_eq!(p(&union, r#"["a",1]"#), json(r#"["a",""]"#));
    let shape = object_value(vec![("label", string_value("")), ("tags", numbers())]).unwrap();
    let union = union_value(vec![shape, enumeration(&["auto"])], Some("auto".into())).unwrap();
    assert_eq!(
        p(&union, r#"{"label":"keep me","tags":[1,null]}"#),
        json(r#"{"label":"keep me","tags":[1,0]}"#)
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > an array no element of which matches keeps its enclosing object
#[test]
fn an_array_no_element_of_which_matches_keeps_its_enclosing_object() {
    let auto = || Some(Value::from("auto"));
    let shape = object_value(vec![("label", string_value("")), ("tags", numbers())]).unwrap();
    let union = union_value(vec![shape, enumeration(&["auto"])], auto()).unwrap();
    assert_eq!(
        p(&union, r#"{"label":"keep me","tags":["banana"]}"#),
        json(r#"{"label":"keep me","tags":[0]}"#)
    );
    let doc = object_value(vec![
        (
            "items",
            array_value(object_value(vec![("id", string_value(""))]).unwrap()).unwrap(),
        ),
        ("title", string_value("")),
    ])
    .unwrap();
    let union = union_value(vec![doc, enumeration(&["auto"])], auto()).unwrap();
    assert_eq!(
        p(&union, r#"{"items":[{"extra":1,"id":"a"}],"title":"T"}"#),
        json(r#"{"items":[{"id":"a"}],"title":"T"}"#)
    );
    let rows = array_value(transform_value(num(0.0), prefix_n, None).unwrap()).unwrap();
    let union = union_value(vec![rows, enumeration(&["none"])], Some("none".into())).unwrap();
    assert_eq!(p(&union, r#"["n1","n2"]"#), json(r#"["n0","n0"]"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > and a catch-all member never outranks one that owns the value
#[test]
fn and_a_catch_all_member_never_outranks_one_that_owns_the_value() {
    let union = union_value(vec![numbers(), raw_value()], empty_array()).unwrap();
    assert_eq!(p(&union, r#"[1,"x"]"#), json("[1,0]"));
    assert!(accepts(&raw_value(), &"anything".into()));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > an absent element says nothing either way
#[test]
fn an_absent_element_says_nothing_either_way() {
    // An absent element is one holding `undefined`.
    assert!(accepts(
        &strings(),
        &Value::Array(vec![Value::Undefined; 2])
    ));
    assert!(accepts(&strings(), &json("[]")));
    let union = union_value(vec![numbers(), strings()], empty_array()).unwrap();
    let holes = Value::Array(vec![Value::Undefined, "a".into()]);
    assert_eq!(union.parse(&holes), json(r#"["","a"]"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > a complete match beats a partial one, whichever comes first
#[test]
fn a_complete_match_beats_a_partial_one_whichever_comes_first() {
    let union = union_value(vec![numbers(), strings()], empty_array()).unwrap();
    assert_eq!(p(&union, r#"["red","42"]"#), json(r#"["red","42"]"#));
    let tags = union_value(
        vec![
            array_value(object_value(vec![("label", string_value(""))]).unwrap()).unwrap(),
            array_value(
                object_value(vec![
                    ("color", optional(string_value(""), false).unwrap()),
                    ("label", string_value("")),
                ])
                .unwrap(),
            )
            .unwrap(),
        ],
        empty_array(),
    )
    .unwrap();
    let value = json(r#"[{"label":"a"},{"color":"red","label":"b"}]"#);
    same(&tags.parse(&value), &value);
}

fn xs_or_ys(default: Option<Value>) -> SerializationSchema {
    union_value(
        vec![
            array_value(object_value(vec![("x", num(0.0))]).unwrap()).unwrap(),
            array_value(object_value(vec![("y", num(0.0))]).unwrap()).unwrap(),
        ],
        default,
    )
    .unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > and still picks the variant whose items match
#[test]
fn and_still_picks_the_variant_whose_items_match() {
    let union = union_value(vec![numbers(), strings()], empty_array()).unwrap();
    assert_eq!(p(&union, r#"["red","blue"]"#), json(r#"["red","blue"]"#));
    assert_eq!(
        p(&xs_or_ys(empty_array()), r#"[{"y":1}]"#),
        json(r#"[{"y":1}]"#)
    );
    let tags = union_value(
        vec![
            object_value(vec![("tags", numbers())]).unwrap(),
            object_value(vec![("tags", strings())]).unwrap(),
        ],
        Some("none".into()),
    )
    .unwrap();
    let value = json(r#"{"tags":["red","blue"]}"#);
    assert_eq!(tags.parse(&value), value);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema answers only for its own element domain > a union falls through to the array variant whose items match
#[test]
fn a_union_falls_through_to_the_array_variant_whose_items_match() {
    let union = union_value(vec![numbers(), strings()], None).unwrap();
    assert_eq!(p(&union, r#"["red","blue"]"#), json(r#"["red","blue"]"#));
    assert_eq!(p(&union, "[1,2]"), json("[1,2]"));
    assert_eq!(p(&xs_or_ys(None), r#"[{"y":1}]"#), json(r#"[{"y":1}]"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema answers only for its own element domain > and an enclosing object still picks the variant its items fit
#[test]
fn and_an_enclosing_object_still_picks_the_variant_its_items_fit() {
    let strings_obj = object_value(vec![("tags", strings())]).unwrap();
    let numbers_obj = object_value(vec![("tags", numbers())]).unwrap();
    assert!(accepts(&strings_obj, &json(r#"{"tags":[1,2]}"#)));
    let none = || Some(Value::from("none"));
    let union = union_value(vec![numbers_obj.clone(), strings_obj.clone()], none()).unwrap();
    assert_eq!(
        p(&union, r#"{"tags":["a","b"]}"#),
        json(r#"{"tags":["a","b"]}"#)
    );
    let union = union_value(vec![strings_obj.clone(), numbers_obj], none()).unwrap();
    assert_eq!(p(&union, r#"{"tags":[1,2]}"#), json(r#"{"tags":[1,2]}"#));
    assert_eq!(
        p(&strings_obj, r#"{"tags":[1,2]}"#),
        json(r#"{"tags":["",""]}"#)
    );
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema answers only for its own element domain > an empty array belongs to every array schema
#[test]
fn an_empty_array_belongs_to_every_array_schema() {
    assert!(accepts(&numbers(), &json("[]")));
    let objects = array_value(object_value(vec![("x", num(0.0))]).unwrap()).unwrap();
    assert!(accepts(&objects, &json("[]")));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union compares by content, not by a member’s comparator > a transformed output compares by content
#[test]
fn a_transformed_output_compares_by_content() {
    let union = union_value(vec![to_array()], Some(json(r#"[""]"#))).unwrap();
    let parse = |s: &str| union.parse(&s.into());
    assert!(is_schema_equal(&union, &parse("a,b"), &parse("a,b")));
    assert!(!is_schema_equal(&union, &parse("a,b"), &parse("a,c")));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union compares by content, not by a member’s comparator > through nullable and optional, whose default is a nil
#[test]
fn through_nullable_and_optional_whose_default_is_a_nil() {
    for member in [
        nullable(to_array(), false).unwrap(),
        optional(to_array(), false).unwrap(),
    ] {
        let union = union_value(vec![member], Some(Value::Null)).unwrap();
        let ab = json(r#"["a","b"]"#);
        assert!(is_schema_equal(&union, &ab, &json(r#"["a","b"]"#)));
        assert!(!is_schema_equal(&union, &ab, &json(r#"["a","c"]"#)));
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union compares by content, not by a member’s comparator > so a wrapper around it can still recognize its own default
#[test]
fn so_a_wrapper_around_it_can_still_recognize_its_own_default() {
    let union = union_value(vec![to_array()], Some(json(r#"[""]"#))).unwrap();
    let schema = optional(union, true).unwrap();
    assert_eq!(p(&schema, r#""""#), Value::Undefined);
    assert_eq!(p(&schema, r#""a,b""#), json(r#"["a","b"]"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a union compares by content, not by a member’s comparator > and two members’ values are never confused for each other
#[test]
fn and_two_members_values_are_never_confused_for_each_other() {
    let to_ids = transform_value(
        string_value(""),
        |v| {
            Value::Array(
                v.as_str()
                    .unwrap_or("")
                    .split(',')
                    .map(|s| obj(&[("id", Value::Number(s.parse().unwrap_or(f64::NAN)))]))
                    .collect(),
            )
        },
        None,
    )
    .unwrap();
    let union = union_value(vec![to_ids, strings()], empty_array()).unwrap();
    assert!(!is_schema_equal(
        &union,
        &json(r#"["red"]"#),
        &json(r#"["blue"]"#)
    ));
    assert!(is_schema_equal(
        &union,
        &json(r#"["red"]"#),
        &json(r#"["red"]"#)
    ));
    let numeric = union_value(
        vec![
            by_id(),
            object_value(vec![("id", num(0.0)), ("tag", num(0.0))]).unwrap(),
        ],
        Some(json(r#"{"id":0,"tag":0}"#)),
    )
    .unwrap();
    assert!(!is_schema_equal(
        &numeric,
        &json(r#"{"id":1,"tag":5}"#),
        &json(r#"{"id":1,"tag":9}"#)
    ));
}

fn point_or_label() -> (SerializationSchema, SerializationSchema) {
    let point = object_value(vec![("x", num(0.0))]).unwrap();
    let label = object_value(vec![("label", string_value(""))]).unwrap();
    let union = union_value(vec![point.clone(), label], None).unwrap();
    (point, union)
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > a union falls through to the variant whose keys match
#[test]
fn a_union_falls_through_to_the_variant_whose_keys_match() {
    let (_, union) = point_or_label();
    assert_eq!(
        p(&union, r#"{"label":"hello"}"#),
        json(r#"{"label":"hello"}"#)
    );
    assert_eq!(p(&union, r#"{"x":5}"#), json(r#"{"x":5}"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > and declines what belongs to neither
#[test]
fn and_declines_what_belongs_to_neither() {
    let (_, union) = point_or_label();
    for value in [json(r#"{"x":"banana"}"#), json("[1,2]"), "str".into()] {
        assert_eq!(union.parse(&value), *union.default_value(), "{value}");
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > an empty object belongs to every variant
#[test]
fn an_empty_object_belongs_to_every_variant() {
    let (_, union) = point_or_label();
    assert_eq!(p(&union, "{}"), json(r#"{"x":0}"#));
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > which is what lets a nested empty object survive a round trip
#[test]
fn which_is_what_lets_a_nested_empty_object_survive_a_round_trip() {
    let nested = union_value(
        vec![
            enumeration(&["auto"]),
            object_value(vec![
                ("label", string_value("")),
                (
                    "options",
                    object_value(vec![("color", optional(string_value(""), true).unwrap())])
                        .unwrap(),
                ),
            ])
            .unwrap(),
        ],
        None,
    )
    .unwrap();
    let expected = json(r#"{"label":"hello","options":{}}"#);
    let parsed = p(&nested, r#"{"label":"hello","options":{"color":""}}"#);
    same(&parsed, &expected);
    same(&p(&nested, &parsed.to_string()), &expected);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > membership is the same question the parse asks
#[test]
fn membership_is_the_same_question_the_parse_asks() {
    let (point, _) = point_or_label();
    assert!(accepts(&point, &json(r#"{"x":1}"#)));
    assert!(!accepts(&point, &json(r#"{"label":"hello"}"#)));
    assert!(!accepts(&point, &json(r#"{"label":"hello","x":1}"#)));
    assert!(accepts(&point, &json("{}")));
    assert!(!accepts(&point, &json("[]")));
}

/// A string leaf that counts how often it is parsed.
fn counted_leaf(counter: &Rc<std::cell::Cell<u32>>) -> SerializationSchema {
    let c = counter.clone();
    transform_value(
        string_value(""),
        move |v| {
            c.set(c.get() + 1);
            v
        },
        None,
    )
    .unwrap()
}

fn parses(schema: &SerializationSchema, value: &Value, counter: &Rc<std::cell::Cell<u32>>) -> u32 {
    counter.set(0);
    schema.parse(value);
    counter.get()
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a value is parsed once per traversal > through an object, a union, and the wrappers
#[test]
fn through_an_object_a_union_and_the_wrappers() {
    let c = Rc::new(std::cell::Cell::new(0));
    let a = json(r#"{"a":"x"}"#);
    let leaf_obj = || object_value(vec![("a", counted_leaf(&c))]).unwrap();
    for schema in [
        leaf_obj(),
        union_value(vec![enumeration(&["auto"]), leaf_obj()], None).unwrap(),
        nullable(leaf_obj(), false).unwrap(),
        optional(union_value(vec![leaf_obj()], None).unwrap(), false).unwrap(),
        aliased_value(leaf_obj(), Vec::new()).unwrap(),
    ] {
        assert_eq!(parses(&schema, &a, &c), 1, "{schema:?}");
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a value is parsed once per traversal > once per element of an array, not once per element per element
#[test]
fn once_per_element_of_an_array_not_once_per_element_per_element() {
    let c = Rc::new(std::cell::Cell::new(0));
    let schema = array_value(counted_leaf(&c)).unwrap();
    assert_eq!(parses(&schema, &json(r#"["a","b","c"]"#), &c), 3);
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a value is parsed once per traversal > and stays linear as unions nest
#[test]
fn and_stays_linear_as_unions_nest() {
    let c = Rc::new(std::cell::Cell::new(0));
    let mut schema = object_value(vec![("a", counted_leaf(&c))]).unwrap();
    let mut value = json(r#"{"a":"x"}"#);
    for _ in 0..10 {
        schema = union_value(
            vec![
                enumeration(&["auto"]),
                object_value(vec![("inner", schema)]).unwrap(),
            ],
            None,
        )
        .unwrap();
        value = obj(&[("inner", value)]);
    }
    assert_eq!(parses(&schema, &value, &c), 1);
}

thread_local! {
    static STORE_WRITES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    static RECORDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A field whose copies are recorded: what JS's `afterCloneFrom` assigns.
#[derive(Debug, Default)]
struct Recorded(String);

impl Clone for Recorded {
    fn clone(&self) -> Self {
        if RECORDING.with(std::cell::Cell::get) {
            STORE_WRITES.with(|w| w.borrow_mut().push(self.0.clone()));
        }
        Recorded(self.0.clone())
    }
}

impl crate::schema::SchemaValue for Recorded {
    fn to_schema_value(&self) -> Value {
        Value::String(self.0.clone())
    }
    fn from_schema_value(value: &Value) -> Option<Self> {
        value.as_str().map(|s| Recorded(s.to_owned()))
    }
}

/// `RestateNode extends StoreNode`, re-declaring StoreNode's `shared`.
#[derive(Clone, Debug, Default)]
struct RestateNode {
    base: NodeBase,
    element: ElementData,
    shared: Recorded,
}

thread_local! {
    static RESTATE_SCHEMA: Vec<NodeSchema> = ["other", ""]
        .map(|default| {
            node_schema(vec![(
                "shared",
                with_field(string_value(default), |n: &RestateNode| &n.shared, |n: &mut RestateNode| &mut n.shared)
                    .unwrap(),
            )])
            .unwrap()
        })
        .to_vec();
}

impl LexicalNode for RestateNode {
    fn get_type(&self) -> &'static str {
        "assign-once-derived"
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
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        let mut chain = RESTATE_SCHEMA.with(Clone::clone);
        chain.push(crate::nodes::element::element_node_schema());
        chain
    }
}

// upstream: lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > a re-declared field is assigned once, by the class that owns it
#[test]
fn a_re_declared_field_is_assigned_once_by_the_class_that_owns_it() {
    // JS also checks which classes get a synthesized `afterCloneFrom`; a
    // Rust node's clone is its `Clone`, which copies each field once
    // however many schemas in its chain declare it.
    let editor = editor_with(vec![NodeClass::of::<RestateNode>()]);
    update(&editor, || {
        let prev = create::<RestateNode>();
        with_writable(prev.get_key(), |n| {
            n.downcast_mut::<RestateNode>().unwrap().shared = Recorded("carried".to_owned());
        });
        STORE_WRITES.with(|w| w.borrow_mut().clear());
        RECORDING.with(|r| r.set(true));
        let copy = prev.get_latest().clone_node();
        RECORDING.with(|r| r.set(false));
        assert_eq!(
            copy.downcast_ref::<RestateNode>().unwrap().shared.0,
            "carried"
        );
        Ok(())
    });
    assert_eq!(STORE_WRITES.with(|w| w.borrow().clone()), ["carried"]);
}
