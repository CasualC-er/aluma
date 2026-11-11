//! The predefined exceptions of [7.5].
//!
//! Exceptions carry no data. Predefined ones are written without a module name in a
//! report; declared ones are module-qualified (`IntList::Empty`).

/// Declares the enum, `ALL` and `name()` from **one** list of names.
///
/// This is not sugar. A hand-written `ALL` beside the enum is a second list that can
/// silently fall out of step: a variant added to the enum and left out of `ALL` still
/// compiles, and a test that checks only the entries `ALL` does contain still passes.
/// Phase 8 requires a conformance test per entry in the [7.5] table, so a quietly
/// short list would mean a quietly missing test. Generating both from one list makes
/// that impossible rather than merely detectable.
macro_rules! predefined_exceptions {
    ($($name:ident),+ $(,)?) => {
        /// Every exception the spec predefines, in the order of the table in [7.5].
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub enum Predefined {
            $($name),+
        }

        impl Predefined {
            /// All of them, in table order.
            pub const ALL: &'static [Predefined] = &[$(Predefined::$name),+];

            /// The name as it appears in source and in a report.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Predefined::$name => stringify!($name)),+
                }
            }
        }
    };
}

predefined_exceptions! {
    Overflow,
    LostPrecision,
    DivideByZero,
    IndexRange,
    NegativeSize,
    NilReference,
    OutOfMemory,
    OutOfSteps,
    StackOverflow,
    DeadTask,
    Deadlock,
    Closed,
    Timeout,
    InvalidCharacter,
    FormatError,
    BadEncoding,
    FileNotFound,
    PermissionDenied,
    ReadOnly,
    BadContext,
    AssertFailed,
    IOError,
}

impl Predefined {
    /// This exception's position in the table of [7.5], counted from 0.
    pub fn index(self) -> usize {
        // Cannot be out of step with `ALL`: both come from the same macro list.
        Predefined::ALL
            .iter()
            .position(|p| *p == self)
            .expect("ALL holds every variant")
    }

    /// Look a name up. Used by the conformance harness, which names expected
    /// exceptions in test manifests, and by the semantic phase, which must let a
    /// `catch` name a predefined exception without it being declared anywhere.
    pub fn from_name(name: &str) -> Option<Predefined> {
        Predefined::ALL.iter().copied().find(|p| p.name() == name)
    }
}

impl std::fmt::Display for Predefined {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_the_spec_table_text() {
        // The names as the [7.5] table spells them, written out independently of the
        // enum so that a typo in the macro list cannot agree with itself. This is the
        // only check that needs to exist: `ALL` and `name()` are generated from the
        // same list as the enum, so neither can omit a variant.
        const SPEC: &str = "Overflow LostPrecision DivideByZero IndexRange NegativeSize \
                            NilReference OutOfMemory OutOfSteps StackOverflow DeadTask \
                            Deadlock Closed Timeout InvalidCharacter FormatError \
                            BadEncoding FileNotFound PermissionDenied ReadOnly BadContext \
                            AssertFailed IOError";
        let want: Vec<&str> = SPEC.split_whitespace().collect();
        let got: Vec<&str> = Predefined::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(got, want);
        assert_eq!(got.len(), 22);
    }

    #[test]
    fn no_duplicate_names() {
        let mut names: Vec<&str> = Predefined::ALL.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate exception name");
    }

    #[test]
    fn names_round_trip() {
        for p in Predefined::ALL {
            assert_eq!(Predefined::from_name(p.name()), Some(*p));
        }
        assert_eq!(Predefined::from_name("Empty"), None);
    }

    #[test]
    fn index_is_the_table_position() {
        for (i, p) in Predefined::ALL.iter().enumerate() {
            assert_eq!(p.index(), i);
        }
    }
}
