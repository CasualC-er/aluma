//! The corpus must stay loadable.
//!
//! Every test directory under `tests/docs/` and `tests/conformance/` is read with the
//! same code path the harness uses. This catches an extractor that emits a manifest the
//! parser rejects, or a hand-edited manifest that has drifted — which matters because a
//! test that fails to load is a test that silently is not run, and with no interpreter
//! the corpus is the safety net (ADR-013).

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is <root>/src/almtest.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// Every directory that holds a `test.manifest`, anywhere under `tests/`.
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
fn every_test_in_the_corpus_loads() {
    let root = repo_root();
    let dirs = test_dirs(&root);
    assert!(
        !dirs.is_empty(),
        "no tests found: has the extractor been run?"
    );

    let mut failures = Vec::new();
    for dir in &dirs {
        if let Err(e) = almtest::manifest::load(dir) {
            failures.push(format!(
                "{}: {e}",
                dir.strip_prefix(&root).unwrap_or(dir).display()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} tests failed to load:\n{}",
        failures.len(),
        dirs.len(),
        failures.join("\n")
    );
}

#[test]
fn every_test_names_a_main_module_that_exists() {
    let root = repo_root();
    let mut missing = Vec::new();
    for dir in test_dirs(&root) {
        let Ok(m) = almtest::manifest::load(&dir) else {
            continue;
        };
        if !m.main_path().exists() {
            missing.push(format!("{}: main = {}", dir.display(), m.main));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn every_extracted_test_records_where_it_came_from() {
    // `# source:` is what makes a failure traceable back to the sentence in the spec
    // that the test exists to pin.
    let root = repo_root();
    let mut anonymous = Vec::new();
    for dir in test_dirs(&root) {
        let Ok(m) = almtest::manifest::load(&dir) else {
            continue;
        };
        if m.source.is_none() {
            anonymous.push(dir.display().to_string());
        }
    }
    assert!(
        anonymous.is_empty(),
        "tests with no `# source:`:\n{}",
        anonymous.join("\n")
    );
}
