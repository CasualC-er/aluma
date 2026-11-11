//! The `almc0` driver.
//!
//! Usage:
//!
//! ```text
//! almc0 [options] <module.alm> [more modules...]
//! ```
//!
//! Options are those of [8.9], parsed by `alm_core::Options`, which is the one
//! definition of them shared with the backend and the runtime: their values are part of
//! a program's identity [8.4], so nothing here may invent or default them separately.
//!
//! What it does today: reads each module, lexes and parses it, and writes any
//! diagnostics to standard error. Semantic analysis is Phase 2 and lowering is Phase 3,
//! so a clean run means "this is syntactically a module", no more. It exists now
//! because without it nothing in the conformance corpus is actually run — the manifests,
//! the modes and the `#!error` annotations are all decoration until something invokes a
//! compiler (ADR-013.4).
//!
//! Import resolution is also Phase 2, so for now every module to compile is named on
//! the command line and `--path` is accepted but unused. When the module loader lands,
//! the driver will take the main module and find the rest itself.
//!
//! ## Exit status
//!
//! | Status | Meaning |
//! |---|---|
//! | 0 | no diagnostics |
//! | 1 | the source was rejected |
//! | 2 | the compiler was invoked wrongly: a bad option, or a file it cannot read |
//!
//! 1 and 2 are kept apart so that the conformance harness can tell "the compiler
//! correctly rejected this program" from "the test is broken". Nothing is written to
//! standard output: the IR is a file, and diagnostics belong on standard error.

use std::process::ExitCode;

use alm_core::diag::{Diagnostics, SourceMap};
use alm_core::options::Options;
use almc0::{lex::lex, parse::parse};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (_options, files) = match Options::parse(&argv) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("almc0: {e}");
            return ExitCode::from(2);
        }
    };
    if files.is_empty() {
        eprintln!("almc0: no source files given");
        eprintln!("usage: almc0 [options] <module.alm> [more modules...]");
        return ExitCode::from(2);
    }

    // Read every module first, so that a missing file is an invocation error rather
    // than a diagnostic half way through a compilation.
    let mut sources = SourceMap::new();
    let mut ids = Vec::new();
    for path in &files {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("almc0: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        // A module is its file name without the extension [4.1]. The directory is not
        // part of the name, and never appears in a diagnostic (see `alm_core::diag`).
        let module = std::path::Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        ids.push(sources.add(module, text));
    }

    let mut diags = Diagnostics::new();
    for id in ids {
        let text = sources.text(id).to_owned();
        let tokens = lex(id, &text, &mut diags);
        let _program = parse(id, &tokens, &mut diags);
    }

    let rendered = diags.render(&sources);
    if !rendered.is_empty() {
        eprint!("{rendered}");
    }
    if diags.has_errors() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
