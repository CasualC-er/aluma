# Alúma: Why It Is the Way It Is

Companion to the Language Reference (first specification). The reference says what the language does. This document says why, what was turned down, and what each choice costs. Section numbers in brackets point into the reference.

Status of the reasons: they come from the design discussions. They are arguments, not measurements. Nothing in the concurrency design has been executed; only the grammar of the examples is machine-checked. Where a choice rests on a number that has not been measured, the text says so. Comparisons with other languages are from memory and are not cited.

---

## 1. What the language is for

**Priorities, in order of influence:**
1. Finance and exact computation.
2. Reproducible models: the same program and inputs give the same result.
3. Live multiuser servers.
4. Deterministic lockstep simulation.

**Non-goals:** floating point, GPU work, neural-network training, low-latency trading, systems programming.

**Why it matters:** most disputed choices were settled by asking which priority they serve. Exact decimals, strict run-time checks and deterministic concurrency come from (1) and (2). Tasks with private heaps and views come from (3) and (4). Anything that would need raw speed, hardware access or approximate arithmetic was left out instead of half-supported.

---

## 2. Surface and grammar [1, 2, 10]

| Choice | Reason | Cost |
|---|---|---|
| Wirth-style grammar with explicit `end` | Small enough to read on one page and to parse with one token of lookahead. Explicit `end` avoids dangling-`else` problems. | More keywords than a brace language. |
| No `for`, `goto`, `case`; loops have `exit` and `next` | Fewer constructs, one way to loop. | Some loops are longer to write. |
| One glyph, one meaning; `#` for comments, no not-equal glyph | Keeps lexing trivial and avoids look-alike operators. | Unfamiliar to C programmers. |
| ASCII identifiers, UTF-8 source, Unicode in strings and comments | Identifiers stay unambiguous and searchable. Text data is not restricted. | Non-English identifiers are not possible. |
| Explicit evaluation order, left to right | Needed for reproducibility and replay. | Gives up compiler reordering. |
| `and` / `or` short-circuit | Conventional, and conditions are safer to write. | None worth noting. |

The name Alúma and the extension `.alm` were chosen by the language's author.

---

## 3. Arithmetic [7.3, 7.4]

**`int` is 64-bit and overflow raises `Overflow`.** Silent wrap-around is the worst failure for money. Wrapping and rotate operations exist as library calls (9.5) for the rare cases that need them. `div` and `mod` are Euclidean so that results do not depend on the sign convention of the hardware. `/` on two `int`s is an error, so integer division is never confused with exact division.

**`fixed(p, s)` is exact decimal with unbounded `p`.** Exact decimal is the point of the language for finance. COBOL-style fixed point is familiar to the target users. Unbounded precision removes "does it fit" as a design question for intermediate results. The cost is that every `fixed` operation is slower than hardware arithmetic.

**Division and rescaling are explicit rounding steps.** `/` gives a quotient at the sum of the scales. If digits would be lost, `LostPrecision` is raised unless the statement says `rounded`. `divide(a, b, k, mode)` and `rescale(x, k, mode)` round once, with a named mode (`RoundHalfAway`, `RoundHalfEven`, `RoundDown`, `RoundUp`). The reasons: rounding is where money errors come from, so it must be visible in the text, and the mode must be the program's choice, not the platform's.

**Known weakness: double rounding.** A `rounded` assignment whose right side has more than one rounding step rounds at each step. The reference only guarantees a single rounding when the right side is a single step. A review suggested making the multi-step case a compile error. This was not adopted; it is listed as a candidate.

**Named number types and tags [7.4].** `+` and `-` accept equal tags, or a tag and an untagged value. Other operations give untagged results. Reason: it catches adding euros to dollars without building a units system. **Known weakness:** `fare := total * 1` compiles, so tags catch accidents only and are not dimensional checking. A candidate rule (keep the tag under `*` and `/` by an untagged `int`) was noted and not adopted.

**No floating point.** Out of scope by priority (1) and (2). Floating-point results differ across platforms in ways the language promises not to.

---

## 4. Types, parameters and names [3, 4, 7.1, 7.2]

| Choice | Reason | Cost |
|---|---|---|
| Name equivalence (compatibility by name) | Two types with the same shape but different names are different on purpose. | More conversion code. |
| Every `ref` is nullable; zero values everywhere | No uninitialised memory, and nothing undefined. `NilReference` is a checked failure. | Null checks are runtime, not static. |
| Parameters by reference, no value parameters | One rule. Large records are not copied. | Callers can be surprised by changes; the guide advises copying before walking a structure. |
| Aliased arguments mean true aliasing, in program order | Added late. "Unspecified" was a hole in the determinism claim: the same program could differ between implementations. By-reference already means shared storage, so program order costs nothing to define. | An implementation cannot keep an aliased parameter in a register. |
| Procedures are second-class: passed as arguments, never stored or returned | Keeps the call graph visible and avoids closures and captured state. | Callbacks need explicit data parameters. |
| Top-level procedures only; recursion through `also` groups | One-pass reading. Mutual recursion is declared on purpose. | An `also` declaration is needed for mutual recursion. |
| No mutable globals; read-only data through `share` | Required by private heaps and replay. `share` values are built before round 1 in declaration order, are read-only, and writes raise `ReadOnly`. | Global counters need a task. |

---

## 5. Errors, clean-up and modules [5.1, 7.5, 4.1, 4.4]

**Exceptions carry no data, and there is no `finally`.** Smaller semantics; failures are named, and the report supplies the context (name, procedure, task, failing expression, call chain). `defer` schedules clean-up calls that run in reverse order when the procedure exits, so clean-up sits next to the acquisition and does not need a nested `try`. The cost is that an exception cannot carry a reason; the program must define a different exception for each case.

**Generic modules with opaque type parameters, one instance per program.** Reuse without templates or inheritance. One instance per program means two uses of the same instance have the same types. Collections will be written this way; they are not specified yet.

**Modules are source files compiled together, with `private` and acyclic imports.** Plain visibility, no cyclic dependency, fast to reason about.

**`std` is auto-imported and shadowable, except a few names.** Short names for a small library, with the option to override. `len` and the task, handle and view calls cannot be hidden, because the task model depends on them. A consequence: new keywords (`reply`, `view`, `publisher`, `task`) and new library names (`round`, `Round`) were added while the language was still unreleased, and the first four are reserved.

---

## 6. Concurrency: the core [8.1 to 8.9]

**The central choice: private heaps, copy-only messages, lockstep rounds.**

- *No shared mutable state.* Data races are impossible by construction.
- *Rounds.* All tasks run, then the round boundary applies messages, publishes, endings and outside events in a fixed order. The same program with the same recorded outside events gives the same run.
- *Copy-only.* A message is a deep copy. Cost is charged: one step per object plus one per 16 units. The constants are guesses, to be set from measurement.
- *Step budget and memory limit in abstract units.* The same program has the same budgets everywhere. Real memory is bounded by the sum of the task limits.
- *Outside events* (input, clock, seed, network, file results, signals, timer expiry) take effect at boundaries and are recorded with their round. A log is enough to replay.

**Alternatives rejected:**
- Shared memory with locks: races and nondeterminism, against priority (2).
- Fully asynchronous message passing without rounds: simpler to implement, but the order of messages depends on timing, so no exact replay.
- Free-running tasks with deterministic merge only at replay: it makes the live system unlike the replayed one.

**Costs, stated honestly:**
- A global round barrier: one slow task is not isolated from the others except by the budget. Running independent groups without a barrier is deferred (item 10).
- Rounds are not strictly ticks unless `--strict-steps` is used (section 11 below).
- Messages are ordered by sender task number within a round, so task 1 wins ties. This was kept: a receiver that wants fairness can batch and shuffle with the seeded `random`.

---

## 7. Addresses, handles and the shape of dependencies [8.2, 8.10]

| Address | Purpose | Key restriction |
|---|---|---|
| **Link** (`mailbox of R`) | Reach a task's inbox. | Received only as a bare `spawn` argument; never sent in messages, `share`, or published values. |
| **Reply** (`reply of R`) | Answer a request. | One answer by default, or up to `n` with `replyTo(queue, n)`. |
| **View** (`view of R`) | Read published state. | Read-only; travels only in messages and `spawn` arguments. |

**Why links are only passed at `spawn`:** every holder of a link to Y is then numbered after Y and descends from Y's parent. Waits go only to higher task numbers (invariant W), so no cycle of waits can exist. The ending procedure's termination argument rests on this. Two tasks therefore cannot hold links to each other.

**What this costs:** no unsolicited push, and no mutual links. The language gives three answers: replies for requests, multi-part replies for answers in parts, and views for state that others read. The remaining need, a lossy notification to tasks that do not depend on the sender, is deferred as "taps" (item 6).

**Why replies are not links:** a reply makes no wait. `answer` is a non-blocking send. So replies can go both ways between two tasks without touching invariant W.

**Handles** (`task of R`, `task`) belong to the parent and are non-transferable. This keeps supervision local: the parent can `send`, `cancel`, `awaitEnd`, `failed`, `disown`, but nobody else can.

**Moves.** `File`, `Net::Listener` and `publisher` move only as bare `spawn` arguments; the variable becomes `nil`. Each such resource then has one owner and one place that closes it.

---

## 8. Ending, failure and views [8.6, 8.11, 8.12]

**Ending in a fixed order.** Children end first; the task's own deferred calls run last. Cancelling X also cancels its holders and descendants. A finished task cancels its descendants but not its holders. All tasks that qualify at one boundary become closing together (a "wave"). This gives clean shutdown without a `Stop` message and without leaving orphans.

**`Closed`** is raised at interruption points only (`sync`, `await`, `awaitChange`, `awaitEnd`, `sleep`, `sleepRounds`, and waits on outside events). It is sticky once a task is closing, never raised in deferred calls, and wins over ready messages. Reason: a task must not be able to ignore its own shutdown by catching and continuing, and clean-up code must not be interrupted.

**New work in a marked task.** `spawn` and `Net::accept` raise `Closed` there (non-sticky, catchable). Starting a task would add new holders after a cancel, which would break the termination argument. A review found exactly this counterexample, and the fix was to make `spawn` refuse a link whose target is marked or ended (`DeadTask`). `Net::connect` stays allowed, because a shutdown flush to a remote ledger over a new connection is a real need and does not touch the task tree.

**Backstop.** A closing task has a rounds counter (`--grace`, counted over rounds the task actually ran) and one cumulative blocked-time budget (`--grace-time`). An earlier version retried for minutes and let empty rounds run at CPU speed; the cumulative budget fixed that. Total time to end a tree is bounded by depth times the backstop. This is argued, not machine-checked.

**Failures.** Every failure is written to stderr at once. It also becomes the parent's single pending failure if the parent is not marked, the child is not disowned and the slot is empty; the lowest-numbered child wins within a round. It is raised at the parent's next `sync`, `await` or `awaitChange`. Observing it with `failed(h)` or `awaitEnd(h)` counts as delivery. A failure nobody observed makes an otherwise-0 exit status 1. `disown(h)` opts a child out, for disposable children such as per-connection sessions.

*Why one slot:* the parent's cost for failures is bounded; a pile of pending failures would be uncharged memory.

**Views.** The owner publishes; each version is an immutable snapshot, visible at the next boundary (last publish in a round wins). Readers read with `latest`, wait with `awaitChange(v, seen)`, which raises `DeadTask` once the owner has ended and nothing is newer.

*Charging.* The owner pays for the current version; each reader pays the full reachable size; queued messages count the version units they reference; after the owner ends, the view is charged to every task that was given it. The runtime tracks those tasks and recounts exactly only if they could exceed the limit, so there is no global scan.

*Why full per-reader charge:* a cheaper rule would let many readers hold a large version for free and break the memory bound. A cheaper scheme (regions, a per-reference charge) is deferred (item 8).

---

## 9. Input, output and `Net` [9]

**Reads block and complete at round boundaries; results are recorded.** This keeps replay possible. Writes are buffered and emitted in task order at the end of the round, so output is deterministic.

**`Net` is implementation-provided** and gives a connection as two `File` halves. Reasons: reuse of the file model, a read and a write can be done by separate tasks, timeouts on reads, a write buffer (`--socket-buffer`), replay from the log. TLS and datagrams are deferred.

**Signals.** An interrupt or terminate signal is an outside event, recorded with its round and applied as a `cancel` of the initial task. A second signal ends the program at once. Exit status is 130 or 143.

---

## 10. Amendments added after the concurrency review

These four were added as small changes after a later review. They are not part of the reviewed design.

1. **True aliasing** replaces "unspecified" (7.2). Reason: closes the one hole in cross-implementation determinism at no cost.
2. **`--strict-steps`** (8.4). An overrun raises `OutOfSteps` instead of yielding. Reason: without it, `--steps` can change results; with it, a run that raises nothing gives the same results for every value of `--steps`. Cost: programs must do bounded work per round.
3. **`round()` and `sleepRounds(n)`** (9.6), with type `Round` = `fixed(36, 0)`. Reason: a simulation needs a logical clock; `now` and `sleep` are wall-clock. `round()` is derived, so it is not recorded. Meaningful as "ticks" only together with `--strict-steps`.
4. **`replyTo(queue, n)`** (8.2): up to `n` answers. Reason: an exchange cannot report repeated partial fills with a one-shot reply, and a multi-part reply creates no link or wait, so invariant W is unaffected. Cost: a requester that stops receiving makes its answerer fail with `OutOfMemory`; for many or slow clients, a view is the safer choice.

---

## 11. What is not decided

From section 12 of the reference, plus items raised in review:

- **Recording format.** The reference specifies which events are recorded and that each carries its round. It does not specify a byte or text layout, a header, per-event schemas, an ordering rule within a round, or divergence handling. Open question: leave it implementation-defined, or make it normative so logs can move between implementations.
- **Standard collections, character classification, shell interop, more file operations.**
- **Program-level signal handling and timer messages; taps (lossy push); views inside published graphs; regions for published data; TLS, datagrams and `select`; worker pools that receive connections.**
- **Checkpoint and restore**, and groups without a global barrier. Feasible given private heaps and copy-only messages, and tied to the recording-format question.
- **Typed enumerations.** Enumerations are plain `int` constants, so a wrong constant in a `kind` field fails silently. Typing them is a separate decision.
- **Constants to be measured:** the copy price, `--grace` (100), `--grace-time` (5000 ms), `--socket-buffer` (65,536), `--handles` (1024).

## 12. What has and has not been checked

- **Checked by machine:** the grammar of every code example in the reference, the guide and the style guide.
- **Argued, not checked:** the termination of the ending procedure (invariants W, C, T), the memory bound for views, and the determinism claims.
- **Never run:** any task code. The sequential interpreter (`chk/interp.py`) has no tasks, and the sample programs in `orders/` still use the old `.src` extension.
