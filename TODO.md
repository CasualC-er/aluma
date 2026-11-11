# Alúma — build list: nothing → self-hosting native compiler

Plan and rationale: [documentation/implementation_plan.md](documentation/implementation_plan.md).
Section numbers in brackets point into [documentation/language_reference.md](documentation/language_reference.md).

Target: native code via textual LLVM IR, x86-64 Linux, no interpreter.
Artifacts: `almrt` (runtime, host), `almbe` (IR → LLVM → binary, host), `almc0` (stage-0
compiler, host), `almc` (the compiler, in Alúma), `std` + `support/` (Alúma).
The self-hosting boundary is the canonical IR file.

Legend: **M1–M9** are the milestones from the plan §5. `(par)` marks work that can run in
parallel with the phase above it.

---

## Phase 0 — Groundwork

- [x] Repo layout: `src/`, `lib/std/`, `lib/support/`, `tests/`, `documentation/adr/`
- [x] Cargo workspace: `alm-core`, `almc0`, `almbe`, `almrt`
- [x] CI: build, clippy, fmt, test; later the `-O0`/`-O2` differential and the bootstrap job
- [x] ADR-001 native via **textual** LLVM IR; `clang` + linker as subprocesses; x86-64 Linux only (D1)
- [x] ADR-002 host language and the four crates (D2)
- [x] ADR-003 IR shape: typed, three-address, places, explicit frames (D3)
- [x] ADR-004 precise GC over the frame list; conservative scanning ruled out by [8.5.1] (D4)
- [x] ADR-005 step charges are IR-level and not coalescable (D5)
- [x] ADR-006 exceptions by status-and-branch, no LLVM unwinding (D6)
- [x] ADR-007 decimal: base-10^18 limbs + scale (D7)
- [x] ADR-008 canonical IR serialization — the fixpoint artifact (D8)
- [x] ADR-009 tasks as OS threads with a round barrier (D9)
- [x] ADR-010 replay log format (D10) — blocks Phase 7
- [x] ADR-011 compiler collections live in `support/`, not `std` (D11)
- [x] ADR-012 RNG algorithm, pinned and documented (D12)
- [x] ADR-013 no interpreter: the four substitutes for an oracle (D13)
- [x] ADR-014 `--strict-steps` × `--steps` invariance as a CI property test (D14)
- [x] `documentation/ir.md` — skeleton, declared normative for both compilers and the backend
- [x] `documentation/amendments.md` — log for spec changes the implementation forces
- [x] **Conformance harness first** (D13.4): a test is a *directory* — main module + further modules + options + `args` + `[stdin]` + expected exit status, stdout, stderr or diagnostics; `run` / `compile-ok` / `compile-error` modes, with `#!error` line annotations for the errors [11] states inline
- [x] Extract every code example from the reference, guide and style guide into `tests/docs/`, with the outcomes the docs state
- [ ] Conformance matrix: every numbered spec rule → its test(s)

## Phase 1 — Lexer and parser (`almc0`)

- [x] UTF-8 source reader; ASCII-only identifiers; byte/char/line/column spans
- [x] Keyword table (the 52 reserved words, `also` through `while`, taken from the [2] list itself, not retyped) [2]; `std` not declarable
- [x] `#` comments to end of line, not inside literals
- [x] Integer literals, `0x` hex, the over-max rule and the under-`_` exception in **both** spellings, `9223372036854775808` and `0x8000000000000000` [2]
- [x] `fixed` literals `12v50`; scale = digits after `v`; decimal digits only — `0xFFv10` is an error, per amendment A-001 ([2] prose over the [10] grammar)
- [x] Char and string literals; escapes `\n \t \r \0 \\ \' \" \u{h}`; reject other escapes, raw line breaks, multi-code-point char literals
- [x] Glyph table incl. `:=` `:` `::` `~` `_` `/`, and the absence of a not-equal glyph [2]
- [ ] Recursive-descent parser for all of [10], one token of lookahead
- [ ] Parser: the over-max integer literal is legal **only** directly under `_` — use `IntValue::negated` there, and report `9223372036854775808` (or `0x8000000000000000`) as an error in every other position [2]; the lexer carries it as `IntValue::MinIntMagnitude` because the rule is about the parse tree, not the token
- [ ] `generic` lines first; `import` lines next; instance imports among declarations [4.4]
- [ ] Declarations: `const`, `enum`, `type`, `exception`, `share`, `proc`, with `also` / `private` prefixes
- [ ] Types: predefined, `fixed(p,s)`, `array [n] of`, `record` with `private` fields, `ref`, `inbox`, `mailbox`, `reply`, `view`, `publisher`, `task [of]`
- [ ] Statements: assignment (+ `rounded`), call, `void` call, `if/elsif/else`, `while`, `exit`, `next`, `defer`, `return`, `raise`, `try/catch`
- [ ] Expression precedence levels 1–8, non-associative relations, `not` looser than comparisons [6]
- [ ] Selectors on call results [6]
- [ ] Diagnostics with spans, and recovery good enough to report several errors in one run
- [ ] Parser tests: round-trip every doc example; a negative test per syntax error

## Phase 2 — Semantics (`almc0`)

- [ ] Module loader: file = module, `--path` order, implementation modules last, `Net` overridable [4.1]
- [ ] Import graph: not transitive, no self-import, acyclic with the cycle named in the error
- [ ] One flat namespace per module; imported module names occupy it [4.1]
- [ ] `std` auto-import; shadowing by top-level, parameter and local declarations; the unhidable name list; `std::name` always reaches the library [4.1]
- [ ] `private` on declarations and record fields; cross-module errors; private types usable in visible signatures [4.1]
- [ ] Declaration-before-use; `also` groups and their rules [4.3]
- [ ] By-value containment cycles and alias loops in type groups, errors naming the cycle [4.3]
- [ ] Name-equivalence type identity; every `record` its own type; `ref` nullable [3.5, 7.1]
- [ ] Zero values for every type [3.3]
- [ ] Constant expression evaluator: `const`, `enum`, sizes, `fixed(p,s)` arguments, unbounded decimal
- [ ] `fixed` scale rules: `+ -` max scale, `*` sum, `/` sum, `int` as scale 0, `int/int` rejected [7.4]
- [ ] Assignment checks: `Overflow`, `LostPrecision`, `rounded` [7.4]
- [ ] The rounding-step set and the single-rounding special case (syntactic) [7.4]
- [ ] `divide`/`rescale` with and without a mode; the six rounding modes [7.4]
- [ ] Named number types and tags: `+ -` and comparison rules, everything else untagged, assignment and argument rules [7.4]
- [ ] `=` only on reference-free values; `~` only on refs/mailboxes/views [7.1]
- [ ] Parameter rules: all by reference, variable arguments match exactly, non-variable arguments become temporaries [7.2]
- [ ] Procedure parameters: `proc(T,...):T` types, exact match, second-class [7.2]
- [ ] Statement checks: `void` required and forbidden cases, `exit`/`next` inside `while` only, `defer` not inside `while`, nameless handler last [5]
- [ ] Message-type reachability: no `inbox`/`mailbox`/`publisher`/`task`/`File`/`Listener` [8.2]
- [ ] `spawn` argument rules: exact types for `mailbox`/`File`/`Listener`/`publisher`, no `inbox`/`task` [8.1]
- [ ] `share` type restrictions and the no-assignment rule [4.2]
- [ ] Published-type restrictions [8.12]
- [ ] `start` in the main module only, in one of the four forms [9.1]
- [ ] Builtin signature checking for the polymorphic set (`len`, `fixedStr`, `strFixed`, `abs`, `min`, `max`, `scaleOf`, `power`, `floor`, `ceiling`, `truncate`, `sign`, `units`, `fromUnits`, `divide`, `rescale` + task/handle/view calls) [9]
- [ ] Generic modules: `generic type/const/proc` lines, argument fitting, procedure-argument exact match, builtins rejected as arguments [4.4]
- [ ] Instance interning: one instance per (generic, arguments) program-wide; argument equality by type identity / constant value / procedure declaration [4.4]
- [ ] Opaque `T`: reject `=`, ordering, arithmetic, `nil` comparison, indexing, field selection, transitively through records and arrays; check once per module [4.4]
- [ ] Nested instantiation depth limit 32, error naming the chain [4.4]
- [ ] Message/share type checks for instantiated types reported at the import [4.4]
- [ ] A negative test per compile error above

## Phase 3 — IR and lowering (`almc0`, `alm-core`)

- [ ] IR types; `documentation/ir.md` filled in
- [ ] Textual IR form (for debugging) and binary form (the artifact)
- [ ] Canonical serialization (D8): first-occurrence ordering, no timestamps, no host paths
- [ ] IR verifier in `alm-core`: types, place legality, slot-map coverage, charge-site presence, `try`-region nesting (D13.3)
- [ ] Place model: frame slot / field / element / read-only object (D3)
- [ ] Argument passing: places for variables, caller temporaries for non-variables, true aliasing in program order [7.2]
- [ ] Explicit frame objects with compile-time slot maps (which slots hold references) (D4)
- [ ] Site tables: per-procedure site index → **snippet text** in a per-procedure string table, not a span — [7.5] needs the source text at run time, when the source file is absent and its path is forbidden by D8 (D4, `documentation/ir.md` §6)
- [ ] Strict left-to-right evaluation, incl. target index expressions before the right-hand side [6]
- [ ] Short-circuit `and`/`or`
- [ ] Value-copy vs reference-copy assignment [3.5]
- [ ] `new(R)` and `new(array of T, n)` with `NegativeSize` [7.6]
- [ ] Bounds checks (`IndexRange`) and `nil` checks (`NilReference`)
- [ ] Selector chains on call results, reading from temporaries [6]
- [ ] `try` regions; multi-name handlers; nameless handler; `raise` and bare re-`raise`
- [ ] `defer` records: bound places + pre-evaluated temporaries; reverse order; run on return, fallthrough and exception; a raising deferred call replaces the outgoing exception [5.1]
- [ ] Charge sites per the step table, one per event, not coalesced (D5, [8.4])
- [ ] Safepoints
- [ ] Size-unit computation per the size table, as compile-time constants where possible [8.4]
- [ ] Lowering golden tests: textual IR snapshots for the doc corpus

## Phase 3b — `almbe`: IR → LLVM → binary

- [ ] `almrt` ABI definition in `alm-core`: frame objects, places, exception status, step counter, allocation and charge entry points
- [ ] Textual `.ll` emission for the whole IR (D1)
- [ ] Exceptions as status-and-branch; no `invoke`, no landing pads, no personality (D6)
- [ ] Frame linking and slot-map emission as data sections (D4)
- [ ] Function and call attributes: nothing `readnone`/`willreturn` that can raise, GC or yield; frame stores ordered before collecting calls; step counter not hoisted (§2.4)
- [ ] `almc0` option parsing for all of [8.9], recorded in the IR; `almbe` bakes the values into the binary as a data section, which is where `almrt` reads its limits from — the options are part of the program's identity [8.4, 8.9] and the generated program takes no switch of its own (`alm-core::options`)
- [ ] `clang` and linker invocation; link against the `almrt` archive
- [ ] `-O0` and `-O2` pipelines
- [ ] **`-O0` vs `-O2` differential harness in CI from the first running program** (D13.2); corpus restricted to programs with no outside events, or run under a pinned replay log and an explicit `--seed` — [8.7.2] guarantees determinism only given the same outside events in the same rounds (ADR-013)
- [ ] LLVM version pinned in CI; emission kept to a conservative subset; tested against two versions

## Phase 4 — `almrt` core, sequential → **M1**, **M2**

- [ ] Object layout and per-task heap
- [ ] Precise GC walking the frame list and slot maps (D4)
- [ ] Live-size counters and the "recount exactly only when it could exceed" rule [8.5.1]
- [ ] `int`: checked arithmetic, `Overflow`, Euclidean `div`/`mod`, `DivideByZero`, the min-int/`_1` case [7.3]
- [ ] Decimal: base-10^18 limbs, exact `+ - *`, division at the sum of scales, comparison by value, `Overflow`/`LostPrecision` at assignment [7.4]
- [ ] `divide`, `rescale`, all six rounding modes; `units`, `fromUnits`, `power` with constant exponent, `floor`, `ceiling`, `truncate`, `sign`, `scaleOf` [7.4, 9.5]
- [ ] `char` as a code point; strings as `ref array of char`; `ord`, `chr` with `InvalidCharacter`
- [ ] Call depth: `--depth` and `StackOverflow` counted in calls [8.5.2]
- [ ] Exception unwinding through frames, handlers and deferred calls, 1 step per frame [8.4]
- [ ] Report formatter [7.5]: `uncaught`/`failed` forms, module-qualified names, `evaluating:` source text, call chain, `parent: task N`, init form
- [ ] Exit statuses: 0, the `int` form 0–255 with `Overflow` on `return`, 1 for uncaught, 1 for undelivered failure
- [ ] Minimal round loop: per-task output buffer flushed at the boundary; reads complete at boundaries [8.3]
- [ ] `write`, `writeLine`, `writeErr`, `writeErrLine`, `readLine()` with `nil` at end of input [9.2]
- [ ] `intStr`, `strInt`, `fixedStr`, `strFixed`, `charStr`; number-text rules and `FormatError` [9.2]
- [ ] UTF-8 encode/decode at the edges with `BadEncoding` [9.2]
- [ ] `assert` showing the source text of its condition [9]
- [ ] `args()`, per-task copy [9.1]
- [ ] Bit and wrap operations: `band`, `bor`, `bxor`, `bnot`, `wrapAdd`, `wrapSub`, `wrapMul`, `rotl`, `rotr`, `shl`, `shr` [9.5]
- [ ] `abs`, `min`, `max`, `gcd` [9.5]
- [ ] `hashInt`, `hashString` — FNV-1a exactly as specified, top bit cleared [9]
- [ ] **M1: `Hello` runs as a native binary**
- [ ] `lib/std/` in Alúma: `concat`, `stringcmp`, `stringEqual`, `substring`, `find`, `startsWith`, `endsWith`, `join`, `split`, `repeat`, `padLeft`, `padRight`, `trim` [9.4]
- [ ] `std` tests per style guide §13
- [ ] **M2: every non-task doc example passes with its stated output and exit status**

## Phase 5 — Files and bytes (`almrt`)

- [ ] `File` opaque type; `open` with `ModeRead`/`ModeWrite`/`ModeAppend`; `close` flushing
- [ ] `stdin`, `stdout`, `stderr` per task
- [ ] `readLine(f)`, `write(f,s)`, `writeLine(f,s)`, `readBytes(f,n)`, `writeBytes(f,a)` with the 0–255 rule
- [ ] Timeout forms `readLine(f,t)`, `readBytes(f,n,t)`; `Timeout`; `NegativeSize` on a negative limit
- [ ] `exists`, `delete`, `rename`, `mkdir`, `list` (sorted by code point, no `.`/`..`)
- [ ] `FileNotFound`, `PermissionDenied`, `IOError`, use-after-close, `nil` use
- [ ] Buffered writes emitted in task order at the boundary; deferred write failure surfacing as `IOError` on the next operation or `close`
- [ ] Reads and path operations as boundary-completing outside events
- [ ] Per-task `--handles` limit
- [ ] Files closed in open order when a task ends, including files it was given
- [ ] Byte = one step and one size unit

## Phase 6 — Concurrency (`almrt`) → **M3**

- [ ] Task representation: thread, stack, heap, inbox, budget, state (D9)
- [ ] Round barrier; tasks run in any order with identical outcome [8.3]
- [ ] Round boundary, the ten steps, in order [8.3]
- [ ] Task numbering at the boundary, by creator number then creation order [8.1]
- [ ] Step budget: refresh, cap, negative balances, idle repayment [8.4]
- [ ] `--strict-steps`: `OutOfSteps`, balance reset to `N` [8.4]
- [ ] `sync()`, and the budget stop that is not an interruption point
- [ ] Deep copy: sharing preserved, cycles preserved, replies/views/shared refs by identity [8.2]
- [ ] `send` to `mailbox` and to `task of R`; `DeadTask` rules; messages discarded when a task ends [8.2]
- [ ] Queue memory charging; a message that does not fit is discarded and fails its sender with `OutOfMemory` [8.2]
- [ ] `receive` (never waits), `await` (interruption point) [8.2]
- [ ] `self(inbox)`; `replyTo(q)` / `replyTo(q, n)`; `answer`; counters; order by task number then send order; discards + `DeadTask` [8.2]
- [ ] `spawn`: argument copying, moves of `File`/`Listener`/`publisher` with nothing moved on failure, `mailbox` holder registration, `DeadTask` refusal for marked/ended targets [8.1, 8.10]
- [ ] Handles: `send`, `link`, `cancel`, `awaitEnd`, `ended`, `closing`, `failed`, `disown` [8.10]
- [ ] Holder graph and the task-number invariants; assert them in debug builds [8.10]
- [ ] Cancel closure at the boundary; marked / closing / ended; waves [8.11]
- [ ] New-work rule: `spawn` and `Net::accept` raise a catchable `Closed` in a marked task; `Net::connect` allowed [8.11]
- [ ] Sticky `Closed` at interruption points; precedence over ready messages/children/versions; never inside deferred calls [8.11]
- [ ] Backstop: `--grace` counter over rounds the task ran; cumulative `--grace-time`; `Timeout` then stop; runtime closes files, listeners, publishers [8.11]
- [ ] Ending order: children first, body deferred calls last; `Closed` leaving the body is a normal end [8.11]
- [ ] Failure handling: report at the boundary, single pending-failure slot, lowest-numbered child wins, delivery at `sync`/`await`/`awaitChange`, `failed`/`awaitEnd` as delivery, `disown`, undelivered → exit status 1 [8.6]
- [ ] `Deadlock` with its exact preconditions [8.6]
- [ ] Standard input with several readers: the lowest-numbered blocked task receives an arriving line and the others stay blocked; a blocked reader is waiting for an outside event, so it prevents `Deadlock` [9.2, 8.6]
- [ ] Program end: initial task finishes → whole tree closes → exit [8.6, 8.11]
- [ ] Signals as boundary events: cancel of the initial task, status 130/143, second signal immediate [8.11, 9.6]
- [ ] Views: `newView`, `publish` (snapshot, last-in-round wins, visible at the boundary), `latest`, `version`, `awaitChange`, `ended(v)` [8.12]
- [ ] Version immutability; `ReadOnly` on writes through a version reference [8.12]
- [ ] View charging: owner, reader in full, message-carried references, and the after-owner-ends charge to every task given the view [8.12]
- [ ] `share` initialization: pseudo-task 0, depth-first module order, read-only copy into the shared region, `ReadOnly` on writes, the `BadContext` operation set, `--init-steps`/`--init-memory`/`--shared-memory`, init failure before round 1 [4.2]
- [ ] `now()` sampled once per round; `sleep`; `round()`; `sleepRounds`; the waiting rules [9.6]
- [ ] `random(n)` per the pinned algorithm, per task, from seed + task number (D12)
- [ ] Property test: same program, many runs, randomized thread scheduling, identical output
- [ ] Property test: `--strict-steps` with a sweep of `--steps` values gives identical results (D14)
- [ ] **M3: all task, view, store/client, shared-data and echo-server examples pass**

## Phase 7 — `Net` and replay (`almrt`) → **M4**

- [ ] `Net::Listener`, `Net::listen` with `AddressInUse`, `Net::close`
- [ ] `Net::accept` (+ timeout form) as new work interruptible by `Closed`; unaccepted connections left alone
- [ ] `Net::connect` (+ timeout form), allowed in marked tasks
- [ ] Connections as two `File` halves; moves; runtime closes what a task owns
- [ ] `--socket-buffer` per half; writes sent at the boundary; a full buffer blocks only the writing task
- [ ] `ConnectionRefused`, `ConnectionReset`, `Unreachable`, `IOError`, `Timeout`, `NegativeSize`
- [ ] Record: every outside event with the round it took effect in, per ADR-010 [8.7]
- [ ] Log header: all compiler options + seed; refuse a mismatched binary
- [ ] Replay mode: no network, reads from the log, writes discarded, accepts/connects/timeouts/clock/seed from the log
- [ ] **M4: a recorded echo-server run replays byte-identically**

## Phase 8 — Hardening (continuous from Phase 2)

- [ ] A negative test for every compile error the spec names
- [ ] A test for every predefined exception in the table [7.5]
- [ ] Conformance matrix complete and checked in CI
- [ ] Lexer/parser fuzzing
- [ ] `-O0`/`-O2` differential sweeps over the whole corpus (D13.2)
- [ ] `almc0`-vs-`almc` IR differentials once Phase 10 starts (D13.1)
- [ ] Randomized thread-interleaving tests asserting identical output
- [ ] Spec amendments logged in `documentation/amendments.md` as they are found

## Phase 9 — `support/` in Alúma `(par with 6)` → **M5**

- [ ] `Vec(T)`: copy-grow over `new(array of T, n)`, insertion order
- [ ] `Map(K, V, equal, hash)` with `hashString`/`hashInt`, insertion-order iteration
- [ ] `Set(T, equal, hash)`
- [ ] `Builder`: string building without quadratic concat
- [ ] `Sort(T, less)`
- [ ] `Decimal`: arbitrary-precision decimal over `ref array of int`, base 10^9, for constant folding (plan §2.6)
- [ ] A `*Test` module for each, per style guide §13
- [ ] **M5: `support/` complete and tested**

## Phase 10 — `almc`: the compiler in Alúma → **M6**

- [ ] `Ctx` record design: sources, interned strings, modules, symbols, types, diagnostics (plan §6)
- [ ] Node-kind `enum` tags + constructor procedures per kind; `else raise Internal` on every dispatch
- [ ] Id-based symbol and type identity (no `=` on reference data)
- [ ] Lexer in Alúma; differential test: identical token streams on the whole corpus
- [ ] Parser in Alúma; AST dump format; differential test: identical dumps
- [ ] Decide per pass: deep recursion vs an explicit `Vec` stack, given `--depth`
- [ ] Semantics in Alúma; differential test: identical diagnostics, text and order
- [ ] Lowering in Alúma; differential test: **byte-identical canonical IR** on the whole corpus
- [ ] Driver: `args()`-based option parsing for all of [8.9] (no environment variables exist [12.3])
- [ ] Measure `almc`'s own step and memory use; set the `--memory`/`--steps` it needs
- [ ] **M6: `almc`, compiled by `almc0` and linked by `almbe`, compiles `Hello` correctly**

## Phase 11 — Bootstrap closure → **M7**, **M8**

- [ ] `almc0` + source → `almc.1.ir`
- [ ] `almc.1` + source → `almc.2.ir`
- [ ] `almc.2` + source → `almc.3.ir`
- [ ] Assert `almc.2.ir == almc.3.ir` byte for byte — **M7: self-hosting**
- [ ] Check `almc.1.ir == almc.2.ir`; if it differs, diff stage1 vs stage2 to locate the divergence
- [ ] Whole conformance suite green under `almc.2` as well as `almc0`
- [ ] Check in `almc.2.ir` as the seed, with the rebuild-from-`almc0` procedure documented
- [ ] CI job: reproduce the full bootstrap from a clean checkout — **M8**

## Phase 12 — After self-hosting → **M9**

- [ ] Freeze `almc0`: no new features, kept as the front-end differential oracle — **M9**
- [ ] Compile-time and runtime performance, with `almc` compiling itself as the benchmark
- [ ] Diagnostic quality and parser error recovery
- [ ] Formatter enforcing style guide §2–§3
- [ ] Measure and set the open constants: copy price, `--grace`, `--grace-time`, `--socket-buffer`, `--handles` [12.12]
- [ ] Port `almbe` to Alúma (textual `.ll` emission is just text) for full closure, with a shell driver running `clang`
- [ ] Further targets beyond x86-64 Linux
- [ ] Deferred language items [12], each as its own spec amendment + ADR: collections into `std`, character classification, typed enumerations, `select`, taps, checkpoint/restore, TLS and datagrams, shell interop, more file operations
