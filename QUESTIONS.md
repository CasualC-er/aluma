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
