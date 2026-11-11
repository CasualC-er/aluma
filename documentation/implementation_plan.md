# Alúma: Implementation Plan (nothing → self-hosting native compiler)

Companion to `language_reference.md` (what the language is), `justification.md` (why) and
`style_guide.md` (how programs are written). This document says **how it gets built**: the
architecture, the decisions that have to be made before code is written, the order of work,
and where the self-hosting boundary falls.

Status: proposal. Nothing in `src/` yet. Section numbers in brackets point into the
Language Reference. Chosen: native code generation through LLVM, x86-64 Linux, no
interpreter.

---

## 0. The target, stated precisely

Alúma compiles to **native code**. The compiler proper — lexer, parser, semantics,
lowering — is written in Alúma and compiles itself to a fixpoint. Code generation is a
host-language stage that consumes the compiler's IR and drives LLVM.

| Artifact | Language | Role | Lifetime |
|---|---|---|---|
| `almrt` | host | runtime: per-task heaps + precise GC, scheduler and round barrier, decimal arithmetic, exceptions and reports, I/O, `Net`, record/replay | permanent |
| `almbe` | host | backend: canonical IR → textual LLVM IR → object → link against `almrt` | permanent; portable to Alúma later (§1 D1) |
| `almc0` | host | stage-0 compiler: `.alm` → canonical IR | bootstrap seed, then permanent differential oracle |
| `almc` | **Alúma** | the compiler: `.alm` → canonical IR | permanent, self-compiling |
| `std` (non-builtin parts), `support/` | **Alúma** | library, and the collections `almc` needs | permanent |

**The self-hosting boundary is the canonical IR file.** `almc` reads Alúma and writes IR;
`almbe` reads IR and produces a binary. The fixpoint test is therefore on IR bytes
(§3, Phase 11), not on machine code.

**This is the ordinary meaning of self-hosted.** rustc is called self-hosted with an LLVM
backend written in C++; the same applies here. Two pieces stay in the host language, for
two different reasons:

- `almrt` **cannot** be Alúma. No pointer arithmetic, no bytes-as-memory, no allocator
  control, no threads, no mutable globals [1, 4, 7.6]. A garbage collector, a round barrier
  and a syscall edge are not expressible.
- `almbe` **could** be Alúma, later, and the plan deliberately keeps that door open. Because
  it emits *textual* LLVM IR (D1) rather than linking `libLLVM`, it is a text-generation
  program — and Alúma can write text files. What blocks it today is only that Alúma cannot
  spawn `clang` [12.3]; a shell driver can. Port it after M9 if full closure is wanted.

---

## 1. Decisions to make before any code (ADR-worthy)

Each should become a short ADR in `documentation/adr/`. The recommendation is given; the
point of the ADR is that the reasoning is written down.

**D1. Native through *textual* LLVM IR, with `clang` and the system linker driven as
subprocesses by the host driver.**
Not `llvm-sys`/`inkwell`: linking `libLLVM` pins an LLVM version, makes the build painful,
and — decisively — makes `almbe` unportable to Alúma forever. Textual `.ll` keeps the
backend a text emitter of a few thousand lines, keeps it inspectable (every miscompile
starts as a diff of readable IR), tolerates LLVM version drift, and leaves the door of §0
open. The cost is process spawns and a textual round trip, which matter only for compile
time; revisit if that becomes the bottleneck. Target: x86-64 Linux, System V ABI, only.

**D2. Host language: Rust, one cargo workspace, four crates.**
`alm-core` (IR types, canonical serialization and verifier, decimal arithmetic, step/size
tables, report formatting), `almc0`, `almbe`, `almrt`. Shared `alm-core` means the stage-0
compiler, the backend and the runtime cannot disagree about the IR or about decimal
semantics.

**D3. The IR: typed, three-address, over explicit frames — not SSA, not a stack machine.**
The shape is forced by [7.2]: **all parameters are by reference, aliasing is real, and
assignments through aliases take effect in program order.** So a parameter is a *place*:
a tagged reference to a frame slot, a record field, an array element, or a read-only
object in the shared region or a version (which rejects writes with `ReadOnly`).
Everything whose address can be taken lives in an addressable frame slot; `mem2reg` in
LLVM promotes the rest. Non-variable arguments materialise a caller temporary and pass a
place to it [7.2]. `defer` records a closed list of places plus pre-evaluated temporaries
— not a closure — and one record per `defer` statement, which bounds the frame [5.1].
The IR carries, explicitly: charge sites (D5), safepoints, place computations with their
bounds and `nil` checks, `try` regions, and per-procedure slot maps.

**D4. Explicit frames give precise GC roots, the call chain, and the reports — all at once.**
[8.5.1] requires live size to be recounted *exactly*, with `OutOfMemory` firing exactly
when the limit would be exceeded. A conservative stack scan retains garbage, so the counter
would depend on register allocation and stack layout: observable nondeterminism, breaking
[8.7.2]. **Conservative GC is therefore ruled out by the spec.** LLVM's `gc.statepoint`
could give real stack maps but is fiddly and constrains the passes. Instead: each procedure
has a frame object with a compile-time slot map saying which slots hold references, linked
to its caller. The collector walks the frame list. The same frame list gives the call chain
and the current site index that the report format needs [7.5] — `evaluating: total + x`,
the module-qualified chain, `assert`'s condition text — with no dependence on DWARF or on
LLVM debug info. The cost is that reference-holding locals stay in memory.

**D5. Step charges are IR-level and cannot be coalesced freely.**
A task yields exactly where its budget crosses zero, and under `--strict-steps`
`OutOfSteps` lands at a specific charge site [8.4]. Fold a basic block's charges into one
subtraction and the yield point moves, which moves the round a following `send` or `write`
lands in — observable. So: a charge site per event in the step table, each a
decrement-and-branch (2–3 instructions). Because the charges are decided in the IR, they
are decided by the self-hosted part of the compiler, and `almbe` only lowers them. That
keeps every determinism-relevant decision inside `almc`, which is where it belongs.

**D6. Exceptions by explicit status-and-branch, not LLVM unwinding.**
No `invoke`, no landing pads, no personality function, no libunwind. Every call that can
raise returns a status the caller tests, branching to its handler or cleanup chain. Reasons:
[8.4] charges **1 step per unwound frame**, which an explicit unwind can count and a
foreign unwinder cannot; the deferred-call rules of [5.1] (reverse order, remaining calls
still run, a raising deferred call replaces the outgoing exception) are ordinary control
flow this way; and it keeps `almbe` free of platform ABI machinery. The cost is a branch
per call, next to a step poll that is already there.

**D7. Decimal representation.** Sign + magnitude in **base 10^18 limbs** plus a scale,
because the size table is `⌈p/18⌉` units and add/sub costs the limb count of the larger
operand [8.4] — limbs must be 18 digits or the published costs are wrong. Multiply needs
`u128`. Intermediates of `+ - *` are unbounded; only assignment range-checks [7.4]. `int`
is checked `i64` with Euclidean `div`/`mod` [7.3]. Lives in `almrt`, called from generated
code.

**D8. Canonical IR serialization.** The fixpoint check (stage2 == stage3) and the
`almc0`-vs-`almc` differential are byte comparisons on the IR file, so its content must be
fully determined by the source: constant pool and string table in first-occurrence order,
module order = the deterministic `share`-init traversal of [4.2], no timestamps, no host
paths beyond module names, and no map-iteration order anywhere in either compiler. Write
this into `documentation/ir.md` as normative for both compilers and the backend.

**D9. Tasks are OS threads with a barrier at the round boundary.**
[8.7.1] guarantees no mutable memory is reachable from two tasks, and [8.3] says tasks
within a round may run in any order or at once with an identical outcome — so real threads
need no locks on program data, and "budget exhausted" is just parking at the barrier.
No coroutines, no CPS transform, no stack switching. Above a few thousand tasks, move to
small-stack green threads; design `almrt`'s task representation so that swap is contained.

**D10. The replay log format must be decided now.** The spec records *which* events are
recorded and that each carries its round, and leaves the layout open (`justification.md`
§11). It blocks Phase 7. Recommend a versioned, length-prefixed binary log, one record per
event — `(round, seq-in-round, kind, payload)` — ordered by round and then by the boundary
step order of [8.3], with a header carrying every compiler option [8.9] and the seed, so a
replay refuses a mismatched binary.

**D11. The compiler's collections are not `std`.** `std` has no list, map or set; they are
deferred [12.1]. `almc` needs all three. Putting them in `std` now pre-empts a language
decision. Recommend `support/` generic modules (`Vec`, `Map`, `Set`, `Builder`, `Sort`),
shipped with the compiler, promoted later if the design proves out.

**D12. Pin the RNG algorithm.** [9.6] fixes the algorithm per implementation version, with
the sequence a function of `--seed` and the task number. Pick and document one (e.g.
SplitMix64 over the seed mixed with the task number); `random(n)` must be rejection-sampled
to stay uniform.

**D13. With no interpreter, these four things replace the oracle.** Chosen: native only.
What that gives up is an independent execution of the same semantics, so the substitutes
have to be deliberate:
1. **`almc0` vs `almc` on canonical IR** — a genuine differential pair for the whole front
   end, free, and byte-exact thanks to D8. This covers everything up to the IR.
2. **`-O0` vs `-O2` differential execution** — the same program, two optimization levels,
   must produce identical observable behaviour. The determinism guarantee [8.7.2] is what
   makes "identical" a meaningful assertion, and this is the cheapest strong test of the
   backend and of LLVM itself.
3. **An IR verifier in `alm-core`**, run on every compile in CI: types, place legality,
   slot-map coverage, charge-site presence, `try`-region nesting.
4. **The conformance suite as the primary check** (Phase 0, Phase 8) — which means it has
   to be built first and kept exhaustive, not grown opportunistically. With no oracle, a
   codegen bug surfaces only as a wrong answer, so coverage *is* the safety net.

**D14. `--strict-steps` invariance is a CI property test.** Under `--strict-steps`, a run
that raises no `OutOfSteps` must give identical results for every `--steps` value [8.4].
That is the cheapest real check on the determinism story. In CI from Phase 6.

---

## 2. What makes this language unusually easy, and unusually hard

**Easy.** The grammar is LL(1) with explicit `end` [10] — hand-written recursive descent,
no tables, no generator, no dangling-else. No floating point. No inheritance, no templates,
no closures, no overloading, no inference. Name equivalence [7.1] makes type identity a
comparison of declaration ids. Modules are files, imports are acyclic, there are no
interface files [4.1], so each phase is one pass over a topologically sorted module list.

**Hard**, in rough order of cost:

1. **The concurrency semantics are the bulk of `almrt`.** Rounds and the ten-step boundary
   [8.3]; copy-only messages preserving sharing and cycles [8.2]; reply cells with answer
   counters and tie-breaking by sender number [8.2]; links, holders and the number
   invariant [8.10]; cancel closure, waves, sticky `Closed` and the two-part backstop
   [8.11]; the single pending-failure slot with `disown` and the exit-status rule [8.6];
   `Deadlock` with its exact preconditions [8.6]; views with four separate charging rules
   [8.12]. **None of it has ever been run** (`justification.md` §12), so expect the
   implementation to find spec bugs, not merely to implement the spec.
2. **Decimal semantics.** Scale propagation, tags on named number types [7.4], the
   rounding-step set, and the single-rounding rule — "if the right-hand side is a single
   division or a single `strFixed`/`divide`/`rescale` without a mode, possibly in
   parentheses, it rounds once, to the smaller of its own scale and the destination's". A
   syntactic special case in the type checker, and a known-weak spot (`justification.md` §3).
3. **Generic modules with opaque parameters, checked once, interned program-wide** [4.4]:
   one instance per (generic, argument tuple) across the whole program, argument equality by
   type identity / constant value / procedure declaration; opaque `T` rejects `=`, ordering,
   arithmetic, `nil` comparison, indexing and field selection, transitively through records
   and arrays containing a `T`; 32-deep nesting limit.
4. **LLVM will not preserve what it is not told about.** Charge sites, step polls and GC
   frame stores are semantics, not bookkeeping. Runtime calls must not be marked `readnone`
   or `willreturn` where they can raise, GC, or yield; frame slot stores must be visible
   before any call that can collect; the step counter must not be hoisted out of a loop.
   Getting the attributes right in `almbe` is a correctness task, and `-O0` vs `-O2`
   differential runs (D13.2) are how it gets caught.
5. **Writing a compiler in Alúma.** No mutable globals [4] and no closures [1]: the whole
   compiler threads one `Ctx` record by reference. No `case`: every dispatch is an
   `if/elsif` chain over `enum` `int` tags — and enums are untyped with no completeness
   checking [4.1, 12], so a missing arm is a silent fallthrough. `=` is undefined on
   anything containing a reference [7.1], so every structural comparison is hand-written.
   Plan for these three from the start (§6).
6. **Arbitrary-precision decimal inside `almc`.** It must constant-fold `fixed(p, s)` with
   unbounded `p` [7.4], but its own `fixed` declarations need a *constant* `p`. So `almc`
   needs a decimal bignum written in Alúma over `ref array of int` (base 10^9, so products
   fit `int`), not the built-in `fixed`. A real subproject; discovered late, a schedule risk.

---

## 3. Phases

Phases 6 and 9/10 run in parallel: **`almc` is a single-task program, so self-hosting does
not depend on the concurrency runtime.** That is the most useful scheduling fact here.

### Phase 0 — Groundwork
Repo layout (`src/`, `lib/std/`, `lib/support/`, `tests/`, `documentation/adr/`), cargo
workspace, CI, ADRs D1–D14, `documentation/ir.md` skeleton, and — with no interpreter to
fall back on (D13.4) — the **conformance harness first**: a test declares its expected exit
status, stdout, stderr (the report, matched exactly) and compiler options. First population:
**every code example in the three docs**, extracted mechanically with the outcomes the docs
state ([11] is effectively an acceptance suite). Plus the spec-conformance matrix: every
numbered rule → the test(s) pinning it.

### Phase 1 — Front end: lexer and parser (needs: 0)
UTF-8 source, ASCII identifiers, keywords, `#` comments, integer/hex/`v`-fixed/char/string
literals with the full escape set including `\u{h}`, the `_` prefix and the
9223372036854775808 case [2]. Recursive-descent parser for all of [10], one token of
lookahead, spans on every node, diagnostics with recovery good enough to report several
errors. Output: AST + retained source.

### Phase 2 — Semantics (needs: 1)
Module loader (`--path`, file = module, acyclic imports with the cycle named) [4.1]; one
flat namespace per module; `std` auto-import with the shadowing rules and the unhidable
names; `private` incl. private record fields [4.1]; declaration-before-use; `also` groups
with by-value containment-cycle and alias-loop detection [4.3]; name-equivalence types;
zero values [3.3]; constant evaluation over unbounded decimal; `fixed` scales, tags on named
number types, `rounded` and the single-rounding rule [7.4]; parameter and
procedure-parameter rules [7.2]; statement rules incl. `void`, `exit`/`next` placement,
`defer` not inside `while`, handler ordering [5]; reachability checks for message types,
`spawn` arguments, `share` and published types [8.2, 8.12, 4.2]; builtin signature checking
for the polymorphic set [9]; generic modules — parameter declaration, instance interning,
opaque-`T` checking once per module, 32-deep nesting [4.4].

### Phase 3 — IR and lowering (needs: 2)
IR types, textual and binary forms, canonical serialization (D8), verifier (D13.3), and
`documentation/ir.md` filled in. Lowering for: places and true aliasing (D3), strict
left-to-right evaluation [6], short-circuit `and`/`or`, value-copy vs reference-copy
assignment [3.5], `new` with `NegativeSize`, bounds and `nil` checks, selectors on call
results [6], `try` regions, `raise` and bare re-`raise`, `defer` records and their reverse
order and exception-replacement rule [5.1], charge sites per the step table (D5), safepoints,
per-procedure slot maps and site tables for reports (D4).

### Phase 3b — `almbe`: IR → LLVM → binary (needs: 3; interleaves with 4)
Textual `.ll` emission (D1); the `almrt` ABI (frame objects, places, status-and-branch
exceptions per D6, step counter, allocation and charge entry points); correct LLVM function
and call attributes (§2.4); `clang` and linker invocation; `-O0`/`-O2` pipelines; and the
`-O0`-vs-`-O2` differential harness (D13.2) wired into CI as soon as anything runs.

### Phase 4 — `almrt` core, sequential (needs: 3b) → **M1**, **M2**
Per-task heap; precise GC over the frame list (D4); the live-size counters and the
"recount exactly only when it could exceed" rule [8.5.1]; checked `int` with Euclidean
`div`/`mod` [7.3]; decimal (D7) with `divide`, `rescale`, all six rounding modes, `units`,
`fromUnits`, exact `power`, `floor`/`ceiling`/`truncate`/`sign`/`scaleOf` [7.4, 9.5];
`char`/UTF-8 strings; `--depth` and `StackOverflow`; exception unwinding through frames,
handlers and deferred calls, charging a step per frame [8.4]; the report formatter [7.5]
in both `uncaught` and `failed` forms; exit statuses (0, the `int` form's 0–255 with
`Overflow`, 1, 130/143). A **minimal round loop** belongs here already: output is buffered
per task and flushed at the boundary, and reads complete at boundaries [8.3, 9.2], which is
observable in single-task programs.
**M1: `Hello` runs as a native binary.** Then the polymorphic builtins [9], then the
ordinary parts of `std` written **in Alúma** [9.4, 9.5] — which exercises the language early
and shrinks the host surface.
**M2: every non-task example in the docs passes with its stated output and exit status.**

### Phase 5 — Files and bytes (needs: 4)
`File`, `open`/`close`/`stdin`/`stdout`/`stderr`, text and byte reads and writes,
`exists`/`delete`/`rename`/`mkdir`/sorted `list`, read timeouts, the per-task `--handles`
limit, buffered writes flushed in task order, deferred write failure surfacing as `IOError`,
blocking reads completing at boundaries, close-on-task-end in open order [9.3].

### Phase 6 — Concurrency (needs: 4; parallel with 9/10)
Threads and the barrier (D9); the boundary's ten steps in order [8.3]; task numbering;
step budget with negative balances, the cap and `--strict-steps` [8.4]; deep copy preserving
sharing and cycles, identity-copying replies/views/shared refs [8.2]; queues and their
charging, incl. the sender-fails-with-`OutOfMemory` rule; `replyTo(q, n)`, answer counters,
tie-break by task number then send order [8.2]; handles, the holder graph and `spawn`'s
`DeadTask` refusal [8.10]; moves of `File`/`Listener`/`publisher`, with nothing moved if
`spawn` raises [8.1]; cancel closure, waves, marked/closing/ended, new-work `Closed`, sticky
`Closed` and its precedence, no `Closed` inside deferred calls, the grace counter and the
cumulative grace time [8.11]; failure handling and the exit-status rule [8.6]; `Deadlock`;
views and **all four charging rules** [8.12]; `share` initialization in pseudo-task 0 with
the `BadContext` set and the init limits [4.2]; `now`/`sleep`/`round`/`sleepRounds`/`random`
[9.6]; signals as boundary events with status 130/143.
**M3: the task, view, store/client, shared-data and echo-server examples pass, plus the
property tests of D14.**

### Phase 7 — `Net` and replay (needs: 5, 6, D10)
`Net::listen`/`accept`/`connect`/`close`, halves as `File`s, `--socket-buffer` with blocking
writes, timeouts, the `Net` exception set, `accept` as new work that `Closed` interrupts
[9.7]; then record and replay of every outside event with its round [8.7], and a replay mode
that touches no network and discards writes.
**M4: a recorded echo-server run replays byte-identically.**

### Phase 8 — Hardening (continuous from Phase 2)
A negative test for every compile error the spec names and every predefined exception in the
table [7.5]; the conformance matrix complete; lexer/parser fuzzing; `-O0`/`-O2` differential
sweeps (D13.2); `almc0`-vs-`almc` IR differentials once Phase 10 starts (D13.1); randomized
thread-interleaving tests asserting identical output; `--strict-steps` × `--steps` sweeps.

### Phase 9 — `support/` collections in Alúma `(par with 6)` → **M5**
`Vec(T)`, `Map(K, V, equal, hash)`, `Set`, `Builder`, `Sort(T, less)` as generic modules over
`new(array of T, n)` with copy-grow (the `Lookup` example [11] is the pattern), insertion-order
iteration [8.5.3], plus the decimal bignum of §2.6. With `*Test` modules per style guide §13.

### Phase 10 — `almc`: the compiler in Alúma (needs: 3, 4, 9) → **M6**
Port front to back, each component differentially tested against `almc0`: lexer (identical
token streams) → parser (identical AST dumps) → semantics (identical diagnostics, text and
order) → lowering (**byte-identical canonical IR** on the whole corpus; D8 is what makes
this possible). Architecture per §6. Option parsing from `args()` for all of [8.9], since no
environment variables exist [12.3].
**M6: `almc`, compiled by `almc0` and linked by `almbe`, compiles `Hello` correctly.**

### Phase 11 — Bootstrap closure (needs: 10) → **M7**, **M8**
```
almc0  + almc-source → almc.1.ir      (stage 1: host compiler builds it)
almc.1 + almc-source → almc.2.ir      (stage 2: it builds itself)
almc.2 + almc-source → almc.3.ir      (stage 3)
assert almc.2.ir == almc.3.ir         byte for byte  ← self-hosting
assert almc.1.ir == almc.2.ir         holds iff almc0 and almc agree exactly; if not,
                                      diff stage1 vs stage2 to locate the divergence
```
Then: the whole conformance suite green under `almc.2` as well as `almc0`; the bootstrap
reproducible from a clean checkout in CI; `almc.2`'s IR checked in as the seed, with the
rebuild-from-`almc0` procedure documented.

### Phase 12 — After self-hosting → **M9**
Freeze `almc0` as a differential oracle (D13.1) and stop adding features to it. Then, in
order of value: compile-time and runtime performance, with `almc` compiling itself as the
benchmark; diagnostics and parser recovery; a formatter enforcing style guide §2–§3;
measuring the open constants [12.12]; **porting `almbe` to Alúma** for full closure (§0);
further targets; and the deferred language items [12], each as its own spec amendment with
an ADR trail.

---

## 4. Order of work

```
0 ──► 1 ──► 2 ──► 3 ──► 3b ──► 4 ──┬──► 5 ──► 7
                                   │
                                   ├──► 6 ──────────────┐
                                   │                    ├──► 11 ──► 12
                                   └──► 9 ──► 10 ───────┘
                          8 (continuous, from 2 onward)
```

## 5. Milestones

| # | Milestone | Means |
|---|---|---|
| M1 | `Hello` runs as a native binary | front end, IR, `almbe`, LLVM and `almrt` are joined up |
| M2 | all sequential doc examples pass | the sequential language is implemented |
| M3 | all task/view examples pass; `--strict-steps` sweeps clean | the concurrency model is real, and the determinism claim has evidence |
| M4 | a recorded run replays byte-identically | [8.7] holds |
| M5 | `support/` complete | the compiler can be written in Alúma |
| M6 | `almc` compiles `Hello` | the Alúma compiler works end to end |
| M7 | stage2 IR == stage3 IR | **self-hosting** |
| M8 | CI reproduces the bootstrap from a clean checkout | self-hosting is maintainable, not a one-off |
| M9 | `almc0` frozen; suite green under `almc` | the host compiler is no longer load-bearing |

## 6. Architecture of `almc` itself (plan it now, not at Phase 10)

The language's restrictions decide the compiler's shape, so decide the shape before writing
30,000 lines of it:

- **One `Ctx` record, passed by reference everywhere.** No mutable globals [4]. It holds the
  source table, interned strings, the module list, symbol and type tables, diagnostics and
  output buffers; every procedure takes it first.
- **Tagged unions by hand.** No sum types: a node kind is an `int` tag from an `enum` plus a
  record with the union of the fields, exactly as [8.8] does for server requests. Reading a
  field that does not apply yields a zero value silently — so each kind gets a constructor
  procedure and no field is written anywhere else.
- **Dispatch is `if/elsif` over tags with a final `else raise Internal`.** No `case`, no
  completeness checking [12] — the trailing `raise` is the only defence against a forgotten
  arm. Make it a style rule and test it.
- **Ids, not references, wherever a graph could be cyclic or need comparison.** `=` is
  undefined on anything containing a `ref` [7.1], so symbol and type identity are `int` ids
  into `Vec`s in `Ctx`.
- **`Map` keyed with `hashString`/`hashInt`** [9], specified identical on every run and in
  every implementation — safe here, provided iteration stays in insertion order [8.5.3].
- **Mind `--depth`.** Recursive descent on deeply nested expressions and recursive AST walks
  both hit `StackOverflow` as a *checked* exception [8.5.2]. Decide per pass: a clean "input
  too deeply nested" diagnostic, or an explicit `Vec` stack.
- **Mind `--memory`.** `almc` runs as a task under a size-unit limit [8.4], and units count
  one per `int`/reference — a large module's AST is easily millions. Measure at M6 and set
  the limits `almc` needs for its own build.
- **Byte-exact output discipline.** Nothing may depend on anything the source does not
  determine (D8).

## 7. Risks, and what to do about them

| Risk | Why it is real | Mitigation |
|---|---|---|
| The concurrency spec has never been executed | `justification.md` §12 says so; termination, the view memory bound and determinism are argued, not checked | A test per rule for [8.3], [8.11], [8.12]; expect amendments; keep `documentation/amendments.md` beside the reference |
| No interpreter means no independent execution of the semantics | chosen trade-off | D13's four substitutes, with the conformance suite built **first**, not last |
| LLVM optimizes away semantics | charge sites, step polls and GC frame stores look like bookkeeping to it | `-O0` vs `-O2` differential runs in CI from Phase 3b; conservative call attributes; IR verifier |
| LLVM version drift | textual `.ll` is version-sensitive at the edges | Pin a version in CI, keep emission to a conservative subset, test against two versions |
| Precise GC without statepoints costs speed | reference locals stay in memory (D4) | Accept it; `mem2reg` still promotes non-reference, non-aliased locals; measure before optimizing |
| `almc` exceeds its memory limit | size units are counts, not bytes | Measure at M6; id/arena structures (§6); raise `--memory` for the compiler's own build |
| Decimal bignum discovered late | §2.6 | It is in Phase 9, before the port, not during it |
| `almc0` and `almc` disagree subtly | two implementations of a 1,600-line spec | D8 plus differentials at every component boundary in Phase 10 — token streams, then AST dumps, then IR |
| Spec gaps block work | log format, collections, copy-price constants, typed enums | D10 and D11 up front; the constants [12.12] get placeholder defaults and a Phase 12 measurement task |
| Single maintainer, large scope | `almrt` alone is a year-scale project | The phase order front-loads a usable language (M2) long before the concurrency runtime completes |

## 8. Out of scope here

Targets other than x86-64 Linux, a linked-in `libLLVM` backend, porting `almbe` to Alúma, a
debugger, a package manager, an LSP server, cross-implementation log compatibility, and every
item in [12] "Deferred to later versions". Each is a project of its own, after M9.
