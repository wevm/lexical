//! serialization/schema: each `LexicalSchema.ts` value builder parsing a
//! table of inputs, and serialization/schema-node: a node whose `$config`
//! declares a `json` schema (`TestSchemaNode`, ops.ts).

use crate::case::{Case, Outcome, same};
use lexical::json::{Value, stringify};
use lexical::schema::{
    NumberValueOptions, SerializationSchema, aliased_value, array_set_value, array_value,
    boolean_value, enum_value, is_schema_default, nullable, number_value, object_value, optional,
    raw_value, string_value, transform_value, union_value,
};
use lexical::{CreateEditorArgs, LexicalError, UpdateOptions, create_editor, get_root};

/// `numberValue(default, {min, max, integer, clamp})`.
fn number(
    default_value: f64,
    min: Option<f64>,
    max: Option<f64>,
    integer: bool,
    clamp: bool,
) -> Result<SerializationSchema, LexicalError> {
    number_value(
        default_value,
        NumberValueOptions {
            min,
            max,
            integer,
            clamp,
        },
    )
}

fn plain_number() -> SerializationSchema {
    number(0.0, None, None, false, false).expect("numberValue()")
}

/// The schema a case names, built from its JS: `SCHEMAS` and
/// `REFUSED_SCHEMAS` in serialization.test.ts.
fn build(name: &str) -> Result<Result<SerializationSchema, LexicalError>, String> {
    let s = |v: &str| Value::from(v);
    Ok(match name {
        "aliasedValue(enumValue(['number', 'bullet']), {ol: 'number', ul: 'bullet'})" => {
            enum_value(vec![s("number"), s("bullet")], None).and_then(|inner| {
                aliased_value(
                    inner,
                    vec![("ol".into(), s("number")), ("ul".into(), s("bullet"))],
                )
            })
        }
        "arraySetValue(enumValue(['a', 'b']))" => {
            enum_value(vec![s("a"), s("b")], None).and_then(array_set_value)
        }
        "arraySetValue(stringValue())" => array_set_value(string_value("")),
        "arrayValue(numberValue())" => array_value(plain_number()),
        "booleanValue()" => Ok(boolean_value(false)),
        "booleanValue(true)" => Ok(boolean_value(true)),
        "enumValue(['a', 'b', 1])" => enum_value(vec![s("a"), s("b"), Value::Number(1.0)], None),
        "enumValue(['ltr', 'rtl', null], null)" => {
            enum_value(vec![s("ltr"), s("rtl"), Value::Null], Some(Value::Null))
        }
        "nullable(objectValue({n: nullable(booleanValue())}))" => {
            nullable(boolean_value(false), false)
                .and_then(|n| object_value(vec![("n", n)]))
                .and_then(|o| nullable(o, false))
        }
        "nullable(stringValue('x'))" => nullable(string_value("x"), false),
        "nullable(stringValue('x'), {defaultAsNull: true})" => nullable(string_value("x"), true),
        "numberValue()" => Ok(plain_number()),
        "numberValue(0, {clamp: true, max: 1, min: -1})" => {
            number(0.0, Some(-1.0), Some(1.0), false, true)
        }
        "numberValue(0, {integer: true})" => number(0.0, None, None, true, false),
        "numberValue(1, {clamp: true, integer: true, max: 3.5, min: 0.5})" => {
            number(1.0, Some(0.5), Some(3.5), true, true)
        }
        "numberValue(5, {max: 10, min: 0})" => number(5.0, Some(0.0), Some(10.0), false, false),
        "objectValue({a: numberValue(), b: optional(stringValue())})" => {
            optional(string_value(""), false)
                .and_then(|b| object_value(vec![("a", plain_number()), ("b", b)]))
        }
        "optional(numberValue(1))" => {
            number(1.0, None, None, false, false).and_then(|inner| optional(inner, false))
        }
        "optional(numberValue(1), {omitDefault: true})" => {
            number(1.0, None, None, false, false).and_then(|inner| optional(inner, true))
        }
        "rawValue()" => Ok(raw_value()),
        "stringValue('x')" => Ok(string_value("x")),
        "stringValue()" => Ok(string_value("")),
        "transformValue(stringValue(), s => s + '!')" => transform_value(
            string_value(""),
            |v| Value::String(format!("{}!", v.as_str().unwrap_or(""))),
            None,
        ),
        "unionValue([booleanValue(), stringValue('d')], 'd')" => {
            union_value(vec![boolean_value(false), string_value("d")], Some(s("d")))
        }
        "unionValue([numberValue(), rawValue()])" => {
            union_value(vec![plain_number(), raw_value()], None)
        }
        "unionValue([numberValue(), stringValue()])" => {
            union_value(vec![plain_number(), string_value("")], None)
        }
        "arraySetValue(numberValue())" => array_set_value(plain_number()),
        "arrayValue(arraySetValue(stringValue()))" => {
            array_set_value(string_value("")).and_then(array_value)
        }
        "enumValue([])" => enum_value(vec![], None),
        "enumValue(['a'], 'b')" => enum_value(vec![s("a")], Some(s("b"))),
        "numberValue(0, {integer: true, max: 1.8, min: 1.2})" => {
            number(0.0, Some(1.2), Some(1.8), true, false)
        }
        "numberValue(0, {max: 1, min: 5})" => number(0.0, Some(5.0), Some(1.0), false, false),
        "objectValue({toString: numberValue()})" => {
            object_value(vec![("toString", plain_number())])
        }
        "unionValue([])" => union_value(vec![], None),
        other => return Err(format!("no Rust schema for {other}")),
    })
}

/// `JSON.stringify(value)` as the vector records it: absent for
/// `undefined`.
fn recorded(value: &Value) -> Option<String> {
    stringify(value)
}

fn optional_str<'a>(case: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match case.get(key) {
        None => Ok(None),
        Some(_) => case.str(key).map(Some),
    }
}

/// A serialization/schema case: the schema's default and, per input, the
/// parsed value and `isSchemaDefault`; or the message of a declaration the
/// builder refuses.
pub fn check(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let name = case.str("schema")?;
    let built = build(name)?;
    if let Some(error) = case.get("error") {
        return match built {
            Ok(_) => Err(format!(
                "{name}: Rust builds it, JS refuses it with {error}"
            )),
            Err(e) => {
                same(name, e.message(), error.as_str().unwrap_or_default()).map(|()| Outcome::Pass)
            }
        };
    }
    let schema = built.map_err(|e| format!("{name}: Rust refuses it: {}", e.message()))?;
    same(
        format_args!("{name} default"),
        recorded(schema.default_value()).as_deref(),
        optional_str(case, "defaultValue")?,
    )?;
    for result in case.arr("results")? {
        let input = result.get("input").cloned().unwrap_or(Value::Undefined);
        let parsed = schema.parse(&input);
        let shown = recorded(&input).unwrap_or_else(|| "undefined".into());
        same(
            format_args!("{name} parsing {shown}"),
            recorded(&parsed).as_deref(),
            optional_str(result, "parsed")?,
        )?;
        same(
            format_args!("{name} isSchemaDefault of {shown}"),
            Some(is_schema_default(&schema, &parsed)),
            result.get("isDefault").and_then(Value::as_bool),
        )?;
    }
    Ok(Outcome::Pass)
}

/// A serialization/schema-node case: `TestSchemaNode` imported from
/// `input` and from each form writes both forms byte for byte, its
/// generated JSON (none: it has only its schema) agrees with the walk, and
/// a clone made in a later update writes the legacy form again.
pub fn check_node(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let input = case.str("input")?;
    let legacy = case.str("legacy")?;
    let compact = case.str("compact")?;
    let editor = create_editor(CreateEditorArgs {
        nodes: crate::scripts::test_schema_nodes(),
        ..CreateEditorArgs::default()
    });
    for (form, text) in [("input", input), ("legacy", legacy), ("compact", compact)] {
        let state = editor
            .parse_editor_state(text)
            .map_err(|e| format!("parsing the {form}: {e}"))?;
        same(
            format_args!("legacy from {form}"),
            state.to_json(false).to_string().as_str(),
            legacy,
        )?;
        same(
            format_args!("compact from {form}"),
            state.to_json(true).to_string().as_str(),
            compact,
        )?;
        super::serialization::generated_agrees_with_schema(&state)
            .map_err(|e| format!("from {form}: {e}"))?;
    }
    let state = editor
        .parse_editor_state(input)
        .map_err(|e| format!("parsing the input: {e}"))?;
    editor
        .set_editor_state(&state, None)
        .map_err(|e| format!("setting the state: {e}"))?;
    editor
        .update(
            || {
                // `getWritable()`: a clone of the node, made in this update.
                get_root().get_first_child().expect("the node").mark_dirty();
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .map_err(|e| format!("cloning: {e}"))?;
    same(
        "legacy of the clone",
        editor
            .get_editor_state()
            .to_json(false)
            .to_string()
            .as_str(),
        legacy,
    )?;
    Ok(Outcome::Pass)
}
