//! Shared core for the Alúma toolchain (ADR-002).
//!
//! Everything in here is depended on by more than one of `almc0`, `almbe` and
//! `almrt`, which is the whole reason it exists: the stage-0 compiler, the backend
//! and the runtime must not be able to disagree about the IR, about decimal
//! semantics, or about what a step costs.
//!
//! Bracketed section numbers in this crate point into
//! `documentation/language_reference.md`.

pub mod decimal;
pub mod diag;
pub mod exception;
pub mod options;
pub mod span;
pub mod steps;

pub use diag::{Diagnostic, Severity};
pub use exception::Predefined;
pub use options::Options;
pub use span::{SourceId, Span};
