//! `LexicalSchema.ts`: serialization schemas. A [`SerializationSchema`]
//! parses an untrusted JSON value into its domain (substituting its default
//! for anything outside it), knows that default, and describes its domain
//! ([`SerializationSchemaMeta`]). A node class declares the properties of
//! its JSON as a [`NodeSchema`] (`nodeSchema`) of [`SchemaField`]s
//! (`withField`, `withAccessors`), which drives its `exportJSON` in both
//! forms and its `updateFromJSON` (`LexicalUtils.ts`' compiled schema,
//! [`walk_export_json`] and [`walk_update_from_json`]); a NodeState can be
//! parsed by one (`createState(key, {parse: schema})`,
//! [`StateValueConfig::from_schema`](crate::StateValueConfig::from_schema)).
//!
//! As in the Swift port, a schema's domain is the JSON value itself
//! ([`Value`], whose `Undefined` is JS's `undefined`), so one type-erased
//! walk serves every node; a field's accessors convert between that and the
//! node's typed storage ([`SchemaValue`]). Values are immutable, so JS's
//! freezing and its record of caller-owned values have nothing to do, and
//! where JS compares two arrays or objects by identity (`===`) the port
//! compares them by content (rust/PARITY.md).

use crate::LexicalError;
use crate::json::{Object, Value};
use crate::node::LexicalNode;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// A schema's parse: `(value: unknown) => T`, where `Value::Undefined` gives
/// the default.
pub type SchemaParse = Rc<dyn Fn(&Value) -> Value>;
/// A schema's `isEqual(a, b)`.
pub type SchemaIsEqual = Rc<dyn Fn(&Value, &Value) -> bool>;
/// A schema's `accepts(value)`.
pub type SchemaAccepts = Rc<dyn Fn(&Value) -> bool>;

/// `SerializationSchemaMeta`: what a schema describes, for tooling that
/// introspects it, and for a union choosing a member.
#[derive(Clone)]
pub enum SerializationSchemaMeta {
    String,
    Number {
        min: Option<f64>,
        max: Option<f64>,
        integer: bool,
        clamp: bool,
    },
    Boolean,
    Enum {
        values: Vec<Value>,
    },
    Array {
        item: SerializationSchema,
    },
    /// `arraySetValue`.
    Set {
        item: SerializationSchema,
    },
    Nullable {
        inner: SerializationSchema,
        default_as_null: bool,
    },
    Optional {
        inner: SerializationSchema,
        omit_default: bool,
    },
    Union {
        members: Vec<SerializationSchema>,
    },
    Raw,
    Object {
        fields: Vec<(String, SerializationSchema)>,
    },
    Aliased {
        inner: SerializationSchema,
        aliases: Vec<(String, Value)>,
    },
    Transform {
        inner: SerializationSchema,
    },
}

impl SerializationSchemaMeta {
    /// `meta.kind`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number { .. } => "number",
            Self::Boolean => "boolean",
            Self::Enum { .. } => "enum",
            Self::Array { .. } => "array",
            Self::Set { .. } => "set",
            Self::Nullable { .. } => "nullable",
            Self::Optional { .. } => "optional",
            Self::Union { .. } => "union",
            Self::Raw => "raw",
            Self::Object { .. } => "object",
            Self::Aliased { .. } => "aliased",
            Self::Transform { .. } => "transform",
        }
    }
}

struct SchemaInner {
    meta: SerializationSchemaMeta,
    default_value: Value,
    is_equal: Option<SchemaIsEqual>,
    accepts: Option<SchemaAccepts>,
    /// Whether `accepts` came from a combinator (`DERIVED_ACCEPTS`) rather
    /// than from the schema's author.
    accepts_is_derived: bool,
    /// `$isCatchAll`, which JS caches per schema.
    is_catch_all: bool,
    parse: SchemaParse,
}

/// `SerializationSchema<T>`: a parser with its default and the description
/// of its domain. Clones are the same schema.
#[derive(Clone)]
pub struct SerializationSchema(Rc<SchemaInner>);

impl fmt::Debug for SerializationSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SerializationSchema({}, default {})",
            self.0.meta.kind(),
            self.0.default_value
        )
    }
}

impl SerializationSchema {
    /// A schema of your own (JS's hand-rolled schema: a parse function with
    /// `meta`, `defaultValue` and optionally `isEqual` and `accepts`). An
    /// `accepts` given here is declared: it narrows what a union will
    /// choose this schema for.
    pub fn new(
        meta: SerializationSchemaMeta,
        default_value: Value,
        is_equal: Option<SchemaIsEqual>,
        accepts: Option<SchemaAccepts>,
        parse: impl Fn(&Value) -> Value + 'static,
    ) -> Self {
        let declared = accepts.is_some();
        Self::build(
            Rc::new(parse),
            meta,
            Some(default_value),
            is_equal,
            accepts,
            !declared,
        )
    }

    /// `makeSchema`: a schema that states no default (or `undefined`)
    /// derives it from parsing `undefined`.
    fn make(
        parse: impl Fn(&Value) -> Value + 'static,
        meta: SerializationSchemaMeta,
        default_value: Option<Value>,
        is_equal: Option<SchemaIsEqual>,
        accepts: impl Fn(&Value) -> bool + 'static,
    ) -> Self {
        Self::build(
            Rc::new(parse),
            meta,
            default_value,
            is_equal,
            Some(Rc::new(accepts)),
            true,
        )
    }

    fn build(
        parse: SchemaParse,
        meta: SerializationSchemaMeta,
        default_value: Option<Value>,
        is_equal: Option<SchemaIsEqual>,
        accepts: Option<SchemaAccepts>,
        accepts_is_derived: bool,
    ) -> Self {
        let default_value = match default_value {
            None | Some(Value::Undefined) => parse(&Value::Undefined),
            Some(value) => value,
        };
        let declared = accepts.is_some() && !accepts_is_derived;
        let is_catch_all = !declared && compute_is_catch_all(&meta);
        SerializationSchema(Rc::new(SchemaInner {
            meta,
            default_value,
            is_equal,
            accepts,
            accepts_is_derived,
            is_catch_all,
            parse,
        }))
    }

    /// Parses an untrusted value; never fails, substituting the default.
    pub fn parse(&self, value: &Value) -> Value {
        (self.0.parse)(value)
    }

    /// `defaultValue`: what `parse(undefined)` returns, which the compact
    /// form omits.
    pub fn default_value(&self) -> &Value {
        &self.0.default_value
    }

    /// `meta`.
    pub fn meta(&self) -> &SerializationSchemaMeta {
        &self.0.meta
    }

    /// `isEqual`, when the schema declares one.
    pub fn is_equal(&self) -> Option<&SchemaIsEqual> {
        self.0.is_equal.as_ref()
    }

    /// `accepts`, when the schema has one.
    pub fn accepts(&self) -> Option<&SchemaAccepts> {
        self.0.accepts.as_ref()
    }

    /// `declaredAccepts(schema)`: the `accepts` a schema's author wrote,
    /// not one a combinator derived.
    pub fn declared_accepts(&self) -> Option<&SchemaAccepts> {
        if self.0.accepts_is_derived {
            None
        } else {
            self.0.accepts.as_ref()
        }
    }

    /// Whether `self` and `other` are the same schema (`===`).
    pub fn is(&self, other: &SerializationSchema) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    fn is_set(&self) -> bool {
        matches!(self.0.meta, SerializationSchemaMeta::Set { .. })
    }
}

/// `isSchemaEqual(schema, a, b)`: whether two values of `schema`'s domain
/// say the same thing: the same value, unless the schema declares
/// otherwise.
pub fn is_schema_equal(schema: &SerializationSchema, a: &Value, b: &Value) -> bool {
    a == b || schema.0.is_equal.as_ref().is_some_and(|eq| eq(a, b))
}

/// `isSchemaDefault(schema, value)`: whether `value` is the one `schema`
/// restores for an absent property, so writing it says nothing.
pub fn is_schema_default(schema: &SerializationSchema, value: &Value) -> bool {
    is_schema_equal(schema, value, &schema.0.default_value)
}

/// `$acceptsValue(schema, value)`: its `accepts` when it has one, otherwise
/// the parse inference of `$schemaMatch`.
fn accepts_value(schema: &SerializationSchema, value: &Value) -> bool {
    match &schema.0.accepts {
        Some(accepts) => accepts(value),
        None => schema_match(schema, value).is_some(),
    }
}

/// `$schemaMatch(schema, value)`: the parsed value when `value` is in the
/// schema's domain. Without `accepts`, a value is in it when it does not
/// parse to the default, or is the default.
fn schema_match(schema: &SerializationSchema, value: &Value) -> Option<Value> {
    if schema.0.accepts.is_some() {
        return accepts_value(schema, value).then(|| schema.parse(value));
    }
    let parsed = schema.parse(value);
    (!is_schema_default(schema, &parsed) || *value == schema.0.default_value).then_some(parsed)
}

/// `$computeIsCatchAll`: a `rawValue`, or a wrapper or union that bottoms
/// out at one (a schema with a declared `accepts` describes something, and
/// is not one; the caller checks that).
fn compute_is_catch_all(meta: &SerializationSchemaMeta) -> bool {
    use SerializationSchemaMeta as M;
    match meta {
        M::Raw => true,
        M::Union { members } => !members.is_empty() && members.iter().all(|m| m.0.is_catch_all),
        M::Nullable { inner, .. }
        | M::Optional { inner, .. }
        | M::Transform { inner }
        | M::Aliased { inner, .. } => inner.0.is_catch_all,
        _ => false,
    }
}

const FIT_WHOLE: u8 = 1;
const FIT_VIA_CATCH_ALL: u8 = 2;
const FIT_COERCIBLE: u8 = 3;
const FIT_NONE: u8 = 4;

/// `$fitOf(schema, value)`: how well `value` fits `schema`, from whole (1)
/// to none (4).
fn fit_of(schema: &SerializationSchema, value: &Value) -> u8 {
    let declared = schema.declared_accepts();
    let narrowed = declared.is_some();
    if let Some(declared) = declared
        && !declared(value)
    {
        return FIT_NONE;
    }
    let fit = meta_fit_of(schema, value, narrowed);
    if narrowed && fit == FIT_NONE {
        FIT_COERCIBLE
    } else {
        fit
    }
}

/// `$metaFitOf`: the part of `$fitOf` that reads only what the combinator
/// recorded.
fn meta_fit_of(schema: &SerializationSchema, value: &Value, narrowed: bool) -> u8 {
    use SerializationSchemaMeta as M;
    match &schema.0.meta {
        M::Raw => {
            if narrowed {
                FIT_WHOLE
            } else {
                FIT_VIA_CATCH_ALL
            }
        }
        M::Array { item } => {
            let Value::Array(items) = value else {
                return FIT_NONE;
            };
            let mut worst = FIT_WHOLE;
            for element in items.iter().filter(|v| !v.is_undefined()) {
                worst = worst.max(fit_of(item, element));
            }
            worst.min(FIT_COERCIBLE)
        }
        M::Object { fields } => {
            let Some(source) = as_plain_object(value) else {
                return FIT_NONE;
            };
            if !narrowed && has_undeclared_key(source, fields) {
                return FIT_NONE;
            }
            let mut worst = FIT_WHOLE;
            for (key, v) in source.iter() {
                if let Some((_, field)) = fields.iter().find(|(k, _)| k == key) {
                    worst = worst.max(fit_of(field, v));
                }
            }
            worst.min(FIT_COERCIBLE)
        }
        M::Union { members } => best_union_member(members, value).0,
        M::Aliased { inner, aliases } => {
            let owned = value
                .as_str()
                .is_some_and(|s| aliases.iter().any(|(k, _)| k == s));
            if owned {
                FIT_WHOLE
            } else {
                fit_of(inner, value)
            }
        }
        M::Nullable { inner, .. } => {
            if value.is_undefined() || value.is_null() {
                FIT_WHOLE
            } else {
                fit_of(inner, value)
            }
        }
        M::Optional { inner, .. } => {
            if value.is_undefined() {
                FIT_WHOLE
            } else {
                fit_of(inner, value)
            }
        }
        M::Transform { inner } => fit_of(inner, value),
        _ => {
            if narrowed || accepts_value(schema, value) {
                FIT_WHOLE
            } else {
                FIT_NONE
            }
        }
    }
}

/// `$bestUnionMember(members, value)`: the first member that fits best,
/// where a catch-all ranks below any member that fits on its own terms,
/// and how well it fits.
fn best_union_member<'a>(
    members: &'a [SerializationSchema],
    value: &Value,
) -> (u8, Option<&'a SerializationSchema>) {
    let mut member = None;
    let mut best_rank = FIT_NONE;
    let mut best_fit = FIT_NONE;
    for candidate in members {
        let fit = fit_of(candidate, value);
        let rank = if fit < FIT_COERCIBLE && candidate.0.is_catch_all {
            FIT_COERCIBLE
        } else {
            fit
        };
        if rank < best_rank {
            member = Some(candidate);
            best_rank = rank;
            best_fit = fit;
            if rank == FIT_WHOLE {
                break;
            }
        }
    }
    (best_fit, member)
}

/// `undeclared(combinator, inner)`: in JS also the refusal of a wrapped
/// schema that names an accessor, which a [`SerializationSchema`] can't
/// (accessors live on [`SchemaField`]), so only the set check remains.
fn undeclared(combinator: &str, inner: &SerializationSchema) -> Result<(), LexicalError> {
    not_nested_set(combinator, inner)
}

/// `notNestedSet(combinator, inner)`.
fn not_nested_set(combinator: &str, inner: &SerializationSchema) -> Result<(), LexicalError> {
    if inner.is_set() {
        return Err(LexicalError::new(format!(
            "{combinator}: arraySetValue can only be the whole value of a createState, not part of another schema or a node property"
        )));
    }
    Ok(())
}

/// `isPlainObject(value)`: a JSON object.
fn as_plain_object(value: &Value) -> Option<&Object> {
    value.as_object()
}

/// `hasUndeclaredKey(source, fields)`.
fn has_undeclared_key(source: &Object, fields: &[(String, SerializationSchema)]) -> bool {
    source
        .keys()
        .any(|key| !fields.iter().any(|(k, _)| k == key))
}

/// `stringValue(defaultValue)`: `value` when it is a string, otherwise
/// `default_value`.
pub fn string_value(default_value: &str) -> SerializationSchema {
    let default_value = default_value.to_owned();
    SerializationSchema::make(
        move |value| match value {
            Value::String(_) => value.clone(),
            _ => Value::String(default_value.clone()),
        },
        SerializationSchemaMeta::String,
        None,
        None,
        |value| matches!(value, Value::String(_)),
    )
}

/// `NumberValueOptions`: the domain of a [`number_value`].
#[derive(Clone, Copy, Debug, Default)]
pub struct NumberValueOptions {
    /// Reject values below this bound (inclusive); with `integer`, rounded
    /// up to the integer it admits.
    pub min: Option<f64>,
    /// Reject values above this bound (inclusive); with `integer`, rounded
    /// down.
    pub max: Option<f64>,
    /// Reject values that are not integers.
    pub integer: bool,
    /// Bring a finite value outside `min`/`max` to the nearest bound
    /// instead of rejecting it.
    pub clamp: bool,
}

/// `JSON_NUMBER`: the JSON number grammar, anchored
/// (`/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$/`).
pub(crate) fn is_json_number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - start
    };
    if b.get(i) == Some(&b'-') {
        i += 1;
    }
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => {
            digits(&mut i);
        }
        _ => return false,
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        if digits(&mut i) == 0 {
            return false;
        }
    }
    if let Some(b'e' | b'E') = b.get(i) {
        i += 1;
        if let Some(b'+' | b'-') = b.get(i) {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return false;
        }
    }
    i == b.len()
}

/// JS `String(n)`.
fn js_number_string(n: f64) -> String {
    crate::json::number_to_string(n)
}

/// `numberValue(defaultValue, options)`: a finite number in the domain, or
/// a string in the JSON number grammar naming one; otherwise
/// `default_value`. An integer domain rounds its bounds inward; an empty
/// domain is refused.
pub fn number_value(
    default_value: f64,
    options: NumberValueOptions,
) -> Result<SerializationSchema, LexicalError> {
    let NumberValueOptions {
        min,
        max,
        integer,
        clamp,
    } = options;
    let lower = if integer { min.map(f64::ceil) } else { min };
    let upper = if integer { max.map(f64::floor) } else { max };
    if let (Some(lo), Some(hi)) = (lower, upper)
        && lo > hi
    {
        let rounded = if integer && (lower != min || upper != max) {
            format!(
                " (rounded inward from {}..{} because the domain is integers)",
                min.map_or("undefined".to_owned(), js_number_string),
                max.map_or("undefined".to_owned(), js_number_string),
            )
        } else {
            String::new()
        };
        return Err(LexicalError::new(format!(
            "numberValue: the domain is empty; min {} is above max {}{rounded}",
            js_number_string(lo),
            js_number_string(hi),
        )));
    }
    let coerce = |value: &Value| -> Option<f64> {
        match value {
            Value::Number(n) => Some(*n),
            Value::String(s) if is_json_number(s) => Some(s.parse().expect("a JSON number")),
            _ => None,
        }
    };
    let is_numeric = move |n: f64| n.is_finite() && (!integer || n.fract() == 0.0);
    let in_domain = move |n: f64| {
        is_numeric(n) && lower.is_none_or(|lo| n >= lo) && upper.is_none_or(|hi| n <= hi)
    };
    Ok(SerializationSchema::make(
        move |value| {
            let parsed = coerce(value);
            if clamp && let Some(n) = parsed.filter(|n| is_numeric(*n)) {
                return Value::Number(match (lower, upper) {
                    (Some(lo), _) if n < lo => lo,
                    (_, Some(hi)) if n > hi => hi,
                    _ => n,
                });
            }
            Value::Number(parsed.filter(|n| in_domain(*n)).unwrap_or(default_value))
        },
        SerializationSchemaMeta::Number {
            min: lower,
            max: upper,
            integer,
            clamp,
        },
        None,
        None,
        move |value| {
            coerce(value).is_some_and(|n| if clamp { is_numeric(n) } else { in_domain(n) })
        },
    ))
}

/// `booleanValue(defaultValue)`.
pub fn boolean_value(default_value: bool) -> SerializationSchema {
    SerializationSchema::make(
        move |value| match value {
            Value::Bool(_) => value.clone(),
            _ => Value::Bool(default_value),
        },
        SerializationSchemaMeta::Boolean,
        None,
        None,
        |value| matches!(value, Value::Bool(_)),
    )
}

/// Membership in a JS `Set` (SameValueZero): `NaN` is a member of itself.
fn same_value_zero(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) if x.is_nan() && y.is_nan() => true,
        _ => a == b,
    }
}

/// `enumValue(values, defaultValue)`: one of `values`, otherwise the
/// default, which is the first value unless given (`Some`, which may be
/// `Value::Undefined`) and must be one of them.
pub fn enum_value(
    values: Vec<Value>,
    default_value: Option<Value>,
) -> Result<SerializationSchema, LexicalError> {
    if values.is_empty() {
        return Err(LexicalError::new(
            "enumValue: values must not be empty; an enum with no members admits no value",
        ));
    }
    let fallback = default_value.unwrap_or_else(|| values[0].clone());
    let allowed = {
        let values = values.clone();
        move |value: &Value| values.iter().any(|m| same_value_zero(m, value))
    };
    if !allowed(&fallback) {
        return Err(LexicalError::new(
            "enumValue: the default value is not one of the values",
        ));
    }
    let parse_allowed = allowed.clone();
    let parse_fallback = fallback.clone();
    let fallback_undefined = fallback.is_undefined();
    Ok(SerializationSchema::make(
        move |value| {
            if !value.is_undefined() && parse_allowed(value) {
                value.clone()
            } else {
                parse_fallback.clone()
            }
        },
        SerializationSchemaMeta::Enum { values },
        Some(fallback),
        None,
        move |value| allowed(value) && (!value.is_undefined() || fallback_undefined),
    ))
}

/// `liftIsEqual(inner)`: `inner`'s equality on a wrapper that adds a nil.
fn lift_is_equal(inner: &SerializationSchema) -> Option<SchemaIsEqual> {
    let is_equal = inner.0.is_equal.clone()?;
    let nil = |v: &Value| v.is_undefined() || v.is_null();
    Some(Rc::new(move |a, b| {
        if nil(a) || nil(b) {
            a == b
        } else {
            is_equal(a, b)
        }
    }))
}

/// `nullable(inner, {defaultAsNull})`: `null` for `null` and `undefined`,
/// otherwise what `inner` parses (`null` again for `inner`'s default with
/// `default_as_null`).
pub fn nullable(
    inner: SerializationSchema,
    default_as_null: bool,
) -> Result<SerializationSchema, LexicalError> {
    undeclared("nullable", &inner)?;
    let parse_inner = inner.clone();
    let accepts_inner = inner.clone();
    let is_equal = lift_is_equal(&inner);
    Ok(SerializationSchema::make(
        move |value| {
            if value.is_undefined() || value.is_null() {
                return Value::Null;
            }
            let parsed = parse_inner.parse(value);
            if default_as_null && is_schema_default(&parse_inner, &parsed) {
                Value::Null
            } else {
                parsed
            }
        },
        SerializationSchemaMeta::Nullable {
            inner,
            default_as_null,
        },
        None,
        is_equal,
        move |value| {
            value.is_undefined() || value.is_null() || accepts_value(&accepts_inner, value)
        },
    ))
}

/// `optional(inner, {omitDefault})`: `undefined` for `undefined`, otherwise
/// what `inner` parses (`undefined` again for `inner`'s default with
/// `omit_default`).
pub fn optional(
    inner: SerializationSchema,
    omit_default: bool,
) -> Result<SerializationSchema, LexicalError> {
    undeclared("optional", &inner)?;
    let parse_inner = inner.clone();
    let accepts_inner = inner.clone();
    let is_equal = lift_is_equal(&inner);
    Ok(SerializationSchema::make(
        move |value| {
            if value.is_undefined() {
                return Value::Undefined;
            }
            let parsed = parse_inner.parse(value);
            if omit_default && is_schema_default(&parse_inner, &parsed) {
                Value::Undefined
            } else {
                parsed
            }
        },
        SerializationSchemaMeta::Optional {
            inner,
            omit_default,
        },
        None,
        is_equal,
        move |value| value.is_undefined() || accepts_value(&accepts_inner, value),
    ))
}

/// `unionValue(members, defaultValue)`: the value as the member that fits
/// it best parses it, otherwise the default (the first member's unless
/// given, as `Some`, which may be `Value::Undefined`).
pub fn union_value(
    members: Vec<SerializationSchema>,
    default_value: Option<Value>,
) -> Result<SerializationSchema, LexicalError> {
    if members.is_empty() {
        return Err(LexicalError::new(
            "unionValue: at least one member schema is required",
        ));
    }
    for member in &members {
        undeclared("unionValue", member)?;
    }
    let fallback = default_value.unwrap_or_else(|| members[0].0.default_value.clone());
    let parse_members = members.clone();
    let accepts_members = members.clone();
    let parse_fallback = fallback.clone();
    let fallback_undefined = fallback.is_undefined();
    Ok(SerializationSchema::make(
        move |value| {
            if value.is_undefined() {
                return parse_fallback.clone();
            }
            match best_union_member(&parse_members, value).1 {
                Some(member) => member.parse(value),
                None => parse_fallback.clone(),
            }
        },
        SerializationSchemaMeta::Union { members },
        Some(fallback),
        // `$sameContent`, which is content equality on JSON values.
        Some(Rc::new(|a: &Value, b: &Value| a == b)),
        move |value| {
            (!value.is_undefined() || fallback_undefined)
                && accepts_members.iter().any(|m| accepts_value(m, value))
        },
    ))
}

/// `aliasedValue(inner, aliases)`: also reads the string names in
/// `aliases` as the values they stand for.
pub fn aliased_value(
    inner: SerializationSchema,
    aliases: Vec<(String, Value)>,
) -> Result<SerializationSchema, LexicalError> {
    undeclared("aliasedValue", &inner)?;
    let alias = |aliases: &[(String, Value)], value: &Value| -> Option<Value> {
        let s = value.as_str()?;
        aliases.iter().find(|(k, _)| k == s).map(|(_, v)| v.clone())
    };
    let parse_inner = inner.clone();
    let parse_aliases = aliases.clone();
    let accepts_inner = inner.clone();
    let accepts_aliases = aliases.clone();
    let default_value = inner.0.default_value.clone();
    let is_equal = inner.0.is_equal.clone();
    Ok(SerializationSchema::make(
        move |value| alias(&parse_aliases, value).unwrap_or_else(|| parse_inner.parse(value)),
        SerializationSchemaMeta::Aliased { inner, aliases },
        Some(default_value),
        is_equal,
        move |value| {
            alias(&accepts_aliases, value).is_some() || accepts_value(&accepts_inner, value)
        },
    ))
}

/// `transformValue(inner, transform, {isEqual})`: parses with `inner`, then
/// maps the result. `is_equal` is only for a transform to an array or an
/// object.
pub fn transform_value(
    inner: SerializationSchema,
    transform: impl Fn(Value) -> Value + 'static,
    is_equal: Option<SchemaIsEqual>,
) -> Result<SerializationSchema, LexicalError> {
    undeclared("transformValue", &inner)?;
    let transform = Rc::new(transform);
    let default_value = transform(inner.0.default_value.clone());
    if is_equal.is_some() && !matches!(default_value, Value::Array(_) | Value::Object(_)) {
        return Err(LexicalError::new(format!(
            "transformValue: isEqual compares reference-typed values, but this schema parses to {}. Two primitives that are not === are two different serialized values; declaring them equal loses one of them in the compact form.",
            js_typeof(&default_value)
        )));
    }
    let parse_inner = inner.clone();
    let accepts_inner = inner.clone();
    let parse_transform = transform.clone();
    Ok(SerializationSchema::make(
        move |value| parse_transform(parse_inner.parse(value)),
        SerializationSchemaMeta::Transform { inner },
        Some(default_value),
        is_equal,
        move |value| accepts_value(&accepts_inner, value),
    ))
}

/// JS `typeof` of a JSON value.
fn js_typeof(value: &Value) -> &'static str {
    match value {
        Value::Undefined => "undefined",
        Value::Null | Value::Array(_) | Value::Object(_) => "object",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
    }
}

/// `rawValue()`: the JSON as it is.
pub fn raw_value() -> SerializationSchema {
    SerializationSchema::make(
        Value::clone,
        SerializationSchemaMeta::Raw,
        None,
        None,
        |_| true,
    )
}

/// `arrayValue(item)`: each element parsed by `item`; anything but an
/// array is the empty array.
pub fn array_value(item: SerializationSchema) -> Result<SerializationSchema, LexicalError> {
    undeclared("arrayValue", &item)?;
    let parse_item = item.clone();
    let equal_item = item.clone();
    Ok(SerializationSchema::make(
        move |value| match value {
            Value::Array(items) => {
                Value::Array(items.iter().map(|v| parse_item.parse(v)).collect())
            }
            _ => Value::Array(Vec::new()),
        },
        SerializationSchemaMeta::Array { item },
        None,
        Some(Rc::new(move |a: &Value, b: &Value| match (a, b) {
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|(x, y)| is_schema_equal(&equal_item, x, y))
            }
            _ => false,
        })),
        |value| matches!(value, Value::Array(_)),
    ))
}

/// JS's `<` on strings: by UTF-16 code units.
fn js_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `arraySetValue(item)`: a set of strings, written as one array sorted as
/// JS sorts strings (by UTF-16 code units) without duplicates; members
/// `item` does not accept are dropped. Only the whole value of a
/// `createState`.
pub fn array_set_value(item: SerializationSchema) -> Result<SerializationSchema, LexicalError> {
    undeclared("arraySetValue", &item)?;
    if !matches!(item.0.default_value, Value::String(_)) {
        return Err(LexicalError::new(
            "arraySetValue: the item schema must produce strings",
        ));
    }
    let parse_item = item.clone();
    Ok(SerializationSchema::make(
        move |value| {
            let Value::Array(items) = value else {
                return Value::Array(Vec::new());
            };
            // A value already canonical (`$isCanonicalSet`) is kept as it
            // is, which the parse below gives anyway.
            let mut members: Vec<String> = items
                .iter()
                .filter(|m| !m.is_undefined() && accepts_value(&parse_item, m))
                .filter_map(|m| match parse_item.parse(m) {
                    Value::String(s) => Some(s),
                    _ => None,
                })
                .collect();
            members.sort_by(|a, b| js_cmp(a, b));
            members.dedup();
            Value::Array(members.into_iter().map(Value::String).collect())
        },
        SerializationSchemaMeta::Set { item },
        None,
        // `isArraySetEqual`: element by element.
        Some(Rc::new(|a: &Value, b: &Value| a == b)),
        |value| matches!(value, Value::Array(_)),
    ))
}

/// The `Object.prototype` members a serialized object inherits.
const OBJECT_PROTOTYPE_MEMBERS: [&str; 12] = [
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
    "__proto__",
    "constructor",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "valueOf",
];

/// `checkFieldNames(combinator, fields)`.
fn check_field_names<'a>(
    combinator: &str,
    fields: impl IntoIterator<Item = (&'a str, &'a SerializationSchema)>,
) -> Result<(), LexicalError> {
    for (key, schema) in fields {
        if key == "__proto__" {
            return Err(LexicalError::new(format!(
                "{combinator}: \"__proto__\" is not a valid field name"
            )));
        }
        if OBJECT_PROTOTYPE_MEMBERS.contains(&key) {
            return Err(LexicalError::new(format!(
                "{combinator}: \"{key}\" is a member of Object.prototype, which a serialized object inherits, so it cannot be told apart from a property the document never carried"
            )));
        }
        not_nested_set(combinator, schema)?;
    }
    Ok(())
}

/// A JS property read on a JSON value, as `objectValue` reads its source:
/// an object's own key, or an array's index or `length`.
fn js_property(value: &Value, key: &str) -> Value {
    match value {
        Value::Object(object) => object.get(key).cloned().unwrap_or(Value::Undefined),
        Value::Array(items) => {
            if key == "length" {
                return Value::Number(items.len() as f64);
            }
            crate::json::array_index(key)
                .and_then(|i| items.get(i as usize))
                .cloned()
                .unwrap_or(Value::Undefined)
        }
        _ => Value::Undefined,
    }
}

/// `objectValue(fields)`: an object with exactly the declared fields, each
/// parsed by its schema.
pub fn object_value(
    fields: Vec<(&str, SerializationSchema)>,
) -> Result<SerializationSchema, LexicalError> {
    let fields: Vec<(String, SerializationSchema)> =
        fields.into_iter().map(|(k, s)| (k.to_owned(), s)).collect();
    check_field_names("objectValue", fields.iter().map(|(k, s)| (k.as_str(), s)))?;
    let parse_fields = fields.clone();
    let equal_fields = fields.clone();
    let accepts_fields = fields.clone();
    Ok(SerializationSchema::make(
        move |value| {
            let mut result = Object::new();
            for (key, schema) in &parse_fields {
                result.insert(key.as_str(), schema.parse(&js_property(value, key)));
            }
            Value::Object(result)
        },
        SerializationSchemaMeta::Object { fields },
        None,
        Some(Rc::new(move |a: &Value, b: &Value| {
            let (Some(a), Some(b)) = (as_plain_object(a), as_plain_object(b)) else {
                return false;
            };
            !has_undeclared_key(a, &equal_fields)
                && !has_undeclared_key(b, &equal_fields)
                && equal_fields.iter().all(|(key, schema)| {
                    is_schema_equal(
                        schema,
                        a.get(key).unwrap_or(&Value::Undefined),
                        b.get(key).unwrap_or(&Value::Undefined),
                    )
                })
        })),
        move |value| {
            as_plain_object(value).is_some_and(|o| !has_undeclared_key(o, &accepts_fields))
        },
    ))
}

/// A Rust type a [`with_field`] property can be stored as, converted to
/// and from the schema's JSON domain.
pub trait SchemaValue: Sized {
    /// The value as the schema's JSON; `Value::Undefined` when it has none.
    fn to_schema_value(&self) -> Value;
    /// The value a parsed JSON value stands for, or `None` when it is not
    /// one.
    fn from_schema_value(value: &Value) -> Option<Self>;
}

impl SchemaValue for String {
    fn to_schema_value(&self) -> Value {
        Value::String(self.clone())
    }
    fn from_schema_value(value: &Value) -> Option<Self> {
        value.as_str().map(str::to_owned)
    }
}

impl SchemaValue for f64 {
    fn to_schema_value(&self) -> Value {
        Value::Number(*self)
    }
    fn from_schema_value(value: &Value) -> Option<Self> {
        value.as_f64()
    }
}

impl SchemaValue for bool {
    fn to_schema_value(&self) -> Value {
        Value::Bool(*self)
    }
    fn from_schema_value(value: &Value) -> Option<Self> {
        value.as_bool()
    }
}

impl SchemaValue for Value {
    fn to_schema_value(&self) -> Value {
        self.clone()
    }
    fn from_schema_value(value: &Value) -> Option<Self> {
        Some(value.clone())
    }
}

/// `None` has no value: `undefined`, or `null` for a property that isn't
/// [`optional`].
impl<T: SchemaValue> SchemaValue for Option<T> {
    fn to_schema_value(&self) -> Value {
        self.as_ref().map_or(Value::Undefined, T::to_schema_value)
    }
    fn from_schema_value(value: &Value) -> Option<Self> {
        if value.is_undefined() || value.is_null() {
            Some(None)
        } else {
            T::from_schema_value(value).map(Some)
        }
    }
}

impl<T: SchemaValue> SchemaValue for Vec<T> {
    fn to_schema_value(&self) -> Value {
        Value::Array(self.iter().map(T::to_schema_value).collect())
    }
    fn from_schema_value(value: &Value) -> Option<Self> {
        value.as_array()?.iter().map(T::from_schema_value).collect()
    }
}

/// How the export walk reads a property: from the node, in the schema's
/// domain.
pub type SchemaGetter = Rc<dyn Fn(&dyn LexicalNode) -> Value>;
/// How the import walk applies a parsed value to a writable node.
pub type SchemaSetter = Rc<dyn Fn(&mut dyn LexicalNode, Value)>;
/// A `when` predicate: whether the property is written at all.
pub type SchemaPredicate = Rc<dyn Fn(&dyn LexicalNode) -> bool>;

/// One property of a node's serialization schema: its schema with the
/// accessors that read it for export and apply it on import, as
/// [`with_field`] and [`with_accessors`] make it. JS also lets a plain
/// schema stand for the conventional `get<Prop>`/`set<Prop>` methods; a
/// Rust node has no methods to find by name, so every property names its
/// accessors.
#[derive(Clone)]
pub struct SchemaField {
    schema: SerializationSchema,
    /// `None` declares the property import-only (`getter: null`).
    getter: Option<SchemaGetter>,
    /// `when`: the property is written only where it holds (and is not the
    /// default).
    when: Option<SchemaPredicate>,
    /// `None` declares the property derived on import (`setter: null`),
    /// which the compact form leaves out.
    setter: Option<SchemaSetter>,
    /// `derived: true` (`@experimental`): the node's own code recomputes
    /// the property from the document, so tools that copy or compare a
    /// node's data can leave it out, and `mergeAdjacent` may not name it.
    derived: bool,
}

impl fmt::Debug for SchemaField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SchemaField")
            .field("schema", &self.schema)
            .field("getter", &self.getter.is_some())
            .field("when", &self.when.is_some())
            .field("setter", &self.setter.is_some())
            .field("derived", &self.derived)
            .finish()
    }
}

impl SchemaField {
    /// The property's schema.
    pub fn schema(&self) -> &SerializationSchema {
        &self.schema
    }

    /// `derived`.
    pub fn is_derived(&self) -> bool {
        self.derived
    }

    /// Whether the property is exported (it has a getter).
    pub fn is_exported(&self) -> bool {
        self.getter.is_some()
    }

    /// Whether the property is applied on import (it has a setter).
    pub fn is_imported(&self) -> bool {
        self.setter.is_some()
    }

    /// `{when}`: writes the property only where `when` holds.
    pub fn when(mut self, when: impl Fn(&dyn LexicalNode) -> bool + 'static) -> Self {
        self.when = Some(Rc::new(when));
        self
    }

    /// `{derived: true}`.
    pub fn derived(mut self) -> Self {
        self.derived = true;
        self
    }

    /// Whether the compact form omits `value`: `undefined`, or the default
    /// (`$isCompactDefaultFor`).
    fn is_compact_default(&self, value: &Value) -> bool {
        value.is_undefined() || is_schema_default(&self.schema, value)
    }
}

/// The node of type `N` a schema accessor was given.
fn node_as<N: LexicalNode>(node: &dyn LexicalNode) -> &N {
    node.downcast_ref::<N>()
        .expect("a schema field of the node's own class")
}

fn node_as_mut<N: LexicalNode>(node: &mut dyn LexicalNode) -> &mut N {
    node.downcast_mut::<N>()
        .expect("a schema field of the node's own class")
}

/// A getter that reads a node of type `N`.
pub fn schema_getter<N: LexicalNode>(get: impl Fn(&N) -> Value + 'static) -> SchemaGetter {
    Rc::new(move |node| get(node_as::<N>(node)))
}

/// A setter that writes a node of type `N`.
pub fn schema_setter<N: LexicalNode>(set: impl Fn(&mut N, Value) + 'static) -> SchemaSetter {
    Rc::new(move |node, value| set(node_as_mut::<N>(node), value))
}

/// `withField(schema, {field})`: the property *is* a field of the node `N`
/// (`field` reads it, `field_mut` writes it), stored as its
/// [`SchemaValue`]. A parsed value the field's type can't hold leaves the
/// field as it is. A Rust node's `Clone` carries every field, so the clone
/// JS synthesizes for a declared field has nothing left to do. Add
/// [`when`](SchemaField::when) and [`derived`](SchemaField::derived) as JS
/// does in the options.
pub fn with_field<N: LexicalNode, V: SchemaValue + 'static>(
    schema: SerializationSchema,
    field: fn(&N) -> &V,
    field_mut: fn(&mut N) -> &mut V,
) -> Result<SchemaField, LexicalError> {
    not_nested_set("withField", &schema)?;
    // An optional property is absent when it has no value; any other one
    // writes `null`.
    let optional = matches!(schema.meta(), SerializationSchemaMeta::Optional { .. });
    Ok(SchemaField {
        schema,
        getter: Some(schema_getter::<N>(move |node| {
            match field(node).to_schema_value() {
                Value::Undefined if !optional => Value::Null,
                value => value,
            }
        })),
        when: None,
        setter: Some(schema_setter::<N>(move |node, value| {
            if let Some(value) = V::from_schema_value(&value) {
                *field_mut(node) = value;
            }
        })),
        derived: false,
    })
}

/// `withAccessors(schema, {getter, setter, derived})`: the property is read
/// with `getter` and applied with `setter`; `None` declares that direction
/// deliberately unsupported (`null`): an import-only property, or one
/// derived on import, which the compact form leaves out.
pub fn with_accessors(
    schema: SerializationSchema,
    getter: Option<SchemaGetter>,
    setter: Option<SchemaSetter>,
) -> Result<SchemaField, LexicalError> {
    not_nested_set("withAccessors", &schema)?;
    Ok(SchemaField {
        schema,
        getter,
        when: None,
        setter,
        derived: false,
    })
}

/// `NodeSerializationSchema`: the node-specific properties of a class's
/// JSON, as `nodeSchema<N>()({...})` declares them, in key order. Clones
/// are the same schema.
#[derive(Clone, Debug)]
pub struct NodeSchema(Rc<[(&'static str, SchemaField)]>);

impl NodeSchema {
    /// `meta.fields`.
    pub fn fields(&self) -> &[(&'static str, SchemaField)] {
        &self.0
    }

    /// This schema as a subclass resolves it when it overrides the setter
    /// that the property `key`'s field write stands in for
    /// (`SchemaFieldBase.method`, `resolveSchemaField`): the same
    /// properties in the same order, `key` applied with `setter`. A Rust
    /// class can't be asked which methods it overrides, so it gives the
    /// schemas of the classes it extends as it resolves them, in its
    /// [`LexicalNode::json_schema_chain`].
    pub fn with_setter(&self, key: &str, setter: SchemaSetter) -> NodeSchema {
        NodeSchema(
            self.0
                .iter()
                .map(|(k, field)| {
                    let mut field = field.clone();
                    if *k == key {
                        field.setter = Some(setter.clone());
                    }
                    (*k, field)
                })
                .collect(),
        )
    }
}

/// `nodeSchema<N>()(fields)`.
pub fn node_schema(fields: Vec<(&'static str, SchemaField)>) -> Result<NodeSchema, LexicalError> {
    check_field_names("nodeSchema", fields.iter().map(|(k, f)| (*k, &f.schema)))?;
    Ok(NodeSchema(fields.into()))
}

/// `ComposedSchema`: the serialization schema of a node class composed
/// across its config chain (`composeSchema`), compiled once per class.
struct ComposedSchema {
    /// Most derived first, the first declaration of a key winning: the
    /// order properties are written in.
    fields_derived_first: Vec<(&'static str, SchemaField)>,
    /// The same fields ancestors first: the order setters are applied in.
    fields_base_first: Vec<(&'static str, SchemaField)>,
}

thread_local! {
    /// `NODE_CLASS_CACHE`'s `composed`, by the node's Rust type.
    static COMPOSED: RefCell<HashMap<TypeId, Rc<ComposedSchema>>> = RefCell::new(HashMap::new());
}

/// `getComposedSchema(klass)`, of the class of `node`: the schemas of
/// [`LexicalNode::json_schema_chain`], composed.
fn composed_schema(node: &dyn LexicalNode) -> Rc<ComposedSchema> {
    let id = node.as_any().type_id();
    if let Some(composed) = COMPOSED.with(|c| c.borrow().get(&id).cloned()) {
        return composed;
    }
    let chain = node.json_schema_chain();
    let mut derived_first: Vec<(&'static str, SchemaField)> = Vec::new();
    for schema in &chain {
        for (key, field) in schema.fields() {
            if !derived_first.iter().any(|(k, _)| k == key) {
                derived_first.push((key, field.clone()));
            }
        }
    }
    let mut base_first: Vec<(&'static str, SchemaField)> = Vec::new();
    for schema in chain.iter().rev() {
        for (key, _) in schema.fields() {
            if !base_first.iter().any(|(k, _)| k == key) {
                let winner = derived_first
                    .iter()
                    .find(|(k, _)| k == key)
                    .expect("a composed key");
                base_first.push(winner.clone());
            }
        }
    }
    let composed = Rc::new(ComposedSchema {
        fields_derived_first: derived_first,
        fields_base_first: base_first,
    });
    COMPOSED.with(|c| c.borrow_mut().insert(id, composed.clone()));
    composed
}

/// `getComposedSchemaFields(klass)`, of the class of `node`: every
/// node-specific property of its JSON, its flat NodeState whose value is a
/// schema first, then the composed schema's fields most derived first.
pub fn get_composed_schema_fields(node: &dyn LexicalNode) -> Vec<(String, SchemaField)> {
    let mut fields: Vec<(String, SchemaField)> = Vec::new();
    for required in node.state_configs() {
        if required.flat
            && let Some(schema) = required.state_config.schema()
            && !fields.iter().any(|(k, _)| k == required.state_config.key())
        {
            fields.push((
                required.state_config.key().to_owned(),
                SchemaField {
                    schema,
                    getter: None,
                    when: None,
                    setter: None,
                    derived: false,
                },
            ));
        }
    }
    for (key, field) in &composed_schema(node).fields_derived_first {
        fields.retain(|(k, _)| k != key);
        fields.push(((*key).to_owned(), field.clone()));
    }
    fields
}

/// `$walkExportJSON(node, compact)`: the JSON the node's schema describes,
/// without its NodeState: `type` first in the compact form, an empty
/// `children` for an element, each exported property most derived first
/// (the compact form leaving out a default and a property derived on
/// import, and a `when` gating its property), then `type` and `version` in
/// the legacy form. What [`LexicalNode::export_json`] does for a class
/// without code of its own (JS's generated code).
pub fn walk_export_json(node: &dyn LexicalNode, compact: bool) -> Object {
    let composed = composed_schema(node);
    let mut json = Object::new();
    if compact {
        json.insert("type", node.get_type().into());
    }
    if node.element().is_some() {
        json.insert("children", Value::Array(Vec::new()));
    }
    for (key, field) in &composed.fields_derived_first {
        let Some(getter) = &field.getter else {
            continue;
        };
        if compact && field.setter.is_none() {
            continue;
        }
        let mut value = getter(node);
        if let Some(when) = &field.when
            && (field.is_compact_default(&value) || !when(node))
        {
            value = Value::Undefined;
        }
        if compact && field.is_compact_default(&value) {
            continue;
        }
        json.insert(*key, value);
    }
    if !compact {
        json.insert("type", node.get_type().into());
        json.insert("version", 1.0.into());
    }
    json
}

/// `$walkJSONSetters`'s setters (`$walkSetters`): each property the node's
/// schema applies, ancestors first, parsed from `json` (a missing one as
/// its default). What [`LexicalNode::update_from_json`] does for a class
/// without code of its own; the flat NodeState is applied by the caller
/// first, as `$applyJSONSetters` does.
pub fn walk_update_from_json(node: &mut dyn LexicalNode, json: &Object) {
    let composed = composed_schema(node);
    for (key, field) in &composed.fields_base_first {
        if let Some(setter) = &field.setter {
            let parsed = field
                .schema
                .parse(json.get(key).unwrap_or(&Value::Undefined));
            setter(node, parsed);
        }
    }
}

/// `$exportNodeJSONOnce(node, compact)`: the class's generated code where
/// it has some ([`LexicalNode::generated_export_json`]), otherwise the
/// walk of its schema.
pub fn export_node_json_once(node: &dyn LexicalNode, compact: bool) -> Object {
    node.generated_export_json(compact)
        .unwrap_or_else(|| walk_export_json(node, compact))
}

/// `$applyJSONSetters(node, serializedNode)` without the flat NodeState,
/// which the caller applies first: the class's generated code where it has
/// some ([`LexicalNode::generated_update_from_json`]), otherwise the walk
/// of its schema.
pub fn apply_json_setters(node: &mut dyn LexicalNode, json: &Object) {
    if !node.generated_update_from_json(json) {
        walk_update_from_json(node, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    /// The values each property of a node is given, one at a time: a
    /// corpus of every type and of the names, numbers and strings the
    /// schemas treat specially, as JS checks its generated parsers against
    /// the walk over the schema's domain and a fixed set of hostile values.
    fn hostile_values() -> Vec<Value> {
        let mut values = vec![Value::Undefined];
        for text in [
            "null",
            "true",
            "false",
            "0",
            "-0",
            "-1",
            "1",
            "1.5",
            "2",
            "3",
            "128",
            "129",
            "1e300",
            "\"\"",
            "\"0\"",
            "\"1\"",
            "\"1.5\"",
            "\"-1\"",
            "\"1e999\"",
            "\"0x10\"",
            "\" 1\"",
            "\"__proto__\"",
            "\"toString\"",
            "\"bold\"",
            "\"code\"",
            "\"unmergeable\"",
            "\"directionless\"",
            "\"token\"",
            "\"segmented\"",
            "\"normal\"",
            "\"ltr\"",
            "\"rtl\"",
            "\"center\"",
            "\"justify\"",
            "\"h1\"",
            "\"h6\"",
            "\"h7\"",
            "\"number\"",
            "\"bullet\"",
            "\"check\"",
            "\"ol\"",
            "\"ul\"",
            "\"x\"",
            "\"https://lexical.dev\"",
            "[]",
            "[\"a\",1,null,\"b\"]",
            "[\"b\",\"a\",\"b\"]",
            "{}",
            "{\"a\":1}",
        ] {
            values.push(parse(text).expect("JSON"));
        }
        values
    }

    /// Whether `node`'s generated `updateFromJSON` and the walk of its
    /// schema leave it the same for each hostile value of each key, and
    /// then whether its generated `exportJSON` is the walk's in both forms
    /// (`exports`, for a node whose export reads no editor state).
    fn check_agrees(node: &dyn LexicalNode, keys: &[&str], exports: bool) {
        for key in keys {
            for value in hostile_values() {
                let mut json = Object::new();
                json.insert(*key, value.clone());
                let mut generated = node.clone_node();
                assert!(generated.generated_update_from_json(&json));
                let mut walked = node.clone_node();
                walk_update_from_json(walked.as_node_mut(), &json);
                assert_eq!(
                    format!("{generated:?}"),
                    format!("{walked:?}"),
                    "{} {key}: {value}",
                    node.get_type()
                );
                if exports {
                    for compact in [false, true] {
                        assert_eq!(
                            Value::Object(
                                generated
                                    .generated_export_json(compact)
                                    .expect("generated code")
                            )
                            .to_string(),
                            Value::Object(walk_export_json(&*generated, compact)).to_string(),
                            "{} {key}: {value} (compact: {compact})",
                            node.get_type()
                        );
                    }
                }
            }
        }
    }

    const ELEMENT_KEYS: [&str; 5] = ["direction", "format", "indent", "textFormat", "textStyle"];
    const TEXT_KEYS: [&str; 5] = ["detail", "format", "mode", "style", "text"];

    fn keys<'a>(own: &[&'a str], base: &[&'a str]) -> Vec<&'a str> {
        own.iter().chain(base).copied().collect()
    }

    // upstream: lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated exportJSON > every generated parser agrees with the schema-driven walk
    #[test]
    fn generated_json_is_the_walk_of_the_schema() {
        check_agrees(
            &crate::nodes::paragraph::ParagraphNode::default(),
            &ELEMENT_KEYS,
            false,
        );
        check_agrees(&crate::nodes::text::TextNode::default(), &TEXT_KEYS, true);
        check_agrees(&crate::nodes::tab::TabNode::default(), &TEXT_KEYS, true);
        check_agrees(
            &crate::nodes::line_break::LineBreakNode::default(),
            &[],
            true,
        );
        #[cfg(feature = "rich-text")]
        {
            check_agrees(
                &crate::rich_text::HeadingNode::default(),
                &keys(&["tag"], &ELEMENT_KEYS),
                false,
            );
            check_agrees(
                &crate::rich_text::QuoteNode::default(),
                &ELEMENT_KEYS,
                false,
            );
        }
        #[cfg(feature = "list")]
        {
            check_agrees(
                &crate::list::ListNode::default(),
                &keys(&["listType", "start", "tag"], &ELEMENT_KEYS),
                false,
            );
            check_agrees(
                &crate::list::ListItemNode::default(),
                &keys(&["checked", "indent", "value"], &ELEMENT_KEYS),
                false,
            );
        }
        #[cfg(feature = "link")]
        {
            let link = ["rel", "target", "title", "url"];
            check_agrees(
                &crate::link::LinkNode::default(),
                &keys(&link, &ELEMENT_KEYS),
                false,
            );
            check_agrees(
                &crate::link::AutoLinkNode::default(),
                &keys(&keys(&["isUnlinked"], &link), &ELEMENT_KEYS),
                false,
            );
        }
        #[cfg(feature = "code")]
        {
            check_agrees(
                &crate::code::CodeNode::default(),
                &keys(&["language", "theme"], &ELEMENT_KEYS),
                false,
            );
            check_agrees(
                &crate::code::CodeHighlightNode::default(),
                &keys(&["highlightType"], &TEXT_KEYS),
                true,
            );
        }
        #[cfg(feature = "mark")]
        check_agrees(
            &crate::mark::MarkNode::default(),
            &keys(&["ids"], &ELEMENT_KEYS),
            false,
        );
        #[cfg(feature = "overflow")]
        check_agrees(
            &crate::overflow::OverflowNode::default(),
            &ELEMENT_KEYS,
            false,
        );
        #[cfg(feature = "hashtag")]
        check_agrees(&crate::hashtag::HashtagNode::default(), &TEXT_KEYS, true);
    }
}
