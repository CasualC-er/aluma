//! Run the conformance corpus against the real compiler binary.
//!
//! This is the test the whole Phase 0 apparatus existed for: until something invoked a
//! compiler, the manifests and the `#!error` annotations were decoration (ADR-013.4).
//! It lives in `almc0` rather than in `almtest` because Cargo exposes a binary's path
//! to the tests of its own package, as `CARGO_BIN_EXE_almc0`.
//!
//! `run` tests report `Pending` until there is a backend (Phase 3b). A pending count
//! that stops falling is a schedule signal, not a passing build, so it is printed.

use std::path::{Path, PathBuf};

use almtest::manifest;
use almtest::runner::{self, Summary};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn test_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("tests")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n == "test.manifest") {
                out.push(dir.clone());
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_corpus_passes_against_the_compiler() {
    let root = repo_root();
    let compiler = PathBuf::from(env!("CARGO_BIN_EXE_almc0"));
    assert!(compiler.exists(), "{} was not built", compiler.display());

    let dirs = test_dirs(&root);
    assert!(!dirs.is_empty(), "no tests found");

    let mut summary = Summary::default();
    for dir in &dirs {
        let m = match manifest::load(dir) {
            Ok(m) => m,
            Err(e) => {
                summary
                    .failures
                    .push((dir.display().to_string(), format!("will not load: {e}")));
                continue;
            }
        };
        summary.add(dir, runner::run(&m, &compiler));
    }

    println!(
        "conformance: {} passed, {} pending, {} failed, of {}",
        summary.passed,
        summary.pending,
        summary.failures.len(),
        summary.total()
    );
    assert!(
        summary.failures.is_empty(),
        "{} of {} tests failed:\n\n{}",
        summary.failures.len(),
        summary.total(),
        summary
            .failures
            .iter()
            .map(|(d, why)| format!("--- {d}\n{why}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    );
    // Every compile-ok test must actually be checked, not quietly pending.
    assert!(
        summary.passed >= 25,
        "only {} tests ran for real",
        summary.passed
    );
}
