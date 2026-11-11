//! Compile options [8.9].
//!
//! These are **fixed when the program is compiled and are part of the program's
//! identity**, so the values live in three places and must agree in all of them:
//!
//! 1. `almc0` (and later `almc`) parses them and records them in the IR;
//! 2. `almbe` bakes them into the binary as a data section, which is where `almrt`
//!    reads its budget, memory and depth limits from — the generated program takes
//!    no such switch of its own;
//! 3. the replay log header carries them, and a replay refuses a binary whose
//!    identity differs (ADR-010, [8.7]).
//!
//! `identity()` is the canonical encoding used for (3). It is deliberately textual,
//! sorted by option name, and independent of host paths (ADR-008): `--path` only
//! affects *which* files are found, not the program's meaning, so it is excluded.
//!
//! ## `--seed` is not like the others
//!
//! [8.9] lists `--seed` among the options "fixed when the program is compiled", but
//! gives its default as "chosen from the operating system and recorded", which is a
//! per-run choice. Only an explicitly given seed can be baked in. We therefore treat
//! the seed as *bound at startup and recorded in the log*: baked in when `--seed` was
//! given, drawn from the OS and recorded otherwise. This is logged as amendment A-002
//! in `documentation/amendments.md`.

use std::fmt::Write as _;

/// A value for `--seed`, which is the one option not fully fixed at compile time.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Seed {
    /// No `--seed` was given: the runtime draws one at startup and records it.
    #[default]
    FromOs,
    /// `--seed N` was given: baked into the binary, identical on every run.
    Fixed(u64),
}

/// The options of [8.9]. Defaults are exactly the table's.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Options {
    /// Step budget `N` per task per round [8.4].
    pub steps: u64,
    /// Budget is a limit, not a time slice: `OutOfSteps` instead of a yield [8.4].
    pub strict_steps: bool,
    /// Live-size limit per task, in size units [8.4].
    pub memory: u64,
    /// Call-depth limit [8.5.2].
    pub depth: u64,
    /// Step limit for each initializer [4.2].
    pub init_steps: u64,
    /// Live-size limit for each initializer, in size units [4.2].
    pub init_memory: u64,
    /// Size limit of the shared region, in size units [4.2].
    pub shared_memory: u64,
    /// Rounds a closing task may run before it is stopped [8.11].
    pub grace: u64,
    /// Milliseconds a closing task may spend blocked before `Timeout` [8.11].
    pub grace_time: u64,
    /// Bytes buffered for each socket half before a write blocks [9.7].
    pub socket_buffer: u64,
    /// Open files and sockets per task [9.3, 9.7].
    pub handles: u64,
    /// Seed for `random` [9.6]. See the note above.
    pub seed: Seed,
    /// Directories searched for modules, in order [4.1]. Not part of identity.
    pub path: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            steps: 1_000_000,
            strict_steps: false,
            memory: 16 * 1024 * 1024,
            depth: 10_000,
            init_steps: 100_000_000,
            init_memory: 64 * 1024 * 1024,
            shared_memory: 256 * 1024 * 1024,
            grace: 100,
            grace_time: 5_000,
            socket_buffer: 65_536,
            handles: 1_024,
            seed: Seed::FromOs,
            path: Vec::new(),
        }
    }
}

/// Why an option list was rejected. Rendered by the driver, not by a panic.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OptionError {
    Unknown(String),
    MissingValue(&'static str),
    NotANumber { option: &'static str, value: String },
    OutOfRange { option: &'static str, value: String },
}

impl std::fmt::Display for OptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionError::Unknown(o) => write!(f, "unknown option {o}"),
            OptionError::MissingValue(o) => write!(f, "option {o} needs a value"),
            OptionError::NotANumber { option, value } => {
                write!(f, "option {option} needs a number, not {value:?}")
            }
            OptionError::OutOfRange { option, value } => {
                write!(f, "value {value:?} for option {option} is out of range")
            }
        }
    }
}

impl std::error::Error for OptionError {}

impl Options {
    /// Parse `--name value` and `--name=value` forms. Arguments that are not options
    /// are returned in order, so the driver can pick the source files out of them.
    ///
    /// Only the options of [8.9] are accepted; an implementation "may offer others",
    /// but adding one has to be a deliberate act, so an unknown `--x` is an error
    /// rather than a file name.
    pub fn parse<I, S>(args: I) -> Result<(Options, Vec<String>), OptionError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut opts = Options::default();
        let mut rest = Vec::new();
        let mut args = args.into_iter().map(|s| s.as_ref().to_owned()).peekable();

        while let Some(arg) = args.next() {
            if !arg.starts_with("--") {
                rest.push(arg);
                continue;
            }
            let (name, inline) = match arg.split_once('=') {
                Some((n, v)) => (n.to_owned(), Some(v.to_owned())),
                None => (arg.clone(), None),
            };

            // A flag takes no value.
            if name == "--strict-steps" {
                if let Some(v) = inline {
                    return Err(OptionError::NotANumber {
                        option: "--strict-steps",
                        value: v,
                    });
                }
                opts.strict_steps = true;
                continue;
            }

            let spec = NUMERIC.iter().find(|s| s.name == name);
            let is_known = spec.is_some() || name == "--seed" || name == "--path";
            if !is_known {
                return Err(OptionError::Unknown(name));
            }
            let value = match inline {
                Some(v) => v,
                None => {
                    let leaked = KNOWN_NAMES
                        .iter()
                        .find(|n| **n == name)
                        .copied()
                        .unwrap_or("option");
                    args.next().ok_or(OptionError::MissingValue(leaked))?
                }
            };

            if let Some(spec) = spec {
                let n = parse_u64(spec.name, &value)?;
                if n < spec.min {
                    return Err(OptionError::OutOfRange {
                        option: spec.name,
                        value,
                    });
                }
                (spec.set)(&mut opts, n);
            } else if name == "--seed" {
                opts.seed = Seed::Fixed(parse_u64("--seed", &value)?);
            } else {
                // `--path` is a list, searched in the order given [4.1].
                opts.path.push(value);
            }
        }
        Ok((opts, rest))
    }

    /// The canonical identity of this program's option set: one `name=value` line
    /// per option, sorted by name, newline-terminated.
    ///
    /// Goes in the replay log header; a replay refuses a binary whose identity
    /// differs (ADR-010). `--path` is excluded: it selects files, it is not part of
    /// the program's meaning, and host paths must not appear (ADR-008). An OS-chosen
    /// seed is recorded as `seed=os`, and the drawn value is a log record of its own,
    /// so a recording and its replay have the same identity string.
    pub fn identity(&self) -> String {
        let mut lines: Vec<String> = NUMERIC
            .iter()
            .map(|s| format!("{}={}", s.name.trim_start_matches("--"), (s.get)(self)))
            .collect();
        lines.push(format!("strict-steps={}", self.strict_steps));
        lines.push(match self.seed {
            Seed::FromOs => "seed=os".to_owned(),
            Seed::Fixed(n) => format!("seed={n}"),
        });
        lines.sort();
        let mut out = String::new();
        for l in lines {
            let _ = writeln!(out, "{l}");
        }
        out
    }
}

fn parse_u64(option: &'static str, value: &str) -> Result<u64, OptionError> {
    // No `+`, no `_`, no hex: option values are plain decimal digits, so that the
    // identity string is a function of the value and not of how it was spelled.
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(OptionError::NotANumber {
            option,
            value: value.to_owned(),
        });
    }
    value.parse().map_err(|_| OptionError::OutOfRange {
        option,
        value: value.to_owned(),
    })
}

struct NumericOption {
    name: &'static str,
    /// Smallest accepted value. A zero step budget would mean no task ever runs,
    /// and a zero depth that no procedure can be called, so those start at 1.
    min: u64,
    get: fn(&Options) -> u64,
    set: fn(&mut Options, u64),
}

const NUMERIC: &[NumericOption] = &[
    NumericOption {
        name: "--steps",
        min: 1,
        get: |o| o.steps,
        set: |o, n| o.steps = n,
    },
    NumericOption {
        name: "--memory",
        min: 1,
        get: |o| o.memory,
        set: |o, n| o.memory = n,
    },
    NumericOption {
        name: "--depth",
        min: 1,
        get: |o| o.depth,
        set: |o, n| o.depth = n,
    },
    NumericOption {
        name: "--init-steps",
        min: 1,
        get: |o| o.init_steps,
        set: |o, n| o.init_steps = n,
    },
    NumericOption {
        name: "--init-memory",
        min: 1,
        get: |o| o.init_memory,
        set: |o, n| o.init_memory = n,
    },
    NumericOption {
        name: "--shared-memory",
        min: 1,
        get: |o| o.shared_memory,
        set: |o, n| o.shared_memory = n,
    },
    // `--grace 0` is meaningful: stop a closing task at the next boundary.
    NumericOption {
        name: "--grace",
        min: 0,
        get: |o| o.grace,
        set: |o, n| o.grace = n,
    },
    NumericOption {
        name: "--grace-time",
        min: 0,
        get: |o| o.grace_time,
        set: |o, n| o.grace_time = n,
    },
    NumericOption {
        name: "--socket-buffer",
        min: 1,
        get: |o| o.socket_buffer,
        set: |o, n| o.socket_buffer = n,
    },
    NumericOption {
        name: "--handles",
        min: 1,
        get: |o| o.handles,
        set: |o, n| o.handles = n,
    },
];

const KNOWN_NAMES: &[&str] = &[
    "--steps",
    "--strict-steps",
    "--memory",
    "--depth",
    "--init-steps",
    "--init-memory",
    "--shared-memory",
    "--grace",
    "--grace-time",
    "--socket-buffer",
    "--handles",
    "--seed",
    "--path",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec_table() {
        let o = Options::default();
        assert_eq!(o.steps, 1_000_000);
        assert!(!o.strict_steps);
        assert_eq!(o.memory, 16_777_216);
        assert_eq!(o.depth, 10_000);
        assert_eq!(o.init_steps, 100_000_000);
        assert_eq!(o.init_memory, 67_108_864);
        assert_eq!(o.shared_memory, 268_435_456);
        assert_eq!(o.grace, 100);
        assert_eq!(o.grace_time, 5_000);
        assert_eq!(o.socket_buffer, 65_536);
        assert_eq!(o.handles, 1_024);
        assert_eq!(o.seed, Seed::FromOs);
        assert!(o.path.is_empty());
    }

    #[test]
    fn both_spellings_parse() {
        let (a, rest) = Options::parse(["--steps", "500", "Main.alm"]).unwrap();
        let (b, _) = Options::parse(["--steps=500"]).unwrap();
        assert_eq!(a.steps, 500);
        assert_eq!(b.steps, 500);
        assert_eq!(rest, vec!["Main.alm".to_owned()]);
    }

    #[test]
    fn flag_takes_no_value() {
        let (o, rest) = Options::parse(["--strict-steps", "Main.alm"]).unwrap();
        assert!(o.strict_steps);
        assert_eq!(rest, vec!["Main.alm".to_owned()]);
    }

    #[test]
    fn path_keeps_the_order_given() {
        let (o, _) = Options::parse(["--path", "a", "--path=b", "--path", "c"]).unwrap();
        assert_eq!(o.path, vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
    }

    #[test]
    fn rejects_unknown_missing_and_malformed() {
        assert_eq!(
            Options::parse(["--nope", "1"]),
            Err(OptionError::Unknown("--nope".to_owned()))
        );
        assert_eq!(
            Options::parse(["--steps"]),
            Err(OptionError::MissingValue("--steps"))
        );
        assert_eq!(
            Options::parse(["--steps", "0x10"]),
            Err(OptionError::NotANumber {
                option: "--steps",
                value: "0x10".to_owned()
            })
        );
        assert_eq!(
            Options::parse(["--steps", "0"]),
            Err(OptionError::OutOfRange {
                option: "--steps",
                value: "0".to_owned()
            })
        );
    }

    #[test]
    fn identity_is_sorted_and_path_free() {
        let (o, _) = Options::parse(["--path", "/home/someone/src", "--steps=7"]).unwrap();
        let id = o.identity();
        let lines: Vec<&str> = id.lines().collect();
        let mut sorted = lines.clone();
        sorted.sort();
        assert_eq!(lines, sorted, "identity lines must be sorted");
        assert!(
            !id.contains("/home/someone"),
            "identity must not carry host paths"
        );
        assert!(id.contains("steps=7"));
        assert!(id.contains("seed=os"));
    }

    #[test]
    fn identity_ignores_how_a_value_was_spelled_and_ordered() {
        let (a, _) = Options::parse(["--steps=7", "--depth", "9"]).unwrap();
        let (b, _) = Options::parse(["--depth=9", "--steps", "7"]).unwrap();
        assert_eq!(a.identity(), b.identity());
    }

    #[test]
    fn identity_of_the_defaults_is_pinned_exactly() {
        // This string goes in the replay-log header, and a replay refuses a binary
        // whose identity differs (ADR-010). Renaming an option, dropping one, or
        // changing a default therefore changes run identity, so the whole string is
        // pinned here rather than spot-checked.
        let expected = "\
depth=10000
grace-time=5000
grace=100
handles=1024
init-memory=67108864
init-steps=100000000
memory=16777216
seed=os
shared-memory=268435456
socket-buffer=65536
steps=1000000
strict-steps=false
";
        assert_eq!(Options::default().identity(), expected);
        // Every [8.9] option that is part of identity appears exactly once.
        assert_eq!(expected.lines().count(), NUMERIC.len() + 2);
    }

    #[test]
    fn a_value_too_large_for_u64_is_rejected_not_wrapped() {
        let huge = "99999999999999999999999999";
        assert_eq!(
            Options::parse(["--steps", huge]),
            Err(OptionError::OutOfRange {
                option: "--steps",
                value: huge.to_owned()
            })
        );
    }

    #[test]
    fn the_flag_rejects_a_value_instead_of_silently_ignoring_it() {
        assert_eq!(
            Options::parse(["--strict-steps=x"]),
            Err(OptionError::NotANumber {
                option: "--strict-steps",
                value: "x".to_owned()
            })
        );
        // And it does not swallow the next argument as its value.
        let (o, rest) = Options::parse(["--strict-steps", "Main.alm"]).unwrap();
        assert!(o.strict_steps);
        assert_eq!(rest, ["Main.alm"]);
    }

    #[test]
    fn a_fixed_seed_is_part_of_identity() {
        let (a, _) = Options::parse(["--seed=1"]).unwrap();
        let (b, _) = Options::parse(["--seed=2"]).unwrap();
        let (c, _) = Options::parse([] as [&str; 0]).unwrap();
        assert_ne!(a.identity(), b.identity());
        assert_ne!(a.identity(), c.identity());
        assert!(c.identity().contains("seed=os"));
    }
}
