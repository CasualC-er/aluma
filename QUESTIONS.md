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

***Answer***: open a pr after phase 2 is complete.

*Done*: noted. I will open it when the Phase 2 checklist in TODO.md is clear.

## 2. A local may shadow a module-level name

[4.1] says a parameter or local hides a *library* name "in the same way" as a top-level
declaration does, and says nothing about shadowing the module's own declarations. I
allow it: `const Size is 10;` and a parameter named `Size` coexist, the parameter
winning inside its procedure. The alternative — making it an error — is also defensible,
and `style_guide.md` may have a view. A test pins the current behaviour
(`resolve.rs::a_parameter_or_local_shadows_a_module_name_inside_its_procedure`), so
changing it is a one-line change plus that test. 

***Answer***: You may not redeclare module scoped values, since diffrentiation becomes undignified

*Done*: a parameter or local that repeats a module-level name — including an imported
module's name, which occupies the namespace [4.1] — is now an error naming the
declaration it clashes with. A *library* name may still be hidden, which [4.1] allows
in so many words, except for the unhidable list. Pinned by
`resolve.rs::a_parameter_or_local_may_not_repeat_a_module_level_name`.


## 3. A local may not repeat a parameter's name

Nothing in the reference says whether `proc f(a: int) var a: int;` is an error. I report
it, on the grounds that it can only be a mistake. Tell me if you would rather it were
allowed (the local shadowing the parameter). 

***Answer***: Correct Call

*Done*: unchanged, now with your reasoning recorded alongside it.

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
***Answer***: Correct.

## 6. Type graph decisions (for the record, no answer needed unless you disagree)

- A named *reference* type (`type Name is String;`) carries **no tag**. [7.4] gives the
  tag rules for "named number types" and says every other operation gives an untagged
  result, so I read tags as a property of `int` and `fixed` types only. `tag()` returns
  `None` for `type Name is String;`.
- A `fixed` precision over 1,000,000 digits is refused. [7.4] says `p` has "no upper
  limit", and nothing a program can notice is limited — the bound exists because a
  scale is held in a `u32`, and casting an `i64` constant straight into one wrapped
  (`fixed(5000000000, 5000000000)` became a narrow type). The message says it is this
  compiler's limit, not the language's. 
  
  [**Critique**]: this should be overrideable and set as a compiler option, and rust allows u128s which would work if we did it in advance.

  *Done, half of it*: it is now the `--max-digits` option, default 1,000,000, and the
  diagnostic names it ("raise `--max-digits` if you need more ([7.4] sets no limit of
  its own)"). [8.9] permits this: "the option names and defaults below are the standard
  ones; an implementation may offer others". It is deliberately **not** part of the
  program's identity, unlike [8.9]'s own options: it decides whether the program
  compiles, not how it runs, so a replay does not need it and two binaries built with
  different limits are the same program.

  *Not done — the `u128` half, and I think it should not be*: the option is capped at
  `u32::MAX` because a *scale* is held in a `u32`. Widening that to `u128` would not
  raise the real ceiling, because the ceiling is memory, not the counter. A scale is a
  count of fractional digits, and the digits have to exist: at `u32::MAX` fractional
  digits one number needs about half a gibibyte of limbs, and a `u128` scale would
  describe numbers needing more memory than addressable. Meanwhile every `Decimal` and
  every `fixed` type in the compiler would carry 16 bytes of scale instead of 4, on a
  structure that the IR serialises and that Phase 10 compares byte for byte.

  If the worry is that `u32` is visible in the language, it is not: `--max-digits`
  defaults to a million, and a program asking for more gets a diagnostic naming the
  option rather than a wrapped type. Say the word if you still want `u128` and I will
  change it — it is a type alias and a handful of casts.

- An import chain deeper than 400 modules is refused, for the same reason [8.5.2] makes
  `StackOverflow` a checked exception rather than letting the machine stack decide: the
  loader recurses per module, and a 20,000-module chain aborted the compiler. A
  diagnostic beats a signal.
