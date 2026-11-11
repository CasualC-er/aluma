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
//! What it does today: loads the main module and everything it imports [4.1], lexes
//! and parses each, and writes any diagnostics to standard error. Name resolution and
//! the type rules are the rest of Phase 2; lowering is Phase 3. So a clean run means
//! "these are syntactically modules and their imports resolve", no more.
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
use almc0::{consts::evaluate, load::load, resolve::resolve};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (options, files) = match Options::parse(&argv) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("almc0: {e}");
            return ExitCode::from(2);
        }
    };
    // [4.1]: "The compiler is given the main module", and finds the rest itself.
    let [main] = files.as_slice() else {
        if files.is_empty() {
            eprintln!("almc0: no main module given");
        } else {
            eprintln!(
                "almc0: give one main module, not {}; the compiler finds the modules it \
                 imports itself [4.1]",
                files.len()
            );
        }
        eprintln!("usage: almc0 [options] <MainModule.alm>");
        return ExitCode::from(2);
    };
    let main = std::path::Path::new(main);
    if !main.is_file() {
        eprintln!("almc0: cannot read the main module: it does not exist");
        return ExitCode::from(2);
    }

    let mut sources = SourceMap::new();
    let mut diags = Diagnostics::new();
    let program = load(main, &options.path, &mut sources, &mut diags);
    // Only resolve if the modules parsed: resolving a tree with holes in it reports
    // names that are missing because the parser could not read them, which buries the
    // real mistake under noise.
    if !diags.has_errors() {
        let _symbols = resolve(&program, &mut diags);
        if !diags.has_errors() {
            let _consts = evaluate(&program, &mut diags);
        }
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
