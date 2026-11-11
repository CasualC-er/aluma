# ADR-012: The RNG algorithm is pinned and documented

Status: accepted. Plan reference: D12.

## Context

[9.6] fixes the random sequence as a function of `--seed` and the task number, for a
given implementation version. That makes the algorithm part of the observable behaviour
of a program, so it cannot be an implementation detail that changes silently.

## Decision

**SplitMix64**, with a per-task stream, and rejection sampling for `random(n)`. Pinned
in full below; a change to any part of it is a new implementation version and a spec
amendment.

**The stream.** A task's state is initialised from the seed and the task number:

```
state(task) = seed + task * 0x9E3779B97F4A7C15      (wrapping, 64-bit)
```

**The generator**, one `u64` per draw:

```
state += 0x9E3779B97F4A7C15                         (wrapping)
z = state
z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9            (wrapping)
z = (z ^ (z >> 27)) * 0x94D049BB133111EB            (wrapping)
draw = z ^ (z >> 31)
```

**`random(n)`**, uniform on `0 ..< n` for `n >= 1`:

```
if n < 1: raise NegativeSize                        (before any draw)
limit = 2^64 - (2^64 mod n)                         (the largest multiple of n)
repeat: d = draw() until d < limit
result = d mod n
```

The rejection loop is what makes it uniform: `draw() mod n` alone is biased whenever `n`
does not divide 2^64.

## Reasons

- SplitMix64 is a few lines, has no state beyond a `u64`, and is well-studied enough
  that "the algorithm" is unambiguous — which is the actual requirement.
- Per-task streams derived by mixing the task number mean a task's sequence does not
  depend on how many other tasks exist or what order they ran in, which it must not,
  because task scheduling within a round is unordered [8.3].
- **Rejection sampling, not modulo.** `seed % n` is biased whenever `n` does not divide
  2^64, and the bias is observable in a program's output. Since the sequence is
  specified, a biased `random(n)` is a wrong answer, not a quality-of-implementation
  issue.

## Consequences

- `random(n)` with `n < 1` raises `NegativeSize` [7.5], checked before any draw, so that
  a rejected draw cannot change the stream.
- The number of draws consumed per `random(n)` call is variable (rejection), so the
  stream position depends on the values drawn. That is fine — it is still a function of
  the seed and the task number — but it means the log cannot assume one draw per call.
