# Hand-written conformance tests

Tests that pin a numbered spec rule which no documentation example happens to cover: a
negative test per compile error the spec names, one per predefined exception in the [7.5]
table, and the rule-by-rule tests for [8.3], [8.11] and [8.12] that plan §7 calls for.

`matrix.md` (Phase 0, remaining) maps every numbered rule to the tests that pin it, and
CI fails when a rule has none.

## Known coverage gaps

Recorded as they are found, so that `matrix.md` can be completed rather than guessed at.

- **Char literals.** None of the three documents writes one, so the char rules of [2] —
  exactly one code point, the escape set, `\u{h}` and its valid-code-point check — have
  no cover in the extracted corpus. `src/almc0/src/lex.rs` unit-tests them against the
  spec text, and `src/almc0/tests/lex_corpus.rs` asserts that this is the *only* kind of
  token the corpus misses, so the gap cannot widen unnoticed. A hand-written test is
  owed here.
