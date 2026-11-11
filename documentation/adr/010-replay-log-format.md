# ADR-010: Replay log format

Status: accepted. Plan reference: D10. Blocks Phase 7.

## Context

[8.7] requires that every outside event be recorded with the round it took effect in,
and that a replay reproduce the run. The spec fixes *which* events are recorded and
leaves the layout open (`justification.md` §11), so the layout is ours to decide — and
it blocks Phase 7, so it is decided now.

## Decision

A versioned, length-prefixed binary log. One record per event:

```
(round, seq-in-round, kind, payload)
```

ordered by round, then by the boundary step order of [8.3]. The header carries
**every compile option [8.9] and the seed**, so a replay can refuse a mismatched binary.

Event kinds are the outside events [8.7.3] names, which is the authoritative list:
input, the clock, the seed, the results of file and network operations, accepts and
connects, the expiry of timers and timeouts, **write completions**, and **signals**.

The last two are easy to overlook and both are observable: a deferred write failure
surfaces as `IOError` on a later operation or at `close` [9.3], so *when* a write
completed decides which call raises it; and a signal is a boundary event that cancels the
initial task and sets the exit status to 130 or 143 [8.11, 9.6], so a replay that does
not reproduce it ends differently.

## Reasons

- Ordering by round and then by the boundary's own step order means the log's order is
  the order the spec already defines, so replay needs no tie-breaking rules of its own.
- Length prefixes let a truncated log (a killed process) be read up to the last complete
  record, which is the common case when debugging a crash.
- The header check is what makes replay trustworthy: the options are part of the
  program's identity [8.4, 8.9], so a log replayed against a binary compiled with a
  different `--steps` would diverge for a legitimate reason and look like a bug.
  `alm-core::options::Options::identity` produces the canonical string compared here.

## The seed

[8.9] lists `--seed` as fixed at compile time, but its default is "chosen from the
operating system and recorded", which is a per-run choice. The seed is therefore bound
at startup: baked in when `--seed` was given, drawn and recorded as a log record
otherwise. The identity string records `seed=os` in the latter case, so a recording and
its replay have the same identity. Logged as amendment A-002.

## Consequences

- Replay mode touches no network, reads from the log, and discards writes.
- Cross-implementation log compatibility is explicitly out of scope (plan §8).
