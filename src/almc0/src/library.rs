//! The names the standard library provides [9].
//!
//! `std` is imported into every module automatically [4.1], so these names resolve
//! unqualified unless a module hides them, and `std::name` always reaches them.
//!
//! The table is names and kinds only. Signatures come later: [9] says `len`,
//! `fixedStr`, `strFixed`, `abs`, `min`, `max`, `scaleOf`, `power`, `floor`, `ceiling`,
//! `truncate`, `sign`, `units`, `fromUnits`, `divide`, `rescale` and the task
//! procedures are "polymorphic built-ins checked by the compiler", which is a later
//! Phase 2 item; everything else is ordinary library code whose signature the compiler
//! will read from `lib/std/` once that exists (Phase 4).

/// What a library name denotes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryKind {
    Procedure,
    Type,
    Constant,
}

/// A library name.
pub struct LibraryName {
    pub name: &'static str,
    pub kind: LibraryKind,
    /// True for a name the language itself relies on, which cannot be hidden [4.1].
    pub unhidable: bool,
    /// True for the polymorphic built-ins the compiler checks itself [9].
    pub builtin: bool,
}

use LibraryKind::{Constant, Procedure, Type};

/// Every name in `std`, in the order [9] introduces them.
///
/// The `unhidable` column is the list [4.1] gives verbatim: "The names `len` and the
/// task, handle and view operations of section 8 ... which the language itself relies
/// on, cannot be hidden."
pub const LIBRARY: &[LibraryName] = &[
    // 9 — the overview table
    lib("len", Procedure, true, true),
    lib("ord", Procedure, false, false),
    lib("chr", Procedure, false, false),
    lib("String", Type, false, false),
    lib("stringcmp", Procedure, false, false),
    lib("concat", Procedure, false, false),
    lib("stringEqual", Procedure, false, false),
    lib("hashInt", Procedure, false, false),
    lib("hashString", Procedure, false, false),
    lib("assert", Procedure, false, false),
    // 8.1, 8.2, 8.3 — tasks and messages; all unhidable
    lib("spawn", Procedure, true, true),
    lib("send", Procedure, true, true),
    lib("receive", Procedure, true, true),
    lib("await", Procedure, true, true),
    lib("sync", Procedure, true, true),
    lib("self", Procedure, true, true),
    lib("replyTo", Procedure, true, true),
    lib("answer", Procedure, true, true),
    // 8.10 — child handles
    lib("link", Procedure, true, true),
    lib("cancel", Procedure, true, true),
    lib("awaitEnd", Procedure, true, true),
    lib("ended", Procedure, true, true),
    lib("failed", Procedure, true, true),
    lib("closing", Procedure, true, true),
    lib("disown", Procedure, true, true),
    // 8.12 — published state
    lib("newView", Procedure, true, true),
    lib("publish", Procedure, true, true),
    lib("latest", Procedure, true, true),
    lib("version", Procedure, true, true),
    lib("awaitChange", Procedure, true, true),
    lib("Version", Type, false, false),
    lib("Round", Type, false, false),
    // 9.1 — entry point
    lib("args", Procedure, false, false),
    // 9.2 — text and standard streams
    lib("write", Procedure, false, false),
    lib("writeLine", Procedure, false, false),
    lib("writeErr", Procedure, false, false),
    lib("writeErrLine", Procedure, false, false),
    lib("readLine", Procedure, false, false),
    lib("intStr", Procedure, false, false),
    lib("fixedStr", Procedure, false, true),
    lib("charStr", Procedure, false, false),
    lib("strInt", Procedure, false, false),
    lib("strFixed", Procedure, false, true),
    // 9.3 — files and bytes
    lib("File", Type, false, false),
    lib("open", Procedure, false, false),
    lib("close", Procedure, false, false),
    lib("ModeRead", Constant, false, false),
    lib("ModeWrite", Constant, false, false),
    lib("ModeAppend", Constant, false, false),
    lib("stdin", Procedure, false, false),
    lib("stdout", Procedure, false, false),
    lib("stderr", Procedure, false, false),
    lib("exists", Procedure, false, false),
    lib("delete", Procedure, false, false),
    lib("rename", Procedure, false, false),
    lib("mkdir", Procedure, false, false),
    lib("list", Procedure, false, false),
    lib("readBytes", Procedure, false, false),
    lib("writeBytes", Procedure, false, false),
    // 9.4 — strings
    lib("substring", Procedure, false, false),
    lib("find", Procedure, false, false),
    lib("startsWith", Procedure, false, false),
    lib("endsWith", Procedure, false, false),
    lib("join", Procedure, false, false),
    lib("split", Procedure, false, false),
    lib("repeat", Procedure, false, false),
    lib("padLeft", Procedure, false, false),
    lib("padRight", Procedure, false, false),
    lib("trim", Procedure, false, false),
    // 9.5 — numbers
    lib("abs", Procedure, false, true),
    lib("min", Procedure, false, true),
    lib("max", Procedure, false, true),
    lib("gcd", Procedure, false, false),
    lib("power", Procedure, false, true),
    lib("floor", Procedure, false, true),
    lib("ceiling", Procedure, false, true),
    lib("truncate", Procedure, false, true),
    lib("sign", Procedure, false, true),
    lib("units", Procedure, false, true),
    lib("fromUnits", Procedure, false, true),
    lib("divide", Procedure, false, true),
    lib("rescale", Procedure, false, true),
    lib("scaleOf", Procedure, false, true),
    lib("RoundHalfAway", Constant, false, false),
    lib("RoundHalfEven", Constant, false, false),
    lib("RoundDown", Constant, false, false),
    lib("RoundUp", Constant, false, false),
    lib("RoundFloor", Constant, false, false),
    lib("RoundCeiling", Constant, false, false),
    lib("band", Procedure, false, false),
    lib("bor", Procedure, false, false),
    lib("bxor", Procedure, false, false),
    lib("bnot", Procedure, false, false),
    lib("wrapAdd", Procedure, false, false),
    lib("wrapSub", Procedure, false, false),
    lib("wrapMul", Procedure, false, false),
    lib("rotl", Procedure, false, false),
    lib("rotr", Procedure, false, false),
    lib("shl", Procedure, false, false),
    lib("shr", Procedure, false, false),
    // 9.6 — time and randomness
    lib("now", Procedure, false, false),
    lib("sleep", Procedure, false, false),
    lib("round", Procedure, false, false),
    lib("sleepRounds", Procedure, false, false),
    lib("random", Procedure, false, false),
];

const fn lib(name: &'static str, kind: LibraryKind, unhidable: bool, builtin: bool) -> LibraryName {
    LibraryName {
        name,
        kind,
        unhidable,
        builtin,
    }
}

pub fn lookup(name: &str) -> Option<&'static LibraryName> {
    LIBRARY.iter().find(|l| l.name == name)
}

/// Can a module, parameter or local declaration hide this library name? [4.1]
pub fn is_unhidable(name: &str) -> bool {
    lookup(name).is_some_and(|l| l.unhidable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_duplicate_names() {
        let mut names: Vec<&str> = LIBRARY.iter().map(|l| l.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "a library name is listed twice");
    }

    #[test]
    fn the_unhidable_list_is_the_one_in_the_spec() {
        // [4.1] gives it verbatim: `len` and the task, handle and view operations.
        let mut got: Vec<&str> = LIBRARY
            .iter()
            .filter(|l| l.unhidable)
            .map(|l| l.name)
            .collect();
        got.sort_unstable();
        let mut want = [
            "len",
            "spawn",
            "send",
            "receive",
            "await",
            "sync",
            "self",
            "replyTo",
            "answer",
            "link",
            "cancel",
            "awaitEnd",
            "ended",
            "failed",
            "closing",
            "disown",
            "newView",
            "publish",
            "latest",
            "version",
            "awaitChange",
        ];
        want.sort_unstable();
        assert_eq!(got, want);
    }

    #[test]
    fn the_polymorphic_builtins_are_the_ones_the_spec_lists() {
        // [9]: "`len`, `fixedStr`, `strFixed`, `abs`, `min`, `max`, `scaleOf`, `power`,
        // `floor`, `ceiling`, `truncate`, `sign`, `units`, `fromUnits`, `divide`,
        // `rescale` and the task procedures are polymorphic built-ins checked by the
        // compiler."
        let got: Vec<&str> = LIBRARY
            .iter()
            .filter(|l| l.builtin && !l.unhidable)
            .map(|l| l.name)
            .collect();
        let mut want = [
            "fixedStr",
            "strFixed",
            "abs",
            "min",
            "max",
            "scaleOf",
            "power",
            "floor",
            "ceiling",
            "truncate",
            "sign",
            "units",
            "fromUnits",
            "divide",
            "rescale",
        ];
        let mut got_sorted = got.clone();
        got_sorted.sort_unstable();
        want.sort_unstable();
        assert_eq!(got_sorted, want);
        // The task procedures are builtins too, and are exactly the unhidable ones.
        assert!(LIBRARY.iter().filter(|l| l.unhidable).all(|l| l.builtin));
    }

    #[test]
    fn the_library_holds_what_the_documents_use() {
        // Names the examples in the three documents call. If one is missing from the
        // table, every example using it would fail to resolve.
        for name in [
            "writeLine",
            "write",
            "readLine",
            "intStr",
            "strInt",
            "len",
            "concat",
            "assert",
            "args",
            "String",
            "File",
            "open",
            "close",
            "spawn",
            "send",
            "receive",
            "await",
            "sync",
            "self",
            "replyTo",
            "answer",
            "now",
            "random",
            "divide",
            "rescale",
            "fixedStr",
            "strFixed",
            "RoundHalfEven",
        ] {
            assert!(
                lookup(name).is_some(),
                "`{name}` is missing from the library table"
            );
        }
    }
}
