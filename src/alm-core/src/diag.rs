//! Compile-time diagnostics.
//!
//! Shared with `almbe` and the harness because the conformance suite matches
//! diagnostics **exactly**: Phase 10 requires `almc` to produce "identical
//! diagnostics, text and order" as `almc0`, which makes the rendering a
//! specification, not a convenience.
//!
//! ## The rendered form
//!
//! A primary line, then zero or more indented notes, each with a position of its own:
//!
//! ```text
//! Stats:12:7: error: `total` is not declared
//!   Stats:4:3: note: a local named `total` is declared here, after its use
//! ```
//!
//! Notes are not decoration. [4.3] requires that a by-value containment cycle be
//! reported "naming the cycle", and a cycle runs through several declarations in
//! possibly several modules, so a single line cannot say it. The same goes for
//! declaration-before-use and for a name that collides with an import.
//!
//! There are no error codes. The specification defines none, and `almc` has to
//! reproduce this text byte for byte in Phase 10 — a code table is extra surface to
//! keep in step for no gain.
//!
//! ## Why a position is a module name, never a path
//!
//! Modules are found through `--path` [4.1], so the same module can be reached by
//! different paths in different invocations. If a diagnostic named a path, its text
//! would depend on how the compiler was called, which breaks the exact matching the
//! conformance suite does and the byte-exactness ADR-008 requires. So a position names
//! the **module** — `Stats`, not `/home/someone/src/Stats.alm` — and the `SourceMap` is
//! the only thing that knows where a module came from.

use crate::span::{Span, line_col};

/// Index of a source file in the compilation's source table.
pub use crate::span::SourceId;

/// One module's name and text.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// The module name [4.1], which is also what a diagnostic prints.
    pub module: String,
    pub text: String,
}

/// Every source the compilation has read.
///
/// Holds the text so that diagnostics can quote it and so that lowering can copy source
/// snippets into the binary for the [7.5] report format (`ir.md` §6).
#[derive(Clone, Default, Debug)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        SourceMap::default()
    }

    /// Add a module. The returned id is its index, and ids are handed out in the order
    /// modules are loaded, which the module loader makes deterministic [4.2].
    pub fn add(&mut self, module: impl Into<String>, text: impl Into<String>) -> SourceId {
        let id = SourceId(self.files.len() as u32);
        self.files.push(SourceFile {
            module: module.into(),
            text: text.into(),
        });
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    pub fn module(&self, id: SourceId) -> &str {
        self.get(id).map(|f| f.module.as_str()).unwrap_or("?")
    }

    pub fn text(&self, id: SourceId) -> &str {
        self.get(id).map(|f| f.text.as_str()).unwrap_or("")
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Render `module:line:column`, the prefix every diagnostic line carries.
    pub fn position(&self, span: Span) -> String {
        let at = line_col(self.text(span.source), span.start);
        format!("{}:{}:{}", self.module(span.source), at.line, at.column)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A secondary line: somewhere else that explains the primary message.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Note {
    pub span: Span,
    pub message: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub span: Span,
    pub message: String,
    /// In the order they were added, which is part of the matched text.
    pub notes: Vec<Note>,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            span,
            message: message.into(),
            notes: Vec::new(),
        }
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            span,
            message: message.into(),
            notes: Vec::new(),
        }
    }

    /// Add a note. Chainable, so a rule can build its whole diagnostic in one
    /// expression and the order of the notes is visible at the call site.
    pub fn note(mut self, span: Span, message: impl Into<String>) -> Self {
        self.notes.push(Note {
            span,
            message: message.into(),
        });
        self
    }

    /// Render against the source map: a primary line, then one indented line per note.
    pub fn render(&self, sources: &SourceMap) -> String {
        let mut out = format!(
            "{}: {}: {}",
            sources.position(self.span),
            self.severity,
            self.message
        );
        for n in &self.notes {
            out.push_str(&format!(
                "\n  {}: note: {}",
                sources.position(n.span),
                n.message
            ));
        }
        out
    }
}

/// Collected diagnostics for one compilation.
///
/// Order is the order they were reported in, and that order is part of the
/// conformance contract, so nothing here sorts or deduplicates.
///
/// The list is **capped**. A compiler that reports a thousand errors is not helping
/// anyone, and a bug in error recovery — a loop that reports without consuming input —
/// otherwise grows this vector until the machine runs out of memory. That is not
/// hypothetical: it happened, and it took the editor down with it. The cap turns that
/// class of bug into a bounded, visible failure instead of an OOM kill.
#[derive(Clone, Default, Debug)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
    dropped: usize,
}

impl Diagnostics {
    /// The most diagnostics one compilation reports.
    pub const LIMIT: usize = 200;

    pub fn new() -> Self {
        Diagnostics::default()
    }

    pub fn push(&mut self, d: Diagnostic) {
        if self.items.len() >= Diagnostics::LIMIT {
            self.dropped += 1;
            return;
        }
        self.items.push(d);
    }

    pub fn error(&mut self, span: Span, message: impl Into<String>) {
        self.push(Diagnostic::error(span, message));
    }

    pub fn items(&self) -> &[Diagnostic] {
        &self.items
    }

    /// How many were dropped because the cap was reached. A driver should say so rather
    /// than let the list look complete.
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    pub fn is_full(&self) -> bool {
        self.items.len() >= Diagnostics::LIMIT
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Every diagnostic, rendered, newline-separated — what a driver writes to standard
    /// error and what the conformance harness compares.
    pub fn render(&self, sources: &SourceMap) -> String {
        let mut out = String::new();
        for d in &self.items {
            out.push_str(&d.render(sources));
            out.push('\n');
        }
        if self.dropped > 0 {
            out.push_str(&format!(
                "{} further diagnostics were not reported\n",
                self.dropped
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_file(text: &str) -> (SourceMap, SourceId) {
        let mut m = SourceMap::new();
        let id = m.add("Stats", text);
        (m, id)
    }

    #[test]
    fn renders_module_line_and_column_never_a_path() {
        let (m, id) = one_file("proc f()\nbegin\n  x := 1;\nend;\n");
        // Offset 17 is the `x` on line 3; the two leading spaces make it column 3.
        let d = Diagnostic::error(Span::new(id, 17, 18), "`x` is not declared");
        assert_eq!(d.render(&m), "Stats:3:3: error: `x` is not declared");
        assert!(!d.render(&m).contains('/'), "a path must never appear");
    }

    #[test]
    fn notes_are_indented_and_keep_their_order() {
        // [4.3] requires a containment cycle to be reported naming the cycle, which
        // takes one note per step.
        let mut m = SourceMap::new();
        let app = m.add("App", "type Node is record t: Tree; end;\n");
        let tree = m.add("Tree", "type Tree is record n: Node; end;\n");
        let d = Diagnostic::error(
            Span::new(app, 0, 4),
            "these types contain each other by value",
        )
        .note(Span::new(app, 5, 9), "`Node` contains `Tree`")
        .note(Span::new(tree, 5, 9), "`Tree` contains `Node`");
        assert_eq!(
            d.render(&m),
            "App:1:1: error: these types contain each other by value\n  \
             App:1:6: note: `Node` contains `Tree`\n  \
             Tree:1:6: note: `Tree` contains `Node`"
        );
    }

    #[test]
    fn a_note_may_point_into_another_module() {
        let mut m = SourceMap::new();
        let a = m.add("A", "x\n");
        let b = m.add("B", "y\n");
        let d = Diagnostic::error(Span::new(a, 0, 1), "collides with an import")
            .note(Span::new(b, 0, 1), "imported here");
        assert!(
            d.render(&m).contains("B:1:1: note: imported here"),
            "{}",
            d.render(&m)
        );
    }

    #[test]
    fn order_is_preserved_not_sorted() {
        let (m, id) = one_file("0123456789\n");
        let mut ds = Diagnostics::new();
        ds.error(Span::new(id, 5, 6), "second position, reported first");
        ds.error(Span::new(id, 0, 1), "first position, reported second");
        assert_eq!(ds.items()[0].span.start, 5);
        assert!(ds.has_errors());
        assert_eq!(ds.len(), 2);
        let rendered = ds.render(&m);
        assert!(
            rendered.starts_with("Stats:1:6: error: second position"),
            "{rendered}"
        );
    }

    #[test]
    fn the_list_is_capped_so_a_recovery_bug_cannot_eat_the_machine() {
        let (m, id) = one_file("x\n");
        let mut ds = Diagnostics::new();
        for i in 0..(Diagnostics::LIMIT + 50) {
            ds.error(Span::new(id, 0, 1), format!("error {i}"));
        }
        assert_eq!(ds.len(), Diagnostics::LIMIT);
        assert_eq!(ds.dropped(), 50);
        assert!(ds.is_full());
        // The ones kept are the earliest, which are the ones worth seeing: later errors
        // are usually consequences of the first.
        assert_eq!(ds.items()[0].message, "error 0");
        assert!(
            ds.render(&m)
                .ends_with("50 further diagnostics were not reported\n")
        );
    }

    #[test]
    fn an_unknown_source_id_does_not_panic() {
        // A synthesised span — for a missing token at end of input, say — must still
        // render, because a crash while reporting an error is the worst outcome.
        let m = SourceMap::new();
        let d = Diagnostic::error(Span::new(SourceId(9), 0, 0), "nothing here");
        assert_eq!(d.render(&m), "?:1:1: error: nothing here");
    }
}
