//! The module loader [4.1].
//!
//! A module is a source file; its name is the file name without the extension. The
//! compiler is given the main module and finds the rest itself: the program is the main
//! module together with every module it imports, directly or through others, all
//! compiled from source. There are no interface files.
//!
//! ## Where a module is looked for
//!
//! `A` is the file `A.alm`, searched for in the `--path` directories in order, first
//! match winning; failing that, the implementation's own modules. `Net` is one of
//! those, "and a program's own `Net.alm` on the path takes its place" — which needs no
//! special case here, because `--path` is searched first. `--path` defaults to the main
//! module's directory [8.9].
//!
//! ## Order
//!
//! Modules come back in depth-first order with a module's imports before itself. That
//! is the order [4.2] gives for `share` initialization ("in declaration order, imports
//! first"), and ADR-008 requires the IR's module order to be exactly that traversal, so
//! the loader produces it once and everything downstream keeps it.
//!
//! ## What this does not do
//!
//! Name resolution, visibility and `private` are the next step. The loader reads,
//! parses and orders; it reports only what it alone can see: a module that cannot be
//! found, a name that cannot be a module, a self-import, an import cycle, and a `start`
//! outside the main module [4.1].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use alm_core::diag::{Diagnostic, Diagnostics, SourceMap};
use alm_core::span::{SourceId, Span};

use crate::ast::{DeclKind, Program};
use crate::lex::lex;
use crate::parse::parse;
use crate::token::Keyword;

/// Modules the implementation provides, which need no file on the path.
///
/// `std` is imported by every module automatically and cannot be declared [2, 4.1].
/// `Net` is an implementation module [9.7] that a program may replace with its own
/// `Net.alm`; that happens for free, since `--path` is searched first.
pub const BUILTIN_MODULES: &[&str] = &["std", "Net"];

/// One module of the program.
#[derive(Clone, Debug)]
pub struct Module {
    pub name: String,
    /// `None` for an implementation module, which has no source to parse yet.
    pub id: Option<SourceId>,
    pub program: Option<Program>,
    /// The modules this one imports, in source order, with the span of each name.
    pub imports: Vec<(String, Span)>,
}

impl Module {
    pub fn is_builtin(&self) -> bool {
        self.id.is_none()
    }
}

/// The program: every module, with a module's imports before itself.
#[derive(Clone, Debug, Default)]
pub struct Loaded {
    pub modules: Vec<Module>,
    /// Index of the main module in `modules`. It is last, being imported by nothing.
    pub main: usize,
}

impl Loaded {
    pub fn find(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.name == name)
    }
}

/// Load the main module and everything it imports.
pub fn load(
    main: &Path,
    search_path: &[String],
    sources: &mut SourceMap,
    diags: &mut Diagnostics,
) -> Loaded {
    // `--path` defaults to the main module's directory [8.9].
    let mut dirs: Vec<PathBuf> = search_path.iter().map(PathBuf::from).collect();
    if dirs.is_empty() {
        dirs.push(main.parent().unwrap_or(Path::new(".")).to_path_buf());
    }

    let Some(main_name) = module_name_of(main) else {
        // The file name, never the path: a diagnostic that named a directory would
        // depend on where the tree lives, which ADR-008 forbids and the harness's
        // exact matching cannot tolerate.
        let shown = main
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| "this file".to_owned());
        diags.error(
            Span::new(SourceId(0), 0, 0),
            format!(
                "`{shown}` cannot be a module: a module is named by its file, and the \
                 name must be an identifier that is not a keyword [4.1]"
            ),
        );
        return Loaded::default();
    };

    let mut loader = Loader {
        dirs,
        sources,
        diags,
        done: BTreeMap::new(),
        order: Vec::new(),
        stack: Vec::new(),
        main_name: main_name.clone(),
    };
    loader.visit(&main_name, Some(main.to_path_buf()), None);
    let order = std::mem::take(&mut loader.order);
    let main = order.iter().position(|m| m.name == main_name).unwrap_or(0);
    Loaded {
        modules: order,
        main,
    }
}

/// A file name turned into a module name, or `None` if it cannot be one.
fn module_name_of(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    if is_module_name(stem) {
        Some(stem.to_owned())
    } else {
        None
    }
}

/// `letter { letter | digit }`, ASCII, and not a keyword [2, 4.1].
fn is_module_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric())
        && Keyword::lookup(s).is_none()
}

/// Where a module is in the DFS.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// On the stack: reaching it again is a cycle.
    Visiting,
    Done,
}

struct Loader<'a> {
    dirs: Vec<PathBuf>,
    sources: &'a mut SourceMap,
    diags: &'a mut Diagnostics,
    done: BTreeMap<String, State>,
    order: Vec<Module>,
    /// The import chain being followed: (module name, the span that imported it).
    stack: Vec<(String, Option<Span>)>,
    main_name: String,
}

impl Loader<'_> {
    /// Visit a module, then its imports, then record it.
    ///
    /// `at` is the span of the `import` item that led here, for the diagnostics; `None`
    /// for the main module, which nothing imported.
    fn visit(&mut self, name: &str, path: Option<PathBuf>, at: Option<Span>) {
        match self.done.get(name) {
            Some(State::Done) => return,
            Some(State::Visiting) => {
                self.report_cycle(name, at);
                return;
            }
            None => {}
        }

        // The path is searched first, then the implementation's own modules [4.1].
        // That order is the whole mechanism by which "a program's own `Net.alm` on the
        // path takes its place": checking the builtin list first would make the rule
        // impossible to honour.
        let found = path.or_else(|| self.search(name));
        if found.is_none() && BUILTIN_MODULES.contains(&name) {
            self.done.insert(name.to_owned(), State::Done);
            self.order.push(Module {
                name: name.to_owned(),
                id: None,
                program: None,
                imports: Vec::new(),
            });
            return;
        }

        let Some(found) = found else {
            // Not on the path and not an implementation module. The message names no
            // directory: a diagnostic must not depend on where the tree lives
            // (ADR-008), and the harness compares its text exactly.
            let span = at.unwrap_or(Span::new(SourceId(0), 0, 0));
            self.diags.error(
                span,
                format!("cannot find module `{name}`: there is no `{name}.alm` on the module path"),
            );
            self.done.insert(name.to_owned(), State::Done);
            return;
        };

        let text = match std::fs::read_to_string(&found) {
            Ok(t) => t,
            Err(e) => {
                let span = at.unwrap_or(Span::new(SourceId(0), 0, 0));
                self.diags.error(
                    span,
                    format!("cannot read module `{name}`: {}", io_reason(&e)),
                );
                self.done.insert(name.to_owned(), State::Done);
                return;
            }
        };

        let id = self.sources.add(name, text.clone());
        let tokens = lex(id, &text, self.diags);
        let program = parse(id, &tokens, self.diags);

        // `start` belongs to the main module alone [4.1].
        if name != self.main_name {
            for d in &program.decls {
                if let DeclKind::Proc {
                    name: proc_name, ..
                } = &d.kind
                    && proc_name.text == "start"
                {
                    self.diags.error(
                        proc_name.span,
                        format!(
                            "only the main module may declare `start`; `{name}` is \
                             imported, not the main module [4.1]"
                        ),
                    );
                }
            }
        }

        // Both places an import can appear: the `import` lines at the top [4.1], and
        // the instance imports that may stand among the declarations [4.4]. Missing the
        // second would leave a generic module unloaded.
        let mut imports: Vec<(String, Span)> = Vec::new();
        let mut collect = |item: &crate::ast::ImportItem| {
            // For `import X is G(...)` the file needed is the generic module `G`, not
            // the instance name `X` [4.4].
            match &item.instance {
                Some(inst) => imports.push((inst.generic.text.clone(), inst.generic.span)),
                None => imports.push((item.name.text.clone(), item.name.span)),
            }
        };
        for line in &program.imports {
            for item in &line.items {
                collect(item);
            }
        }
        for decl in &program.decls {
            if let DeclKind::Instance(line) = &decl.kind {
                for item in &line.items {
                    collect(item);
                }
            }
        }

        self.done.insert(name.to_owned(), State::Visiting);
        self.stack.push((name.to_owned(), at));
        for (import, span) in &imports {
            if import == name {
                self.diags
                    .error(*span, "a module cannot import itself [4.1]");
                continue;
            }
            if !is_module_name(import) {
                // The parser guarantees an identifier, so this can only be a keyword
                // used as a module name, which the parser accepts as an `ident`.
                self.diags.error(
                    *span,
                    format!("`{import}` is a keyword, so it cannot name a module [4.1]"),
                );
                continue;
            }
            self.visit(import, None, Some(*span));
        }
        self.stack.pop();
        self.done.insert(name.to_owned(), State::Done);
        self.order.push(Module {
            name: name.to_owned(),
            id: Some(id),
            program: Some(program),
            imports,
        });
    }

    /// The first `<name>.alm` in the `--path` directories, in order [4.1].
    fn search(&self, name: &str) -> Option<PathBuf> {
        for dir in &self.dirs {
            let candidate = dir.join(format!("{name}.alm"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Report an import cycle, naming it [4.1].
    ///
    /// The primary error sits at the import that closes the cycle, and one note per
    /// step names the chain. A single line cannot say this, which is why a diagnostic
    /// carries notes at all (see `alm_core::diag`).
    fn report_cycle(&mut self, name: &str, at: Option<Span>) {
        let span = at.unwrap_or(Span::new(SourceId(0), 0, 0));
        let from = self.stack.iter().position(|(n, _)| n == name).unwrap_or(0);
        let chain: Vec<String> = self.stack[from..].iter().map(|(n, _)| n.clone()).collect();
        let mut d = Diagnostic::error(
            span,
            format!(
                "these imports form a cycle: {} imports `{name}` again [4.1]",
                chain
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(" imports ")
            ),
        );
        // One note per step, at the import that made it.
        for window in self.stack[from..].windows(2) {
            if let [(importer, _), (imported, imported_at)] = window
                && let Some(s) = imported_at
            {
                d = d.note(*s, format!("`{importer}` imports `{imported}` here"));
            }
        }
        self.diags.push(d);
    }
}

/// An I/O failure in words that do not name a path.
fn io_reason(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => "the file does not exist".to_owned(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_owned(),
        std::io::ErrorKind::InvalidData => "it is not valid UTF-8".to_owned(),
        _ => e.kind().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway directory holding the given `name.alm` files.
    struct Tree(PathBuf);

    impl Tree {
        fn new(label: &str, files: &[(&str, &str)]) -> Tree {
            // A unique directory per test, so tests can run in parallel.
            let dir = std::env::temp_dir().join(format!(
                "almc0-load-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("a writable temporary directory");
            for (name, text) in files {
                std::fs::write(dir.join(name), text).expect("writable");
            }
            Tree(dir)
        }

        fn sub(&self, name: &str, files: &[(&str, &str)]) -> PathBuf {
            let dir = self.0.join(name);
            std::fs::create_dir_all(&dir).expect("writable");
            for (n, text) in files {
                std::fs::write(dir.join(n), text).expect("writable");
            }
            dir
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Load, and return (module names in order, rendered diagnostics).
    fn load_tree(main: &Path, dirs: &[String]) -> (Vec<String>, Vec<String>) {
        let mut sources = SourceMap::new();
        let mut diags = Diagnostics::new();
        let loaded = load(main, dirs, &mut sources, &mut diags);
        let names = loaded.modules.iter().map(|m| m.name.clone()).collect();
        let rendered = diags.items().iter().map(|d| d.render(&sources)).collect();
        (names, rendered)
    }

    #[test]
    fn a_module_imports_come_before_it() {
        // [4.2] initialises shared values "in declaration order, imports first", and
        // ADR-008 requires the IR's module order to be that traversal, so the loader
        // produces it: depth first, a module after everything it imports.
        let t = Tree::new(
            "order",
            &[
                ("Main.alm", "import Middle;\nproc start()\nbegin\nend;\n"),
                ("Middle.alm", "import Leaf;\nproc go()\nbegin\nend;\n"),
                ("Leaf.alm", "proc go()\nbegin\nend;\n"),
            ],
        );
        let (names, errs) = load_tree(&t.path("Main.alm"), &[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(names, ["Leaf", "Middle", "Main"]);
    }

    #[test]
    fn a_diamond_loads_the_shared_module_once() {
        let t = Tree::new(
            "diamond",
            &[
                (
                    "Main.alm",
                    "import Left, Right;\nproc start()\nbegin\nend;\n",
                ),
                ("Left.alm", "import Shared;\nproc go()\nbegin\nend;\n"),
                ("Right.alm", "import Shared;\nproc go()\nbegin\nend;\n"),
                ("Shared.alm", "proc go()\nbegin\nend;\n"),
            ],
        );
        let (names, errs) = load_tree(&t.path("Main.alm"), &[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(names, ["Shared", "Left", "Right", "Main"]);
        assert_eq!(names.iter().filter(|n| *n == "Shared").count(), 1);
    }

    #[test]
    fn the_first_directory_on_the_path_wins() {
        // [4.1]: "searched for in the directories of the `--path` option; the first
        // match wins."
        let t = Tree::new(
            "pathorder",
            &[("Main.alm", "import Dep;\nproc start()\nbegin\nend;\n")],
        );
        let first = t.sub("first", &[("Dep.alm", "const Which is 1;\n")]);
        let second = t.sub("second", &[("Dep.alm", "const Which is 2;\n")]);

        let mut sources = SourceMap::new();
        let mut diags = Diagnostics::new();
        let dirs = vec![
            t.0.display().to_string(),
            first.display().to_string(),
            second.display().to_string(),
        ];
        let loaded = load(&t.path("Main.alm"), &dirs, &mut sources, &mut diags);
        assert!(!diags.has_errors(), "{:?}", diags.items());
        let dep = loaded.find("Dep").expect("Dep was loaded");
        assert!(
            sources
                .text(dep.id.expect("from source"))
                .contains("Which is 1"),
            "the first directory on the path must win"
        );
    }

    #[test]
    fn the_path_defaults_to_the_main_modules_directory() {
        // [8.9]: the default for `--path` is "the main module's directory".
        let t = Tree::new(
            "default",
            &[
                ("Main.alm", "import Dep;\nproc start()\nbegin\nend;\n"),
                ("Dep.alm", "proc go()\nbegin\nend;\n"),
            ],
        );
        let (names, errs) = load_tree(&t.path("Main.alm"), &[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(names, ["Dep", "Main"]);
    }

    #[test]
    fn an_implementation_module_needs_no_file() {
        // `Net` is one of the implementation's own modules [4.1, 9.7], and `std` is
        // imported by every module automatically.
        let t = Tree::new(
            "builtin",
            &[("Main.alm", "import Net;\nproc start()\nbegin\nend;\n")],
        );
        let (names, errs) = load_tree(&t.path("Main.alm"), &[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(names, ["Net", "Main"]);
        let mut sources = SourceMap::new();
        let mut diags = Diagnostics::new();
        let loaded = load(&t.path("Main.alm"), &[], &mut sources, &mut diags);
        assert!(loaded.find("Net").expect("Net").is_builtin());
    }

    #[test]
    fn a_programs_own_net_takes_the_implementations_place() {
        // [4.1]: "`Net` is one of them, and a program's own `Net.alm` on the path takes
        // its place." It needs no special case: `--path` is searched first.
        let t = Tree::new(
            "ownnet",
            &[
                ("Main.alm", "import Net;\nproc start()\nbegin\nend;\n"),
                ("Net.alm", "proc listen()\nbegin\nend;\n"),
            ],
        );
        let mut sources = SourceMap::new();
        let mut diags = Diagnostics::new();
        let loaded = load(&t.path("Main.alm"), &[], &mut sources, &mut diags);
        assert!(!diags.has_errors(), "{:?}", diags.items());
        let net = loaded.find("Net").expect("Net");
        assert!(!net.is_builtin(), "the program's own Net.alm must win");
    }

    #[test]
    fn an_instance_import_loads_the_generic_module() {
        // `import Names is Lookup(...)` needs the file `Lookup.alm` [4.4], not `Names`.
        let t = Tree::new(
            "instance",
            &[
                (
                    "Main.alm",
                    "import Names is Lookup(int);\nproc start()\nbegin\nend;\n",
                ),
                ("Lookup.alm", "generic type T;\nproc go()\nbegin\nend;\n"),
            ],
        );
        let (names, errs) = load_tree(&t.path("Main.alm"), &[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(names, ["Lookup", "Main"]);
    }

    #[test]
    fn a_cycle_is_reported_naming_the_cycle() {
        // [4.1]: "the imports must not form a cycle (a compile error that names the
        // cycle)".
        let t = Tree::new(
            "cycle",
            &[
                ("A.alm", "import B;\nproc start()\nbegin\nend;\n"),
                ("B.alm", "import C;\nproc go()\nbegin\nend;\n"),
                ("C.alm", "import A;\nproc go()\nbegin\nend;\n"),
            ],
        );
        let (_, errs) = load_tree(&t.path("A.alm"), &[]);
        assert_eq!(errs.len(), 1, "one diagnostic for one cycle: {errs:?}");
        assert!(
            errs[0].contains("`A` imports `B` imports `C` imports `A` again"),
            "{errs:?}"
        );
        // The chain is named by notes, one per step, which is what notes exist for.
        assert!(
            errs[0].contains("A:1:8: note: `A` imports `B` here"),
            "{errs:?}"
        );
        assert!(
            errs[0].contains("B:1:8: note: `B` imports `C` here"),
            "{errs:?}"
        );
    }

    #[test]
    fn a_module_cannot_import_itself() {
        let t = Tree::new(
            "self",
            &[("S.alm", "import S;\nproc start()\nbegin\nend;\n")],
        );
        let (_, errs) = load_tree(&t.path("S.alm"), &[]);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("cannot import itself"), "{errs:?}");
    }

    #[test]
    fn a_missing_module_names_no_directory() {
        // ADR-008: a diagnostic must not depend on where the tree lives, and the
        // harness compares its text exactly.
        let t = Tree::new(
            "missing",
            &[("Main.alm", "import Gone;\nproc start()\nbegin\nend;\n")],
        );
        let (_, errs) = load_tree(&t.path("Main.alm"), &[]);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("cannot find module `Gone`"), "{errs:?}");
        assert!(!errs[0].contains('/'), "no path may appear: {errs:?}");
    }

    #[test]
    fn only_the_main_module_may_declare_start() {
        let t = Tree::new(
            "start",
            &[
                ("Main.alm", "import Worker;\nproc start()\nbegin\nend;\n"),
                ("Worker.alm", "proc start()\nbegin\nend;\n"),
            ],
        );
        let (_, errs) = load_tree(&t.path("Main.alm"), &[]);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(
            errs[0].contains("only the main module may declare `start`"),
            "{errs:?}"
        );
        assert!(errs[0].starts_with("Worker:1:6:"), "{errs:?}");
    }

    #[test]
    fn what_may_be_a_module_name() {
        // [4.1]: the file name without the extension, "which must be an identifier that
        // is not a keyword". Identifiers are ASCII, with no underscores [2].
        for good in ["A", "Stats", "a1", "IntList", "x9y"] {
            assert!(is_module_name(good), "{good} should be a module name");
        }
        for bad in [
            "",
            "1A",
            "my-module",
            "my_module",
            "ñ",
            "end",
            "proc",
            "std2 x",
        ] {
            assert!(!is_module_name(bad), "{bad} should not be a module name");
        }
        // `std` is a name the language reserves [2], but it is a *module* name — the
        // rule that it cannot be declared belongs to name resolution, not here.
        assert!(is_module_name("std"));
    }
}
