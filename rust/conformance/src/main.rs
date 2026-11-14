//! `cargo run -p lexical-conformance -- [--summary PATH] [FILTER]`
//!
//! Replays the conformance vectors and prints a Markdown report. Exits 1 when
//! a replayed case differs from JS. `--summary` also appends the report to a
//! file, such as `$GITHUB_STEP_SUMMARY`.

use lexical_conformance::run;
use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut summary = None;
    let mut filter = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--summary" => summary = args.next(),
            _ if !arg.starts_with('-') => filter = Some(arg),
            _ => {
                eprintln!("usage: lexical-conformance [--summary PATH] [FILTER]");
                return ExitCode::from(2);
            }
        }
    }
    // A panicking case is reported as a failure; don't print each one.
    std::panic::set_hook(Box::new(|_| {}));
    let report = match run(filter.as_deref()) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let text = format!("## Rust conformance\n\n{}", report.markdown());
    print!("{text}");
    if let Some(path) = summary {
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| f.write_all(text.as_bytes()));
        if let Err(e) = written {
            eprintln!("can't write {path}: {e}");
            return ExitCode::FAILURE;
        }
    }
    if report.ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
