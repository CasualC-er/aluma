# Hand-written conformance tests

Tests that pin a numbered spec rule which no documentation example happens to cover: a
negative test per compile error the spec names, one per predefined exception in the [7.5]
table, and the rule-by-rule tests for [8.3], [8.11] and [8.12] that plan §7 calls for.

`matrix.md` (Phase 0, remaining) maps every numbered rule to the tests that pin it, and
CI fails when a rule has none.
