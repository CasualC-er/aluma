# ADR-002: Host language Rust, one workspace, four crates

Status: accepted. Plan reference: D2.

## Context

Two pieces of the toolchain cannot be written in Alúma. `almrt` needs pointer
arithmetic, allocator control, threads and syscalls, none of which Alúma has
[1, 4, 7.6]; `almbe` could be Alúma later (ADR-001) but not before it can spawn a
process. Those pieces need a host language, and they must not be able to disagree with
the compiler about the IR or about decimal semantics.

## Decision

Rust, one cargo workspace:

| Crate | Role |
|---|---|
| `alm-core` | IR types, canonical serialization and verifier, decimal arithmetic, step and size tables, compile options, report formatting |
| `almc0` | stage-0 compiler: `.alm` → canonical IR |
| `almbe` | backend: canonical IR → `.ll` → object → linked binary |
| `almrt` | runtime: per-task heaps and precise GC, scheduler and round barrier, decimal arithmetic, exceptions and reports, I/O, `Net`, record/replay |

`almtest` is added as a fifth crate for the conformance harness (ADR-013), which is
tooling rather than toolchain.

## Reasons

- A shared `alm-core` means the three consumers cannot drift: the step table is one
  table, the exception list is one list, the options are one struct.
- Memory safety matters more here than usual, because the runtime manages a garbage
  collector and the thing being debugged is "the answer is subtly wrong".
- `cargo test` gives the differential and property harnesses a home with no extra
  machinery.

## Consequences

- The toolchain requires a Rust toolchain; pinned in `rust-toolchain.toml` so CI, the
  bootstrap job and a developer machine agree.
- `unsafe` is forbidden in `alm-core`, `almc0`, `almbe` and `almtest` by a crate-level
  lint. `almrt` cannot forbid it — a precise collector and a syscall edge need it — so
  there it must be local, commented and justified.
- Overflow checks stay on in release builds: a silent wrap inside the compiler would be
  indistinguishable from a miscompile, and the whole testing strategy (ADR-013) rests on
  a wrong answer being the signal.
