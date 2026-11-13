//! Every replayed vector case matches JS. Files no suite replays yet are
//! listed in the report, not failed; `cargo run -p lexical-conformance`
//! prints it.

use lexical::json::{self, Value};

fn vectors_or_skip() -> Option<std::path::PathBuf> {
    // The mirror (wevm/lexical-rust) has rust/ without the monorepo's
    // vectors. In the monorepo they're always there.
    let dir = lexical_conformance::vectors_dir();
    if dir.exists() {
        Some(dir)
    } else {
        eprintln!(
            "skipped: no vectors at {}; set LEXICAL_CONFORMANCE_VECTORS",
            dir.display()
        );
        None
    }
}

#[test]
fn replayed_vectors_match_js() {
    if vectors_or_skip().is_none() {
        return;
    }
    let report = lexical_conformance::run(None).expect("vectors");
    assert!(report.ok(), "{}", report.markdown());
}

/// The suites fail when JS's answer differs: in a copy of
/// `serialization/core.json` with one byte of an expected legacy form
/// changed and one compact property dropped, those two cases fail, and only
/// those.
#[test]
fn doctored_vectors_fail() {
    let Some(vectors) = vectors_or_skip() else {
        return;
    };
    let text = std::fs::read_to_string(vectors.join("serialization/core.json")).unwrap();
    let mut file = json::parse(&text).unwrap();
    let Value::Object(object) = &mut file else {
        panic!("not an object")
    };
    let Some(Value::Array(cases)) = object.get_mut("cases") else {
        panic!("no cases")
    };
    let mut doctor = |i: usize, key: &str, from: &str, to: &str| {
        let Value::Object(case) = &mut cases[i] else {
            panic!("case {i}")
        };
        let text = case.get(key).and_then(Value::as_str).unwrap();
        assert!(text.contains(from), "case {i} has no {from}");
        let doctored = text.replacen(from, to, 1);
        case.insert(key, doctored.into());
    };
    doctor(0, "legacy", "\"indent\":0", "\"indent\":1");
    doctor(1, "compact", ",\"format\":1", "");
    let total = cases.len();

    let dir = std::env::temp_dir().join(format!(
        "lexical-conformance-doctored-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("serialization")).unwrap();
    std::fs::write(dir.join("serialization/core.json"), file.to_string()).unwrap();
    let report = lexical_conformance::run_in(&dir, Some("serialization/core"));
    std::fs::remove_dir_all(&dir).unwrap();
    let report = report.unwrap();
    let f = report
        .files
        .iter()
        .find(|f| f.file == "serialization/core")
        .unwrap();
    assert_eq!(
        (f.cases, f.passed, f.failed),
        (total, total - 2, 2),
        "{}",
        report.markdown()
    );
}
