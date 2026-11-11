//! Lex every module in the conformance corpus.
//!
//! The corpus is the documentation's own examples (ADR-013.4), so a diagnostic here
//! means either the lexer is wrong about [2] or the extraction produced something the
//! documents do not actually claim is a program. Both are worth knowing, and this is
//! the first test in the project that checks the implementation against real Alúma
//! rather than against a snippet written to exercise one rule.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use alm_core::diag::{Diagnostics, SourceMap};
use alm_core::span::SourceId;
use almc0::lex::lex;
use almc0::token::TokenKind;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// Every `.alm` file under `tests/`, with its path relative to the root.
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
fn every_module_in_the_corpus_lexes_without_a_diagnostic() {
    let root = repo_root();
    let files = modules(&root);
    assert!(
        files.len() > 50,
        "expected the extracted corpus, found {} files",
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
        // A diagnostic names the module, not a path (see `alm_core::diag`), but a
        // failure here must still say which file to look at, so the test prefixes the
        // repository-relative path itself.
        let module = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut sources = SourceMap::new();
        sources.add(module, src.clone());
        let mut diags = Diagnostics::new();
        let tokens = lex(SourceId(0), &src, &mut diags);
        for d in diags.items() {
            failures.push(format!("{rel}: {}", d.render(&sources)));
        }
        assert_eq!(
            tokens.last().map(|t| &t.kind),
            Some(&TokenKind::Eof),
            "{rel}: token stream must end with Eof"
        );
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
fn the_corpus_covers_every_token_kind_but_the_ones_named_here() {
    // A token kind that never appears in the documentation's examples has no corpus
    // cover at all: its only cover is a unit test written from the spec, which cannot
    // catch a disagreement between the spec and real Alúma. So the gap is asserted
    // rather than tolerated — if an example with a char literal is ever added, or an
    // extraction change loses one of the covered kinds, this test says so.
    let root = repo_root();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for path in modules(&root) {
        let src = std::fs::read_to_string(&path).expect("readable module");
        let mut diags = Diagnostics::new();
        for t in lex(SourceId(0), &src, &mut diags) {
            let name = match t.kind {
                TokenKind::Int(_) => "integer literals",
                TokenKind::Fixed { .. } => "fixed literals",
                TokenKind::Char(_) => "char literals",
                TokenKind::Str(_) => "string literals",
                TokenKind::Ident(_) => "identifiers",
                TokenKind::Keyword(_) => "keywords",
                TokenKind::Glyph(_) => "glyphs",
                TokenKind::Eof => continue,
            };
            *counts.entry(name).or_default() += 1;
        }
    }

    let absent: Vec<&str> = [
        "integer literals",
        "fixed literals",
        "char literals",
        "string literals",
        "identifiers",
        "keywords",
        "glyphs",
    ]
    .into_iter()
    .filter(|k| !counts.contains_key(k))
    .collect();

    // Known gap: the three documents never write a char literal. [2]'s char rules —
    // one code point, the escape set, `\u{h}` — are therefore pinned only by the unit
    // tests in `lex.rs`, and a hand-written conformance test is owed (see
    // tests/conformance/README.md).
    assert_eq!(
        absent,
        ["char literals"],
        "the set of token kinds the corpus does not cover has changed; counts: {counts:?}"
    );
}
