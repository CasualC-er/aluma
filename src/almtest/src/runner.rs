//! Running a test. The toolchain does not exist yet, so a run reports `Pending`
//! rather than a failure: the corpus is built first (ADR-013) and lights up as
//! `almc0`, `almbe` and `almrt` come online.

use crate::manifest::Manifest;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Pass,
    Fail(String),
    /// The toolchain cannot run this test yet.
    Pending(&'static str),
}

#[derive(Clone, Debug)]
pub struct Report {
    pub name: String,
    pub outcome: Outcome,
}

/// Run one test. Until Phase 3b lands there is nothing to run it with.
pub fn run(m: &Manifest) -> Report {
    Report {
        name: m.name.clone(),
        outcome: Outcome::Pending("no toolchain yet (Phase 3b)"),
    }
}
