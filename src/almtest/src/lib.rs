//! The conformance harness (ADR-013, TODO.md Phase 0).
//!
//! With no interpreter there is no oracle, so the conformance suite *is* the safety
//! net, and it is built before the compiler rather than grown after it.
//!
//! ## A test is a directory
//!
//! Not a single file. The spec's own examples need more than one module — the
//! `Stack` + `Lookup` + `Demo` set in [11] is three files and a `--path` — and the
//! negative tests of Phase 2 and Phase 8 expect a diagnostic and no run at all. So:
//!
//! ```text
//! tests/docs/reference/11-lookup/
//!   test.manifest     the expectations
//!   Demo.alm          the main module
//!   Stack.alm         further modules, found through --path
//!   Lookup.alm
//! ```
//!
//! Every `.alm` file in the directory is a module of the program; `main` names the
//! entry module. The directory itself is always on the module path [4.1].
//!
//! ## The manifest
//!
//! ```text
//! # name: uncaught Overflow is reported with its call chain
//! # source: documentation/language_reference.md:7.5
//! mode = run
//! main = App.alm
//! options = --strict-steps --steps 1000
//! args = --verbose "a file.txt"
//! exit = 1
//!
//! [stdin]
//! a line the program reads with readLine()
//!
//! [stdout]
//! partial output before the failure
//!
//! [stderr]
//! uncaught Overflow in Stats::sum (task 1)
//!   evaluating: total + x
//!   called from: Stats::average
//!   called from: App::start
//! ```
//!
//! Keys before the first section, sections after. Each key and each section may appear
//! once; a repeat is an error rather than a silent overwrite.
//!
//! | Key | Meaning |
//! |---|---|
//! | `mode` | `run` (the default) or `compile-error` |
//! | `main` | the entry module, relative to the test directory |
//! | `options` | compiler options [8.9]; the test directory is added to `--path` |
//! | `args` | arguments the program reads with `args()` [9.1] |
//! | `exit` | expected exit status, for `mode = run` |
//!
//! `options` and `args` split on whitespace, with double quotes for an argument that
//! contains spaces.
//!
//! ### Sections
//!
//! `[stdout]`, `[stderr]` and `[diagnostics]` hold text matched **exactly**, because
//! [7.5]'s report format and the diagnostic text are both part of the specification.
//! `[stdin]` is not matched — it is the input the program is given. A missing output
//! section means "expected empty"; `[stdout] ignore` means the test does not pin it.
//!
//! A section body is read as lines, so it cannot carry a final newline of its own.
//! Hence:
//!
//! - **One trailing blank line is the separator** before the next section and is
//!   dropped. Write two to mean "the output ends with a blank line".
//! - At comparison time, **one** trailing newline is removed from the *actual* output,
//!   so a program's last `writeLine` cancels against a body that cannot express a
//!   terminator. Nothing is removed from the expected side.
//! - `[stdout] exact` compares byte for byte and drops nothing. It is the only way to
//!   tell `write("a")` from `writeLine("a")` [9.2], and the way to give standard input
//!   a last line with no terminator.
//!
//! The one output a section cannot hold is a line that is *exactly* `[stdout]`,
//! `[stderr]`, `[diagnostics]` or `[stdin]` and nothing else; such a line opens a
//! section. Any other bracketed line, `[1, 2, 3]` included, is content.
//!
//! ### Outside events must be pinned
//!
//! [8.7.2] guarantees determinism only *given the same outside events, taking effect in
//! the same rounds*. A test that calls `now`, `sleep` or `random` must therefore pin
//! what it depends on — `random` with `--seed N` in `options` — and a test that depends
//! on the network or on timing cannot be a plain `run` test at all; it belongs to the
//! recorded-and-replayed set of Phase 7 (ADR-010), which this format will gain a key
//! for when replay exists. The `-O0`/`-O2` differential corpus (ADR-013) excludes
//! outside events entirely.
//!
//! ## Compile errors stated inline
//!
//! [11] states compile errors *inside* otherwise-valid examples ("`price + fare` and
//! `total := fare` are compile errors"), so a whole-program verdict cannot express
//! them. A test annotates the offending line in the source instead:
//!
//! ```text
//! total := fare;   #!error: cannot assign a Fare to a Price
//! x := y + 1;      #!error
//! ```
//!
//! `#` starts a comment in Alúma [2], so the annotation does not change the program.
//! `#!error: text` requires an error on that line whose message contains `text`;
//! `#!error` alone requires one with any message. A test with annotations has
//! `mode = compile-error`, and the harness checks that the set of error lines is
//! exactly the set of annotated lines — an unannotated error fails the test too, so a
//! sloppy diagnostic cannot pass by accident. That requires the compiler to keep
//! reporting past the first error, which Phase 1 plans for ("recovery good enough to
//! report several errors in one run"); a cascade of secondary errors from one mistake
//! is a diagnostics-quality bug, and this is where it surfaces.
//!
//! A `compile-error` test that has neither an annotation nor a `[diagnostics]` section
//! is rejected at load: it would otherwise pass on any failure at all.

pub mod manifest;
pub mod runner;

pub use manifest::{Expect, Manifest, Mode, ParseError};
pub use runner::{Outcome, Report};
