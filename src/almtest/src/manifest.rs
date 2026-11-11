//! Parsing of `test.manifest` and of the `#!error` annotations in sources.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The sections a manifest may contain. Only these names start a section.
const SECTIONS: &[&str] = &["stdout", "stderr", "diagnostics", "stdin"];

/// What a test asserts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Compile, link, run; compare exit status, stdout and stderr.
    Run,
    /// Compilation must fail; compare the diagnostics. Nothing is run.
    CompileError,
}

/// An expected output stream.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Expect {
    /// Compared after removing **at most one** trailing newline from the actual
    /// output, so that a program's final `writeLine` cancels against a body that
    /// cannot express a terminator. Everything else is content, so a trailing *blank*
    /// line is content and is compared.
    Exact(String),
    /// Compared byte for byte, with nothing removed. The way to pin the difference
    /// between `write("a")` and `writeLine("a")` [9.2], which the lenient form cannot
    /// express. Written `[stdout] exact`.
    ///
    /// A body is read as lines, so the manifest file's own final newline is invisible
    /// and cannot carry meaning: a trailing newline in the expected output is written
    /// as a trailing **blank line**. `[stdout] exact` with the body `a` expects `a`
    /// with no terminator; with the body `a` followed by a blank line it expects
    /// `a\n`.
    Verbatim(String),
    /// Not pinned by this test. Written `[stdout] ignore`.
    Ignore,
}

impl Expect {
    pub fn matches(&self, actual: &str) -> bool {
        match self {
            Expect::Ignore => true,
            Expect::Verbatim(want) => want == actual,
            Expect::Exact(want) => want == strip_one_newline(actual),
        }
    }

    /// The expected text, whichever form this is.
    pub fn text(&self) -> Option<&str> {
        match self {
            Expect::Exact(s) | Expect::Verbatim(s) => Some(s),
            Expect::Ignore => None,
        }
    }
}

/// Remove at most one trailing newline.
///
/// Applied to the **actual** output only, never to the expected text. A section body is
/// read as lines, so it never carries a final terminator of its own and there is
/// nothing on that side to remove; what this cancels is the newline a program's last
/// `writeLine` emits. Every other byte — a trailing *blank* line, interior blank lines
/// — is content. `[stdout] exact` opts out entirely.
fn strip_one_newline(s: &str) -> &str {
    s.strip_suffix('\n').unwrap_or(s)
}

/// An expected error at a particular line of a particular module, from a `#!error`
/// annotation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExpectedError {
    pub module: String,
    /// One-based line number of the annotated line.
    pub line: u32,
    /// `Some(text)`: the message must contain `text`. `None`: any message.
    pub contains: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Manifest {
    /// The test directory, which is also its name in reports.
    pub dir: PathBuf,
    /// Human-readable name, from `# name:`.
    pub name: String,
    /// Where the test came from, from `# source:` — a doc path and section.
    pub source: Option<String>,
    pub mode: Mode,
    /// Entry module file name, relative to the test directory.
    pub main: String,
    /// Extra compiler options, already split into arguments.
    pub options: Vec<String>,
    /// Arguments passed to the compiled program, which it reads with `args()` [9.1].
    pub args: Vec<String>,
    /// Text fed to the program's standard input. `readLine()` returns `nil` once it is
    /// exhausted [9.2], so an absent `[stdin]` section means "end of input at once".
    pub stdin: String,
    /// Expected process exit status, for `mode = run`.
    pub exit: i32,
    pub stdout: Expect,
    pub stderr: Expect,
    /// Expected whole diagnostic text, for `mode = compile-error`.
    pub diagnostics: Expect,
    /// Per-line expectations gathered from the sources' `#!error` annotations.
    pub expected_errors: Vec<ExpectedError>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ParseError {
    /// A line before the first section that is neither blank, a comment nor `k = v`.
    BadLine {
        line: u32,
        text: String,
    },
    UnknownKey {
        line: u32,
        key: String,
    },
    UnknownSection {
        line: u32,
        name: String,
    },
    BadValue {
        line: u32,
        key: &'static str,
        value: String,
    },
    MissingKey(&'static str),
    /// `mode = run` but the sources carry `#!error` annotations, or the reverse.
    ModeConflict(String),
    /// A `compile-error` test that pins nothing about the diagnostics.
    NothingAsserted(String),
    /// The same key or section given twice, which would silently take the last one.
    Duplicate {
        line: u32,
        what: &'static str,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::BadLine { line, text } => {
                write!(f, "line {line}: expected `key = value`, found {text:?}")
            }
            ParseError::UnknownKey { line, key } => write!(f, "line {line}: unknown key {key:?}"),
            ParseError::UnknownSection { line, name } => {
                write!(f, "line {line}: unknown section [{name}]")
            }
            ParseError::BadValue { line, key, value } => {
                write!(f, "line {line}: bad value {value:?} for {key}")
            }
            ParseError::MissingKey(k) => write!(f, "manifest is missing `{k}`"),
            ParseError::ModeConflict(m) | ParseError::NothingAsserted(m) => write!(f, "{m}"),
            ParseError::Duplicate { line, what } => {
                write!(f, "line {line}: {what} is given twice")
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl Manifest {
    /// Parse a manifest. `dir` is recorded for reporting; nothing is read from disk.
    pub fn parse(dir: impl Into<PathBuf>, text: &str) -> Result<Manifest, ParseError> {
        let dir = dir.into();
        let mut name = None;
        let mut source = None;
        let mut keys: BTreeMap<&str, (u32, String)> = BTreeMap::new();
        let mut sections: BTreeMap<&'static str, Expect> = BTreeMap::new();

        let mut current: Option<(&'static str, Vec<&str>, SectionMode, u32)> = None;

        for (i, raw) in text.lines().enumerate() {
            let lineno = i as u32 + 1;
            let trimmed = raw.trim_end();

            // A line that names a known section always opens one. An unknown `[name]`
            // is a typo in the header area, but ordinary content inside a section body
            // — expected output such as `[1, 2, 3]` must survive verbatim.
            match section_header(trimmed) {
                Some((sec, mode)) if SECTIONS.contains(&sec) => {
                    let known = SECTIONS.iter().find(|s| **s == sec).copied().unwrap();
                    flush(&mut current, &mut sections)?;
                    current = Some((known, Vec::new(), mode, lineno));
                    continue;
                }
                Some((other, _)) if current.is_none() => {
                    return Err(ParseError::UnknownSection {
                        line: lineno,
                        name: other.to_owned(),
                    });
                }
                _ => {}
            }

            if let Some((_, lines, ..)) = current.as_mut() {
                lines.push(raw);
                continue;
            }

            // Header area: comments carry the metadata, blanks are ignored.
            let t = trimmed.trim_start();
            if t.is_empty() {
                continue;
            }
            if let Some(c) = t.strip_prefix('#') {
                let c = c.trim();
                if let Some(v) = c.strip_prefix("name:") {
                    name = Some(v.trim().to_owned());
                } else if let Some(v) = c.strip_prefix("source:") {
                    source = Some(v.trim().to_owned());
                }
                continue;
            }
            let Some((k, v)) = t.split_once('=') else {
                return Err(ParseError::BadLine {
                    line: lineno,
                    text: t.to_owned(),
                });
            };
            let k = k.trim();
            let k: &'static str = match k {
                "mode" => "mode",
                "main" => "main",
                "options" => "options",
                "args" => "args",
                "exit" => "exit",
                _ => {
                    return Err(ParseError::UnknownKey {
                        line: lineno,
                        key: k.to_owned(),
                    });
                }
            };
            if keys.insert(k, (lineno, v.trim().to_owned())).is_some() {
                return Err(ParseError::Duplicate {
                    line: lineno,
                    what: k,
                });
            }
        }
        flush(&mut current, &mut sections)?;

        let (mode_line, mode_text) = keys.get("mode").cloned().unwrap_or((0, "run".to_owned()));
        let mode = match mode_text.as_str() {
            "run" => Mode::Run,
            "compile-error" => Mode::CompileError,
            _ => {
                return Err(ParseError::BadValue {
                    line: mode_line,
                    key: "mode",
                    value: mode_text,
                });
            }
        };
        let main = keys
            .get("main")
            .map(|(_, v)| v.clone())
            .ok_or(ParseError::MissingKey("main"))?;
        let split = |k: &str| -> Vec<String> {
            keys.get(k).map(|(_, v)| split_args(v)).unwrap_or_default()
        };
        let exit = match keys.get("exit") {
            Some((line, v)) => v.parse().map_err(|_| ParseError::BadValue {
                line: *line,
                key: "exit",
                value: v.clone(),
            })?,
            // A `run` test with no `exit` expects success; a compile-error test does
            // not run at all, and the compiler's own status is checked instead.
            None => 0,
        };
        // Input is given, not matched, so the body is taken as written. `[stdin]` ends
        // with a newline unless written `[stdin] exact` without a trailing blank line,
        // which is how a last line with no terminator is expressed — [9.2] lets
        // `readLine` see end of input without a final newline.
        let stdin = match sections.remove("stdin") {
            Some(Expect::Exact(s)) => {
                if s.is_empty() {
                    s
                } else {
                    format!("{s}\n")
                }
            }
            Some(Expect::Verbatim(s)) => s,
            Some(Expect::Ignore) | None => String::new(),
        };

        Ok(Manifest {
            dir,
            name: name.unwrap_or_else(|| "unnamed".to_owned()),
            source,
            mode,
            main,
            options: split("options"),
            args: split("args"),
            stdin,
            exit,
            stdout: sections
                .remove("stdout")
                .unwrap_or(Expect::Exact(String::new())),
            stderr: sections
                .remove("stderr")
                .unwrap_or(Expect::Exact(String::new())),
            diagnostics: sections.remove("diagnostics").unwrap_or(Expect::Ignore),
            expected_errors: Vec::new(),
        })
    }

    /// Fold in the `#!error` annotations of one module's source, and check that they
    /// agree with the declared mode.
    pub fn add_annotations(&mut self, module: &str, source: &str) -> Result<(), ParseError> {
        let found = annotations(module, source);
        if !found.is_empty() && self.mode == Mode::Run {
            return Err(ParseError::ModeConflict(format!(
                "{module} carries #!error annotations but mode = run"
            )));
        }
        self.expected_errors.extend(found);
        Ok(())
    }

    /// Check that the test asserts something. Called once every module's annotations
    /// have been folded in.
    ///
    /// Without this, a `compile-error` test with no annotations and no `[diagnostics]`
    /// section passes on *any* compiler failure — including one about a different line,
    /// or a crash — which is worse than no test, because it looks like coverage.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.mode == Mode::CompileError
            && self.expected_errors.is_empty()
            && self.diagnostics == Expect::Ignore
        {
            return Err(ParseError::NothingAsserted(format!(
                "{}: a compile-error test needs at least one #!error annotation or a \
                 [diagnostics] section, or it passes on any failure",
                self.dir.display()
            )));
        }
        Ok(())
    }

    /// Path of the entry module.
    pub fn main_path(&self) -> PathBuf {
        self.dir.join(&self.main)
    }

    /// The full option list for a compile: the test's own options, plus the test
    /// directory on the module path so that sibling modules are found [4.1].
    pub fn compile_options(&self) -> Vec<String> {
        let mut v = self.options.clone();
        v.push("--path".to_owned());
        v.push(self.dir.to_string_lossy().into_owned());
        v
    }
}

/// Close the section being accumulated, if any.
///
/// The body is the lines verbatim, joined with newlines: nothing is trimmed, because
/// `Expect` decides at comparison time what counts (see `strip_one_newline`). To mean
/// "the output ends with a blank line", end the body with a blank line.
fn flush(
    current: &mut Option<(&'static str, Vec<&str>, SectionMode, u32)>,
    sections: &mut BTreeMap<&'static str, Expect>,
) -> Result<(), ParseError> {
    if let Some((name, mut lines, mode, line)) = current.take() {
        // One trailing empty line is the blank separator before the next section, not
        // content. Write two to mean "the output really does end with a blank line".
        // `exact` drops nothing, so an `exact` section must be followed immediately by
        // the next header or by end of file.
        if mode != SectionMode::Verbatim && lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        let body = lines.join("\n");
        let value = match mode {
            SectionMode::Ignore => Expect::Ignore,
            SectionMode::Lenient => Expect::Exact(body),
            SectionMode::Verbatim => Expect::Verbatim(body),
        };
        if sections.insert(name, value).is_some() {
            return Err(ParseError::Duplicate { line, what: name });
        }
    }
    Ok(())
}

/// How a section's body is compared.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SectionMode {
    /// `[name]`
    Lenient,
    /// `[name] exact`
    Verbatim,
    /// `[name] ignore`
    Ignore,
}

/// `[name]`, `[name] exact` or `[name] ignore`, as a whole line.
///
/// Only a line of exactly this shape, naming one of `SECTIONS`, opens a section. A
/// body line such as `[1, 2, 3]` or `[done]` is content. The one shape a body cannot
/// hold is a line that is exactly `[stdout]`, `[stderr]`, `[diagnostics]` or `[stdin]`
/// with nothing else on it; such output is pinned with `ignore`, or by another stream.
fn section_header(line: &str) -> Option<(&str, SectionMode)> {
    let t = line.trim();
    let t = t.strip_prefix('[')?;
    let (name, rest) = t.split_once(']')?;
    let name = name.trim();
    // A section name is one lowercase word; `[1, 2, 3]` is not a header.
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let mode = match rest.trim() {
        "" => SectionMode::Lenient,
        "exact" => SectionMode::Verbatim,
        "ignore" => SectionMode::Ignore,
        _ => return None,
    };
    Some((name, mode))
}

/// Split an option or argument list, honouring double quotes so that an argument may
/// contain spaces: `args = --name "two words"`. A backslash escapes the next byte.
fn split_args(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut started = false;
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    cur.push(next);
                    started = true;
                }
            }
            '"' => {
                in_quotes = !in_quotes;
                // An empty quoted string is still an argument.
                started = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(cur);
    }
    out
}

/// Collect `#!error` annotations from a source file.
///
/// The annotation is a comment, so it can only appear where a comment can. An
/// annotation inside a string literal is not one: `"#!error"` is text [2]. Rather
/// than lex Alúma here, the scan skips char and string literals, which is enough
/// because an annotation is always the last thing on its line.
pub fn annotations(module: &str, source: &str) -> Vec<ExpectedError> {
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        if let Some(rest) = comment_body(line) {
            let rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix("!error") {
                let contains = match after.trim_start().strip_prefix(':') {
                    Some(text) => {
                        let text = text.trim();
                        (!text.is_empty()).then(|| text.to_owned())
                    }
                    None => None,
                };
                out.push(ExpectedError {
                    module: module.to_owned(),
                    line: i as u32 + 1,
                    contains,
                });
            }
        }
    }
    out
}

/// The text after the first `#` that starts a comment on this line, if any.
fn comment_body(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == b'\\' {
                    i += 2;
                    continue;
                }
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'\'' | b'"' => quote = Some(b),
                b'#' => return Some(&line[i + 1..]),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

/// Read a test directory: its manifest, plus the annotations of every module in it.
pub fn load(dir: &Path) -> Result<Manifest, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(dir.join("test.manifest"))?;
    let mut m = Manifest::parse(dir, &text)?;
    let mut modules: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "alm"))
        .collect();
    // Sorted so that the expectation order does not depend on the file system.
    modules.sort();
    for path in modules {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(&path)?;
        m.add_annotations(&name, &src)?;
    }
    m.validate()?;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_run_test() {
        let m = Manifest::parse(
            "t",
            "# name: hello\n# source: documentation/guide.md:1\nmode = run\nmain = Main.alm\n\
             options = --steps 10 --strict-steps\nexit = 0\n\n[stdout]\nHello, world!\n",
        )
        .unwrap();
        assert_eq!(m.name, "hello");
        assert_eq!(m.source.as_deref(), Some("documentation/guide.md:1"));
        assert_eq!(m.mode, Mode::Run);
        assert_eq!(m.main, "Main.alm");
        assert_eq!(m.options, ["--steps", "10", "--strict-steps"]);
        assert_eq!(m.exit, 0);
        assert_eq!(m.stdout, Expect::Exact("Hello, world!".to_owned()));
        // An absent section means empty, not unchecked.
        assert_eq!(m.stderr, Expect::Exact(String::new()));
        assert!(m.args.is_empty());
        assert_eq!(m.stdin, "");
        m.validate().unwrap();
    }

    #[test]
    fn the_test_directory_is_always_on_the_module_path() {
        let m = Manifest::parse("tests/docs/11-lookup", "main = Demo.alm\n").unwrap();
        assert_eq!(m.compile_options(), ["--path", "tests/docs/11-lookup"]);
    }

    #[test]
    fn multi_line_sections_keep_their_shape() {
        let m = Manifest::parse(
            "t",
            "main = App.alm\nexit = 1\n\n[stderr]\nuncaught Overflow in Stats::sum (task 1)\n  \
             evaluating: total + x\n  called from: App::start\n",
        )
        .unwrap();
        let Expect::Exact(e) = &m.stderr else {
            panic!("expected an exact match")
        };
        assert_eq!(
            e,
            "uncaught Overflow in Stats::sum (task 1)\n  evaluating: total + x\n  \
             called from: App::start"
        );
        assert_eq!(m.exit, 1);
    }

    #[test]
    fn a_bracketed_output_line_is_content_not_a_section() {
        // Regression: expected output is arbitrary text, and plenty of programs print
        // brackets. Only the known section names open a section.
        let m = Manifest::parse(
            "t",
            "main = M.alm\n\n[stdout]\n[1, 2, 3]\n[done]\n[STDOUT]\nnot [a] header\n",
        )
        .unwrap();
        assert_eq!(
            m.stdout,
            Expect::Exact("[1, 2, 3]\n[done]\n[STDOUT]\nnot [a] header".to_owned())
        );
    }

    #[test]
    fn an_unknown_section_in_the_header_area_is_still_an_error() {
        assert!(matches!(
            Manifest::parse("t", "main = M.alm\n\n[stdoutt]\n"),
            Err(ParseError::UnknownSection { .. })
        ));
    }

    #[test]
    fn stdin_and_args_are_carried() {
        // [9.2]'s readLine() and [9.1]'s args() are used by the spec's own examples,
        // so the corpus has to be able to supply both.
        let m = Manifest::parse(
            "t",
            "main = Echo.alm\nargs = --verbose file.txt\n\n[stdin]\nfirst line\nsecond line\n\n\
             [stdout]\nfirst line\nsecond line\n",
        )
        .unwrap();
        assert_eq!(m.args, ["--verbose", "file.txt"]);
        // Input is text a program reads with readLine, so it ends with a newline.
        assert_eq!(m.stdin, "first line\nsecond line\n");
        assert_eq!(
            m.stdout,
            Expect::Exact("first line\nsecond line".to_owned())
        );
    }

    #[test]
    fn stdin_can_omit_its_final_newline() {
        // [9.2]: readLine() returns nil at end of input, which a last line with no
        // terminator must still be able to produce.
        let m = Manifest::parse("t", "main = M.alm\n\n[stdin] exact\nno newline here").unwrap();
        assert_eq!(m.stdin, "no newline here");
    }

    #[test]
    fn an_argument_may_contain_spaces() {
        let m = Manifest::parse("t", "main = M.alm\nargs = --name \"two words\" plain\n").unwrap();
        assert_eq!(m.args, ["--name", "two words", "plain"]);
    }

    #[test]
    fn a_repeated_key_or_section_is_an_error() {
        // Silently taking the last one would make a copy-paste mistake invisible.
        assert!(matches!(
            Manifest::parse("t", "main = A.alm\nmain = B.alm\n"),
            Err(ParseError::Duplicate { what: "main", .. })
        ));
        assert!(matches!(
            Manifest::parse("t", "main = M.alm\n\n[stdout]\na\n\n[stdout]\nb\n"),
            Err(ParseError::Duplicate { what: "stdout", .. })
        ));
    }

    #[test]
    fn exact_mode_pins_the_final_newline() {
        // `write("a")` and `writeLine("a")` [9.2] differ by exactly one byte, and the
        // lenient form cannot tell them apart. This is the escape hatch.
        let write = Manifest::parse("t", "main = M.alm\n\n[stdout] exact\na").unwrap();
        assert_eq!(write.stdout, Expect::Verbatim("a".to_owned()));
        assert!(write.stdout.matches("a"));
        assert!(
            !write.stdout.matches("a\n"),
            "exact mode must see the newline"
        );

        // A trailing newline is written as a blank line: a file's own final newline is
        // invisible to a line-based reader, so it cannot mean anything.
        let write_line = Manifest::parse("t", "main = M.alm\n\n[stdout] exact\na\n\n").unwrap();
        assert_eq!(write_line.stdout, Expect::Verbatim("a\n".to_owned()));
        assert!(write_line.stdout.matches("a\n"));
        assert!(!write_line.stdout.matches("a"));
    }

    #[test]
    fn a_section_can_opt_out() {
        let m = Manifest::parse("t", "main = M.alm\n\n[stdout] ignore\n").unwrap();
        assert_eq!(m.stdout, Expect::Ignore);
        assert!(m.stdout.matches("anything at all"));
    }

    #[test]
    fn one_trailing_blank_line_is_the_section_separator() {
        // The parser and the matcher must agree, or a test could never pass.
        let m = Manifest::parse("t", "main = M.alm\n\n[stdout]\na\nb\n\n").unwrap();
        assert_eq!(m.stdout, Expect::Exact("a\nb".to_owned()));
        assert!(m.stdout.matches("a\nb"));
        assert!(
            m.stdout.matches("a\nb\n"),
            "the program's own final newline cancels"
        );
        assert!(
            !m.stdout.matches("a\nb\n\n"),
            "a real blank line is a difference"
        );
        assert!(!m.stdout.matches("a\nb\nc"));
        assert!(
            !m.stdout.matches("a\n\nb"),
            "interior blank lines are content"
        );
    }

    #[test]
    fn two_trailing_blank_lines_pin_a_blank_last_line() {
        let m = Manifest::parse("t", "main = M.alm\n\n[stdout]\na\n\n\n").unwrap();
        assert_eq!(m.stdout, Expect::Exact("a\n".to_owned()));
        // The expected text is `a` then a blank line; the program's own final newline
        // is what cancels. So only output that really has the blank line matches.
        assert!(m.stdout.matches("a\n\n"));
        assert!(!m.stdout.matches("a\n"));
        assert!(!m.stdout.matches("a"));
    }

    #[test]
    fn interior_blank_lines_survive_parsing() {
        let m = Manifest::parse("t", "main = M.alm\n\n[stdout]\na\n\nb\n").unwrap();
        assert_eq!(m.stdout, Expect::Exact("a\n\nb".to_owned()));
    }

    #[test]
    fn rejects_malformed_manifests() {
        assert!(matches!(
            Manifest::parse("t", "main = M.alm\nmode = sideways\n"),
            Err(ParseError::BadValue { key: "mode", .. })
        ));
        assert!(matches!(
            Manifest::parse("t", "mode = run\n"),
            Err(ParseError::MissingKey("main"))
        ));
        assert!(matches!(
            Manifest::parse("t", "main = M.alm\nwibble = 1\n"),
            Err(ParseError::UnknownKey { .. })
        ));
        assert!(matches!(
            Manifest::parse("t", "main = M.alm\nnot a pair\n"),
            Err(ParseError::BadLine { .. })
        ));
    }

    #[test]
    fn a_compile_error_test_must_assert_something() {
        let bare = Manifest::parse("t", "mode = compile-error\nmain = M.alm\n").unwrap();
        assert!(matches!(
            bare.validate(),
            Err(ParseError::NothingAsserted(_))
        ));

        let with_section = Manifest::parse(
            "t",
            "mode = compile-error\nmain = M.alm\n\n[diagnostics]\nM.alm:1:1: error: nope\n",
        )
        .unwrap();
        with_section.validate().unwrap();

        let mut with_annotation =
            Manifest::parse("t", "mode = compile-error\nmain = M.alm\n").unwrap();
        with_annotation
            .add_annotations("M.alm", "x := 1; #!error\n")
            .unwrap();
        with_annotation.validate().unwrap();
    }

    #[test]
    fn annotations_are_found_with_and_without_text() {
        let src = "proc f() is\n  total := fare;   #!error: cannot assign a Fare to a Price\n  \
                   x := y + 1;      #!error\n  ok := 1;\nend\n";
        let a = annotations("M.alm", src);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].line, 2);
        assert_eq!(
            a[0].contains.as_deref(),
            Some("cannot assign a Fare to a Price")
        );
        assert_eq!(a[1].line, 3);
        assert_eq!(a[1].contains, None);
    }

    #[test]
    fn a_hash_inside_a_literal_is_not_an_annotation() {
        // [2]: "A `#` inside a char or string literal is an ordinary character."
        let src = "  writeLine(\"#!error: not an annotation\");\n  c := '#';\n";
        assert!(annotations("M.alm", src).is_empty());
    }

    #[test]
    fn an_annotation_after_a_literal_is_still_found() {
        let src = "  writeLine(\"hi\");  #!error: wrong type\n";
        let a = annotations("M.alm", src);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].contains.as_deref(), Some("wrong type"));
    }

    #[test]
    fn an_escaped_quote_does_not_end_the_literal() {
        let src = "  writeLine(\"say \\\"#!error\\\" loudly\");\n";
        assert!(annotations("M.alm", src).is_empty());
    }

    #[test]
    fn annotations_conflict_with_run_mode() {
        let mut m = Manifest::parse("t", "mode = run\nmain = M.alm\n").unwrap();
        let err = m.add_annotations("M.alm", "x := 1; #!error\n").unwrap_err();
        assert!(matches!(err, ParseError::ModeConflict(_)));

        let mut ok = Manifest::parse("t", "mode = compile-error\nmain = M.alm\n").unwrap();
        ok.add_annotations("M.alm", "x := 1; #!error\n").unwrap();
        assert_eq!(ok.expected_errors.len(), 1);
    }
}
