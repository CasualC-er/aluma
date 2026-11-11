# Open questions

Decisions I took while you were away that you may want to overrule, and questions that
did not justify stopping work. Newest last. Nothing here blocks progress — each entry
says what I did so that the answer is a change, not a start.

Spec *ambiguities* go in `documentation/amendments.md` instead: those are things the
reference does not settle and the implementation had to. This file is for choices that
are mine rather than the language's.

---

## 1. Branch name is stale

`phase-1-lexer` now holds the whole of Phase 1 and the start of Phase 2. I have kept
committing to it rather than opening a PR per phase, since you merged #1 yourself and I
did not want to leave a trail of unmerged branches. Say if you would rather have one
branch per phase, or a PR now.

## 2. A local may shadow a module-level name

[4.1] says a parameter or local hides a *library* name "in the same way" as a top-level
declaration does, and says nothing about shadowing the module's own declarations. I
allow it: `const Size is 10;` and a parameter named `Size` coexist, the parameter
winning inside its procedure. The alternative — making it an error — is also defensible,
and `style_guide.md` may have a view. A test pins the current behaviour
(`resolve.rs::a_parameter_or_local_shadows_a_module_name_inside_its_procedure`), so
changing it is a one-line change plus that test.

## 3. A local may not repeat a parameter's name

Nothing in the reference says whether `proc f(a: int) var a: int;` is an error. I report
it, on the grounds that it can only be a mistake. Tell me if you would rather it were
allowed (the local shadowing the parameter).

## 4. A procedure call is not a constant expression

[4] says constants are "compile-time constant expressions" without saying which calls,
if any, fold. `strFixed`'s `k` and `power`'s exponent are *described* as constant
expressions, but that is about their arguments, not about the calls themselves being
foldable. I reject every call in a constant expression, with a message saying why.

That is the direction that cannot be wrong later: allowing a call now and discovering
the reference meant otherwise would break programs that rely on it, while allowing more
folding later breaks nothing. If you want `power(2, 10)` or `strFixed("1.5", 2)` to fold,
say so and it is a contained change in `consts.rs`.

## 5. An inexact constant division is rejected outright

A division is a rounding step, and losing a non-zero digit at one raises
`LostPrecision` unless the statement says `rounded` [7.4]. A `const` declaration has no
`rounded`, so `const X is 1v00 / 3v00;` cannot be written at all — the compiler rejects
it and suggests writing the value out or using `divide(a, b, k, mode)`. I believe that
follows from the spec rather than being a choice, but it is the kind of thing worth a
second opinion, since the alternative reading is that a constant simply rounds.
