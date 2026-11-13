//! Replays the conformance vectors that `packages/lexical-conformance`
//! records from the JS packages against the Rust port, and reports, file by
//! file, how many cases match JS.
//!
//! Every vector file is reported. A file no suite replays yet is listed as
//! not implemented, and a case that needs an upstream package not ported yet
//! is counted apart, so a report never reads as more coverage than it has.
//! Each case is checked on its own: a failing or panicking case is counted
//! and the rest still run.

mod case;
mod suites;

pub use case::{Case, Outcome};

use lexical::json::{self, Value};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

/// The vector format this runner reads.
pub const FORMAT_VERSION: f64 = 1.0;

/// A check of one case: `Ok` with how it matched, or `Err` with how it
/// differs from JS. `file` is the whole vector file.
pub type Check = fn(case: &Value, file: &Value) -> Result<Outcome, String>;

/// The check for each case of one vector file.
#[derive(Clone, Copy)]
pub struct Suite {
    /// The file's path under `vectors/`, without `.json`.
    pub file: &'static str,
    /// What the suite checks when it checks only part of each case, such as
    /// `JSON and text`.
    pub variant: Option<&'static str>,
    pub check: Check,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Replayed; see the counts.
    Replayed,
    /// No suite replays this file yet.
    NotImplemented,
    /// The file couldn't be read or parsed.
    Unreadable(String),
}

/// What a run found for one vector file.
#[derive(Debug)]
pub struct FileReport {
    pub file: String,
    pub variant: Option<&'static str>,
    pub status: Status,
    pub cases: usize,
    pub passed: usize,
    /// Cases that match under a documented exception, by exception.
    pub exceptions: BTreeMap<&'static str, usize>,
    /// Cases not checked because they need a package not ported yet, by
    /// package.
    pub not_ported: BTreeMap<&'static str, usize>,
    pub failed: usize,
    /// The first few failures' messages.
    pub failures: Vec<String>,
}

/// How many failure messages a report keeps per file.
const KEPT_FAILURES: usize = 5;

#[derive(Debug)]
pub struct Report {
    pub files: Vec<FileReport>,
}

impl Report {
    pub fn failed(&self) -> usize {
        self.files.iter().map(|f| f.failed).sum()
    }

    /// Whether every checked case of every replayed file matches JS.
    pub fn ok(&self) -> bool {
        self.files
            .iter()
            .all(|f| f.failed == 0 && !matches!(f.status, Status::Unreadable(_)))
    }

    /// The report as a Markdown table, with totals and the failures kept.
    pub fn markdown(&self) -> String {
        let mut out = String::new();
        let mut totals = [0usize; 5];
        out.push_str(
            "| Vectors | Cases | Match JS | Match with exception | Needs a package not ported | Fail | Status |\n",
        );
        out.push_str("|---|---:|---:|---:|---:|---:|---|\n");
        for f in &self.files {
            let exceptions: usize = f.exceptions.values().sum();
            let not_ported: usize = f.not_ported.values().sum();
            let status = match &f.status {
                Status::Replayed if f.failed > 0 => "**FAIL**".to_owned(),
                Status::Replayed if not_ported > 0 => "partial".to_owned(),
                Status::Replayed => "pass".to_owned(),
                Status::NotImplemented => "not implemented".to_owned(),
                Status::Unreadable(e) => format!("**unreadable**: {e}"),
            };
            let replayed = f.status == Status::Replayed;
            let n = |v: usize| {
                if replayed {
                    v.to_string()
                } else {
                    "–".to_owned()
                }
            };
            let variant = f.variant.map(|v| format!(" ({v})")).unwrap_or_default();
            let _ = writeln!(
                out,
                "| `{}`{variant} | {} | {} | {} | {} | {} | {status} |",
                f.file,
                n(f.cases),
                n(f.passed),
                n(exceptions),
                n(not_ported),
                n(f.failed),
            );
            if replayed {
                for (t, v) in totals
                    .iter_mut()
                    .zip([f.cases, f.passed, exceptions, not_ported, f.failed])
                {
                    *t += v;
                }
            }
        }
        let _ = writeln!(
            out,
            "| **Total** | {} | {} | {} | {} | {} | |",
            totals[0], totals[1], totals[2], totals[3], totals[4]
        );
        let mut by_package: BTreeMap<&str, usize> = BTreeMap::new();
        for f in &self.files {
            for (package, n) in &f.not_ported {
                *by_package.entry(package).or_default() += n;
            }
        }
        if !by_package.is_empty() {
            out.push_str("\nCases waiting on a package: ");
            let list: Vec<String> = by_package
                .iter()
                .map(|(p, n)| format!("`{p}` {n}"))
                .collect();
            out.push_str(&list.join(", "));
            out.push('\n');
        }
        for f in &self.files {
            for message in &f.failures {
                let _ = writeln!(out, "\n- `{}`: {message}", f.file);
            }
        }
        out
    }
}

/// `packages/lexical-conformance/vectors`, or `$LEXICAL_CONFORMANCE_VECTORS`.
pub fn vectors_dir() -> PathBuf {
    match std::env::var_os("LEXICAL_CONFORMANCE_VECTORS") {
        Some(dir) => PathBuf::from(dir),
        None => {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/lexical-conformance/vectors")
        }
    }
}

/// The vector files under `dir`, as paths without `.json`, sorted.
fn vector_files(dir: &Path) -> Result<Vec<String>, String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_dir() {
                walk(&entry.path(), &format!("{prefix}{name}/"), out)?;
            } else if let Some(stem) = name.strip_suffix(".json") {
                out.push(format!("{prefix}{stem}"));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, "", &mut files).map_err(|e| format!("can't read {}: {e}", dir.display()))?;
    files.sort();
    Ok(files)
}

/// Replays every vector file whose name contains `filter` (all when
/// `None`).
pub fn run(filter: Option<&str>) -> Result<Report, String> {
    run_in(&vectors_dir(), filter)
}

/// [`run`] on the vectors in `dir`.
pub fn run_in(dir: &Path, filter: Option<&str>) -> Result<Report, String> {
    let suites = suites::all();
    let mut files = Vec::new();
    for file in vector_files(dir)? {
        if filter.is_some_and(|f| !file.contains(f)) {
            continue;
        }
        let matching: Vec<&Suite> = suites.iter().filter(|s| s.file == file).collect();
        if matching.is_empty() {
            files.push(FileReport::new(file, None, Status::NotImplemented));
            continue;
        }
        for suite in matching {
            files.push(replay(dir, suite));
        }
    }
    for suite in &suites {
        if !files.iter().any(|f| f.file == suite.file)
            && filter.is_none_or(|f| suite.file.contains(f))
        {
            files.push(FileReport::new(
                suite.file.to_owned(),
                suite.variant,
                Status::Unreadable("missing".to_owned()),
            ));
        }
    }
    Ok(Report { files })
}

impl FileReport {
    fn new(file: String, variant: Option<&'static str>, status: Status) -> Self {
        Self {
            file,
            variant,
            status,
            cases: 0,
            passed: 0,
            exceptions: BTreeMap::new(),
            not_ported: BTreeMap::new(),
            failed: 0,
            failures: Vec::new(),
        }
    }
}

fn load(dir: &Path, file: &str) -> Result<Value, String> {
    let path = dir.join(format!("{file}.json"));
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let value = json::parse(&text).map_err(|e| e.to_string())?;
    let version = value.get("formatVersion").and_then(Value::as_f64);
    if version != Some(FORMAT_VERSION) {
        return Err(format!(
            "format version {version:?}; this runner reads {FORMAT_VERSION}"
        ));
    }
    Ok(value)
}

fn replay(dir: &Path, suite: &Suite) -> FileReport {
    let mut report = FileReport::new(suite.file.to_owned(), suite.variant, Status::Replayed);
    let file = match load(dir, suite.file) {
        Ok(file) => file,
        Err(e) => {
            report.status = Status::Unreadable(e);
            return report;
        }
    };
    let Some(cases) = file.get("cases").and_then(Value::as_array) else {
        report.status = Status::Unreadable("no cases".to_owned());
        return report;
    };
    for (i, case) in cases.iter().enumerate() {
        report.cases += 1;
        let result =
            catch_unwind(AssertUnwindSafe(|| (suite.check)(case, &file))).unwrap_or_else(|panic| {
                let message = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("a panic");
                Err(format!("panicked: {message}"))
            });
        match result {
            Ok(Outcome::Pass) => report.passed += 1,
            Ok(Outcome::Exception(name)) => *report.exceptions.entry(name).or_default() += 1,
            Ok(Outcome::NotPorted(package)) => *report.not_ported.entry(package).or_default() += 1,
            Err(message) => {
                report.failed += 1;
                if report.failures.len() < KEPT_FAILURES {
                    let name = case
                        .get("name")
                        .and_then(Value::as_str)
                        .map_or_else(|| format!("#{i}"), str::to_owned);
                    report.failures.push(format!("{name}: {message}"));
                }
            }
        }
    }
    report
}
