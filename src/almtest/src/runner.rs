//! Running a test against a compiler.
//!
//! The harness invokes the compiler as a subprocess rather than linking it, for two
//! reasons: the thing under test is the whole toolchain including its exit status
//! [9.1], and from Phase 3b onward the pipeline is several processes anyway (`almc0`,
//! `almbe`, `clang`). A test that passes here is a test that would pass for a user.
//!
//! ## What is checked, per mode
//!
//! | Mode | Checked |
//! |---|---|
//! | `compile-ok` | the compiler exits 0 and reports nothing |
//! | `compile-error` | the compiler exits 1, and the lines carrying an error are exactly the annotated ones (or the `[diagnostics]` section matches exactly) |
//! | `run` | `Pending` until there is a backend to produce a binary (Phase 3b) |
//!
//! Exit status 2 from the compiler is always a failure, whatever the mode: it means the
//! harness invoked it wrongly, which is a broken test rather than a rejected program
//! (see `almc0`'s `main`).

use std::path::Path;
use std::process::Command;

use crate::manifest::{Expect, Manifest, Mode};

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

/// One diagnostic as the harness reads it back: a primary line only.
///
/// Notes are indented by two spaces and are *not* errors in their own right — counting
/// them would make a cycle error look like three mistakes and break the "one annotation
/// per error line" rule.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReportedError {
    pub module: String,
    pub line: u32,
    pub message: String,
}

/// Parse what the compiler wrote to standard error.
///
/// The format is `module:line:column: error: message`, with notes indented. This is the
/// one place the harness depends on the diagnostic format, which is why `alm_core::diag`
/// documents it as an interface rather than a convenience.
pub fn parse_diagnostics(stderr: &str) -> Vec<ReportedError> {
    let mut out = Vec::new();
    for raw in stderr.lines() {
        // An indented line is a note belonging to the diagnostic above it.
        if raw.starts_with(' ') || raw.is_empty() {
            continue;
        }
        let mut parts = raw.splitn(4, ':');
        let (Some(module), Some(line), Some(_col), Some(rest)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(line) = line.trim().parse::<u32>() else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(message) = rest.strip_prefix("error: ") else {
            continue;
        };
        out.push(ReportedError {
            module: module.to_owned(),
            line,
            message: message.to_owned(),
        });
    }
    out
}

/// Run one test with the given compiler binary.
pub fn run(m: &Manifest, compiler: &Path) -> Report {
    let outcome = match m.mode {
        Mode::Run => Outcome::Pending("no backend yet (Phase 3b)"),
        Mode::CompileOk => compile_ok(m, compiler),
        Mode::CompileError => compile_error(m, compiler),
    };
    Report {
        name: m.name.clone(),
        outcome,
    }
}

/// Invoke the compiler on the test's modules.
///
/// Every module in the directory is named on the command line, because import
/// resolution is Phase 2; once the module loader exists this passes the main module and
/// lets the compiler find the rest through `--path`.
fn invoke(m: &Manifest, compiler: &Path) -> Result<(i32, String), String> {
    let mut modules: Vec<std::path::PathBuf> = match std::fs::read_dir(&m.dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "alm"))
            .collect(),
        Err(e) => return Err(format!("cannot read {}: {e}", m.dir.display())),
    };
    // The main module first, then the rest in a fixed order: the order modules are
    // given decides the order diagnostics come out in, and that is compared.
    modules.sort();
    let main = m.main_path();
    modules.retain(|p| *p != main);
    modules.insert(0, main);

    let output = Command::new(compiler)
        .args(m.compile_options())
        .args(&modules)
        .output()
        .map_err(|e| format!("cannot run {}: {e}", compiler.display()))?;
    let status = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Ok((status, stderr))
}

fn compile_ok(m: &Manifest, compiler: &Path) -> Outcome {
    let (status, stderr) = match invoke(m, compiler) {
        Ok(pair) => pair,
        Err(e) => return Outcome::Fail(e),
    };
    match status {
        0 if stderr.trim().is_empty() => Outcome::Pass,
        0 => Outcome::Fail(format!("compiled, but reported:\n{stderr}")),
        2 => Outcome::Fail(format!("the compiler was invoked wrongly:\n{stderr}")),
        _ => Outcome::Fail(format!("expected this to compile, but:\n{stderr}")),
    }
}

fn compile_error(m: &Manifest, compiler: &Path) -> Outcome {
    let (status, stderr) = match invoke(m, compiler) {
        Ok(pair) => pair,
        Err(e) => return Outcome::Fail(e),
    };
    if status == 2 {
        return Outcome::Fail(format!("the compiler was invoked wrongly:\n{stderr}"));
    }
    if status == 0 {
        return Outcome::Fail("expected this to be rejected, but it compiled".to_owned());
    }

    // An explicit `[diagnostics]` section is matched whole, which is how a test pins
    // the exact wording of a message.
    if matches!(m.diagnostics, Expect::Exact(_) | Expect::Verbatim(_))
        && !m.diagnostics.matches(&stderr)
    {
        return Outcome::Fail(format!(
            "diagnostics do not match.\nexpected:\n{}\nactual:\n{stderr}",
            m.diagnostics.text().unwrap_or("")
        ));
    }

    if m.expected_errors.is_empty() {
        return Outcome::Pass;
    }

    let problems = check_annotations(&m.expected_errors, &parse_diagnostics(&stderr));
    if problems.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Fail(problems.join("\n"))
    }
}

/// Compare the annotated lines with the errors the compiler reported.
///
/// The set of lines carrying an error must be **exactly** the annotated set: a missing
/// error and an unannotated one are both failures, so neither a rule that was never
/// implemented nor a diagnostic that fires on the wrong line can pass by accident.
///
/// A pure function, deliberately: it is the part of the harness most worth testing, and
/// a harness whose own comparison is untested is a harness that always passes.
pub fn check_annotations(
    expected: &[crate::manifest::ExpectedError],
    reported: &[ReportedError],
) -> Vec<String> {
    let mut problems = Vec::new();
    for want in expected {
        // A manifest annotation names the file (`M.alm`); a diagnostic names the
        // module (`M`). See `alm_core::diag` for why.
        let module = want.module.trim_end_matches(".alm");
        let found: Vec<&ReportedError> = reported
            .iter()
            .filter(|r| r.module == module && r.line == want.line)
            .collect();
        if found.is_empty() {
            problems.push(format!(
                "{module}:{}: expected an error here, none",
                want.line
            ));
            continue;
        }
        if let Some(text) = &want.contains
            && !found.iter().any(|r| r.message.contains(text))
        {
            problems.push(format!(
                "{module}:{}: no error mentions {text:?}; got {:?}",
                want.line,
                found.iter().map(|r| &r.message).collect::<Vec<_>>()
            ));
        }
    }
    for got in reported {
        let annotated = expected
            .iter()
            .any(|w| w.module.trim_end_matches(".alm") == got.module && w.line == got.line);
        if !annotated {
            problems.push(format!(
                "{}:{}: unexpected error: {}",
                got.module, got.line, got.message
            ));
        }
    }
    problems
}

/// Run a whole set of tests and summarise.
#[derive(Clone, Default, Debug)]
pub struct Summary {
    pub passed: usize,
    pub pending: usize,
    pub failures: Vec<(String, String)>,
}

impl Summary {
    pub fn add(&mut self, dir: &Path, report: Report) {
        match report.outcome {
            Outcome::Pass => self.passed += 1,
            Outcome::Pending(_) => self.pending += 1,
            Outcome::Fail(why) => self.failures.push((dir.display().to_string(), why)),
        }
    }

    pub fn total(&self) -> usize {
        self.passed + self.pending + self.failures.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_not_counted_as_an_error() {
        // [4.3]'s cycle error is one diagnostic with two notes. Counting the notes
        // would make it look like three mistakes on three lines.
        let stderr = "App:1:1: error: these types contain each other by value\n  \
                      App:1:6: note: `Node` contains `Tree`\n  \
                      Tree:1:6: note: `Tree` contains `Node`\n";
        let got = parse_diagnostics(stderr);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].module, "App");
        assert_eq!(got[0].line, 1);
        assert_eq!(got[0].message, "these types contain each other by value");
    }

    #[test]
    fn several_diagnostics_are_read_in_order() {
        let stderr = "A:3:1: error: first\nB:9:4: error: second\n";
        let got = parse_diagnostics(stderr);
        assert_eq!(got.len(), 2);
        assert_eq!((got[0].line, got[1].line), (3, 9));
        assert_eq!(got[1].module, "B");
    }

    #[test]
    fn a_warning_is_not_an_error() {
        let got = parse_diagnostics("A:1:1: warning: careful\nA:2:1: error: no\n");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].line, 2);
    }

    #[test]
    fn a_message_containing_a_colon_survives() {
        // The message is everything after the fourth field, so colons in it are safe.
        let got = parse_diagnostics("A:1:1: error: expected `;`, found `:`\n");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].message, "expected `;`, found `:`");
    }

    fn want(module: &str, line: u32, contains: Option<&str>) -> crate::manifest::ExpectedError {
        crate::manifest::ExpectedError {
            module: module.to_owned(),
            line,
            contains: contains.map(str::to_owned),
        }
    }

    fn got(module: &str, line: u32, message: &str) -> ReportedError {
        ReportedError {
            module: module.to_owned(),
            line,
            message: message.to_owned(),
        }
    }

    #[test]
    fn annotations_and_errors_must_match_exactly() {
        let expected = [want("M.alm", 6, Some("do not chain"))];

        // The happy case.
        assert!(
            check_annotations(&expected, &[got("M", 6, "comparisons do not chain")]).is_empty()
        );

        // A rule that was never implemented: the annotation is there, the error is not.
        let missing = check_annotations(&expected, &[]);
        assert_eq!(missing.len(), 1);
        assert!(
            missing[0].contains("expected an error here, none"),
            "{missing:?}"
        );

        // A diagnostic that fires on the wrong line fails twice over, which is right:
        // the annotated line has no error, and line 7 has one nobody asked for.
        let wrong_line = check_annotations(&expected, &[got("M", 7, "comparisons do not chain")]);
        assert_eq!(wrong_line.len(), 2, "{wrong_line:?}");
        assert!(
            wrong_line.iter().any(|p| p.contains("unexpected error")),
            "{wrong_line:?}"
        );

        // The right line, the wrong message.
        let wrong_text = check_annotations(&expected, &[got("M", 6, "expected `;`")]);
        assert_eq!(wrong_text.len(), 1);
        assert!(
            wrong_text[0].contains("no error mentions"),
            "{wrong_text:?}"
        );

        // An error in another module is not this annotation's.
        let other = check_annotations(&expected, &[got("Other", 6, "comparisons do not chain")]);
        assert_eq!(other.len(), 2, "{other:?}");
    }

    #[test]
    fn an_annotation_without_text_accepts_any_message_on_that_line() {
        let expected = [want("M.alm", 3, None)];
        assert!(check_annotations(&expected, &[got("M", 3, "anything at all")]).is_empty());
        assert!(!check_annotations(&expected, &[got("M", 4, "anything at all")]).is_empty());
    }

    #[test]
    fn two_errors_on_one_annotated_line_are_allowed() {
        // One mistake may legitimately produce a primary error and a second on the same
        // line; the rule is about the set of *lines*, not the count.
        let expected = [want("M.alm", 2, Some("first"))];
        let reported = [got("M", 2, "first thing"), got("M", 2, "second thing")];
        assert!(check_annotations(&expected, &reported).is_empty());
    }

    #[test]
    fn noise_is_ignored_rather_than_misread() {
        // A line the compiler wrote that is not a diagnostic — a panic message, say —
        // must not be read as an error at line 0 of a module called "thread".
        let got = parse_diagnostics("thread 'main' panicked at src/x.rs:1:1:\nboom\n");
        assert!(got.is_empty(), "{got:?}");
    }
}
