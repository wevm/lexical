//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts`:
//! the block id format, frozen.
//!
//! Upstream checks the derivation against Node's crypto, an independent
//! implementation of its specification. The crate has no such dependency,
//! so those values are literals computed with Python's `hashlib` from the
//! same specification (and upstream's own pinned values, which agree).

#![cfg(feature = "rich-text")]

use crate::extension::positional_block_id::{
    canonical_document, canonical_json, derive_duplicate_id, derive_positional_id, document_key,
    sha256,
};
use crate::extension::{
    LexicalEditorWithDispose, block_id_extension, block_id_state, build_editor_from_extensions,
};
use crate::json::{Object, Value};
use crate::{
    AnyLexicalExtensionArgument, InitialEditorStateType, LexicalExtension, define_extension,
};
use std::collections::HashSet;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `/^[A-Za-z0-9_-]{21}$/`.
fn is_id(id: &str) -> bool {
    id.len() == 21
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A document stored without ids, as compact JSON.
const STORED: &str = r#"{"root":{"children":[{"children":[{"text":"Hello, wörld 👋","type":"text"}],"type":"paragraph"},{"children":[{"text":"Second","type":"text"}],"tag":"h2","type":"heading"},{"children":[],"type":"paragraph"}],"type":"root"}}"#;

fn editor_with(
    name: &str,
    mut dependencies: Vec<AnyLexicalExtensionArgument>,
) -> LexicalEditorWithDispose {
    dependencies.push(block_id_extension().into());
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn load(editor: &LexicalEditorWithDispose, json: &str) {
    editor
        .set_editor_state(&editor.parse_editor_state(json).unwrap(), None)
        .unwrap();
}

/// `loaded()`.
fn loaded() -> LexicalEditorWithDispose {
    let editor = editor_with(
        "[format]",
        vec![crate::rich_text::rich_text_extension().into()],
    );
    load(&editor, STORED);
    editor
}

/// The `blockId` of each child of the root, in the legacy export.
fn root_child_ids(editor: &LexicalEditorWithDispose) -> Vec<String> {
    let json = editor.get_editor_state().to_json(false);
    json.get("root")
        .and_then(|r| r.get("children"))
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|child| {
            child
                .get("$")
                .and_then(|s| s.get("blockId"))
                .and_then(Value::as_str)
                .unwrap()
                .to_owned()
        })
        .collect()
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > sha256 matches the standard on every padding boundary and beyond
#[test]
fn sha256_matches_the_standard() {
    let mut seed: u64 = 7;
    let mut next = || {
        seed = (seed * 16807) % 2147483647;
        (seed & 0xff) as u8
    };
    for (length, digest) in [
        (
            0,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            1,
            "7da59d0dfbe21f43e842e8afb43e12a6445bbac07c2fc26984c71d0de3f99c9c",
        ),
        (
            3,
            "ad3f4a8472e5343338f443d76cd1ce802541e80aacd6367ba7fa5c1904bf56ae",
        ),
        (
            55,
            "937365c24868838e86c7ef3617fee9ede9fd9660c15813bd9c634c5cbd2f5d57",
        ),
        (
            56,
            "23bf3d748309a100dfded953f30bbd1828d41ebf69ac13fc32b6b0382d70ac24",
        ),
        (
            57,
            "3d0f0f4881f99fc1bcbe4885965558b971b51644994f4e986a5278fb55c6448d",
        ),
        (
            63,
            "a74c568993e4152d2292bf0893adc6bce3f44e625f846c5d57466e96c859b016",
        ),
        (
            64,
            "9f65f39d0ebb8a31bf11cca25d3e3908ec400dce4ac5a8be1ab143638e570ae7",
        ),
        (
            65,
            "2e9d96235f30cc008d6909575b1c258b234c1b37cd7828313f6d1a34a8167c86",
        ),
        (
            119,
            "d7b0bc3c7434b203a51b87d734103162546be61f4a681dffaec95e320b20c909",
        ),
        (
            120,
            "4cabcbe839cb42c11343f621b58755f9949a485e8253a03087c437de3f858d05",
        ),
        (
            128,
            "967bee342ef500f4ee0b83b4ecaaf32ce55c73d44c03c85ffe556325c32611bf",
        ),
        (
            1000,
            "d13ba0485e4f5f3bce9492c2263737aa00950a6d8cabd3b1e361e092c3d06c46",
        ),
        (
            100_000,
            "6263d936874538d6ef59ba9ab7ee1f28cd5ab1f23b89af04d0d0de1ed2d60249",
        ),
    ] {
        let bytes: Vec<u8> = (0..length).map(|_| next()).collect();
        assert_eq!(hex(&sha256(&bytes)), digest, "length {length}");
    }
    assert_eq!(
        hex(&sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    let mut object = Object::new();
    for (key, value) in entries {
        object.insert(key, value);
    }
    Value::Object(object)
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > canonical JSON sorts keys at every level and leaves out undefined
#[test]
fn canonical_json_sorts_keys_and_leaves_out_undefined() {
    // Built from entries, so that the keys are unsorted.
    let unsorted = object(vec![
        (
            "b",
            Value::Array(vec![object(vec![
                ("z", Value::Number(1.0)),
                ("a", Value::Undefined),
                ("m", Value::from("x")),
            ])]),
        ),
        ("a", Value::Null),
        ("c", Value::from("é\"")),
    ]);
    assert_eq!(
        canonical_json(&unsorted),
        r#"{"a":null,"b":[{"m":"x","z":1}],"c":"é\""}"#
    );
    // Keys that are array indices come first, in numeric order, then the
    // others by UTF-16 code units.
    let indices = object(
        ["b", "10", "a", "9", "B", ""]
            .iter()
            .map(|k| (*k, Value::Number(1.0)))
            .collect(),
    );
    assert_eq!(
        canonical_json(&indices),
        r#"{"9":1,"10":1,"":1,"B":1,"a":1,"b":1}"#
    );
}

/// The value without any `"$"` that holds nothing but a `blockId`
/// (upstream's `.replace(/"\$":\{"blockId":"…"\},/g, '')`).
fn without_ids(value: &Value) -> Value {
    match value {
        Value::Object(o) => {
            let mut out = Object::new();
            for (key, v) in o.iter() {
                if key == "$"
                    && v.as_object()
                        .is_some_and(|s| s.len() == 1 && s.contains_key("blockId"))
                {
                    continue;
                }
                out.insert(key, without_ids(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(without_ids).collect()),
        other => other.clone(),
    }
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > the document hashed is the compact export without ids
#[test]
fn the_document_hashed_is_the_compact_export_without_ids() {
    let editor = loaded();
    let canonical = editor.read(canonical_document).unwrap();
    // The load named every block, but ids are not part of what is hashed.
    assert!(!canonical.contains("blockId"));
    let compact = editor.get_editor_state().to_json(true);
    assert_eq!(
        canonical,
        canonical_json(&without_ids(compact.get("root").unwrap()))
    );
    // The key is SHA-256 of the canonical document, pinned upstream.
    let key = editor.read(document_key).unwrap();
    assert_eq!(key, sha256(canonical.as_bytes()));
    assert_eq!(
        hex(&key),
        "a5b08ad0c2466a1e221f46f78d5aa99a46566c9196b447369d279e01725d6cd3"
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > positional ids follow the specification and are pinned
#[test]
fn positional_ids_follow_the_specification() {
    let editor = loaded();
    let key = editor.read(document_key).unwrap();
    for (place, spec) in [
        ("0", "Obj4Jnk2Uq2o1Jcgsuvda"),
        ("1", "m81kLbLYKGetZFUBn8iGg"),
        ("2", "tTZ_qAifsgwmKg2wFCDwn"),
        ("0.0", "01M9btZTmuIzFg2o2pgz9"),
        ("12.3.4", "Axzfcx8mMHfz1TXtDMogg"),
        ("0#1", "mFBw6txrzaNCXLhCe_CLM"),
        ("1#2", "c5PdYRr4fc3Byt2qkDanR"),
    ] {
        assert_eq!(derive_positional_id(&key, place), spec, "{place}");
    }
    let ids = root_child_ids(&editor);
    assert_eq!(
        ids,
        ["0", "1", "2"].map(|place| derive_positional_id(&key, place))
    );
    // Frozen: these are the ids every reader gives this stored document.
    assert_eq!(
        hex(&key),
        "a5b08ad0c2466a1e221f46f78d5aa99a46566c9196b447369d279e01725d6cd3"
    );
    assert_eq!(
        ids,
        [
            "Obj4Jnk2Uq2o1Jcgsuvda",
            "m81kLbLYKGetZFUBn8iGg",
            "tTZ_qAifsgwmKg2wFCDwn"
        ]
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > ids of later holders of a repeated id follow the specification and are pinned
#[test]
fn duplicate_ids_follow_the_specification() {
    let long = "y".repeat(64);
    for (id, ordinal, spec) in [
        ("x", "1", "-rK-MX7NTIVuS6OIwyd26"),
        ("x", "2", "I8N4JHaH_LC1SSZw_HJ8r"),
        ("V1StGXR8_Z5jdHi6B-myT", "1", "atPwgUJO7TzlWVO7K9Scw"),
        ("wörld 👋", "3", "MTjPC2mYL6Yd8hRaD8fNq"),
        (long.as_str(), "1#1", "9Ei_ZS5wIhrsxyA-qNJdY"),
    ] {
        let derived = derive_duplicate_id(id, ordinal);
        assert_eq!(derived, spec, "{id} {ordinal}");
        assert!(is_id(&derived));
    }
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > a loaded document with repeated ids: the first holder keeps the id, the later ones are renamed by ordinal
#[test]
fn a_loaded_document_with_repeated_ids_renames_later_holders() {
    let editor = editor_with(
        "[format-dup]",
        vec![crate::rich_text::rich_text_extension().into()],
    );
    let paragraph = |text: &str, id: &str| {
        format!(
            r#"{{"$":{{"blockId":"{id}"}},"children":[{{"text":"{text}","type":"text"}}],"type":"paragraph"}}"#
        )
    };
    load(
        &editor,
        &format!(
            r#"{{"root":{{"children":[{},{},{},{}],"type":"root"}}}}"#,
            paragraph("a", "x"),
            paragraph("b", "x"),
            paragraph("c", "y"),
            paragraph("d", "x")
        ),
    );
    assert_eq!(
        root_child_ids(&editor),
        ["x", "-rK-MX7NTIVuS6OIwyd26", "y", "I8N4JHaH_LC1SSZw_HJ8r"]
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > the documented end-to-end vector
#[cfg(feature = "list")]
#[test]
fn the_documented_end_to_end_vector() {
    let stored = concat!(
        r#"{"root":{"children":["#,
        r#"{"$":{"10":1.5,"9":"x","note":"tab\tquote\""},"#,
        r#""children":[{"text":"Hello, wörld 👋","type":"text"}],"type":"paragraph"},"#,
        r#"{"children":["#,
        r#"{"children":[{"text":"one","type":"text"}],"type":"listitem","value":1},"#,
        r#"{"children":[{"text":"two","type":"text"}],"type":"listitem","value":2}],"#,
        r#""listType":"bullet","start":1,"tag":"ul","type":"list"},"#,
        r#"{"$":{"blockId":"kept-id"},"children":[],"type":"paragraph"}"#,
        r#"],"type":"root"}}"#
    );
    let canonical = concat!(
        r#"{"children":["#,
        r#"{"$":{"9":"x","10":1.5,"note":"tab\tquote\""},"#,
        r#""children":[{"text":"Hello, wörld 👋","type":"text"}],"type":"paragraph"},"#,
        r#"{"children":["#,
        r#"{"children":[{"text":"one","type":"text"}],"type":"listitem"},"#,
        r#"{"children":[{"text":"two","type":"text"}],"type":"listitem","value":2}],"#,
        r#""listType":"bullet","type":"list"},"#,
        r#"{"children":[],"type":"paragraph"}"#,
        r#"],"type":"root"}"#
    );
    let key = "9fa59b03bb40d3d4ac45b31a472f965a0825ffc10693d4165b1e4dcf956b5dfa";
    let expected = [
        (Some("0"), "paragraph", "qfDxX6r4J9KOyXNjM0x2X"),
        (Some("1"), "list", "nWeVEnzogQyDvzB4_4gMw"),
        (Some("1.0"), "listitem", "L9_QRFvuohaGhhqAlzLEB"),
        (Some("1.1"), "listitem", "BCreOq5xUSiXSxTd8xMLr"),
        (None, "paragraph", "kept-id"),
    ];
    let editor = editor_with(
        "[format-vector]",
        vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
        ],
    );
    load(&editor, stored);
    assert_eq!(editor.read(canonical_document).unwrap(), canonical);
    assert_eq!(hex(&sha256(canonical.as_bytes())), key);
    let document_key = editor.read(document_key).unwrap();
    assert_eq!(hex(&document_key), key);
    let mut actual: Vec<(String, String)> = Vec::new();
    fn visit(node: &Value, actual: &mut Vec<(String, String)>) {
        if let Some(id) = node
            .get("$")
            .and_then(|s| s.get("blockId"))
            .and_then(Value::as_str)
        {
            let node_type = node.get("type").and_then(Value::as_str).unwrap();
            actual.push((node_type.to_owned(), id.to_owned()));
        }
        for child in node
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            visit(child, actual);
        }
    }
    let json = editor.get_editor_state().to_json(false);
    for child in json
        .get("root")
        .and_then(|r| r.get("children"))
        .and_then(Value::as_array)
        .unwrap()
    {
        visit(child, &mut actual);
    }
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(_, t, id)| ((*t).to_owned(), (*id).to_owned()))
            .collect::<Vec<_>>()
    );
    for (place, _, id) in expected {
        if let Some(place) = place {
            assert_eq!(derive_positional_id(&document_key, place), id);
        }
    }
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > positional ids are well formed, use every symbol, and do not repeat
#[test]
fn positional_ids_are_well_formed_and_distinct() {
    let document_key = sha256(b"{}");
    let mut seen = HashSet::new();
    let mut symbols = HashSet::new();
    for i in 0..20_000 {
        let id = derive_positional_id(&document_key, &format!("{}.{i}", i % 200));
        assert!(is_id(&id), "{id}");
        assert_eq!(
            block_id_state().parse(&Value::from(id.as_str())),
            Some(id.clone())
        );
        symbols.extend(id.chars());
        seen.insert(id);
    }
    assert_eq!(seen.len(), 20_000);
    assert_eq!(symbols.len(), 64);
}
