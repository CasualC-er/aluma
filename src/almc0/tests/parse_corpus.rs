//! Parse every module in the conformance corpus, and dump what parsed.
//!
//! These are the documentation's own examples (ADR-013.4), so a diagnostic here is
//! either a parser that disagrees with [10] or an extraction that produced something
//! the documents do not claim is a program.

use std::path::{Path, PathBuf};

use alm_core::diag::Diagnostics;
use alm_core::span::SourceId;
use almc0::{ast, lex::lex, parse::parse};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn modules(root: &Path) -> Vec<PathBuf> {
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
            } else if p.extension().is_some_and(|x| x == "alm") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_module_in_the_corpus_parses() {
    let root = repo_root();
    let files = modules(&root);
    assert!(
        files.len() > 50,
        "expected the extracted corpus, found {}",
        files.len()
    );

    let mut failures = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path).expect("readable module");
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        let mut diags = Diagnostics::new();
        let tokens = lex(SourceId(0), &src, &mut diags);
        let _ = parse(SourceId(0), &tokens, &mut diags);
        for d in diags.items() {
            failures.push(d.render(&rel, &src));
        }
    }
    assert!(
        failures.is_empty(),
        "{} diagnostics across {} modules:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

#[test]
fn the_dump_is_a_function_of_the_source_alone() {
    // ADR-008: the Phase 10 differential compares AST dumps between two compilers, so
    // the dump must not depend on anything the source does not determine — no
    // addresses, no iteration order, no timestamps. Parsing the same text twice must
    // give the same bytes, and so must parsing it from a different path.
    let root = repo_root();
    for path in modules(&root).into_iter().take(20) {
        let src = std::fs::read_to_string(&path).expect("readable module");
        let dump_once = {
            let mut d = Diagnostics::new();
            let tokens = lex(SourceId(0), &src, &mut d);
            ast::dump(&parse(SourceId(0), &tokens, &mut d))
        };
        let dump_again = {
            let mut d = Diagnostics::new();
            // A different source id stands for the same file read from elsewhere.
            let tokens = lex(SourceId(7), &src, &mut d);
            ast::dump(&parse(SourceId(7), &tokens, &mut d))
        };
        assert_eq!(dump_once, dump_again, "{}", path.display());
        assert!(!dump_once.is_empty(), "{} dumped nothing", path.display());
    }
}
